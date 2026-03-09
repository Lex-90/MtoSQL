/// Orchestrates parse → resolve → translate → emit pipeline.
use std::path::Path;

use crate::dialect::Dialect;
use crate::emitter::sql_writer::write_sql;
use crate::error::{CredentialRedactor, DiagLevel, Diagnostic, M2SqlError};
use crate::parser;
use crate::parser::ast::*;
use crate::resolver::source::{detect_parameters, generate_param_header, DetectedParameter};
use crate::translator;
use crate::translator::context::{OnError, TranslationContext};
use indexmap::IndexMap;
use std::collections::HashSet;

/// Result of processing a single input.
pub struct ProcessResult {
    /// Output queries: (query_name, sql_text).
    pub queries: Vec<(String, String)>,
    /// Diagnostics emitted during processing.
    pub diagnostics: Vec<Diagnostic>,
    /// Whether any translation errors occurred.
    pub has_errors: bool,
    /// Number of unique parameters detected in this file.
    pub params_detected: usize,
}

/// Process a single M source string.
pub fn process_source(
    source: &str,
    file_name: &str,
    file_stem: &str,
    dialect: &dyn Dialect,
    on_error: OnError,
    inline_singles: bool,
    input_file_stems: &std::collections::HashSet<String>,
) -> Result<ProcessResult, M2SqlError> {
    // Parse
    let is_tmdl = file_name.ends_with(".tmdl");

    let documents = if is_tmdl {
        parser::parse_tmdl(source).map_err(|e| M2SqlError::Parse {
            file: file_name.to_string(),
            line: e.line,
            message: e.message,
        })?
    } else {
        let doc = parser::parse(source).map_err(|e| M2SqlError::Parse {
            file: file_name.to_string(),
            line: e.line,
            message: e.message,
        })?;
        vec![(file_stem.to_string(), doc)]
    };

    let mut all_queries = Vec::new();
    let mut all_diagnostics = Vec::new();
    let mut has_errors = false;
    let mut total_params_detected = 0usize;

    for (query_name, doc) in &documents {
        let mut ctx = TranslationContext::new(dialect, on_error, file_name, inline_singles);
        ctx.input_file_stems = input_file_stems.clone();

        // Detect Power Query parameters
        let let_bindings = collect_let_bindings(doc);
        let expr = match doc {
            MDocument::Expression(e) => e,
            MDocument::Section { bindings, .. } => {
                // For section syntax, detect across all bindings
                let mut all_params: IndexMap<String, DetectedParameter> = IndexMap::new();
                for (_, _, e) in bindings {
                    let params = detect_parameters(e, &let_bindings, input_file_stems, file_name);
                    for (k, v) in params {
                        all_params.entry(k).or_insert(v);
                    }
                }
                // Emit parameter diagnostics and header for section docs
                if !all_params.is_empty() {
                    total_params_detected += all_params.len();
                    emit_param_warnings(&all_params, &mut all_diagnostics);
                    ctx.detected_params = all_params;
                }
                // Continue with normal translation
                let results = translator::translate(doc, query_name, &mut ctx);
                for (name, sql_query) in results {
                    let sql_text = write_sql(&sql_query, dialect);
                    let (redacted_sql, redact_diags) =
                        CredentialRedactor::redact(&sql_text, Some(file_name));
                    let final_sql = if !redact_diags.is_empty() {
                        CredentialRedactor::insert_security_comments(&redacted_sql)
                    } else {
                        redacted_sql
                    };
                    // Prepend parameter header if parameters detected
                    let final_sql = if !ctx.detected_params.is_empty() {
                        let header = generate_param_header(&ctx.detected_params);
                        format!("{}{}", header, final_sql)
                    } else {
                        final_sql
                    };
                    all_diagnostics.extend(redact_diags);
                    all_queries.push((name, final_sql));
                }
                if ctx.has_errors {
                    has_errors = true;
                }
                all_diagnostics.extend(ctx.diagnostics);
                continue;
            }
        };

        let params = detect_parameters(expr, &let_bindings, input_file_stems, file_name);
        if !params.is_empty() {
            total_params_detected += params.len();
            emit_param_warnings(&params, &mut all_diagnostics);
            ctx.detected_params = params;
        }

        // Translate (parameter identifiers are handled in translate_expr via ctx.detected_params)
        let results = translator::translate(doc, query_name, &mut ctx);

        // Emit SQL
        for (name, sql_query) in results {
            let sql_text = write_sql(&sql_query, dialect);

            // Credential redaction
            let (redacted_sql, redact_diags) =
                CredentialRedactor::redact(&sql_text, Some(file_name));
            let final_sql = if !redact_diags.is_empty() {
                CredentialRedactor::insert_security_comments(&redacted_sql)
            } else {
                redacted_sql
            };

            // Prepend parameter header if parameters detected
            let final_sql = if !ctx.detected_params.is_empty() {
                let header = generate_param_header(&ctx.detected_params);
                format!("{}{}", header, final_sql)
            } else {
                final_sql
            };

            all_diagnostics.extend(redact_diags);
            all_queries.push((name, final_sql));
        }

        if ctx.has_errors {
            has_errors = true;
        }
        all_diagnostics.extend(ctx.diagnostics);
    }

    Ok(ProcessResult {
        queries: all_queries,
        diagnostics: all_diagnostics,
        has_errors,
        params_detected: total_params_detected,
    })
}

/// Get the file stem from a path.
pub fn file_stem_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("query")
        .to_string()
}

/// Sanitize a file name for output (replace illegal chars).
pub fn sanitize_filename(name: &str) -> String {
    let mut result = String::new();
    for ch in name.chars() {
        match ch {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => result.push('_'),
            _ => result.push(ch),
        }
    }
    result
}

/// Collect all let binding names from an M document (for parameter detection).
fn collect_let_bindings(doc: &MDocument) -> HashSet<String> {
    let mut bindings = HashSet::new();
    match doc {
        MDocument::Expression(expr) => collect_let_bindings_from_expr(expr, &mut bindings),
        MDocument::Section {
            bindings: section_bindings,
            ..
        } => {
            for (_, name, expr) in section_bindings {
                bindings.insert(name.clone());
                collect_let_bindings_from_expr(expr, &mut bindings);
            }
        }
    }
    bindings
}

/// Collect let binding names from an M expression.
fn collect_let_bindings_from_expr(expr: &MExpr, bindings: &mut HashSet<String>) {
    if let MExpr::Let {
        bindings: let_bindings,
        body,
    } = expr
    {
        for (name, e) in let_bindings {
            bindings.insert(name.clone());
            collect_let_bindings_from_expr(e, bindings);
        }
        collect_let_bindings_from_expr(body, bindings);
    }
}

/// Emit PARAM_REFERENCE warnings for detected parameters.
fn emit_param_warnings(
    params: &IndexMap<String, DetectedParameter>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (name, info) in params {
        diagnostics.push(Diagnostic {
            level: DiagLevel::Warn,
            file: Some(info.file.clone()),
            line: Some(info.line),
            code: "PARAM_REFERENCE".to_string(),
            message: format!(
                "Parameter reference: '{}' — replaced with /* PARAM: {} */; substitute before running",
                name, name
            ),
            fragment: None,
        });
    }
}

/// Check for path traversal.
pub fn check_path_traversal(
    output_dir: &Path,
    query_name: &str,
) -> Result<std::path::PathBuf, M2SqlError> {
    let sanitized = sanitize_filename(query_name);
    let output_path = output_dir.join(format!("{}.sql", sanitized));

    // Resolve to absolute and check it's within output_dir
    let canonical_dir = output_dir
        .canonicalize()
        .unwrap_or_else(|_| output_dir.to_path_buf());
    let canonical_file = output_path
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_else(|| output_path.parent().unwrap_or(Path::new(".")).to_path_buf());

    if !canonical_file.starts_with(&canonical_dir) {
        return Err(M2SqlError::PathTraversal(query_name.to_string()));
    }

    Ok(output_path)
}

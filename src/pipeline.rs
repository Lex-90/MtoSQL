/// Orchestrates parse → resolve → translate → emit pipeline.
use std::path::Path;

use crate::dialect::Dialect;
use crate::emitter::sql_writer::write_sql;
use crate::error::{CredentialRedactor, Diagnostic, M2SqlError};
use crate::parser;
use crate::translator;
use crate::translator::context::{OnError, TranslationContext};

/// Result of processing a single input.
pub struct ProcessResult {
    /// Output queries: (query_name, sql_text).
    pub queries: Vec<(String, String)>,
    /// Diagnostics emitted during processing.
    pub diagnostics: Vec<Diagnostic>,
    /// Whether any translation errors occurred.
    pub has_errors: bool,
}

/// Process a single M source string.
pub fn process_source(
    source: &str,
    file_name: &str,
    file_stem: &str,
    dialect: &dyn Dialect,
    on_error: OnError,
    inline_singles: bool,
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

    for (query_name, doc) in &documents {
        let mut ctx = TranslationContext::new(dialect, on_error, file_name, inline_singles);

        // Translate
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

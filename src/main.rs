#![allow(dead_code)]
/// CLI entry point for m2sql.
mod cli;
mod dialect;
mod emitter;
mod error;
mod parser;
mod pipeline;
mod resolver;
mod translator;

use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Instant;

use clap::Parser;

use cli::Cli;
use dialect::dialect_from_name;
use error::{DiagLevel, Diagnostic};
use pipeline::{check_path_traversal, file_stem_from_path, process_source, sanitize_filename};

/// Maximum input file size (10 MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

fn main() {
    let args = Cli::parse();

    // Validate arguments
    if let Err(e) = args.validate_dialect() {
        eprintln!("ERROR: {}", e);
        std::process::exit(2);
    }

    let on_error = match args.on_error_mode() {
        Ok(mode) => mode,
        Err(e) => {
            eprintln!("ERROR: {}", e);
            std::process::exit(2);
        }
    };

    let dialect = match dialect_from_name(&args.dialect) {
        Some(d) => d,
        None => {
            eprintln!("ERROR: Unknown dialect: {}", args.dialect);
            std::process::exit(2);
        }
    };

    let start = Instant::now();
    let mut total_translated = 0usize;
    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;
    let mut any_translation_errors = false;
    let mut name_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    if args.input.is_empty() {
        // Read from stdin
        let mut source = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut source) {
            emit_error(&format!("Failed to read stdin: {}", e), &args);
            std::process::exit(2);
        }

        match process_source(
            &source,
            "<stdin>",
            &args.query_name,
            dialect.as_ref(),
            on_error,
            args.inline_singles,
        ) {
            Ok(result) => {
                for diag in &result.diagnostics {
                    emit_diagnostic(diag, &args);
                    match diag.level {
                        DiagLevel::Error => total_errors += 1,
                        DiagLevel::Warn | DiagLevel::Security => total_warnings += 1,
                        _ => {}
                    }
                }

                if result.has_errors {
                    any_translation_errors = true;
                } else {
                    for (name, sql) in &result.queries {
                        if args.stdout {
                            println!("-- [query: {}]", name);
                            println!("{}", sql);
                        } else if let Err(e) =
                            write_output_file(&args.output_dir, name, sql, &mut name_counts)
                        {
                            emit_error(&format!("{}", e), &args);
                            any_translation_errors = true;
                        }
                        total_translated += 1;
                    }
                }
            }
            Err(e) => {
                emit_error(&format!("{}", e), &args);
                std::process::exit(2);
            }
        }
    } else {
        // Process input files
        for path in &args.input {
            let path_str = path.to_string_lossy().to_string();

            // Check file size
            match std::fs::metadata(path) {
                Ok(meta) => {
                    if meta.len() > MAX_FILE_SIZE {
                        let diag = Diagnostic {
                            level: DiagLevel::Error,
                            file: Some(path_str.clone()),
                            line: None,
                            code: "FILE_TOO_LARGE".to_string(),
                            message: format!("File too large: {} bytes (max 10 MB)", meta.len()),
                            fragment: None,
                        };
                        emit_diagnostic(&diag, &args);
                        total_errors += 1;
                        any_translation_errors = true;
                        continue;
                    }
                }
                Err(e) => {
                    emit_error(&format!("Cannot read {}: {}", path_str, e), &args);
                    total_errors += 1;
                    any_translation_errors = true;
                    continue;
                }
            }

            // Read file
            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    let diag = Diagnostic {
                        level: DiagLevel::Error,
                        file: Some(path_str.clone()),
                        line: None,
                        code: "IO_ERROR".to_string(),
                        message: format!("Cannot read file: {}", e),
                        fragment: None,
                    };
                    emit_diagnostic(&diag, &args);
                    total_errors += 1;
                    any_translation_errors = true;
                    continue;
                }
            };

            let file_stem = file_stem_from_path(path);

            match process_source(
                &source,
                &path_str,
                &file_stem,
                dialect.as_ref(),
                on_error,
                args.inline_singles,
            ) {
                Ok(result) => {
                    for diag in &result.diagnostics {
                        emit_diagnostic(diag, &args);
                        match diag.level {
                            DiagLevel::Error => total_errors += 1,
                            DiagLevel::Warn | DiagLevel::Security => total_warnings += 1,
                            _ => {}
                        }
                    }

                    if result.has_errors {
                        any_translation_errors = true;
                        // With --on-error fail, still process all files but don't write errored ones
                        if on_error != translator::context::OnError::Fail {
                            for (name, sql) in &result.queries {
                                output_query(
                                    &args,
                                    name,
                                    sql,
                                    &mut name_counts,
                                    &mut total_translated,
                                );
                            }
                        }
                    } else {
                        for (name, sql) in &result.queries {
                            output_query(&args, name, sql, &mut name_counts, &mut total_translated);
                        }
                    }
                }
                Err(e) => {
                    let diag = Diagnostic {
                        level: DiagLevel::Error,
                        file: Some(path_str),
                        line: None,
                        code: "PROCESS_ERROR".to_string(),
                        message: format!("{}", e),
                        fragment: None,
                    };
                    emit_diagnostic(&diag, &args);
                    total_errors += 1;
                    any_translation_errors = true;
                }
            }
        }
    }

    let elapsed = start.elapsed();

    // Print summary
    let summary_msg = format!(
        "{} translated, {} errors, {} warnings",
        total_translated, total_errors, total_warnings
    );

    if args.log_json {
        eprintln!(
            "{{\"level\":\"info\",\"file\":null,\"line\":null,\"code\":\"SUMMARY\",\"message\":\"{}\",\"duration_ms\":{}}}",
            summary_msg,
            elapsed.as_millis()
        );
    } else {
        eprintln!("m2sql: {}  [{:.1}s]", summary_msg, elapsed.as_secs_f64());
    }

    // Exit code
    if any_translation_errors && on_error == translator::context::OnError::Fail {
        std::process::exit(1);
    }
}

/// Output a query (stdout or file).
fn output_query(
    args: &Cli,
    name: &str,
    sql: &str,
    name_counts: &mut std::collections::HashMap<String, usize>,
    total_translated: &mut usize,
) {
    if args.stdout {
        println!("-- [query: {}]", name);
        println!("{}", sql);
    } else if let Err(e) = write_output_file(&args.output_dir, name, sql, name_counts) {
        emit_error(&format!("{}", e), args);
    }
    *total_translated += 1;
}

/// Write a SQL output file.
fn write_output_file(
    output_dir: &Path,
    query_name: &str,
    sql: &str,
    name_counts: &mut std::collections::HashMap<String, usize>,
) -> Result<(), error::M2SqlError> {
    std::fs::create_dir_all(output_dir)?;

    let sanitized = sanitize_filename(query_name);

    // Handle duplicate names
    let final_name = {
        let count = name_counts.entry(sanitized.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            format!("{}_{}", sanitized, count)
        } else {
            sanitized
        }
    };

    // Check for path traversal
    let output_path = check_path_traversal(output_dir, &final_name)?;

    let mut file = std::fs::File::create(&output_path)?;
    file.write_all(sql.as_bytes())?;
    file.write_all(b"\n")?;

    Ok(())
}

/// Emit a diagnostic to stderr.
fn emit_diagnostic(diag: &Diagnostic, args: &Cli) {
    if args.log_json {
        eprintln!("{}", diag.to_json());
    } else {
        eprintln!("{}", diag.to_plain_text());
    }
}

/// Emit an error message to stderr.
fn emit_error(message: &str, args: &Cli) {
    if args.log_json {
        eprintln!(
            "{{\"level\":\"error\",\"file\":null,\"line\":null,\"code\":\"ERROR\",\"message\":\"{}\"}}",
            message.replace('"', "\\\"")
        );
    } else {
        eprintln!("ERROR: {}", message);
    }
}

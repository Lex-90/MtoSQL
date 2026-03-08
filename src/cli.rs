use crate::translator::context::OnError;
/// CLI argument definitions and validation.
use clap::Parser;
use std::path::PathBuf;

/// m2sql — M Code to SQL CLI Translator.
///
/// Translates Power Query M code into equivalent SQL queries.
#[derive(Parser, Debug)]
#[command(name = "m2sql", version, about, long_about = None)]
pub struct Cli {
    /// Input files (.pq, .m, or .tmdl). Reads from stdin if omitted.
    #[arg(value_name = "INPUT")]
    pub input: Vec<PathBuf>,

    /// Target SQL dialect [tsql|postgres|bigquery|snowflake|duckdb].
    #[arg(short = 'd', long, value_name = "DIALECT")]
    pub dialect: String,

    /// Directory where .sql files are written. Created if absent.
    #[arg(short = 'o', long, default_value = "./output", value_name = "PATH")]
    pub output_dir: PathBuf,

    /// Behaviour on untranslatable expression [fail|comment|warn].
    #[arg(short = 'e', long, default_value = "warn", value_name = "MODE")]
    pub on_error: String,

    /// Print all SQL to stdout instead of writing files.
    #[arg(long)]
    pub stdout: bool,

    /// Name for the query when reading from stdin.
    #[arg(short = 'n', long, default_value = "query", value_name = "NAME")]
    pub query_name: String,

    /// Inline CTE bindings referenced exactly once.
    #[arg(long)]
    pub inline_singles: bool,

    /// Emit diagnostics as newline-delimited JSON to stderr.
    #[arg(long)]
    pub log_json: bool,

    /// Disable ANSI colour in terminal output.
    #[arg(long)]
    pub no_color: bool,
}

impl Cli {
    /// Parse the on-error mode.
    pub fn on_error_mode(&self) -> Result<OnError, String> {
        match self.on_error.as_str() {
            "fail" => Ok(OnError::Fail),
            "comment" => Ok(OnError::Comment),
            "warn" => Ok(OnError::Warn),
            other => Err(format!(
                "Invalid --on-error value: '{}'. Expected: fail, comment, warn",
                other
            )),
        }
    }

    /// Validate the dialect name.
    pub fn validate_dialect(&self) -> Result<(), String> {
        match self.dialect.as_str() {
            "tsql" | "postgres" | "bigquery" | "snowflake" | "duckdb" => Ok(()),
            other => Err(format!("Invalid --dialect value: '{}'. Expected: tsql, postgres, bigquery, snowflake, duckdb", other)),
        }
    }
}

/// Error and warning types for m2sql.
use std::fmt;

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagLevel {
    /// Informational message.
    Info,
    /// Warning — translation continued with a placeholder.
    Warn,
    /// Error — translation failed for this construct.
    Error,
    /// Security — credential redacted.
    Security,
}

impl fmt::Display for DiagLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagLevel::Info => write!(f, "info"),
            DiagLevel::Warn => write!(f, "warn"),
            DiagLevel::Error => write!(f, "error"),
            DiagLevel::Security => write!(f, "security"),
        }
    }
}

/// A single diagnostic message.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Severity level.
    pub level: DiagLevel,
    /// Source file name, if applicable.
    pub file: Option<String>,
    /// 1-indexed line number, if applicable.
    pub line: Option<usize>,
    /// Machine-readable code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Original M fragment, if applicable.
    pub fragment: Option<String>,
}

impl Diagnostic {
    /// Format as plain text for stderr output.
    pub fn to_plain_text(&self) -> String {
        let level_str = match self.level {
            DiagLevel::Info => "INFO ",
            DiagLevel::Warn => "WARN ",
            DiagLevel::Error => "ERROR",
            DiagLevel::Security => "WARN ",
        };
        let location = match (&self.file, self.line) {
            (Some(f), Some(l)) => format!(" [{}:{}]", f, l),
            (Some(f), None) => format!(" [{}]", f),
            _ => String::new(),
        };
        format!("{}{} {}", level_str, location, self.message)
    }

    /// Format as a JSON object for `--log-json` output.
    pub fn to_json(&self) -> String {
        let file = match &self.file {
            Some(f) => format!("\"{}\"", escape_json(f)),
            None => "null".to_string(),
        };
        let line = match self.line {
            Some(l) => l.to_string(),
            None => "null".to_string(),
        };
        let fragment = match &self.fragment {
            Some(f) => format!("\"{}\"", escape_json(f)),
            None => "null".to_string(),
        };
        format!(
            "{{\"level\":\"{}\",\"file\":{},\"line\":{},\"code\":\"{}\",\"message\":\"{}\",\"fragment\":{}}}",
            self.level,
            file,
            line,
            escape_json(&self.code),
            escape_json(&self.message),
            fragment
        )
    }
}

/// Escape a string for JSON output.
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Error type for m2sql operations.
#[derive(Debug, thiserror::Error)]
pub enum M2SqlError {
    /// I/O error reading or writing files.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Parse error in M source.
    #[error("Parse error in {file}:{line}: {message}")]
    Parse {
        /// Source file name.
        file: String,
        /// Line number.
        line: usize,
        /// Error description.
        message: String,
    },

    /// Translation error.
    #[error("Translation error in {file}:{line}: {message}")]
    Translation {
        /// Source file name.
        file: String,
        /// Line number.
        line: usize,
        /// Error description.
        message: String,
        /// Original M fragment.
        fragment: Option<String>,
    },

    /// CLI argument error.
    #[error("CLI error: {0}")]
    Cli(String),

    /// File too large.
    #[error("File too large: {0} ({1} bytes, max 10 MB)")]
    FileTooLarge(String, u64),

    /// Path traversal detected.
    #[error("Path traversal detected: {0}")]
    PathTraversal(String),
}

/// Credential patterns for redaction.
pub struct CredentialRedactor;

impl CredentialRedactor {
    /// Redact credentials from SQL output, returning the redacted string and any diagnostics.
    pub fn redact(input: &str, file: Option<&str>) -> (String, Vec<Diagnostic>) {
        let mut result = input.to_string();
        let mut diagnostics = Vec::new();

        let patterns = [
            (
                "password",
                r#"(?i)(Password\s*=\s*)"[^"]*""#,
                "${1}/* REDACTED */",
            ),
            (
                "accountkey",
                r#"(?i)(AccountKey\s*=\s*)"[^"]*""#,
                "${1}/* REDACTED */",
            ),
            (
                "accesskey",
                r#"(?i)(AccessKey\s*=\s*)"[^"]*""#,
                "${1}/* REDACTED */",
            ),
            (
                "secretkey",
                r#"(?i)(SecretKey\s*=\s*)"[^"]*""#,
                "${1}/* REDACTED */",
            ),
            (
                "credentials",
                r#"(?i)(Credentials\s*=\s*)\[[^\]]*\]"#,
                "${1}/* REDACTED */",
            ),
            ("pwd", r#"(?i)(pwd\s*=\s*)[^;]*;"#, "${1}***;"),
        ];

        for (key_name, pattern, replacement) in &patterns {
            let re = regex::Regex::new(pattern).unwrap();
            if re.is_match(&result) {
                diagnostics.push(Diagnostic {
                    level: DiagLevel::Security,
                    file: file.map(|s| s.to_string()),
                    line: None,
                    code: "CREDENTIAL_REDACTED".to_string(),
                    message: format!("key '{}' redacted", key_name),
                    fragment: None,
                });
                result = re.replace_all(&result, *replacement).to_string();
            }
        }

        (result, diagnostics)
    }

    /// Insert security comments before redacted lines.
    pub fn insert_security_comments(sql: &str) -> String {
        let mut lines: Vec<String> = Vec::new();
        for line in sql.lines() {
            if line.contains("/* REDACTED */") || line.contains("***") {
                lines.push("/* SECURITY: credential redacted by m2sql */".to_string());
            }
            lines.push(line.to_string());
        }
        lines.join("\n")
    }
}

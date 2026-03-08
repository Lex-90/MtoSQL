/// Translation context — holds dialect, error mode, and accumulated diagnostics.
use crate::dialect::Dialect;
use crate::error::{DiagLevel, Diagnostic};

/// Error handling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnError {
    /// Report errors and exit with code 1 after processing all files.
    Fail,
    /// Replace untranslatable expressions with SQL comments.
    Comment,
    /// Same as Comment, plus print warnings to stderr.
    Warn,
}

/// Translation context.
pub struct TranslationContext<'a> {
    /// The target SQL dialect.
    pub dialect: &'a dyn Dialect,
    /// Error handling mode.
    pub on_error: OnError,
    /// Whether to inline single-use CTEs.
    pub inline_singles: bool,
    /// Source file name.
    pub file_name: String,
    /// Accumulated diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Whether any translation errors occurred.
    pub has_errors: bool,
    /// Known columns for the current scope (table name -> column list).
    pub known_columns: indexmap::IndexMap<String, Vec<String>>,
    /// Nested join info: maps binding name -> (left_table, right_table, right_key_col, new_col_name).
    pub nested_joins: indexmap::IndexMap<String, NestedJoinInfo>,
}

/// Information about a nested join for ExpandTableColumn resolution.
#[derive(Debug, Clone)]
pub struct NestedJoinInfo {
    /// Left table reference name.
    pub left_table: String,
    /// Right table reference name.
    pub right_table: String,
    /// The new column name from NestedJoin.
    #[allow(dead_code)]
    pub new_col: String,
    /// Left key columns.
    pub left_keys: Vec<String>,
    /// Right key columns.
    pub right_keys: Vec<String>,
    /// Join type.
    pub join_type: crate::emitter::sql_writer::JoinType,
    /// Name of the binding that holds the NestedJoin result.
    pub join_binding: String,
}

impl<'a> TranslationContext<'a> {
    /// Create a new translation context.
    pub fn new(
        dialect: &'a dyn Dialect,
        on_error: OnError,
        file_name: &str,
        inline_singles: bool,
    ) -> Self {
        Self {
            dialect,
            on_error,
            inline_singles,
            file_name: file_name.to_string(),
            diagnostics: Vec::new(),
            has_errors: false,
            known_columns: indexmap::IndexMap::new(),
            nested_joins: indexmap::IndexMap::new(),
        }
    }

    /// Record a warning diagnostic.
    pub fn warn(&mut self, line: usize, code: &str, message: &str, fragment: Option<&str>) {
        self.diagnostics.push(Diagnostic {
            level: DiagLevel::Warn,
            file: Some(self.file_name.clone()),
            line: Some(line),
            code: code.to_string(),
            message: message.to_string(),
            fragment: fragment.map(|s| truncate(s, 120)),
        });
    }

    /// Record an error diagnostic.
    pub fn error(&mut self, line: usize, code: &str, message: &str, fragment: Option<&str>) {
        self.has_errors = true;
        self.diagnostics.push(Diagnostic {
            level: DiagLevel::Error,
            file: Some(self.file_name.clone()),
            line: Some(line),
            code: code.to_string(),
            message: message.to_string(),
            fragment: fragment.map(|s| truncate(s, 120)),
        });
    }

    /// Generate a placeholder for untranslatable expressions based on error mode.
    pub fn untranslatable(&mut self, line: usize, m_fragment: &str, reason: &str) -> String {
        let truncated = truncate(m_fragment, 120);
        match self.on_error {
            OnError::Fail => {
                self.error(line, "UNTRANSLATABLE", reason, Some(&truncated));
                format!("/* UNTRANSLATABLE: {} */", truncated)
            }
            OnError::Comment => {
                format!("/* UNTRANSLATABLE: {} */", truncated)
            }
            OnError::Warn => {
                self.warn(line, "UNTRANSLATABLE", reason, Some(&truncated));
                format!("/* UNTRANSLATABLE: {} */", truncated)
            }
        }
    }
}

/// Truncate a string to a maximum length.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

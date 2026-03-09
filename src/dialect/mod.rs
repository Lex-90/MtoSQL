pub mod bigquery;
pub mod duckdb;
pub mod postgres;
pub mod snowflake;
/// SQL dialect module — trait and implementations.
pub mod tsql;

use crate::parser::ast::MType;

/// Syntax for limiting rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopNSyntax {
    /// `TOP n` (T-SQL).
    Top,
    /// `LIMIT n` (most dialects).
    Limit,
}

/// Trait for SQL dialect-specific behavior.
pub trait Dialect: Send + Sync {
    /// Dialect name.
    fn name(&self) -> &'static str;

    /// The opening quote character for identifiers.
    fn identifier_quote(&self) -> char;

    /// The closing quote character for identifiers.
    fn identifier_close_quote(&self) -> char {
        self.identifier_quote()
    }

    /// The quote character for string literals.
    fn string_quote(&self) -> char {
        '\''
    }

    /// Generate a CAST expression.
    fn cast_syntax(&self, expr: &str, ty: &str) -> String {
        format!("CAST({} AS {})", expr, ty)
    }

    /// Whether CTEs are supported.
    fn supports_cte(&self) -> bool {
        true
    }

    /// Whether ILIKE is supported for case-insensitive matching.
    fn ilike_supported(&self) -> bool {
        false
    }

    /// Whether `SELECT * EXCEPT (col1, col2)` syntax is supported.
    fn supports_select_except(&self) -> bool {
        false
    }

    /// Top-N syntax style.
    fn top_n_syntax(&self) -> TopNSyntax {
        TopNSyntax::Limit
    }

    /// Boolean literal representation.
    fn boolean_literal(&self, val: bool) -> &'static str {
        if val {
            "TRUE"
        } else {
            "FALSE"
        }
    }

    /// Current timestamp expression.
    fn current_timestamp(&self) -> &'static str {
        "CURRENT_TIMESTAMP"
    }

    /// Map an M type to the corresponding SQL type string.
    fn map_type(&self, m_type: &MType) -> &'static str;

    /// Whether this dialect supports error-safe casting (TRY_CAST or equivalent).
    fn supports_try_cast(&self) -> bool {
        false
    }

    /// Generate an error-safe cast expression (TRY_CAST, SAFE_CAST, etc.).
    /// Returns None if the dialect does not support error-safe casting.
    fn try_cast_syntax(&self, expr: &str, ty: &str) -> Option<String> {
        if self.supports_try_cast() {
            Some(format!("TRY_CAST({} AS {})", expr, ty))
        } else {
            None
        }
    }

    /// Quote an identifier.
    fn quote_identifier(&self, name: &str) -> String {
        format!(
            "{}{}{}",
            self.identifier_quote(),
            name,
            self.identifier_close_quote()
        )
    }
}

/// Create a dialect from its name.
pub fn dialect_from_name(name: &str) -> Option<Box<dyn Dialect>> {
    match name {
        "tsql" => Some(Box::new(tsql::TSql)),
        "postgres" => Some(Box::new(postgres::Postgres)),
        "bigquery" => Some(Box::new(bigquery::BigQuery)),
        "snowflake" => Some(Box::new(snowflake::Snowflake)),
        "duckdb" => Some(Box::new(duckdb::DuckDb)),
        _ => None,
    }
}

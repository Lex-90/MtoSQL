/// DuckDB dialect implementation.
use super::{Dialect, TopNSyntax};
use crate::parser::ast::MType;

/// DuckDB dialect.
pub struct DuckDb;

impl Dialect for DuckDb {
    fn name(&self) -> &'static str {
        "duckdb"
    }

    fn identifier_quote(&self) -> char {
        '"'
    }

    fn cast_syntax(&self, expr: &str, ty: &str) -> String {
        format!("{}::{}", expr, ty)
    }

    fn ilike_supported(&self) -> bool {
        true
    }

    fn supports_select_except(&self) -> bool {
        true
    }

    fn top_n_syntax(&self) -> TopNSyntax {
        TopNSyntax::Limit
    }

    fn current_timestamp(&self) -> &'static str {
        "now()"
    }

    fn map_type(&self, m_type: &MType) -> &'static str {
        match m_type {
            MType::Text => "VARCHAR",
            MType::Number => "DOUBLE",
            MType::Integer => "BIGINT",
            MType::Logical => "BOOLEAN",
            MType::Date => "DATE",
            MType::DateTime => "TIMESTAMP",
            MType::DateTimeZone => "TIMESTAMPTZ",
            MType::Duration => "INTERVAL",
            MType::Binary => "BLOB",
            MType::Table => "/* UNSUPPORTED_TYPE */",
            MType::Any => "VARCHAR",
            MType::Unknown(_) => "/* UNSUPPORTED_TYPE */",
        }
    }
}

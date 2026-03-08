/// PostgreSQL dialect implementation.
use super::{Dialect, TopNSyntax};
use crate::parser::ast::MType;

/// PostgreSQL dialect.
pub struct Postgres;

impl Dialect for Postgres {
    fn name(&self) -> &'static str {
        "postgres"
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

    fn top_n_syntax(&self) -> TopNSyntax {
        TopNSyntax::Limit
    }

    fn current_timestamp(&self) -> &'static str {
        "NOW()"
    }

    fn map_type(&self, m_type: &MType) -> &'static str {
        match m_type {
            MType::Text => "TEXT",
            MType::Number => "DOUBLE PRECISION",
            MType::Integer => "BIGINT",
            MType::Logical => "BOOLEAN",
            MType::Date => "DATE",
            MType::DateTime => "TIMESTAMP",
            MType::DateTimeZone => "TIMESTAMPTZ",
            MType::Duration => "INTERVAL",
            MType::Binary => "BYTEA",
            MType::Table => "/* UNSUPPORTED_TYPE */",
            MType::Any => "TEXT",
            MType::Unknown(_) => "/* UNSUPPORTED_TYPE */",
        }
    }
}

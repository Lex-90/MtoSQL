/// Translator module — converts M AST to SQL AST.
pub mod cast;
pub mod context;
pub mod cte;
pub mod expr;
pub mod functions;

use crate::emitter::sql_writer::SqlQuery;
use crate::parser::ast::*;
use context::TranslationContext;
use cte::translate_let_to_query;

/// Translate an M document to one or more SQL queries.
/// Returns a list of (query_name, sql_query) pairs.
pub fn translate(
    doc: &MDocument,
    file_stem: &str,
    ctx: &mut TranslationContext,
) -> Vec<(String, SqlQuery)> {
    match doc {
        MDocument::Expression(expr) => {
            let query = translate_expr_to_query(expr, ctx);
            vec![(file_stem.to_string(), query)]
        }
        MDocument::Section { bindings, .. } => {
            let mut results = Vec::new();
            for (is_shared, name, expr) in bindings {
                if *is_shared {
                    let query = translate_expr_to_query(expr, ctx);
                    results.push((name.clone(), query));
                }
            }
            results
        }
    }
}

/// Translate a single M expression to a SQL query.
fn translate_expr_to_query(expr: &MExpr, ctx: &mut TranslationContext) -> SqlQuery {
    match expr {
        MExpr::Let { bindings, body } => translate_let_to_query(bindings, body, ctx),
        _ => {
            // Wrap non-let expressions
            let bindings = vec![("Result".to_string(), expr.clone())];
            let body = MExpr::Identifier("Result".to_string());
            translate_let_to_query(&bindings, &body, ctx)
        }
    }
}

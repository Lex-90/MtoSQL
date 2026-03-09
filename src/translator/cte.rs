use super::context::TranslationContext;
use super::functions::{translate_table_expr, TableOpResult};
use crate::emitter::sql_writer::*;
use crate::parser::ast::*;
use crate::resolver::source::{format_expr, resolve_source};
/// let…in → CTE chain builder.
use indexmap::IndexMap;

/// Translate a complete M `let ... in` expression to a SQL query with CTEs.
pub fn translate_let_to_query(
    bindings: &[(String, MExpr)],
    body: &MExpr,
    ctx: &mut TranslationContext,
) -> SqlQuery {
    let mut ctes: Vec<Cte> = Vec::new();
    let mut binding_types: IndexMap<String, BindingType> = IndexMap::new();
    let mut ref_counts: IndexMap<String, usize> = IndexMap::new();
    // Track data source schemas: binding_name -> schema
    let mut data_sources: IndexMap<String, Option<String>> = IndexMap::new();

    // First pass: count references to each binding
    for (name, _) in bindings {
        ref_counts.insert(name.clone(), 0);
    }
    for (_, expr) in bindings {
        count_refs(expr, &mut ref_counts);
    }
    count_refs(body, &mut ref_counts);

    // Pre-scan: identify NestedJoin bindings that are absorbed by ExpandTableColumn
    let mut absorbed_joins: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_name, expr) in bindings {
        if let MExpr::FunctionCall {
            name: fn_name,
            args,
        } = expr
        {
            if fn_name == "Table.ExpandTableColumn" && !args.is_empty() {
                if let MExpr::Identifier(ref_name) = &args[0] {
                    absorbed_joins.insert(ref_name.clone());
                }
            }
        }
    }

    // Populate combine_bindings for Table.Combine validation
    for (name, _) in bindings {
        ctx.combine_bindings.insert(name.clone());
    }

    // Pre-scan: record type coercion context from TransformColumnTypes for ReplaceErrorValues
    for (name, expr) in bindings {
        if let MExpr::FunctionCall {
            name: fn_name,
            args,
        } = expr
        {
            if fn_name == "Table.TransformColumnTypes" && args.len() >= 2 {
                // Validate first arg is an identifier (table reference)
                if !matches!(&args[0], MExpr::Identifier(_)) {
                    continue;
                }
                if let MExpr::List(type_specs) = &args[1] {
                    for spec in type_specs {
                        if let MExpr::List(items) = spec {
                            if items.len() >= 2 {
                                let col_name = super::expr::extract_string(&items[0]);
                                let m_type = match &items[1] {
                                    MExpr::Identifier(tn) => {
                                        super::cast::resolve_m_type(tn)
                                    }
                                    _ => continue,
                                };
                                let sql_type = ctx.dialect.map_type(&m_type);
                                if !sql_type.contains("UNSUPPORTED") {
                                    // Record: binding_name has a type coercion for col_name
                                    ctx.known_columns
                                        .entry(format!("{}_types", name))
                                        .or_insert_with(Vec::new)
                                        .push(col_name.clone());
                                    ctx.known_columns
                                        .insert(format!("{}_{}_type", name, col_name), vec![sql_type.to_string()]);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Second pass: identify data source bindings and translate each binding
    for (name, expr) in bindings {
        // Skip NestedJoin bindings that will be absorbed by ExpandTableColumn
        if absorbed_joins.contains(name) {
            if let MExpr::FunctionCall { name: fn_name, .. } = expr {
                if fn_name == "Table.NestedJoin" {
                    // Still translate to register the join info in context, but don't emit CTE
                    let _result = classify_and_translate(expr, &data_sources, ctx);
                    binding_types.insert(name.clone(), BindingType::Table);
                    continue;
                }
            }
        }

        let result = classify_and_translate(expr, &data_sources, ctx);

        match result {
            BindingResult::DataSourceDecl { schema } => {
                data_sources.insert(name.clone(), schema);
                binding_types.insert(name.clone(), BindingType::DataSource);
            }
            BindingResult::DataSource { schema, table } => {
                let mut stmt = SelectStmt::new();
                stmt.columns = vec![SelectCol::Wildcard];
                stmt.from = Some(TableRef {
                    schema,
                    name: table,
                    alias: None,
                });
                binding_types.insert(name.clone(), BindingType::Table);

                let should_inline =
                    ctx.inline_singles && ref_counts.get(name).copied().unwrap_or(0) <= 1;

                if !should_inline {
                    ctes.push(Cte {
                        name: sanitize_cte_name(name),
                        select: stmt,
                    });
                }
            }
            BindingResult::TableOp(stmt) => {
                binding_types.insert(name.clone(), BindingType::Table);

                let should_inline =
                    ctx.inline_singles && ref_counts.get(name).copied().unwrap_or(0) <= 1;

                if !should_inline {
                    ctes.push(Cte {
                        name: sanitize_cte_name(name),
                        select: stmt,
                    });
                }
            }
            BindingResult::Scalar => {
                binding_types.insert(name.clone(), BindingType::Scalar);
            }
            BindingResult::Reference(_ref_name) => {
                binding_types.insert(name.clone(), BindingType::Alias);
            }
            BindingResult::Raw(raw) => {
                // Untranslatable — still emit a CTE with the raw comment
                let mut stmt = SelectStmt::new();
                stmt.columns = vec![SelectCol::Expr {
                    expr: SqlExpr::Raw(raw),
                    alias: None,
                }];
                binding_types.insert(name.clone(), BindingType::Table);
                ctes.push(Cte {
                    name: sanitize_cte_name(name),
                    select: stmt,
                });
            }
        }
    }

    // Final select from the body expression
    let final_select = match body {
        MExpr::Identifier(name) => {
            let mut stmt = SelectStmt::new();
            stmt.columns = vec![SelectCol::Wildcard];
            stmt.from = Some(TableRef {
                schema: None,
                name: sanitize_cte_name(name),
                alias: None,
            });
            stmt
        }
        _ => {
            let mut stmt = SelectStmt::new();
            stmt.columns = vec![SelectCol::Wildcard];
            let ref_name = format_expr(body);
            stmt.from = Some(TableRef {
                schema: None,
                name: sanitize_cte_name(&ref_name),
                alias: None,
            });
            stmt
        }
    };

    SqlQuery { ctes, final_select }
}

/// Classify a binding and translate it.
fn classify_and_translate(
    expr: &MExpr,
    data_sources: &IndexMap<String, Option<String>>,
    ctx: &mut TranslationContext,
) -> BindingResult {
    match expr {
        // Sql.Database — data source declaration (no CTE)
        MExpr::FunctionCall {
            name: fn_name,
            args,
        } if fn_name == "Sql.Database" || fn_name == "Excel.Workbook" => {
            let resolved = resolve_source(expr);
            if fn_name == "Sql.Database" {
                BindingResult::DataSourceDecl {
                    schema: resolved.schema,
                }
            } else {
                BindingResult::DataSourceDecl { schema: None }
            }
        }

        // Source{[Name="Table"]}[Data] — table reference
        MExpr::FieldAccess {
            expr: source_expr,
            field,
        } if field == "Data" => {
            // Try to resolve the source reference
            if let MExpr::IndexAccess {
                expr: parent_expr,
                key: _,
            } = source_expr.as_ref()
            {
                // Get schema from parent if it's a known data source
                let schema = if let MExpr::Identifier(parent_name) = parent_expr.as_ref() {
                    data_sources.get(parent_name).cloned().flatten()
                } else {
                    None
                };

                let resolved = resolve_source(expr);
                if let Some(table) = resolved.table {
                    return BindingResult::DataSource {
                        schema: resolved.schema.or(schema),
                        table,
                    };
                }
            }

            let resolved = resolve_source(expr);
            if let Some(table) = resolved.table {
                BindingResult::DataSource {
                    schema: resolved.schema,
                    table,
                }
            } else {
                BindingResult::Reference(format_expr(expr))
            }
        }

        // Table function calls
        MExpr::FunctionCall { .. } => match translate_table_expr(expr, ctx) {
            TableOpResult::Select(stmt) => BindingResult::TableOp(stmt),
            TableOpResult::SourceTable { schema, table } => {
                BindingResult::DataSource { schema, table }
            }
            TableOpResult::Reference(r) => BindingResult::Reference(r),
            TableOpResult::Raw(r) => BindingResult::Raw(r),
        },

        // Simple identifier reference
        MExpr::Identifier(ref_name) => BindingResult::Reference(ref_name.clone()),

        // Literal — scalar
        MExpr::Literal(_) => BindingResult::Scalar,

        _ => match translate_table_expr(expr, ctx) {
            TableOpResult::Select(stmt) => BindingResult::TableOp(stmt),
            TableOpResult::SourceTable { schema, table } => {
                BindingResult::DataSource { schema, table }
            }
            TableOpResult::Reference(r) => BindingResult::Reference(r),
            TableOpResult::Raw(r) => BindingResult::Raw(r),
        },
    }
}

/// Type of a binding.
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum BindingType {
    /// Data source connection (not a CTE).
    DataSource,
    /// Table-valued binding.
    Table,
    /// Scalar-valued binding (inlined).
    Scalar,
    /// Alias for another binding.
    Alias,
}

/// Result of translating a binding.
enum BindingResult {
    /// A data source connection declaration (Sql.Database etc.) — no CTE emitted.
    DataSourceDecl { schema: Option<String> },
    /// A resolved data source table (schema + table) — emit CTE.
    DataSource {
        schema: Option<String>,
        table: String,
    },
    /// A table operation (SELECT statement).
    TableOp(SelectStmt),
    /// A scalar binding (not a CTE).
    Scalar,
    /// An alias/reference to another binding.
    Reference(String),
    /// Raw SQL.
    #[allow(dead_code)]
    Raw(String),
}

/// Count references to bindings in an expression.
fn count_refs(expr: &MExpr, counts: &mut IndexMap<String, usize>) {
    match expr {
        MExpr::Identifier(name) => {
            if let Some(count) = counts.get_mut(name) {
                *count += 1;
            }
        }
        MExpr::FunctionCall { args, .. } => {
            for arg in args {
                count_refs(arg, counts);
            }
        }
        MExpr::BinaryOp { left, right, .. } => {
            count_refs(left, counts);
            count_refs(right, counts);
        }
        MExpr::UnaryOp { expr, .. } => {
            count_refs(expr, counts);
        }
        MExpr::Let { bindings, body } => {
            for (_, e) in bindings {
                count_refs(e, counts);
            }
            count_refs(body, counts);
        }
        MExpr::EachExpr(inner) => count_refs(inner, counts),
        MExpr::FieldAccess { expr, .. } => count_refs(expr, counts),
        MExpr::IndexAccess { expr, key } => {
            count_refs(expr, counts);
            count_refs(key, counts);
        }
        MExpr::List(items) => {
            for item in items {
                count_refs(item, counts);
            }
        }
        MExpr::Record(fields) => {
            for (_, v) in fields {
                count_refs(v, counts);
            }
        }
        MExpr::TypeAnnotation { expr, .. } => count_refs(expr, counts),
        _ => {}
    }
}

/// Sanitise a binding name to a valid SQL CTE identifier.
pub fn sanitize_cte_name(name: &str) -> String {
    let mut result = String::new();
    for ch in name.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            result.push(ch);
        } else {
            result.push('_');
        }
    }
    if result.is_empty() {
        result = "cte".to_string();
    }
    if result.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        result = format!("_{}", result);
    }
    result
}

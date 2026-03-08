use super::context::TranslationContext;
/// M expression → SQL expression translation.
use crate::emitter::sql_writer::*;
use crate::parser::ast::*;
use crate::resolver::source::format_expr;

/// Translate an M expression to a SQL expression.
pub fn translate_expr(expr: &MExpr, ctx: &mut TranslationContext) -> SqlExpr {
    match expr {
        MExpr::Literal(lit) => translate_literal(lit, ctx),

        MExpr::Identifier(name) => {
            // Check for well-known M constants
            match name.as_str() {
                "_" => SqlExpr::Raw("*".to_string()),
                _ => SqlExpr::Column {
                    table: None,
                    name: name.clone(),
                },
            }
        }

        MExpr::RowField(name) => SqlExpr::Column {
            table: None,
            name: name.clone(),
        },

        MExpr::BinaryOp { op, left, right } => {
            let sql_op = translate_binop(op);
            let left_expr = translate_expr(left, ctx);
            let right_expr = translate_expr(right, ctx);

            // Wrap OR sub-expressions in parens when combined with AND
            let left_wrapped = if *op == BinOp::And {
                maybe_wrap_or(left_expr)
            } else {
                left_expr
            };
            let right_wrapped = if *op == BinOp::And {
                maybe_wrap_or(right_expr)
            } else {
                right_expr
            };

            SqlExpr::BinaryOp {
                op: sql_op,
                left: Box::new(left_wrapped),
                right: Box::new(right_wrapped),
            }
        }

        MExpr::UnaryOp { op, expr: inner } => {
            let sql_op = match op {
                UnOp::Not => SqlUnOp::Not,
                UnOp::Neg => SqlUnOp::Neg,
            };
            SqlExpr::UnaryOp {
                op: sql_op,
                expr: Box::new(translate_expr(inner, ctx)),
            }
        }

        MExpr::EachExpr(inner) => {
            // `each` just establishes row context; translate the inner expression
            translate_expr(inner, ctx)
        }

        MExpr::FunctionCall { name, args } => translate_function_in_expr(name, args, ctx),

        MExpr::FieldAccess { expr: inner, field } => {
            // expr[field] in expression context is typically a column reference
            if let MExpr::Identifier(table) = inner.as_ref() {
                SqlExpr::Column {
                    table: Some(table.clone()),
                    name: field.clone(),
                }
            } else {
                SqlExpr::Column {
                    table: None,
                    name: field.clone(),
                }
            }
        }

        MExpr::TypeAnnotation { expr: inner, ty } => {
            let sql_expr = translate_expr(inner, ctx);
            let sql_type = ctx.dialect.map_type(ty);
            if sql_type.contains("UNSUPPORTED") {
                ctx.warn(
                    0,
                    "UNSUPPORTED_TYPE",
                    &format!(
                        "type {} is not supported in {}",
                        crate::parser::grammar::type_to_string(ty),
                        ctx.dialect.name()
                    ),
                    None,
                );
                sql_expr
            } else {
                SqlExpr::Cast {
                    expr: Box::new(sql_expr),
                    ty: sql_type.to_string(),
                }
            }
        }

        MExpr::List(items) => {
            // A list in expression context — could be used in IN clause
            if items.is_empty() {
                SqlExpr::Raw("()".to_string())
            } else {
                let first = translate_expr(&items[0], ctx);
                // If it's a single-element list, just return the element
                if items.len() == 1 {
                    first
                } else {
                    // Multiple items — could be an error, treat as first item
                    first
                }
            }
        }

        MExpr::Record(fields) => {
            // Records in expression context are unusual; render as raw
            let parts: Vec<String> = fields.iter().map(|(k, _)| k.clone()).collect();
            SqlExpr::Raw(format!("/* RECORD: {} */", parts.join(", ")))
        }

        _ => {
            let frag = format_expr(expr);
            let placeholder = ctx.untranslatable(
                0,
                &frag,
                &format!("{} is not translatable in expression context", frag),
            );
            SqlExpr::Raw(placeholder)
        }
    }
}

/// Translate a literal.
fn translate_literal(lit: &MLiteral, _ctx: &TranslationContext) -> SqlExpr {
    match lit {
        MLiteral::Text(s) => SqlExpr::Literal(SqlLiteral::String(s.clone())),
        MLiteral::Integer(n) => SqlExpr::Literal(SqlLiteral::Integer(*n)),
        MLiteral::Number(n) => SqlExpr::Literal(SqlLiteral::Float(*n)),
        MLiteral::Bool(b) => SqlExpr::Literal(SqlLiteral::Boolean(*b)),
        MLiteral::Null => SqlExpr::Literal(SqlLiteral::Null),
    }
}

/// Translate a binary operator.
fn translate_binop(op: &BinOp) -> SqlBinOp {
    match op {
        BinOp::Eq => SqlBinOp::Eq,
        BinOp::Neq => SqlBinOp::Neq,
        BinOp::Lt => SqlBinOp::Lt,
        BinOp::Lte => SqlBinOp::Lte,
        BinOp::Gt => SqlBinOp::Gt,
        BinOp::Gte => SqlBinOp::Gte,
        BinOp::And => SqlBinOp::And,
        BinOp::Or => SqlBinOp::Or,
        BinOp::Add => SqlBinOp::Add,
        BinOp::Sub => SqlBinOp::Sub,
        BinOp::Mul => SqlBinOp::Mul,
        BinOp::Div => SqlBinOp::Div,
        BinOp::Concat => SqlBinOp::Concat,
    }
}

/// Wrap an expression in parens if it's an OR operation (for AND precedence).
fn maybe_wrap_or(expr: SqlExpr) -> SqlExpr {
    if let SqlExpr::BinaryOp {
        op: SqlBinOp::Or, ..
    } = &expr
    {
        SqlExpr::Parens(Box::new(expr))
    } else {
        expr
    }
}

/// Translate function calls that appear in expression context.
fn translate_function_in_expr(name: &str, args: &[MExpr], ctx: &mut TranslationContext) -> SqlExpr {
    match name {
        "Text.StartsWith" => {
            if args.len() >= 2 {
                let col_expr = translate_expr(&args[0], ctx);
                let val = extract_string(&args[1]);
                let case_insensitive = args.get(2).map_or(false, is_ordinal_ignore_case);
                SqlExpr::Like {
                    expr: Box::new(col_expr),
                    pattern: format!("{}%", escape_like(&val)),
                    negated: false,
                    case_insensitive,
                }
            } else {
                SqlExpr::Raw(ctx.untranslatable(
                    0,
                    &format!("Text.StartsWith({} args)", args.len()),
                    "Text.StartsWith requires 2 arguments",
                ))
            }
        }

        "Text.EndsWith" => {
            if args.len() >= 2 {
                let col_expr = translate_expr(&args[0], ctx);
                let val = extract_string(&args[1]);
                let case_insensitive = args.get(2).map_or(false, is_ordinal_ignore_case);
                SqlExpr::Like {
                    expr: Box::new(col_expr),
                    pattern: format!("%{}", escape_like(&val)),
                    negated: false,
                    case_insensitive,
                }
            } else {
                SqlExpr::Raw(ctx.untranslatable(
                    0,
                    &format!("Text.EndsWith({} args)", args.len()),
                    "Text.EndsWith requires 2 arguments",
                ))
            }
        }

        "Text.Contains" => {
            if args.len() >= 2 {
                let col_expr = translate_expr(&args[0], ctx);
                let val = extract_string(&args[1]);
                let case_insensitive = args.get(2).map_or(false, is_ordinal_ignore_case);
                SqlExpr::Like {
                    expr: Box::new(col_expr),
                    pattern: format!("%{}%", escape_like(&val)),
                    negated: false,
                    case_insensitive,
                }
            } else {
                SqlExpr::Raw(ctx.untranslatable(
                    0,
                    &format!("Text.Contains({} args)", args.len()),
                    "Text.Contains requires 2 arguments",
                ))
            }
        }

        "List.Contains" => {
            if args.len() >= 2 {
                let list_expr = &args[0];
                let item_expr = translate_expr(&args[1], ctx);
                if let MExpr::List(items) = list_expr {
                    let sql_items: Vec<SqlExpr> =
                        items.iter().map(|i| translate_expr(i, ctx)).collect();
                    SqlExpr::InList {
                        expr: Box::new(item_expr),
                        list: sql_items,
                        negated: false,
                    }
                } else {
                    SqlExpr::Raw(ctx.untranslatable(
                        0,
                        "List.Contains with non-list first arg",
                        "List.Contains expects a list literal",
                    ))
                }
            } else {
                SqlExpr::Raw(ctx.untranslatable(
                    0,
                    "List.Contains",
                    "List.Contains requires 2 arguments",
                ))
            }
        }

        "Date.From" | "DateTime.From" => {
            if !args.is_empty() {
                let inner = translate_expr(&args[0], ctx);
                let target_type = if name == "Date.From" {
                    ctx.dialect.map_type(&crate::parser::ast::MType::Date)
                } else {
                    ctx.dialect.map_type(&crate::parser::ast::MType::DateTime)
                };
                SqlExpr::Cast {
                    expr: Box::new(inner),
                    ty: target_type.to_string(),
                }
            } else {
                SqlExpr::Raw(ctx.untranslatable(0, name, &format!("{} requires an argument", name)))
            }
        }

        "Value.ReplaceType" => {
            if args.len() >= 2 {
                let inner = translate_expr(&args[0], ctx);
                if let MExpr::Identifier(type_name) = &args[1] {
                    let m_type = crate::translator::cast::resolve_m_type(type_name);
                    let sql_type = ctx.dialect.map_type(&m_type);
                    SqlExpr::Cast {
                        expr: Box::new(inner),
                        ty: sql_type.to_string(),
                    }
                } else {
                    inner
                }
            } else {
                SqlExpr::Raw(ctx.untranslatable(0, "Value.ReplaceType", "requires 2 arguments"))
            }
        }

        // Aggregate functions (used within Table.Group)
        "List.Sum" => translate_aggregate("SUM", args, false, ctx),
        "List.Average" => translate_aggregate("AVG", args, false, ctx),
        "List.Count" => translate_aggregate("COUNT", args, false, ctx),
        "List.Min" => translate_aggregate("MIN", args, false, ctx),
        "List.Max" => translate_aggregate("MAX", args, false, ctx),
        "List.CountDistinct" => translate_aggregate("COUNT", args, true, ctx),
        "Table.RowCount" => SqlExpr::FunctionCall {
            name: "COUNT".to_string(),
            args: vec![SqlExpr::Raw("*".to_string())],
            distinct: false,
        },

        _ => {
            let frag = format!(
                "{}({})",
                name,
                args.iter()
                    .map(|a| format_expr(a))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            SqlExpr::Raw(ctx.untranslatable(
                0,
                &frag,
                &format!("{} is not in the v1 feature set", name),
            ))
        }
    }
}

/// Translate an aggregate function call.
fn translate_aggregate(
    sql_name: &str,
    args: &[MExpr],
    distinct: bool,
    ctx: &mut TranslationContext,
) -> SqlExpr {
    if !args.is_empty() {
        let inner = translate_expr(&args[0], ctx);
        SqlExpr::FunctionCall {
            name: sql_name.to_string(),
            args: vec![inner],
            distinct,
        }
    } else {
        SqlExpr::FunctionCall {
            name: sql_name.to_string(),
            args: vec![SqlExpr::Raw("*".to_string())],
            distinct,
        }
    }
}

/// Extract a string from an M literal expression.
pub fn extract_string(expr: &MExpr) -> String {
    match expr {
        MExpr::Literal(MLiteral::Text(s)) => s.clone(),
        MExpr::Identifier(s) => s.clone(),
        _ => format_expr(expr),
    }
}

/// Check if an expression is the OrdinalIgnoreCase comparer.
fn is_ordinal_ignore_case(expr: &MExpr) -> bool {
    matches!(expr, MExpr::Identifier(name) if name == "Comparer.OrdinalIgnoreCase")
}

/// Escape special characters in a LIKE pattern.
fn escape_like(s: &str) -> String {
    s.replace('%', "\\%").replace('_', "\\_")
}

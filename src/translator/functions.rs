use super::context::{NestedJoinInfo, TranslationContext};
use super::expr::{extract_string, translate_expr};
/// Per-function translators for M table operations.
use crate::emitter::sql_writer::*;
use crate::parser::ast::*;
use crate::resolver::source::{format_expr, resolve_source};

/// Result of translating a table operation — either a new CTE or modifications to a parent SELECT.
#[derive(Debug)]
pub enum TableOpResult {
    /// A new SELECT statement to use as a CTE.
    Select(SelectStmt),
    /// A source table reference (no CTE needed, just FROM).
    SourceTable {
        /// Schema name.
        schema: Option<String>,
        /// Table name.
        table: String,
    },
    /// An identifier reference to another binding.
    Reference(String),
    /// Raw SQL placeholder.
    Raw(String),
}

/// Translate a table-level M expression into a SQL operation.
pub fn translate_table_expr(expr: &MExpr, ctx: &mut TranslationContext) -> TableOpResult {
    match expr {
        MExpr::FunctionCall { name, args } => translate_table_function(name, args, ctx),

        MExpr::FieldAccess {
            expr: source,
            field,
        } if field == "Data" => {
            // Source{[Name="Table"]}[Data] pattern
            let resolved = resolve_source(expr);
            if let Some(table) = resolved.table {
                TableOpResult::SourceTable {
                    schema: resolved.schema,
                    table,
                }
            } else if let Some(raw) = resolved.raw {
                TableOpResult::Raw(format!("/* SOURCE: {} */", raw))
            } else {
                TableOpResult::Reference(format_expr(expr))
            }
        }

        MExpr::Identifier(name) => TableOpResult::Reference(name.clone()),

        MExpr::IndexAccess { .. } | MExpr::FieldAccess { .. } => {
            let resolved = resolve_source(expr);
            if let Some(table) = resolved.table {
                TableOpResult::SourceTable {
                    schema: resolved.schema,
                    table,
                }
            } else {
                TableOpResult::Reference(format_expr(expr))
            }
        }

        _ => {
            let frag = format_expr(expr);
            let placeholder =
                ctx.untranslatable(0, &frag, "Expression is not a recognized table operation");
            TableOpResult::Raw(placeholder)
        }
    }
}

/// Translate a table-level function call.
fn translate_table_function(
    name: &str,
    args: &[MExpr],
    ctx: &mut TranslationContext,
) -> TableOpResult {
    match name {
        "Table.SelectRows" => translate_select_rows(args, ctx),
        "Table.SelectColumns" => translate_select_columns(args, ctx),
        "Table.RenameColumns" => translate_rename_columns(args, ctx),
        "Table.Join" => translate_join(args, false, ctx),
        "Table.NestedJoin" => translate_join(args, true, ctx),
        "Table.ExpandTableColumn" => translate_expand_table_column(args, ctx),
        "Table.Group" => translate_group(args, ctx),
        "Table.AddColumn" => translate_add_column(args, ctx),
        "Table.RemoveColumns" => translate_remove_columns(args, ctx),
        "Table.Combine" => translate_combine(args, ctx),
        "Table.TransformColumnTypes" => translate_transform_column_types(args, ctx),
        "Sql.Database" => {
            let resolved = resolve_source(&MExpr::FunctionCall {
                name: name.to_string(),
                args: args.to_vec(),
            });
            if let Some(schema) = resolved.schema {
                TableOpResult::Raw(format!("/* DATABASE: {} */", schema))
            } else {
                TableOpResult::Raw("/* DATABASE */".to_string())
            }
        }
        _ => {
            let frag = format!(
                "{}({})",
                name,
                args.iter()
                    .map(|a| format_expr(a))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let placeholder =
                ctx.untranslatable(0, &frag, &format!("{} is not in the v1 feature set", name));
            // Produce a SELECT with the untranslatable comment, referencing input table if possible
            if !args.is_empty() {
                let table_ref = get_table_ref(&args[0]);
                let mut stmt = SelectStmt::new();
                stmt.columns = vec![SelectCol::Expr {
                    expr: SqlExpr::Raw(placeholder),
                    alias: None,
                }];
                stmt.from = Some(TableRef {
                    schema: None,
                    name: table_ref,
                    alias: None,
                });
                TableOpResult::Select(stmt)
            } else {
                TableOpResult::Raw(placeholder)
            }
        }
    }
}

/// Translate `Table.SelectRows(table, each <condition>)` → WHERE.
fn translate_select_rows(args: &[MExpr], ctx: &mut TranslationContext) -> TableOpResult {
    if args.len() < 2 {
        return TableOpResult::Raw(ctx.untranslatable(
            0,
            "Table.SelectRows",
            "requires 2 arguments",
        ));
    }

    let table_ref = get_table_ref(&args[0]);
    let condition = translate_expr(&args[1], ctx);

    let mut stmt = SelectStmt::new();
    stmt.columns = vec![SelectCol::Wildcard];
    stmt.from = Some(TableRef {
        schema: None,
        name: table_ref,
        alias: None,
    });
    stmt.where_clause = Some(condition);

    TableOpResult::Select(stmt)
}

/// Translate `Table.SelectColumns(table, {"Col1", "Col2"})` → SELECT cols.
fn translate_select_columns(args: &[MExpr], ctx: &mut TranslationContext) -> TableOpResult {
    if args.len() < 2 {
        return TableOpResult::Raw(ctx.untranslatable(
            0,
            "Table.SelectColumns",
            "requires 2 arguments",
        ));
    }

    let table_ref = get_table_ref(&args[0]);
    let columns = extract_string_list(&args[1]);

    let mut stmt = SelectStmt::new();
    stmt.columns = columns
        .iter()
        .map(|c| SelectCol::Expr {
            expr: SqlExpr::Column {
                table: None,
                name: c.clone(),
            },
            alias: None,
        })
        .collect();
    stmt.from = Some(TableRef {
        schema: None,
        name: table_ref,
        alias: None,
    });

    TableOpResult::Select(stmt)
}

/// Translate `Table.RenameColumns(table, {{"Old", "New"}, ...})` → SELECT Old AS New.
fn translate_rename_columns(args: &[MExpr], ctx: &mut TranslationContext) -> TableOpResult {
    if args.len() < 2 {
        return TableOpResult::Raw(ctx.untranslatable(
            0,
            "Table.RenameColumns",
            "requires 2 arguments",
        ));
    }

    let table_ref = get_table_ref(&args[0]);
    let rename_pairs = extract_rename_pairs(&args[1]);

    let mut stmt = SelectStmt::new();

    // Check if we know the full column list for this table
    let known_cols = ctx.known_columns.get(&table_ref).cloned();

    if let Some(cols) = known_cols {
        // Emit explicit column list
        stmt.columns = cols
            .iter()
            .map(|col| {
                if let Some((_, new_name)) = rename_pairs.iter().find(|(old, _)| old == col) {
                    SelectCol::Expr {
                        expr: SqlExpr::Column {
                            table: None,
                            name: col.clone(),
                        },
                        alias: Some(new_name.clone()),
                    }
                } else {
                    SelectCol::Expr {
                        expr: SqlExpr::Column {
                            table: None,
                            name: col.clone(),
                        },
                        alias: None,
                    }
                }
            })
            .collect();
    } else {
        // Unknown full column list — emit rename aliases + warning
        ctx.warn(
            0,
            "PARTIAL_RENAME",
            "full column list unknown; rename translated as partial alias list",
            None,
        );
        stmt.columns = rename_pairs
            .iter()
            .map(|(old, new)| SelectCol::Expr {
                expr: SqlExpr::Column {
                    table: None,
                    name: old.clone(),
                },
                alias: Some(new.clone()),
            })
            .collect();
        // Add a wildcard comment for remaining columns
    }

    stmt.from = Some(TableRef {
        schema: None,
        name: table_ref,
        alias: None,
    });

    TableOpResult::Select(stmt)
}

/// Translate `Table.Join` / `Table.NestedJoin` → JOIN.
fn translate_join(args: &[MExpr], is_nested: bool, ctx: &mut TranslationContext) -> TableOpResult {
    // Table.Join(left, leftKey, right, rightKey, joinKind)
    // Table.NestedJoin(left, leftKey, right, rightKey, newCol, joinKind)
    let min_args = if is_nested { 5 } else { 4 };
    if args.len() < min_args {
        let name = if is_nested {
            "Table.NestedJoin"
        } else {
            "Table.Join"
        };
        return TableOpResult::Raw(ctx.untranslatable(
            0,
            name,
            &format!("requires at least {} arguments", min_args),
        ));
    }

    let left_ref = get_table_ref(&args[0]);
    let left_keys = extract_key_columns(&args[1]);
    let right_ref = get_table_ref(&args[2]);
    let right_keys = extract_key_columns(&args[3]);

    let (new_col, join_kind_idx) = if is_nested {
        let new_col = extract_string(&args[4]);
        (Some(new_col), 5)
    } else {
        (None, 4)
    };

    let join_kind = if args.len() > join_kind_idx {
        parse_join_kind(&args[join_kind_idx])
    } else {
        JoinType::Inner
    };

    // Build ON condition
    let on_condition = build_join_condition(&left_ref, &left_keys, &right_ref, &right_keys);

    // For anti-joins, add WHERE clause
    let anti_where = match join_kind {
        JoinType::LeftAnti => {
            if let Some(key) = right_keys.first() {
                Some(SqlExpr::IsNull {
                    expr: Box::new(SqlExpr::Column {
                        table: Some(right_ref.clone()),
                        name: key.clone(),
                    }),
                    negated: false,
                })
            } else {
                None
            }
        }
        JoinType::RightAnti => {
            if let Some(key) = left_keys.first() {
                Some(SqlExpr::IsNull {
                    expr: Box::new(SqlExpr::Column {
                        table: Some(left_ref.clone()),
                        name: key.clone(),
                    }),
                    negated: false,
                })
            } else {
                None
            }
        }
        _ => None,
    };

    // For nested joins, don't emit a CTE — store info for ExpandTableColumn
    if is_nested {
        if let Some(ref nc) = new_col {
            ctx.nested_joins.insert(
                nc.clone(),
                NestedJoinInfo {
                    left_table: left_ref.clone(),
                    right_table: right_ref.clone(),
                    new_col: nc.clone(),
                    left_keys: left_keys.clone(),
                    right_keys: right_keys.clone(),
                    join_type: join_kind,
                    join_binding: String::new(), // Will be set by CTE builder
                },
            );
        }
        // Emit a placeholder — the ExpandTableColumn will replace this
        let mut stmt = SelectStmt::new();
        stmt.columns = vec![SelectCol::QualifiedWildcard(left_ref.clone())];
        stmt.from = Some(TableRef {
            schema: None,
            name: left_ref,
            alias: None,
        });
        stmt.joins = vec![JoinClause {
            join_type: join_kind,
            table: TableRef {
                schema: None,
                name: right_ref,
                alias: None,
            },
            on: on_condition,
        }];
        if let Some(where_clause) = anti_where {
            stmt.where_clause = Some(where_clause);
        }
        return TableOpResult::Select(stmt);
    }

    let mut stmt = SelectStmt::new();
    stmt.columns = vec![SelectCol::Wildcard];

    stmt.from = Some(TableRef {
        schema: None,
        name: left_ref.clone(),
        alias: None,
    });

    stmt.joins = vec![JoinClause {
        join_type: join_kind,
        table: TableRef {
            schema: None,
            name: right_ref.clone(),
            alias: None,
        },
        on: on_condition,
    }];

    if let Some(where_clause) = anti_where {
        stmt.where_clause = Some(where_clause);
    }

    TableOpResult::Select(stmt)
}

/// Translate `Table.ExpandTableColumn(table, "NestedCol", {"Col1"}, {"Alias1"})`.
fn translate_expand_table_column(args: &[MExpr], ctx: &mut TranslationContext) -> TableOpResult {
    if args.len() < 3 {
        return TableOpResult::Raw(ctx.untranslatable(
            0,
            "Table.ExpandTableColumn",
            "requires at least 3 arguments",
        ));
    }

    let table_ref = get_table_ref(&args[0]);
    let nested_col = extract_string(&args[1]);
    let expand_cols = extract_string_list(&args[2]);
    let aliases = if args.len() > 3 {
        extract_string_list(&args[3])
    } else {
        Vec::new()
    };

    // Look up nested join info
    let join_info = ctx.nested_joins.get(&nested_col).cloned();

    if let Some(info) = join_info {
        // Build a combined SELECT with JOIN and column projections
        let mut stmt = SelectStmt::new();

        // Select left.* and specific right columns with aliases
        stmt.columns = vec![SelectCol::QualifiedWildcard(info.left_table.clone())];

        for (i, col) in expand_cols.iter().enumerate() {
            let alias = aliases.get(i);
            stmt.columns.push(SelectCol::Expr {
                expr: SqlExpr::Column {
                    table: Some(info.right_table.clone()),
                    name: col.clone(),
                },
                alias: alias.cloned(),
            });
        }

        // FROM left_table JOIN right_table ON ...
        stmt.from = Some(TableRef {
            schema: None,
            name: info.left_table.clone(),
            alias: None,
        });

        let on_condition = build_join_condition(
            &info.left_table,
            &info.left_keys,
            &info.right_table,
            &info.right_keys,
        );

        stmt.joins = vec![JoinClause {
            join_type: info.join_type,
            table: TableRef {
                schema: None,
                name: info.right_table.clone(),
                alias: None,
            },
            on: on_condition,
        }];

        TableOpResult::Select(stmt)
    } else {
        // Not a join expansion
        let frag = format!(
            "Table.ExpandTableColumn({}, \"{}\", ...)",
            table_ref, nested_col
        );
        TableOpResult::Raw(ctx.untranslatable(0, &frag, "ExpandTableColumn on non-join source"))
    }
}

/// Translate `Table.Group(table, {"Key"}, {{"Agg", each func, type}})` → GROUP BY.
fn translate_group(args: &[MExpr], ctx: &mut TranslationContext) -> TableOpResult {
    if args.len() < 3 {
        return TableOpResult::Raw(ctx.untranslatable(0, "Table.Group", "requires 3 arguments"));
    }

    let table_ref = get_table_ref(&args[0]);
    let group_keys = extract_string_list(&args[1]);

    let mut stmt = SelectStmt::new();

    // Add group key columns
    for key in &group_keys {
        stmt.columns.push(SelectCol::Expr {
            expr: SqlExpr::Column {
                table: None,
                name: key.clone(),
            },
            alias: None,
        });
    }

    // Parse aggregation specifications
    if let MExpr::List(agg_specs) = &args[2] {
        for spec in agg_specs {
            if let MExpr::List(spec_items) = spec {
                if spec_items.len() >= 2 {
                    let agg_name = extract_string(&spec_items[0]);
                    let agg_expr = translate_expr(&spec_items[1], ctx);
                    stmt.columns.push(SelectCol::Expr {
                        expr: agg_expr,
                        alias: Some(agg_name),
                    });
                }
            }
        }
    }

    // GROUP BY
    stmt.group_by = group_keys
        .iter()
        .map(|k| SqlExpr::Column {
            table: None,
            name: k.clone(),
        })
        .collect();

    stmt.from = Some(TableRef {
        schema: None,
        name: table_ref,
        alias: None,
    });

    TableOpResult::Select(stmt)
}

/// Translate `Table.AddColumn(table, "NewCol", each <expr>, type)` → computed column.
fn translate_add_column(args: &[MExpr], ctx: &mut TranslationContext) -> TableOpResult {
    if args.len() < 3 {
        return TableOpResult::Raw(ctx.untranslatable(
            0,
            "Table.AddColumn",
            "requires at least 3 arguments",
        ));
    }

    let table_ref = get_table_ref(&args[0]);
    let col_name = extract_string(&args[1]);
    let col_expr = translate_expr(&args[2], ctx);

    // If there's a type annotation (4th arg), apply CAST
    let final_expr = if args.len() > 3 {
        if let MExpr::Identifier(type_name) = &args[3] {
            let m_type = crate::translator::cast::resolve_m_type(type_name);
            let sql_type = ctx.dialect.map_type(&m_type);
            if sql_type.contains("UNSUPPORTED") {
                col_expr
            } else {
                SqlExpr::Cast {
                    expr: Box::new(col_expr),
                    ty: sql_type.to_string(),
                }
            }
        } else {
            col_expr
        }
    } else {
        col_expr
    };

    let mut stmt = SelectStmt::new();
    stmt.columns = vec![
        SelectCol::Wildcard,
        SelectCol::Expr {
            expr: final_expr,
            alias: Some(col_name),
        },
    ];
    stmt.from = Some(TableRef {
        schema: None,
        name: table_ref,
        alias: None,
    });

    TableOpResult::Select(stmt)
}

/// Translate `Table.TransformColumnTypes(table, {{"Col", type}, ...})` → CAST.
fn translate_transform_column_types(args: &[MExpr], ctx: &mut TranslationContext) -> TableOpResult {
    if args.len() < 2 {
        return TableOpResult::Raw(ctx.untranslatable(
            0,
            "Table.TransformColumnTypes",
            "requires 2 arguments",
        ));
    }

    let table_ref = get_table_ref(&args[0]);
    let mut stmt = SelectStmt::new();

    if let MExpr::List(type_specs) = &args[1] {
        for spec in type_specs {
            if let MExpr::List(items) = spec {
                if items.len() >= 2 {
                    let col_name = extract_string(&items[0]);
                    let m_type = resolve_type_expr(&items[1]);
                    let sql_type = ctx.dialect.map_type(&m_type);

                    if sql_type.contains("UNSUPPORTED") {
                        ctx.warn(
                            0,
                            "UNSUPPORTED_TYPE",
                            &format!(
                                "type {} is not supported in {}",
                                crate::parser::grammar::type_to_string(&m_type),
                                ctx.dialect.name()
                            ),
                            None,
                        );
                        stmt.columns.push(SelectCol::Expr {
                            expr: SqlExpr::Raw(format!("{} /* UNSUPPORTED_TYPE */", col_name)),
                            alias: None,
                        });
                    } else {
                        stmt.columns.push(SelectCol::Expr {
                            expr: SqlExpr::Cast {
                                expr: Box::new(SqlExpr::Column {
                                    table: None,
                                    name: col_name,
                                }),
                                ty: sql_type.to_string(),
                            },
                            alias: None,
                        });
                    }
                }
            }
        }
    }

    stmt.from = Some(TableRef {
        schema: None,
        name: table_ref,
        alias: None,
    });

    TableOpResult::Select(stmt)
}

/// Translate `Table.RemoveColumns(table, {"Col1", "Col2"})` → column exclusion.
fn translate_remove_columns(args: &[MExpr], ctx: &mut TranslationContext) -> TableOpResult {
    if args.len() < 2 {
        return TableOpResult::Raw(ctx.untranslatable(
            0,
            "Table.RemoveColumns",
            "requires 2 arguments",
        ));
    }

    let table_ref = get_table_ref(&args[0]);
    let remove_cols = extract_string_list(&args[1]);

    let mut stmt = SelectStmt::new();

    // Check if we know the full column list for this table
    let known_cols = ctx.known_columns.get(&table_ref).cloned();

    if let Some(cols) = known_cols {
        // Emit explicit column list excluding the removed columns
        let remaining: Vec<String> = cols
            .into_iter()
            .filter(|c| !remove_cols.contains(c))
            .collect();
        stmt.columns = remaining
            .iter()
            .map(|c| SelectCol::Expr {
                expr: SqlExpr::Column {
                    table: None,
                    name: c.clone(),
                },
                alias: None,
            })
            .collect();
    } else if ctx.dialect.supports_select_except() {
        // BigQuery/DuckDB: use SELECT * EXCEPT (col1, col2)
        ctx.warn(
            0,
            "UNKNOWN_COLUMNS",
            "full column list unknown; RemoveColumns translated as EXCEPT clause",
            None,
        );
        stmt.columns = vec![SelectCol::Except {
            columns: remove_cols,
        }];
    } else {
        // Dialect doesn't support EXCEPT and column list is unknown
        let frag = format!(
            "Table.RemoveColumns({}, {{{}}})",
            table_ref,
            remove_cols.join(", ")
        );
        let placeholder = ctx.untranslatable(
            0,
            &frag,
            &format!(
                "RemoveColumns on unknown column set for dialect {}",
                ctx.dialect.name()
            ),
        );
        stmt.columns = vec![SelectCol::Expr {
            expr: SqlExpr::Raw(placeholder),
            alias: None,
        }];
    }

    stmt.from = Some(TableRef {
        schema: None,
        name: table_ref,
        alias: None,
    });

    TableOpResult::Select(stmt)
}

/// Translate `Table.Combine({Table1, Table2, ...})` → UNION ALL.
fn translate_combine(args: &[MExpr], ctx: &mut TranslationContext) -> TableOpResult {
    if args.is_empty() {
        return TableOpResult::Raw(ctx.untranslatable(
            0,
            "Table.Combine",
            "requires 1 argument (list of tables)",
        ));
    }

    let tables = match &args[0] {
        MExpr::List(items) => items,
        _ => {
            return TableOpResult::Raw(ctx.untranslatable(
                0,
                "Table.Combine",
                "argument must be a list of tables",
            ));
        }
    };

    if tables.is_empty() {
        return TableOpResult::Raw(ctx.untranslatable(
            0,
            "Table.Combine",
            "empty table list",
        ));
    }

    // Build UNION ALL: first table becomes the main SELECT, rest go into union_all
    let mut selects: Vec<SelectStmt> = Vec::new();

    for table_expr in tables {
        let table_name = match table_expr {
            MExpr::Identifier(name) => name.clone(),
            _ => {
                let frag = format_expr(table_expr);
                // Non-identifier members are not supported
                ctx.warn(
                    0,
                    "UNRESOLVED_COMBINE_TABLE",
                    &format!("'{}' is not a valid table reference in Table.Combine", frag),
                    Some(&frag),
                );
                continue;
            }
        };

        // Validate: must be a known binding or input file stem
        let is_known_binding = ctx.known_columns.contains_key(&table_name)
            || ctx.combine_bindings.contains(&table_name);
        let is_known_file_stem = ctx.input_file_stems.contains(&table_name);

        if !is_known_binding && !is_known_file_stem {
            match ctx.on_error {
                super::context::OnError::Fail => {
                    ctx.error(
                        0,
                        "UNRESOLVED_COMBINE_TABLE",
                        &format!(
                            "'{}' is not a binding or a known input file stem",
                            table_name
                        ),
                        Some(&table_name),
                    );
                    continue;
                }
                super::context::OnError::Comment | super::context::OnError::Warn => {
                    if ctx.on_error == super::context::OnError::Warn {
                        ctx.warn(
                            0,
                            "UNRESOLVED_COMBINE_TABLE",
                            &format!(
                                "'{}' is not a binding or a known input file stem",
                                table_name
                            ),
                            Some(&table_name),
                        );
                    }
                    // Emit a placeholder SELECT for unresolved member
                    let mut member_stmt = SelectStmt::new();
                    member_stmt.columns = vec![SelectCol::Expr {
                        expr: SqlExpr::Raw(format!("/* UNRESOLVED: {} */", table_name)),
                        alias: None,
                    }];
                    selects.push(member_stmt);
                    continue;
                }
            }
        }

        let mut member_stmt = SelectStmt::new();
        member_stmt.columns = vec![SelectCol::Wildcard];
        member_stmt.from = Some(TableRef {
            schema: None,
            name: table_name,
            alias: None,
        });
        selects.push(member_stmt);
    }

    if selects.is_empty() {
        return TableOpResult::Raw(ctx.untranslatable(
            0,
            "Table.Combine",
            "no valid tables to combine",
        ));
    }

    let mut first = selects.remove(0);
    first.union_all = selects;

    TableOpResult::Select(first)
}

// Helper functions

/// Get a table reference name from an M expression (identifier name or formatted expression).
fn get_table_ref(expr: &MExpr) -> String {
    match expr {
        MExpr::Identifier(name) => name.clone(),
        _ => format_expr(expr),
    }
}

/// Extract a list of strings from an M list expression.
pub fn extract_string_list(expr: &MExpr) -> Vec<String> {
    match expr {
        MExpr::List(items) => items.iter().map(|i| extract_string(i)).collect(),
        _ => vec![extract_string(expr)],
    }
}

/// Extract key columns (may be a single string or a list).
fn extract_key_columns(expr: &MExpr) -> Vec<String> {
    match expr {
        MExpr::List(items) => items.iter().map(|i| extract_string(i)).collect(),
        MExpr::Literal(MLiteral::Text(s)) => vec![s.clone()],
        MExpr::Identifier(s) => vec![s.clone()],
        _ => vec![extract_string(expr)],
    }
}

/// Extract rename pairs from a list of {{"OldName", "NewName"}, ...}.
fn extract_rename_pairs(expr: &MExpr) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    if let MExpr::List(items) = expr {
        for item in items {
            if let MExpr::List(pair) = item {
                if pair.len() >= 2 {
                    pairs.push((extract_string(&pair[0]), extract_string(&pair[1])));
                }
            }
        }
    }
    pairs
}

/// Parse a JoinKind enum value.
fn parse_join_kind(expr: &MExpr) -> JoinType {
    let name = match expr {
        MExpr::Identifier(name) => name.as_str(),
        _ => return JoinType::Inner,
    };
    match name {
        "JoinKind.Inner" => JoinType::Inner,
        "JoinKind.Left" => JoinType::Left,
        "JoinKind.Right" => JoinType::Right,
        "JoinKind.Full" => JoinType::FullOuter,
        "JoinKind.LeftAnti" => JoinType::LeftAnti,
        "JoinKind.RightAnti" => JoinType::RightAnti,
        _ => JoinType::Inner,
    }
}

/// Build a JOIN ON condition from key columns.
fn build_join_condition(
    left_table: &str,
    left_keys: &[String],
    right_table: &str,
    right_keys: &[String],
) -> SqlExpr {
    let mut conditions: Vec<SqlExpr> = Vec::new();
    for (lk, rk) in left_keys.iter().zip(right_keys.iter()) {
        conditions.push(SqlExpr::BinaryOp {
            op: SqlBinOp::Eq,
            left: Box::new(SqlExpr::Column {
                table: Some(left_table.to_string()),
                name: lk.clone(),
            }),
            right: Box::new(SqlExpr::Column {
                table: Some(right_table.to_string()),
                name: rk.clone(),
            }),
        });
    }

    if conditions.len() == 1 {
        conditions.into_iter().next().unwrap()
    } else {
        let mut result = conditions.remove(0);
        for cond in conditions {
            result = SqlExpr::BinaryOp {
                op: SqlBinOp::And,
                left: Box::new(result),
                right: Box::new(cond),
            };
        }
        result
    }
}

/// Resolve a type expression to an MType.
fn resolve_type_expr(expr: &MExpr) -> MType {
    match expr {
        MExpr::Identifier(name) => crate::translator::cast::resolve_m_type(name),
        _ => MType::Unknown(format_expr(expr)),
    }
}

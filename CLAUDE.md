# CLAUDE.md — m2sql Project Guide

## What is this?

**m2sql** is a deterministic, fully-offline Rust CLI that translates Power Query M code into SQL queries. It reads `.pq`/`.m` files, `.tmdl` files (Power BI `.pbip` projects), or raw M code from stdin, and emits `.sql` files — rendering multi-step `let … in` pipelines as CTEs (`WITH` clauses).

## Quick Reference

```bash
# Build
cargo build --release       # binary: target/release/m2sql

# Run tests (35 integration tests)
cargo test

# Basic usage
m2sql -d tsql orders.pq                      # file → ./output/orders.sql
m2sql -d postgres --stdout *.pq              # multiple files → stdout
echo 'let ... in ...' | m2sql -d tsql --stdout  # stdin → stdout
m2sql -d bigquery model.tmdl                 # TMDL file
```

## Supported SQL Dialects

`tsql` (T-SQL), `postgres`, `bigquery`, `snowflake`, `duckdb`

## Architecture

The processing pipeline is: **Parse → Resolve → Translate → Emit → Redact**

```
src/
├── main.rs              CLI entry point, file I/O, summary output
├── cli.rs               clap argument definitions (Cli struct)
├── pipeline.rs          Orchestrates the full pipeline per source file
├── error.rs             Diagnostic, M2SqlError, CredentialRedactor
│
├── parser/
│   ├── mod.rs           parse() and parse_tmdl() entry points
│   ├── lexer.rs         M language tokeniser
│   ├── grammar.rs       Recursive-descent parser → MDocument AST
│   └── ast.rs           AST node types (MDocument, MExpr, BinOp, MType…)
│
├── resolver/
│   ├── mod.rs
│   └── source.rs        Infers schema/table from Sql.Database, index records
│
├── translator/
│   ├── mod.rs           translate() → dispatches MDocument to SQL
│   ├── context.rs       TranslationContext (state, diagnostics, error modes)
│   ├── cte.rs           let…in → CTE chain builder
│   ├── expr.rs          M expression → SQL expression translator
│   ├── functions.rs     Per-function translators (Table.SelectRows, etc.)
│   └── cast.rs          M type → SQL type string
│
├── dialect/
│   ├── mod.rs           Dialect trait + dialect_from_name() factory
│   ├── tsql.rs          T-SQL: [brackets], BIT, NVARCHAR(MAX), TOP
│   ├── postgres.rs      PostgreSQL: "double-quotes", ::CAST, ILIKE
│   ├── bigquery.rs      BigQuery: `backticks`, INT64, FLOAT64, EXCEPT
│   ├── snowflake.rs     Snowflake: "double-quotes", VARIANT
│   └── duckdb.rs        DuckDB: "double-quotes", ILIKE, EXCEPT
│
└── emitter/
    ├── mod.rs
    └── sql_writer.rs    SQL AST types (SqlQuery, SelectStmt, SqlExpr…) + pretty-printer

tests/
├── integration_tests.rs   35 CLI integration tests via assert_cmd
└── fixtures/              21 .pq/.tmdl fixture files

documentation/
├── PRD_m2sql.md           Product Requirements Document
└── NFR_m2sql.md           Non-Functional Requirements
```

## Key Design Decisions

- **Two-AST approach**: M code → `MExpr` AST → `SqlExpr`/`SqlQuery` SQL AST → formatted string. This allows clean separation between parsing, translation, and emission.
- **CTE-first strategy**: Each `let` binding maps to a CTE. The `--inline-singles` flag can inline single-use CTEs for conciseness.
- **Dialect trait**: All SQL-dialect differences (identifier quoting, type names, CAST syntax, ILIKE support, TOP vs LIMIT) are encapsulated behind the `Dialect` trait.
- **Nested join tracking**: `TranslationContext` maintains a `nested_joins` map so that `Table.ExpandTableColumn` can correctly expand columns from preceding `Table.NestedJoin` calls.
- **Column tracking**: `known_columns` in the context propagates column lists through the CTE chain for operations like `Table.RemoveColumns`.
- **Deterministic output**: `indexmap::IndexMap` preserves insertion order everywhere. The determinism test runs 10 iterations to verify.

## Translated M Constructs (v1)

`Table.SelectRows` → WHERE, `Table.SelectColumns` → SELECT, `Table.RenameColumns` → AS, `Table.Join`/`Table.NestedJoin` → JOIN, `Table.ExpandTableColumn` → JOIN column projections, `Table.Group` → GROUP BY, `Table.AddColumn` → computed SELECT column, `Table.TransformColumnTypes` → CAST, `Table.RemoveColumns` → EXCEPT (BigQuery/DuckDB) or UNTRANSLATABLE, `Table.Combine` → UNION ALL, `Text.StartsWith`/`EndsWith`/`Contains` → LIKE, `List.Contains` → IN, anti-joins → LEFT JOIN … WHERE IS NULL.

## Error Handling

Three modes via `--on-error`: `fail` (exit 1), `comment` (inline `/* UNTRANSLATABLE: … */`), `warn` (comment + stderr warning). Diagnostics have structured fields: `level`, `file`, `line`, `code`, `message`, `fragment`. Supports `--log-json` for CI/CD integration.

## Security

- **Credential redaction**: Regex-based detection of `Password=`, `AccountKey=`, `SecretKey=`, etc. in SQL output, replaced with `/* REDACTED */`.
- **Path traversal protection**: Output filenames are sanitised; canonical paths are checked against the output directory.
- **10 MB file-size guard**.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` (4, derive) | CLI argument parsing |
| `anyhow` (1) | Error propagation |
| `thiserror` (1) | Typed error definitions |
| `indexmap` (2) | Insertion-ordered maps for determinism |
| `regex` (1) | Credential pattern matching |
| `once_cell` (1) | Lazy static initialisation |

Dev: `assert_cmd`, `predicates`, `tempfile`, `insta` (snapshot testing, not actively used yet).

## Rust Toolchain

Stable channel, minimum `1.82`. Defined in `rust-toolchain.toml`.

## Exit Codes

`0` = success, `1` = translation errors (only with `--on-error fail`), `2` = CLI argument or I/O error.

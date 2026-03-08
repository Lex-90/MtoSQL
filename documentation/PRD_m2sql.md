# PRD: `m2sql` — M Code to SQL CLI Translator

**Version:** 1.0.0  
**Status:** Ready for implementation  
**Runtime:** Rust (stable toolchain)  
**Audience:** Claude Code (AI coding agent)

---

## 1. Purpose

`m2sql` is a deterministic, fully-offline command-line tool that translates Power Query M code into equivalent SQL queries. It accepts M source from `.pq`/`.m` files, `.tmdl` files extracted from Power BI `.pbip` projects, or raw M code piped via stdin. For each logical M query it emits one `.sql` file, rendering multi-step `let … in` pipelines as CTEs (`WITH` clauses). The target SQL dialect is chosen by the user at runtime.

---

## 2. Goals & Non-Goals

### Goals
- Deterministic translation: identical input always produces identical output.
- Fully offline: zero network calls at any point.
- Dialect-aware SQL emission for: `tsql`, `postgres`, `bigquery`, `snowflake`, `duckdb`.
- One `.sql` output file per logical M query.
- Multi-step pipelines rendered as CTEs.
- Configurable behaviour for untranslatable expressions.
- Source table names inferred from M data-source references.

### Non-Goals (v1)
- GUI or web interface.
- Round-trip SQL → M translation.
- Power BI semantic-model evaluation (DAX, relationships).
- M features beyond the v1 scope defined in §6.
- `.pbix` binary file parsing (only `.pbip`/`.tmdl` plain-text projects).

---

## 3. CLI Interface

### 3.1 Binary name
```
m2sql
```

### 3.2 Synopsis
```
m2sql [OPTIONS] [INPUT...]
```

### 3.3 Arguments

| Argument | Type | Description |
|---|---|---|
| `INPUT` | Zero or more file paths | `.pq`, `.m`, or `.tmdl` files to translate. If omitted, reads from stdin. |

### 3.4 Options

| Flag | Short | Type | Default | Description |
|---|---|---|---|---|
| `--dialect` | `-d` | enum | *(required)* | Target SQL dialect. One of: `tsql`, `postgres`, `bigquery`, `snowflake`, `duckdb`. |
| `--output-dir` | `-o` | path | `./output` | Directory where `.sql` files are written. Created if absent. |
| `--on-error` | `-e` | enum | `warn` | Behaviour when an untranslatable expression is encountered. One of: `fail`, `comment`, `warn`. See §7. |
| `--stdout` | | bool flag | false | Print all SQL to stdout instead of writing files. Queries are separated by `-- [query: <name>]` banners. |
| `--no-color` | | bool flag | false | Disable ANSI colour in terminal output. |
| `--version` | `-V` | | | Print version and exit. |
| `--help` | `-h` | | | Print help and exit. |

### 3.5 Exit codes

| Code | Meaning |
|---|---|
| 0 | All queries translated successfully (warnings may have been emitted). |
| 1 | One or more queries failed translation and `--on-error fail` was set. |
| 2 | Bad CLI arguments or unreadable input files. |

### 3.6 Example invocations

```bash
# Translate a single .pq file to DuckDB SQL
m2sql --dialect duckdb Sales.pq

# Translate all .tmdl files in a directory to Snowflake SQL, fail on any error
m2sql --dialect snowflake --on-error fail model/tables/*.tmdl

# Pipe M code from stdin and print to stdout
echo 'let Source = Sql.Database("srv","db"), T = Source{[Name="Orders"]}[Data] in T' \
  | m2sql --dialect postgres --stdout

# Translate a directory of .m files with BigQuery dialect, output to /tmp/sql
m2sql --dialect bigquery --output-dir /tmp/sql queries/*.m
```

---

## 4. Project Structure

```
m2sql/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── src/
│   ├── main.rs              # CLI entry point (clap)
│   ├── cli.rs               # Argument definitions and validation
│   ├── pipeline.rs          # Orchestrates parse → resolve → translate → emit
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── lexer.rs         # M tokeniser
│   │   ├── grammar.rs       # M recursive-descent parser → AST
│   │   └── ast.rs           # AST node types
│   ├── resolver/
│   │   ├── mod.rs
│   │   └── source.rs        # Infers table/schema names from data-source expressions
│   ├── translator/
│   │   ├── mod.rs
│   │   ├── context.rs       # Translation context (dialect, error mode, warnings)
│   │   ├── cte.rs           # let…in → CTE chain builder
│   │   ├── expr.rs          # M expression → SQL expression
│   │   ├── functions.rs     # Per-function translators (SelectRows, Join, etc.)
│   │   └── cast.rs          # M type → SQL type coercion rules
│   ├── dialect/
│   │   ├── mod.rs           # Dialect trait
│   │   ├── tsql.rs
│   │   ├── postgres.rs
│   │   ├── bigquery.rs
│   │   ├── snowflake.rs
│   │   └── duckdb.rs
│   ├── emitter/
│   │   ├── mod.rs
│   │   └── sql_writer.rs    # Pretty-prints SQL AST to string
│   └── error.rs             # Error and Warning types
├── tests/
│   ├── fixtures/            # .pq / .tmdl input files
│   │   ├── select_rows.pq
│   │   ├── join.pq
│   │   ├── group_by.pq
│   │   ├── add_column.pq
│   │   ├── cast.pq
│   │   ├── multi_step.pq
│   │   └── tmdl_table.tmdl
│   ├── snapshots/           # Expected .sql output per dialect
│   │   ├── tsql/
│   │   ├── postgres/
│   │   ├── bigquery/
│   │   ├── snowflake/
│   │   └── duckdb/
│   ├── integration_tests.rs
│   └── unit/
│       ├── parser_tests.rs
│       ├── resolver_tests.rs
│       └── translator_tests.rs
```

---

## 5. Input Formats

### 5.1 `.pq` / `.m` files
Standard Power Query M source files. One file may contain one top-level `let … in` expression or a named query section (`section <name>; shared <name> = …`).

### 5.2 `.tmdl` files (Power BI `.pbip` projects)
TMDL (Tabular Model Definition Language) files contain M partition expressions inside `m` blocks. Example structure:

```tmdl
table Sales
    partition Sales = m
        mode: import
        source = ```
            let
                Source = Sql.Database("server", "db"),
                Sales = Source{[Name="Sales"]}[Data]
            in
                Sales
        ```
```

The parser must extract the content of each ` ``` … ``` ` block tagged as `source =` within a partition definition and treat it as an M expression. The enclosing `table <name>` value is used as the output file name.

### 5.3 Stdin
Raw M code. The query is named `query` by default; override with `--query-name <name>`.

---

## 6. M Feature Surface (v1 Scope)

All translation rules must be implemented for every supported dialect. Unsupported constructs trigger the configured `--on-error` behaviour.

### 6.1 Data source inference (`resolver/source.rs`)

Recognise these canonical data-source patterns and extract schema/table names:

| M pattern | Resolved as |
|---|---|
| `Sql.Database("server", "db")` | schema = `db` |
| `Source{[Name="TableName"]}[Data]` | table = `TableName` |
| `Source{[Schema="s", Item="t"]}[Data]` | schema = `s`, table = `t` |
| `Excel.Workbook(...)` | table name from sheet name if deterministic |
| Unrecognised source | emit placeholder `/* SOURCE: <expr> */` |

### 6.2 `let … in` → CTE chain

Each binding in the `let` block that produces a table value becomes a CTE. The `in` expression names the final CTE or is inlined as the outer `SELECT`.

**Rules:**
- Bindings that are scalar (not table-valued) are inlined as SQL expressions, not CTEs.
- Bindings used only once may be inlined (optional optimisation, off by default; enable with `--inline-singles`).
- CTE names are the M binding names, sanitised to valid SQL identifiers (replace spaces and special chars with `_`).

**Example M:**
```m
let
    Source    = Sql.Database("srv", "AdventureWorks"),
    RawOrders = Source{[Name="Orders"]}[Data],
    Filtered  = Table.SelectRows(RawOrders, each [Amount] > 100),
    Result    = Table.SelectColumns(Filtered, {"OrderID", "Amount"})
in
    Result
```

**Expected SQL (all dialects):**
```sql
WITH RawOrders AS (
    SELECT *
    FROM AdventureWorks.Orders
),
Filtered AS (
    SELECT *
    FROM RawOrders
    WHERE Amount > 100
),
Result AS (
    SELECT OrderID, Amount
    FROM Filtered
)
SELECT * FROM Result;
```

### 6.3 `Table.SelectRows` → `WHERE`

```m
Table.SelectRows(table, each <condition>)
```

- `each` introduces a row context; `_[ColumnName]` and `[ColumnName]` both refer to the current row.
- Supported condition operators: `=`, `<>`, `>`, `<`, `>=`, `<=`, `and`, `or`, `not`.
- Supported functions within conditions: `Text.StartsWith`, `Text.EndsWith`, `Text.Contains`, `List.Contains`, `Date.From`, `DateTime.From`.
- Translate `Text.StartsWith(x, v)` → `x LIKE 'v%'`, `Text.Contains(x, v)` → `x LIKE '%v%'`, etc.
- `List.Contains({v1,v2}, x)` → `x IN (v1, v2)`.

### 6.4 `Table.SelectColumns` → `SELECT <cols>`

```m
Table.SelectColumns(table, {"Col1", "Col2", ...})
-- or with rename:
Table.SelectColumns(table, {{"NewName", each [OldName]}, ...})
```

Translate to `SELECT Col1, Col2, … FROM <table>` or `SELECT OldName AS NewName, …`.

### 6.5 `Table.Join` / `Table.NestedJoin` → `JOIN`

```m
Table.Join(left, leftKey, right, rightKey, joinKind)
Table.NestedJoin(left, leftKey, right, rightKey, newCol, joinKind)
```

**Join kind mapping:**

| M JoinKind | SQL |
|---|---|
| `JoinKind.Inner` (default) | `INNER JOIN` |
| `JoinKind.Left` | `LEFT JOIN` |
| `JoinKind.Right` | `RIGHT JOIN` |
| `JoinKind.Full` | `FULL OUTER JOIN` |
| `JoinKind.LeftAnti` | `LEFT JOIN … WHERE right.key IS NULL` |
| `JoinKind.RightAnti` | `RIGHT JOIN … WHERE left.key IS NULL` |

For `Table.NestedJoin`, the nested column is expanded into a `JOIN` (the nested table column is not materialised; the result columns must be subsequently expanded by a `Table.ExpandTableColumn` step, which is translated as additional `SELECT` projections).

Composite keys (list literals) → `ON a.k1 = b.k1 AND a.k2 = b.k2`.

### 6.6 `Table.Group` → `GROUP BY`

```m
Table.Group(table, {"GroupKey"}, {
    {"AggCol", each List.Sum([Value]), type number},
    ...
})
```

**Aggregate function mapping:**

| M function | SQL |
|---|---|
| `List.Sum([col])` | `SUM(col)` |
| `List.Average([col])` | `AVG(col)` |
| `List.Count([col])` / `Table.RowCount(_)` | `COUNT(*)` or `COUNT(col)` |
| `List.Min([col])` | `MIN(col)` |
| `List.Max([col])` | `MAX(col)` |
| `List.CountDistinct([col])` | `COUNT(DISTINCT col)` |

### 6.7 `Table.AddColumn` → computed column in `SELECT`

```m
Table.AddColumn(table, "NewCol", each <expr>, type <type>)
```

Translate as a new `SELECT` column: `<expr> AS NewCol`. The `type` hint is used for explicit `CAST` if the expression is ambiguous. `AddColumn` wraps the preceding CTE in a new CTE or is merged into the parent `SELECT`.

### 6.8 Type coercions & `CAST`

**M type → SQL type mapping per dialect:**

| M type | tsql | postgres | bigquery | snowflake | duckdb |
|---|---|---|---|---|---|
| `type text` | `NVARCHAR(MAX)` | `TEXT` | `STRING` | `VARCHAR` | `VARCHAR` |
| `type number` | `FLOAT` | `DOUBLE PRECISION` | `FLOAT64` | `FLOAT` | `DOUBLE` |
| `type integer` / `Int64.Type` | `BIGINT` | `BIGINT` | `INT64` | `BIGINT` | `BIGINT` |
| `type logical` | `BIT` | `BOOLEAN` | `BOOL` | `BOOLEAN` | `BOOLEAN` |
| `type date` | `DATE` | `DATE` | `DATE` | `DATE` | `DATE` |
| `type datetime` | `DATETIME2` | `TIMESTAMP` | `DATETIME` | `TIMESTAMP_NTZ` | `TIMESTAMP` |
| `type datetimezone` | `DATETIMEOFFSET` | `TIMESTAMPTZ` | `TIMESTAMP` | `TIMESTAMP_TZ` | `TIMESTAMPTZ` |
| `type duration` | `— (unsupported)` | `INTERVAL` | `— (unsupported)` | `— (unsupported)` | `INTERVAL` |
| `type binary` | `VARBINARY(MAX)` | `BYTEA` | `BYTES` | `BINARY` | `BLOB` |

`Table.TransformColumnTypes` → emit `CAST(col AS <type>)` for each transformed column, wrapping in a CTE or merging into the parent `SELECT`.

`Value.ReplaceType(expr, type)` → `CAST(expr AS <type>)`.

Unsupported types emit a warning and substitute the literal token `/* UNSUPPORTED_TYPE */`.

---

## 7. Error Handling

Controlled by `--on-error`:

| Mode | Behaviour |
|---|---|
| `fail` | Halt immediately on first untranslatable expression. Exit code 1. Print the M expression, file name, and line number to stderr. |
| `comment` | Replace the untranslatable expression with a SQL comment: `/* UNTRANSLATABLE: <original M> */`. Continue processing. Exit code 0. |
| `warn` (default) | Same as `comment`, but additionally print a structured warning to stderr: `WARN [file:line] Untranslatable: <description>`. Exit code 0. |

### Warning format (stderr)
```
WARN  [Sales.pq:14] Untranslatable expression: Table.Pivot — not supported in v1
WARN  [Sales.pq:22] Unsupported type: type duration — substituted with comment
```

### Error format (stderr, `--on-error fail`)
```
ERROR [Sales.pq:14] Cannot translate: Table.Pivot
  Source: Table.Pivot(Unpivoted, List.Distinct(Unpivoted[Attribute]), "Attribute", "Value")
  Dialect: postgres
  Reason: Table.Pivot is not in the v1 feature set.
```

---

## 8. Output Naming Convention

| Source | Output file name |
|---|---|
| `Sales.pq` | `Sales.sql` |
| `Orders.m` | `Orders.sql` |
| `model/tables/Sales.tmdl` (table `Sales`) | `Sales.sql` |
| stdin, `--query-name Foo` | `Foo.sql` |
| stdin, no name | `query.sql` |

If two queries resolve to the same output name, append `_2`, `_3`, etc.

---

## 9. Dialect-Specific Emission Rules

Implemented in `src/dialect/<name>.rs`, each implementing the `Dialect` trait:

```rust
pub trait Dialect: Send + Sync {
    fn name(&self) -> &'static str;
    fn identifier_quote(&self) -> char;          // " for most, ` for bigquery
    fn string_quote(&self) -> char;              // ' for all
    fn cast_syntax(&self, expr: &str, ty: &str) -> String; // CAST(x AS t) vs x::t
    fn supports_cte(&self) -> bool;              // all true in v1
    fn ilike_supported(&self) -> bool;           // postgres, snowflake, duckdb = true
    fn top_n_syntax(&self) -> TopNSyntax;        // TOP vs LIMIT vs QUALIFY
    fn boolean_literal(&self, val: bool) -> &'static str; // TRUE/FALSE vs 1/0
    fn current_timestamp(&self) -> &'static str;
    fn map_type(&self, m_type: &MType) -> &'static str;
}
```

**Dialect-specific notes:**

- **T-SQL**: Use `[identifier]` quoting. `CAST` not `::`. `TOP n` instead of `LIMIT`. `BIT` for booleans. `GETDATE()` for current timestamp.
- **PostgreSQL**: Use `"identifier"` quoting. Support `::` cast shorthand. `ILIKE` for case-insensitive `LIKE`. `NOW()` for current timestamp.
- **BigQuery**: Use `` `identifier` `` quoting. `CAST(x AS type)` only. Table references as `` `project.dataset.table` `` if fully qualified. `CURRENT_TIMESTAMP()`.
- **Snowflake**: Use `"identifier"` quoting. `ILIKE` supported. `CURRENT_TIMESTAMP()`.
- **DuckDB**: Use `"identifier"` quoting. `::` cast shorthand. `ILIKE` supported. `now()` for current timestamp.

---

## 10. AST Design

### 10.1 M AST (simplified)

```rust
// src/parser/ast.rs

pub enum MExpr {
    Let { bindings: Vec<(String, MExpr)>, body: Box<MExpr> },
    FunctionCall { name: String, args: Vec<MExpr> },
    FieldAccess { expr: Box<MExpr>, field: String },
    IndexAccess { expr: Box<MExpr>, key: Box<MExpr> },
    EachExpr(Box<MExpr>),                      // each <expr>
    RowField(String),                          // [ColumnName] inside each
    BinaryOp { op: BinOp, left: Box<MExpr>, right: Box<MExpr> },
    UnaryOp { op: UnOp, expr: Box<MExpr> },
    Literal(MLiteral),
    Identifier(String),
    List(Vec<MExpr>),
    Record(Vec<(String, MExpr)>),
    TypeAnnotation { expr: Box<MExpr>, ty: MType },
}

pub enum MLiteral { Text(String), Number(f64), Integer(i64), Bool(bool), Null }

pub enum MType {
    Text, Number, Integer, Logical, Date, DateTime, DateTimeZone,
    Duration, Binary, Table, Any, Unknown(String),
}

pub enum BinOp { Eq, Neq, Lt, Lte, Gt, Gte, And, Or, Add, Sub, Mul, Div }
pub enum UnOp  { Not, Neg }
```

### 10.2 SQL AST (simplified)

```rust
// src/emitter/sql_writer.rs

pub struct SqlQuery {
    pub ctes: Vec<Cte>,
    pub final_select: SelectStmt,
}

pub struct Cte { pub name: String, pub select: SelectStmt }

pub struct SelectStmt {
    pub columns: Vec<SelectCol>,
    pub from: Option<TableRef>,
    pub joins: Vec<JoinClause>,
    pub where_: Option<SqlExpr>,
    pub group_by: Vec<SqlExpr>,
    pub having: Option<SqlExpr>,
    pub order_by: Vec<OrderByItem>,
    pub limit: Option<u64>,
}

pub enum SelectCol {
    Wildcard,
    Expr { expr: SqlExpr, alias: Option<String> },
}

pub enum SqlExpr {
    Column { table: Option<String>, name: String },
    Literal(SqlLiteral),
    BinaryOp { op: SqlBinOp, left: Box<SqlExpr>, right: Box<SqlExpr> },
    UnaryOp { op: SqlUnOp, expr: Box<SqlExpr> },
    FunctionCall { name: String, args: Vec<SqlExpr>, distinct: bool },
    Cast { expr: Box<SqlExpr>, ty: String },
    Like { expr: Box<SqlExpr>, pattern: String, negated: bool, case_insensitive: bool },
    InList { expr: Box<SqlExpr>, list: Vec<SqlExpr>, negated: bool },
    IsNull { expr: Box<SqlExpr>, negated: bool },
    Raw(String), // escape hatch for dialect-specific fragments
}
```

---

## 11. Testing Strategy

### 11.1 Unit tests
- `parser_tests.rs`: parse each M construct into expected AST nodes.
- `resolver_tests.rs`: verify source inference for all recognised patterns.
- `translator_tests.rs`: per-function translation, per-dialect type mapping.

### 11.2 Snapshot / integration tests
Use `insta` crate for snapshot testing.

For each fixture in `tests/fixtures/`, assert that the translated SQL matches the snapshot in `tests/snapshots/<dialect>/`.

Run: `cargo test` and `cargo insta review` for snapshot approval.

### 11.3 Required test fixtures (minimum)

| Fixture | M construct exercised |
|---|---|
| `select_rows_simple.pq` | Single `Table.SelectRows` with comparison |
| `select_rows_text.pq` | `Text.StartsWith`, `Text.Contains` |
| `select_rows_list.pq` | `List.Contains` |
| `select_cols.pq` | `Table.SelectColumns` with rename |
| `join_inner.pq` | `Table.Join` — inner |
| `join_left.pq` | `Table.Join` — left |
| `join_anti.pq` | `Table.Join` — left anti |
| `join_composite.pq` | `Table.Join` — composite key |
| `nested_join.pq` | `Table.NestedJoin` + `Table.ExpandTableColumn` |
| `group_basic.pq` | `Table.Group` — SUM, COUNT |
| `group_multi.pq` | `Table.Group` — multiple aggregates |
| `add_column.pq` | `Table.AddColumn` with arithmetic expr |
| `cast_types.pq` | `Table.TransformColumnTypes` — multiple types |
| `multi_step.pq` | Full `let … in` pipeline → CTE chain |
| `tmdl_table.tmdl` | TMDL file with embedded M partition |
| `untranslatable.pq` | Unknown function → `--on-error` modes |
| `stdin_query` | (tested via CLI process spawn) |

### 11.4 Determinism test
Run the same input 100 times and assert byte-for-byte identical output. Include in CI.

---

## 12. Dependencies (`Cargo.toml`)

```toml
[dependencies]
clap          = { version = "4", features = ["derive"] }
anyhow        = "1"
thiserror     = "1"
indexmap      = "2"          # Preserve insertion order for CTEs
regex         = "1"
once_cell     = "1"
insta         = { version = "1", features = ["yaml"] }  # snapshot testing

[dev-dependencies]
insta         = { version = "1", features = ["yaml"] }
assert_cmd    = "2"          # CLI integration tests
predicates    = "3"
```

No network-capable crates. No async runtime required (all I/O is synchronous file operations).

---

## 13. Constraints

- **Offline**: No `reqwest`, `hyper`, or any HTTP client. No DNS lookups.
- **Deterministic**: No random identifiers, no timestamp-based names, no hash-based ordering unless the hash is seeded from content.
- **No unsafe Rust** outside of explicitly justified FFI blocks (none expected in v1).
- **MSRV**: Rust 1.75 stable.
- **Single binary**: No dynamic linking to non-system libraries. Build with `cargo build --release`.

---

## 14. Implementation Order (suggested)

1. `src/parser/` — lexer + grammar + AST (cover all v1 M constructs).
2. `src/resolver/source.rs` — data source inference.
3. `src/dialect/` — `Dialect` trait + all five implementations.
4. `src/translator/expr.rs` — M expression → SQL expression (no table ops yet).
5. `src/translator/functions.rs` — one function at a time, test fixture per function.
6. `src/translator/cte.rs` — `let … in` → CTE chain.
7. `src/translator/cast.rs` — type mapping table.
8. `src/emitter/sql_writer.rs` — SQL AST pretty-printer.
9. `src/pipeline.rs` — wire all stages together.
10. `src/cli.rs` + `src/main.rs` — argument parsing, file I/O, stdin handling.
11. Integration tests + snapshot baselines.
12. TMDL parser (builds on top of the M parser; extracts `source = ``` … ```  ` blocks).

---

## 15. Acceptance Criteria

- [ ] All 18 test fixtures translate without errors for all 5 dialects.
- [ ] All snapshot tests pass (`cargo insta test`).
- [ ] `--on-error fail` exits with code 1 on `untranslatable.pq`.
- [ ] `--on-error comment` exits with code 0 and embeds `/* UNTRANSLATABLE: … */`.
- [ ] `--on-error warn` exits with code 0 and prints `WARN` lines to stderr.
- [ ] Determinism test passes (100 runs, byte-identical output).
- [ ] `cargo clippy -- -D warnings` produces zero warnings.
- [ ] `cargo test` passes on Linux, macOS, and Windows.
- [ ] Binary size ≤ 10 MB release build.
- [ ] `--help` output documents all flags with types and defaults.

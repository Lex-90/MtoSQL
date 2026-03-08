# PRD: `m2sql` — M Code to SQL CLI Translator

**Version:** 1.3.0
**Status:** Ready for implementation
**Runtime:** Rust (stable toolchain)
**Audience:** Claude Code (AI coding agent)

> **Changelog from v1.2.0**
> - §6.11: New — `Table.RemoveColumns` → dialect-aware column exclusion (`SELECT * EXCEPT` or explicit projection).
> - §6.12: New — `Table.Combine` → `UNION ALL`, with resolver-level validation that every referenced table is either a binding in the same `let` expression or the stem of an input file supplied on the CLI.
> - §11.3: Added `remove_cols.pq` and `combine_tables.pq` fixtures.
> - §15: Added acceptance criteria for `Table.RemoveColumns` and `Table.Combine`.
>
> **Changelog from v1.1.0**
> - §5.2: Documented indentation-based `source =` block format for TMDL files (in addition to backtick-delimited blocks).
> - §11.3: Added `tmdl_indent.tmdl` fixture for indentation-based TMDL source blocks.
> - §15: Added acceptance criterion for indentation-based TMDL parsing.
>
> **Changelog from v1.0.0 → v1.1.0**
> - §3.4: Added missing flags `--query-name`, `--inline-singles`, `--log-json`; clarified `--stdout` separator format.
> - §5.1: Documented M section syntax (`section … ; shared …`) and its output mapping.
> - §6.3: Added `Text.EndsWith` → `LIKE '%v'` translation (was listed as supported but never specced).
> - §6.4: Corrected `Table.SelectColumns` rename syntax (the `each [OldName]` form does not exist in M); column rename is now a separate function — see new §6.9.
> - §6.6: Split `List.Count` / `Table.RowCount` into distinct rows with correct SQL mappings.
> - §6.9: New — `Table.RenameColumns` → `SELECT OldName AS NewName`.
> - §6.10: New — `Table.ExpandTableColumn` → column projections in JOIN output.
> - §7: Corrected `--on-error fail` semantics from "halt on first error" to "report-and-exit after processing all files".
> - §8: Added output naming rule for section-syntax shared bindings.
> - §9: Added `identifier_close_quote()` to `Dialect` trait to support T-SQL `[…]` bracket quoting.
> - §11.3: Added `rename_cols.pq` and `section_syntax.pq` fixtures; updated fixture descriptions.
> - §12: Moved `insta` to `[dev-dependencies]` only; added `criterion` to `[dev-dependencies]`.
> - §15: Updated acceptance criteria fixture count.

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
| `--stdout` | | bool flag | false | Print all SQL to stdout instead of writing files. Queries are separated by `-- [query: <name>]` banners, where `<name>` is the query name / output file stem (e.g. `-- [query: Sales]`). |
| `--query-name` | `-n` | string | `query` | Name for the query when reading from stdin. Used as the output file stem. Ignored when `INPUT` file arguments are provided. |
| `--inline-singles` | | bool flag | false | Inline CTE bindings that are referenced exactly once directly into the referencing expression, instead of emitting a named CTE for them. Off by default; opt-in optimisation. |
| `--log-json` | | bool flag | false | Emit all diagnostic output (warnings, errors, summary) as newline-delimited JSON objects to stderr instead of plain text. Field schema defined in §8 of the NFR. |
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

# Pipe M code from stdin and print to stdout, naming the query
echo 'let Source = Sql.Database("srv","db"), T = Source{[Name="Orders"]}[Data] in T' \
  | m2sql --dialect postgres --stdout --query-name Orders

# Translate a directory of .m files with BigQuery dialect, output to /tmp/sql
m2sql --dialect bigquery --output-dir /tmp/sql queries/*.m

# JSON log output, useful for CI log parsing
m2sql --dialect tsql --log-json model/tables/*.tmdl

# Enable single-use CTE inlining
m2sql --dialect duckdb --inline-singles Sales.pq
```

---

## 4. Project Structure

```
m2sql/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml          # Pins the exact Rust toolchain version
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
│   │   ├── select_rows_simple.pq
│   │   ├── select_rows_text.pq
│   │   ├── select_rows_list.pq
│   │   ├── select_cols.pq
│   │   ├── rename_cols.pq
│   │   ├── join_inner.pq
│   │   ├── join_left.pq
│   │   ├── join_anti.pq
│   │   ├── join_composite.pq
│   │   ├── nested_join.pq
│   │   ├── group_basic.pq
│   │   ├── group_multi.pq
│   │   ├── add_column.pq
│   │   ├── cast_types.pq
│   │   ├── multi_step.pq
│   │   ├── section_syntax.pq
│   │   ├── tmdl_table.tmdl
│   │   └── untranslatable.pq
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

Two sub-formats are supported:

**Single `let … in` expression** — the entire file is one M expression:
```m
let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data]
in
    Orders
```
Output: one `.sql` file named after the input file stem (e.g. `Orders.pq` → `Orders.sql`).

**Section syntax with `shared` bindings** — the file begins with a `section` declaration and contains one or more named, exported queries:
```m
section MyQueries;
shared Orders = let
    Source = Sql.Database("srv", "db"),
    Data   = Source{[Name="Orders"]}[Data]
in Data;

shared Customers = let
    Source = Sql.Database("srv", "db"),
    Data   = Source{[Name="Customers"]}[Data]
in Data;
```
Output: **one `.sql` file per `shared` binding**, named after the binding name (e.g. `Orders.sql`, `Customers.sql`). Non-`shared` (private) bindings within the section are treated as internal helpers and are not emitted as top-level queries; if a `shared` binding references a private one, the private expression is inlined or emitted as a CTE within the referencing query's output file.

### 5.2 `.tmdl` files (Power BI `.pbip` projects)
TMDL (Tabular Model Definition Language) files contain M partition expressions inside `m` blocks. Two source block formats are supported:

**Backtick-delimited source blocks** — the M code is enclosed in `` ``` … ``` ``:

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

**Indentation-based source blocks** — the `source =` line ends with `=` and the M code follows on subsequent lines indented deeper than the `source` line:

```tmdl
table D_Agenti
    partition D_Agenti = m
        mode: import
        queryGroup: Dimensions
        source =
                let
                    Origine = Sql.Database(sql_server, sql_db),
                    dbo_V_PBI_ANAG_AGENTI = Origine{[Schema="dbo",Item="V_PBI_ANAG_AGENTI"]}[Data],
                    #"Rimosse colonne" = Table.RemoveColumns(dbo_V_PBI_ANAG_AGENTI,{"PARTITA_IVA", "COD_SDI", "PEC"})
                in
                    #"Rimosse colonne"
```

For both formats, the parser extracts the M source from within the `source =` block and treats it as an M expression. The enclosing `table <n>` value is used as the output file name.

The indentation-based format is the native format used by Power BI Desktop when exporting `.pbip` projects. The block ends when a line at the same or lesser indentation level as the `source` keyword is encountered, or at end of file.

### 5.3 Stdin
Raw M code (single `let … in` expression). The query is named `query` by default; override with `--query-name <n>`.

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
- Bindings used only once may be inlined (optional optimisation; enable with `--inline-singles`).
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

| M function | SQL translation |
|---|---|
| `Text.StartsWith(x, v)` | `x LIKE 'v%'` |
| `Text.EndsWith(x, v)` | `x LIKE '%v'` |
| `Text.Contains(x, v)` | `x LIKE '%v%'` |
| `List.Contains({v1,v2}, x)` | `x IN (v1, v2)` |

For dialects where `ILIKE` is supported (`postgres`, `snowflake`, `duckdb`), `Text.*` comparisons are case-sensitive LIKE by default. An optional third argument `Comparer.OrdinalIgnoreCase` maps to `ILIKE`; all other comparers emit a warning and fall back to `LIKE`.

### 6.4 `Table.SelectColumns` → `SELECT <cols>`

```m
Table.SelectColumns(table, {"Col1", "Col2", ...})
```

Translates to `SELECT Col1, Col2, … FROM <table>`. Column order in the output follows the order of the list literal.

> **Note on renaming:** `Table.SelectColumns` does **not** support inline column renaming in the M language. To rename columns, Power Query uses `Table.RenameColumns` as a separate step (see §6.9). The pattern `{{"NewName", each [OldName]}, ...}` does not exist in the M specification and must not be generated or accepted.

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

For `Table.NestedJoin`, the nested column (`newCol`) is not materialised as a table-valued column; the step must be followed by `Table.ExpandTableColumn` (see §6.10), which is translated as additional `SELECT` projections in the same CTE. A `Table.NestedJoin` without a subsequent `Table.ExpandTableColumn` emits a warning and selects only the left-side columns.

Composite keys (list literals) → `ON a.k1 = b.k1 AND a.k2 = b.k2`.

### 6.6 `Table.Group` → `GROUP BY`

```m
Table.Group(table, {"GroupKey"}, {
    {"AggCol", each List.Sum([Value]), type number},
    ...
})
```

**Aggregate function mapping:**

| M function | SQL | Notes |
|---|---|---|
| `List.Sum([col])` | `SUM(col)` | |
| `List.Average([col])` | `AVG(col)` | |
| `List.Count([col])` | `COUNT(col)` | Counts non-null values in the column |
| `Table.RowCount(_)` | `COUNT(*)` | Counts all rows regardless of nulls |
| `List.Min([col])` | `MIN(col)` | |
| `List.Max([col])` | `MAX(col)` | |
| `List.CountDistinct([col])` | `COUNT(DISTINCT col)` | |

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

### 6.9 `Table.RenameColumns` → column aliases in `SELECT`

```m
Table.RenameColumns(table, {{"OldName", "NewName"}, {"OldName2", "NewName2"}, ...})
```

Translates to a new CTE (or merges into the parent SELECT when `--inline-singles` is active) that projects each renamed column as `OldName AS NewName`. Columns not appearing in the rename list are passed through unchanged via `SELECT *` expansion or explicit listing depending on whether any non-renamed columns need to survive.

**Implementation rule:** Because SQL `SELECT * EXCEPT (...)` is not universally supported, emit all column aliases explicitly when the rename list is partial. If the full set of columns is not statically known at translation time (e.g. the source is `/* SOURCE: … */`), emit `/* WARN: full column list unknown; rename translated as partial alias list */`.

**Example M:**
```m
Table.RenameColumns(Orders, {{"CustomerNo", "CustomerID"}, {"Amt", "Amount"}})
```

**Expected SQL:**
```sql
SELECT CustomerNo AS CustomerID, Amt AS Amount, <other cols>
FROM Orders
```

### 6.10 `Table.ExpandTableColumn` → JOIN column projections

```m
Table.ExpandTableColumn(table, "NestedCol", {"Col1", "Col2", ...}, {"Alias1", "Alias2", ...})
```

Supported only when immediately following a `Table.NestedJoin` step (the column list form). Translates by absorbing the expansion into the JOIN CTE generated by the `NestedJoin`, adding the specified columns from the right-hand table to the `SELECT` list.

**Rules:**
- The `"NestedCol"` argument must match the `newCol` parameter of the preceding `Table.NestedJoin`; a mismatch emits an error.
- The optional fourth argument (alias list) maps to `right.ColN AS AliasN` in the SELECT. When absent, columns are projected without aliases.
- Only the column-list form is supported in v1. An `ExpandTableColumn` that targets a non-join nested column (e.g. from `Table.AddColumn`) emits `/* UNTRANSLATABLE: ExpandTableColumn on non-join source */`.

**Example M:**
```m
let
    Source    = Sql.Database("srv", "db"),
    Orders    = Source{[Name="Orders"]}[Data],
    Customers = Source{[Name="Customers"]}[Data],
    Joined    = Table.NestedJoin(Orders, "CustomerID", Customers, "ID", "CustData", JoinKind.Left),
    Expanded  = Table.ExpandTableColumn(Joined, "CustData", {"Name", "Email"}, {"CustName", "CustEmail"})
in
    Expanded
```

**Expected SQL:**
```sql
WITH Orders AS (
    SELECT * FROM db.Orders
),
Customers AS (
    SELECT * FROM db.Customers
),
Expanded AS (
    SELECT
        Orders.*,
        Customers.Name  AS CustName,
        Customers.Email AS CustEmail
    FROM Orders
    LEFT JOIN Customers ON Orders.CustomerID = Customers.ID
)
SELECT * FROM Expanded;
```

### 6.11 `Table.RemoveColumns` → column exclusion in `SELECT`

```m
Table.RemoveColumns(table, {"Col1", "Col2", ...})
```

Translates to a `SELECT` that returns every column of `table` **except** the listed columns. Because SQL has no universally portable `SELECT * EXCEPT (…)` syntax, the translation strategy is dialect-dependent:

| Dialect | Strategy |
|---|---|
| `bigquery` | `SELECT * EXCEPT (Col1, Col2)` — natively supported |
| `duckdb` | `SELECT * EXCEPT (Col1, Col2)` — natively supported |
| `tsql` | Explicit column list: `SELECT ColA, ColB, … FROM <table>` (columns that are **not** in the remove list) |
| `postgres` | Explicit column list (same as T-SQL) |
| `snowflake` | Explicit column list (same as T-SQL; Snowflake does not support `EXCEPT` in `SELECT *`) |

**Column list resolution:** To emit an explicit projection, the translator must know the full column set of the input table. The resolver attempts to infer this from the preceding CTE step. If the full column list cannot be determined statically (e.g. the source is an unresolved `/* SOURCE: … */` placeholder):
- Emit `/* WARN: full column list unknown; RemoveColumns translated as EXCEPT clause */` and fall back to `SELECT * EXCEPT (Col1, Col2)` regardless of dialect.
- If the dialect does not support `EXCEPT`, emit `/* UNTRANSLATABLE: RemoveColumns on unknown column set for dialect <d> */` and apply the configured `--on-error` behaviour.

**Example M:**
```m
Table.RemoveColumns(Orders, {"InternalCode", "AuditTimestamp"})
```

**Expected SQL (tsql / postgres / snowflake — full column list known as `{OrderID, CustomerID, Amount, InternalCode, AuditTimestamp}`):**
```sql
SELECT OrderID, CustomerID, Amount
FROM Orders
```

**Expected SQL (bigquery / duckdb — or fallback when column list is unknown):**
```sql
SELECT * EXCEPT (InternalCode, AuditTimestamp)
FROM Orders
```

### 6.12 `Table.Combine` → `UNION ALL`

```m
Table.Combine({Table1, Table2, Table3, ...})
```

Translates to a `UNION ALL` of all listed tables. Column alignment follows M semantics: columns are matched by name, not position; missing columns in any member table are filled with `NULL`.

**Translation:**
```sql
SELECT * FROM Table1
UNION ALL
SELECT * FROM Table2
UNION ALL
SELECT * FROM Table3
```

If column sets differ across the combined tables and the full column list can be statically determined, the translator emits explicit column lists with `NULL AS <missing_col>` fill-ins per member. When the column set is not statically known, `SELECT *` per member is emitted with a warning comment.

#### Cross-file reference validation (resolver requirement)

> [!IMPORTANT]
> Every table identifier passed inside the list argument to `Table.Combine` **must** resolve to one of the following:
> 1. A binding defined in the same `let … in` expression (i.e. a CTE name that appears earlier in the same binding chain), or
> 2. The file stem of one of the input files supplied on the CLI for the current run (e.g. a file `Customers.pq` satisfies a reference to the identifier `Customers`).

If a referenced identifier cannot be resolved against either source, the tool must:
- Emit an `UNRESOLVED_COMBINE_TABLE` error/warning identifying the file, line number, and the unresolved identifier.
- Apply the configured `--on-error` behaviour (fail / comment / warn) for that individual unresolved reference.
- Under `comment` or `warn` modes, replace the unresolvable member with `/* UNRESOLVED: <identifier> */` and continue.

This validation is performed in `src/resolver/source.rs` as a new `resolve_combine_tables` pass that runs after the per-file resolver has completed for all input files, so it has access to the full set of input file stems.

**CLI context note:** When reading from stdin (no file arguments), there are no input file stems to validate against; `Table.Combine` members must be bindings within the same `let` expression, or they are flagged as unresolved.

**Example M (valid — both `Orders` and `Returns` are bindings in the same `let`):**
```m
let
    Source  = Sql.Database("srv", "db"),
    Orders  = Source{[Name="Orders"]}[Data],
    Returns = Source{[Name="Returns"]}[Data],
    All     = Table.Combine({Orders, Returns})
in
    All
```

**Expected SQL:**
```sql
WITH Orders AS (
    SELECT * FROM db.Orders
),
Returns AS (
    SELECT * FROM db.Returns
),
All AS (
    SELECT * FROM Orders
    UNION ALL
    SELECT * FROM Returns
)
SELECT * FROM All;
```

**Example M (valid — `Customers.pq` is among the CLI input files):**
```m
-- In file: Orders.pq, CLI call: m2sql Orders.pq Customers.pq
let
    Source = Sql.Database("srv", "db"),
    Orders = Source{[Name="Orders"]}[Data],
    Combined = Table.Combine({Orders, Customers})  -- Customers resolved from Customers.pq
in
    Combined
```

**Example M (invalid — `ExternalFeed` is neither a binding nor an input file):**
```m
Table.Combine({Orders, ExternalFeed})
-- ERROR: UNRESOLVED_COMBINE_TABLE — 'ExternalFeed' is not a binding or a known input file stem
```

---

## 7. Error Handling

Controlled by `--on-error`:

| Mode | Behaviour |
|---|---|
| `fail` | Process **all** input files completely, collecting every translation error encountered. After all files are processed, print each error to stderr (or as JSON entries if `--log-json`) and **exit with code 1**. No `.sql` file is written for any file that contained at least one translation error; successfully translated files are written normally. |
| `comment` | Replace each untranslatable expression with a SQL comment: `/* UNTRANSLATABLE: <original M> */`. Continue processing. Exit code 0. |
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
| `Sales.pq` (single `let…in`) | `Sales.sql` |
| `Orders.m` (single `let…in`) | `Orders.sql` |
| `Queries.pq` (section syntax, binding `Orders`) | `Orders.sql` |
| `Queries.pq` (section syntax, binding `Customers`) | `Customers.sql` |
| `model/tables/Sales.tmdl` (table `Sales`) | `Sales.sql` |
| stdin, `--query-name Foo` | `Foo.sql` |
| stdin, no `--query-name` | `query.sql` |

If two queries resolve to the same output name, append `_2`, `_3`, etc. in the order they are encountered.

---

## 9. Dialect-Specific Emission Rules

Implemented in `src/dialect/<n>.rs`, each implementing the `Dialect` trait:

```rust
pub trait Dialect: Send + Sync {
    fn name(&self) -> &'static str;

    /// The opening quote character for identifiers.
    /// e.g. `"` for most dialects, `` ` `` for BigQuery, `[` for T-SQL.
    fn identifier_quote(&self) -> char;

    /// The closing quote character for identifiers.
    /// Defaults to `identifier_quote()`, which is correct for symmetric quoting styles.
    /// T-SQL overrides this to return `]`.
    fn identifier_close_quote(&self) -> char {
        self.identifier_quote()
    }

    fn string_quote(&self) -> char;              // ' for all dialects
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

- **T-SQL**: Use `[identifier]` quoting (`identifier_quote() = '['`, `identifier_close_quote() = ']'`). `CAST(x AS t)` syntax only (no `::` shorthand). `TOP n` instead of `LIMIT`. `BIT` for booleans. `GETDATE()` for current timestamp.
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

| Fixture | M construct(s) exercised |
|---|---|
| `select_rows_simple.pq` | `Table.SelectRows` with comparison operators |
| `select_rows_text.pq` | `Text.StartsWith`, `Text.EndsWith`, `Text.Contains` |
| `select_rows_list.pq` | `List.Contains` |
| `select_cols.pq` | `Table.SelectColumns` — column selection only |
| `rename_cols.pq` | `Table.RenameColumns` — full and partial rename lists |
| `join_inner.pq` | `Table.Join` — inner |
| `join_left.pq` | `Table.Join` — left |
| `join_anti.pq` | `Table.Join` — left anti |
| `join_composite.pq` | `Table.Join` — composite key |
| `nested_join.pq` | `Table.NestedJoin` + `Table.ExpandTableColumn` |
| `group_basic.pq` | `Table.Group` — `List.Sum`, `Table.RowCount` |
| `group_multi.pq` | `Table.Group` — multiple aggregates, `List.CountDistinct` |
| `add_column.pq` | `Table.AddColumn` with arithmetic expression |
| `cast_types.pq` | `Table.TransformColumnTypes` — all supported M types per dialect |
| `multi_step.pq` | Full `let … in` pipeline → CTE chain |
| `section_syntax.pq` | M section syntax — multiple `shared` bindings → separate `.sql` files |
| `tmdl_table.tmdl` | TMDL file with backtick-delimited M partition |
| `tmdl_indent.tmdl` | TMDL file with indentation-based M partition |
| `remove_cols.pq` | `Table.RemoveColumns` — known column set (explicit projection) and unknown column set (EXCEPT fallback) |
| `combine_tables.pq` | `Table.Combine` — same-`let` bindings; cross-file references (valid and unresolved) |
| `untranslatable.pq` | Unknown function → all three `--on-error` modes |
| `stdin_query` | (tested via CLI process spawn with `--query-name`) |

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

[dev-dependencies]
insta         = { version = "1", features = ["yaml"] }  # snapshot testing (test-only)
assert_cmd    = "2"          # CLI integration tests
predicates    = "3"
criterion     = { version = "0.5", features = ["html_reports"] }  # benchmarks (NFR §1.4)
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

1. `src/parser/` — lexer + grammar + AST (cover all v1 M constructs, including section syntax).
2. `src/resolver/source.rs` — data source inference.
3. `src/dialect/` — `Dialect` trait + all five implementations.
4. `src/translator/expr.rs` — M expression → SQL expression (no table ops yet).
5. `src/translator/functions.rs` — one function at a time, test fixture per function.
6. `src/translator/cte.rs` — `let … in` → CTE chain.
7. `src/translator/cast.rs` — type mapping table.
8. `src/emitter/sql_writer.rs` — SQL AST pretty-printer.
9. `src/pipeline.rs` — wire all stages together.
10. `src/cli.rs` + `src/main.rs` — argument parsing, file I/O, stdin handling (including all flags from §3.4).
11. Integration tests + snapshot baselines.
12. TMDL parser (builds on top of the M parser; extracts `source = ``` … ``` ` blocks).

---

## 15. Acceptance Criteria

- [ ] All 22 test fixtures translate without errors for all 5 dialects.
- [ ] All snapshot tests pass (`cargo insta test`).
- [ ] `--on-error fail` processes all files, then exits with code 1; no partial `.sql` written for errored files.
- [ ] `--on-error comment` exits with code 0 and embeds `/* UNTRANSLATABLE: … */`.
- [ ] `--on-error warn` exits with code 0 and prints `WARN` lines to stderr.
- [ ] `--log-json` emits all diagnostics as newline-delimited JSON to stderr.
- [ ] `--query-name` correctly names stdin queries in both file output and `--stdout` separator banners.
- [ ] `--inline-singles` inlines single-reference CTEs and snapshot output differs from default.
- [ ] Section-syntax `.pq` files produce one `.sql` per `shared` binding.
- [ ] `Table.RenameColumns` produces correct column aliases for all 5 dialects.
- [ ] `Table.ExpandTableColumn` is correctly absorbed into the preceding `NestedJoin` CTE.
- [ ] `Table.RemoveColumns` emits `SELECT * EXCEPT` for `bigquery`/`duckdb` and explicit column projection for `tsql`/`postgres`/`snowflake` when the column set is known; falls back to `EXCEPT` with a warning when the column set is unknown.
- [ ] `Table.Combine` emits correct `UNION ALL` for bindings within the same `let` expression.
- [ ] `Table.Combine` resolves cross-file table references against CLI input file stems and emits `UNRESOLVED_COMBINE_TABLE` errors/warnings for any reference that matches neither a binding nor an input file stem.
- [ ] `Table.Combine` on stdin input (no file arguments) validates members against `let` bindings only; unmatched identifiers are flagged as unresolved.
- [ ] Indentation-based TMDL `source =` blocks are parsed correctly alongside backtick-delimited blocks.
- [ ] Determinism test passes (100 runs, byte-identical output).
- [ ] `cargo clippy -- -D warnings` produces zero warnings.
- [ ] `cargo test` passes on Linux, macOS, and Windows.
- [ ] Binary size ≤ 10 MB release build.
- [ ] `--help` output documents all flags with types and defaults.

# m2sql — M Code to SQL CLI Translator

A deterministic, fully-offline command-line tool that translates
[Power Query M](https://learn.microsoft.com/en-us/powerquery-m/) code into
equivalent SQL queries. It reads `.pq`/`.m` files, `.tmdl` files from
Power BI `.pbip` projects, or raw M code from stdin, and emits one `.sql` file
per logical query — rendering multi-step `let … in` pipelines as CTEs
(`WITH` clauses).

## Supported SQL Dialects

| Flag value   | Target                        |
|--------------|-------------------------------|
| `tsql`       | Microsoft T-SQL (SQL Server)  |
| `postgres`   | PostgreSQL                    |
| `bigquery`   | Google BigQuery               |
| `snowflake`  | Snowflake                     |
| `duckdb`     | DuckDB                        |

Each dialect uses the correct identifier quoting, type mappings, and
`CAST` syntax for its target platform.

## Translated M Constructs (v1)

| M Function / Pattern                | SQL Equivalent                       |
|--------------------------------------|--------------------------------------|
| `let … in`                           | CTE chain (`WITH … AS (…)`)         |
| `Table.SelectRows`                   | `WHERE`                              |
| `Table.SelectColumns`                | `SELECT col1, col2`                  |
| `Table.RenameColumns`                | `SELECT old AS new`                  |
| `Table.Join` / `Table.NestedJoin`    | `INNER/LEFT/RIGHT/FULL OUTER JOIN`   |
| `Table.ExpandTableColumn`            | Column projections from JOINs        |
| `Table.Group`                        | `GROUP BY` with aggregates           |
| `Table.AddColumn`                    | Computed column in `SELECT`          |
| `Table.TransformColumnTypes`         | `CAST(col AS type)`                  |
| `Text.StartsWith/EndsWith/Contains`  | `LIKE` / `ILIKE`                     |
| `List.Contains`                      | `IN (…)`                             |
| Anti-joins (`JoinKind.LeftAnti`)      | `LEFT JOIN … WHERE key IS NULL`      |
| Composite join keys                  | Multi-column `ON` clause             |
| Section syntax (`section … shared`)  | Multiple output files                |
| TMDL files                           | Extracted M expressions              |

Expressions outside the v1 scope are handled according to the `--on-error`
mode (see below).

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) stable toolchain (1.82+)

### Build from Source

```bash
git clone https://github.com/Lex-90/MtoSQL.git
cd MtoSQL
cargo build --release
```

The binary is produced at `target/release/m2sql`.

### Install via Cargo

```bash
cargo install --path .
```

This places `m2sql` on your `$PATH`.

## Usage

```
m2sql [OPTIONS] --dialect <DIALECT> [INPUT...]
```

### Options

| Flag                  | Short | Default      | Description                                                    |
|-----------------------|-------|--------------|----------------------------------------------------------------|
| `--dialect`           | `-d`  | *(required)* | Target SQL dialect: `tsql`, `postgres`, `bigquery`, `snowflake`, `duckdb` |
| `--output-dir`        | `-o`  | `./output`   | Directory where `.sql` files are written (created if absent)   |
| `--on-error`          | `-e`  | `warn`       | Behaviour on untranslatable expressions: `fail`, `comment`, `warn` |
| `--stdout`            |       | off          | Print SQL to stdout instead of writing files                   |
| `--query-name`        | `-n`  | `query`      | Name for the query when reading from stdin                     |
| `--inline-singles`    |       | off          | Inline CTE bindings referenced exactly once                    |
| `--log-json`          |       | off          | Emit diagnostics as newline-delimited JSON to stderr           |
| `--no-color`          |       | off          | Disable ANSI colour in terminal output                         |

### Examples

**Translate a single file to T-SQL:**

```bash
m2sql -d tsql orders.pq
# Writes ./output/orders.sql
```

**Translate multiple files to PostgreSQL, output to a custom directory:**

```bash
m2sql -d postgres -o ./sql_out *.pq
```

**Pipe from stdin:**

```bash
echo 'let
  Source = Sql.Database("srv", "db"),
  T = Source{[Name="Orders"]}[Data],
  Filtered = Table.SelectRows(T, each [Amount] > 100)
in Filtered' | m2sql -d tsql --stdout
```

Output:

```sql
WITH T AS (
    SELECT *
    FROM db.Orders
),
Filtered AS (
    SELECT *
    FROM T
    WHERE Amount > 100
)
SELECT *
FROM Filtered;
```

**Translate a Power BI .tmdl file:**

```bash
m2sql -d bigquery model.tmdl
```

**Strict mode (exit code 1 on untranslatable expressions):**

```bash
m2sql -d tsql --on-error fail queries/*.pq
```

**JSON diagnostics for CI integration:**

```bash
m2sql -d postgres --log-json --on-error warn queries/*.pq 2>diagnostics.jsonl
```

### Exit Codes

| Code | Meaning                                                |
|------|--------------------------------------------------------|
| `0`  | All queries translated successfully                    |
| `1`  | Translation errors occurred (only with `--on-error fail`) |
| `2`  | CLI argument error or I/O failure                      |

### Error Handling Modes

| Mode      | Behaviour                                                            |
|-----------|----------------------------------------------------------------------|
| `fail`    | Collect all errors, write only error-free files, exit with code 1    |
| `comment` | Replace untranslatable expressions with `/* UNTRANSLATABLE: … */`    |
| `warn`    | Same as `comment`, plus print warnings to stderr (default)           |

## Security

- **Credential redaction**: Connection strings containing `Password=`,
  `AccountKey=`, `SecretKey=`, and similar patterns are automatically
  redacted in SQL output with `/* REDACTED */` comments.
- **Path traversal protection**: Output file paths are sanitised to prevent
  writing outside the output directory.
- **File size guard**: Input files larger than 10 MB are rejected.

## Running Tests

```bash
cargo test
```

The test suite includes 35 integration tests covering all M constructs,
all five dialects, error modes, stdin/stdout, JSON logging, file output,
section syntax, TMDL parsing, and determinism.

## Project Documentation

Detailed specifications are available in the [`documentation/`](documentation/)
directory:

- **[PRD_m2sql.md](documentation/PRD_m2sql.md)** — Product Requirements
  Document. Defines the CLI interface, M-to-SQL translation rules for every
  supported function, dialect type mappings, CTE generation strategy, error
  handling semantics, and test fixtures.

- **[NFR_m2sql.md](documentation/NFR_m2sql.md)** — Non-Functional Requirements.
  Covers performance targets, determinism guarantees, memory limits, security
  (credential redaction, path traversal), diagnostics format, build
  reproducibility, and the traceability matrix.

## Architecture

```
src/
  main.rs              CLI entry point
  cli.rs               clap argument definitions
  pipeline.rs          parse → resolve → translate → emit orchestration
  parser/
    lexer.rs           M tokeniser
    grammar.rs         Recursive-descent parser
    ast.rs             AST node types
  resolver/
    source.rs          Data source inference (Sql.Database, table references)
  translator/
    context.rs         Translation state, diagnostics, error modes
    cte.rs             let…in → CTE chain builder
    expr.rs            M expression → SQL expression
    functions.rs       Per-function table operation translators
    cast.rs            M type → SQL type coercion
  dialect/
    mod.rs             Dialect trait
    tsql.rs            T-SQL dialect
    postgres.rs        PostgreSQL dialect
    bigquery.rs        BigQuery dialect
    snowflake.rs       Snowflake dialect
    duckdb.rs          DuckDB dialect
  emitter/
    sql_writer.rs      SQL AST → formatted SQL string
  error.rs             Diagnostics, credential redaction
```

## License

See repository for license details.

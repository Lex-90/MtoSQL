#[allow(deprecated)]
use assert_cmd::Command;
use predicates::prelude::*;

#[allow(deprecated)]
fn m2sql() -> Command {
    Command::cargo_bin("m2sql").unwrap()
}

// ── Basic translation tests ────────────────────────────────────────

#[test]
fn test_version() {
    m2sql()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("m2sql"));
}

#[test]
fn test_help() {
    m2sql()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--dialect"))
        .stdout(predicate::str::contains("--output-dir"))
        .stdout(predicate::str::contains("--on-error"))
        .stdout(predicate::str::contains("--stdout"))
        .stdout(predicate::str::contains("--query-name"))
        .stdout(predicate::str::contains("--inline-singles"))
        .stdout(predicate::str::contains("--log-json"))
        .stdout(predicate::str::contains("--no-color"));
}

#[test]
fn test_bad_dialect() {
    m2sql()
        .args(["--dialect", "oracle", "--stdout"])
        .write_stdin("let x = 1 in x")
        .assert()
        .code(2);
}

// ── Fixture-based tests per dialect ────────────────────────────────

fn test_fixture(fixture: &str, dialect: &str, expected_contains: &[&str]) {
    let mut cmd = m2sql();
    cmd.args([
        "--dialect",
        dialect,
        "--stdout",
        &format!("tests/fixtures/{}", fixture),
    ]);
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in expected_contains {
        assert!(
            stdout.contains(expected),
            "Expected output to contain '{}' for fixture {} dialect {}, got:\n{}",
            expected,
            fixture,
            dialect,
            stdout
        );
    }
}

#[test]
fn test_select_rows_simple_tsql() {
    test_fixture(
        "select_rows_simple.pq",
        "tsql",
        &[
            "WHERE Amount > 100",
            "SELECT OrderID, Amount",
            "FROM AdventureWorks.Orders",
        ],
    );
}

#[test]
fn test_select_rows_simple_postgres() {
    test_fixture(
        "select_rows_simple.pq",
        "postgres",
        &["WHERE Amount > 100", "SELECT OrderID, Amount"],
    );
}

#[test]
fn test_select_rows_text_tsql() {
    test_fixture(
        "select_rows_text.pq",
        "tsql",
        &["LIKE 'A%'", "LIKE '%z'", "LIKE '%special%'"],
    );
}

#[test]
fn test_select_rows_list() {
    test_fixture("select_rows_list.pq", "tsql", &["IN ('Active', 'Pending')"]);
}

#[test]
fn test_select_cols() {
    test_fixture(
        "select_cols.pq",
        "tsql",
        &["OrderID", "CustomerID", "Amount", "OrderDate"],
    );
}

#[test]
fn test_rename_cols() {
    test_fixture(
        "rename_cols.pq",
        "tsql",
        &["CustomerNo AS CustomerID", "Amt AS Amount"],
    );
}

#[test]
fn test_join_inner() {
    test_fixture(
        "join_inner.pq",
        "tsql",
        &["INNER JOIN", "ON Orders.CustomerID = Customers.ID"],
    );
}

#[test]
fn test_join_left() {
    test_fixture("join_left.pq", "tsql", &["LEFT JOIN"]);
}

#[test]
fn test_join_anti() {
    test_fixture("join_anti.pq", "tsql", &["LEFT JOIN", "IS NULL"]);
}

#[test]
fn test_join_composite() {
    test_fixture(
        "join_composite.pq",
        "tsql",
        &[
            "Sales.ProductID = Products.ID",
            "Sales.RegionID = Products.Region",
        ],
    );
}

#[test]
fn test_nested_join() {
    test_fixture(
        "nested_join.pq",
        "tsql",
        &[
            "LEFT JOIN Customers ON Orders.CustomerID = Customers.ID",
            "Customers.Name AS CustName",
            "Customers.Email AS CustEmail",
            "Orders.*",
        ],
    );
}

#[test]
fn test_group_basic() {
    test_fixture(
        "group_basic.pq",
        "tsql",
        &[
            "GROUP BY CustomerID",
            "SUM(Amount) AS TotalAmount",
            "COUNT(*) AS OrderCount",
        ],
    );
}

#[test]
fn test_group_multi() {
    test_fixture(
        "group_multi.pq",
        "tsql",
        &[
            "GROUP BY Region, Category",
            "SUM(Amount) AS TotalSales",
            "AVG(Price) AS AvgPrice",
            "COUNT(DISTINCT ProductID) AS UniqueProducts",
        ],
    );
}

#[test]
fn test_add_column() {
    test_fixture(
        "add_column.pq",
        "tsql",
        &["AS Total", "Quantity * UnitPrice"],
    );
}

#[test]
fn test_cast_types_tsql() {
    test_fixture(
        "cast_types.pq",
        "tsql",
        &[
            "CAST(OrderID AS BIGINT)",
            "CAST(Amount AS FLOAT)",
            "CAST(OrderDate AS DATE)",
            "CAST(IsActive AS BIT)",
            "CAST(Notes AS NVARCHAR(MAX))",
        ],
    );
}

#[test]
fn test_cast_types_postgres() {
    test_fixture(
        "cast_types.pq",
        "postgres",
        &[
            "OrderID::BIGINT",
            "Amount::DOUBLE PRECISION",
            "OrderDate::DATE",
            "IsActive::BOOLEAN",
            "Notes::TEXT",
        ],
    );
}

#[test]
fn test_cast_types_bigquery() {
    test_fixture(
        "cast_types.pq",
        "bigquery",
        &[
            "CAST(OrderID AS INT64)",
            "CAST(Amount AS FLOAT64)",
            "CAST(IsActive AS BOOL)",
            "CAST(Notes AS STRING)",
        ],
    );
}

#[test]
fn test_multi_step() {
    test_fixture(
        "multi_step.pq",
        "tsql",
        &[
            "WITH RawOrders",
            "FROM AdventureWorks.Orders",
            "WHERE Amount > 100",
            "SELECT OrderID, Amount",
        ],
    );
}

#[test]
fn test_section_syntax() {
    let mut cmd = m2sql();
    cmd.args([
        "--dialect",
        "tsql",
        "--stdout",
        "tests/fixtures/section_syntax.pq",
    ]);
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("-- [query: Orders]"),
        "Should contain Orders query"
    );
    assert!(
        stdout.contains("-- [query: Customers]"),
        "Should contain Customers query"
    );
    assert!(
        stdout.contains("FROM db.Orders"),
        "Should reference Orders table"
    );
    assert!(
        stdout.contains("FROM db.Customers"),
        "Should reference Customers table"
    );
}

#[test]
fn test_tmdl_table() {
    test_fixture(
        "tmdl_table.tmdl",
        "tsql",
        &["-- [query: Sales]", "FROM db.Sales"],
    );
}

#[test]
fn test_tmdl_indent_based() {
    test_fixture(
        "tmdl_indent.tmdl",
        "tsql",
        &["-- [query: D_Agenti]", "dbo.V_PBI_ANAG_AGENTI"],
    );
}

// ── Error handling tests ───────────────────────────────────────────

#[test]
fn test_on_error_warn() {
    let mut cmd = m2sql();
    cmd.args([
        "--dialect",
        "tsql",
        "--stdout",
        "--on-error",
        "warn",
        "tests/fixtures/untranslatable.pq",
    ]);
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("UNTRANSLATABLE"),
        "SQL should contain UNTRANSLATABLE comment"
    );
    assert!(stderr.contains("WARN"), "stderr should contain warning");
}

#[test]
fn test_on_error_comment() {
    let mut cmd = m2sql();
    cmd.args([
        "--dialect",
        "tsql",
        "--stdout",
        "--on-error",
        "comment",
        "tests/fixtures/untranslatable.pq",
    ]);
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("UNTRANSLATABLE"),
        "SQL should contain UNTRANSLATABLE comment"
    );
}

#[test]
fn test_on_error_fail() {
    m2sql()
        .args([
            "--dialect",
            "tsql",
            "--stdout",
            "--on-error",
            "fail",
            "tests/fixtures/untranslatable.pq",
        ])
        .assert()
        .code(1);
}

// ── Stdin tests ────────────────────────────────────────────────────

#[test]
fn test_stdin_query() {
    m2sql()
        .args(["--dialect", "tsql", "--stdout", "--query-name", "MyQuery"])
        .write_stdin(
            "let Source = Sql.Database(\"srv\", \"db\"), T = Source{[Name=\"Orders\"]}[Data] in T",
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("-- [query: MyQuery]"))
        .stdout(predicate::str::contains("FROM db.Orders"));
}

#[test]
fn test_stdin_default_query_name() {
    m2sql()
        .args(["--dialect", "tsql", "--stdout"])
        .write_stdin(
            "let Source = Sql.Database(\"srv\", \"db\"), T = Source{[Name=\"Orders\"]}[Data] in T",
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("-- [query: query]"));
}

// ── JSON log tests ─────────────────────────────────────────────────

#[test]
fn test_log_json() {
    let mut cmd = m2sql();
    cmd.args([
        "--dialect",
        "tsql",
        "--stdout",
        "--log-json",
        "tests/fixtures/untranslatable.pq",
    ]);
    let assert = cmd.assert();
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("\"level\""),
        "JSON log should contain level field"
    );
    assert!(
        stderr.contains("\"code\""),
        "JSON log should contain code field"
    );
    assert!(
        stderr.contains("SUMMARY"),
        "JSON log should contain summary"
    );
}

// ── All dialects for key fixtures ──────────────────────────────────

#[test]
fn test_all_dialects_select_rows() {
    for dialect in &["tsql", "postgres", "bigquery", "snowflake", "duckdb"] {
        test_fixture("select_rows_simple.pq", dialect, &["WHERE Amount > 100"]);
    }
}

#[test]
fn test_all_dialects_join() {
    for dialect in &["tsql", "postgres", "bigquery", "snowflake", "duckdb"] {
        test_fixture("join_inner.pq", dialect, &["INNER JOIN"]);
    }
}

#[test]
fn test_all_dialects_group() {
    for dialect in &["tsql", "postgres", "bigquery", "snowflake", "duckdb"] {
        test_fixture("group_basic.pq", dialect, &["GROUP BY", "SUM", "COUNT"]);
    }
}

// ── Determinism test ───────────────────────────────────────────────

#[test]
fn test_determinism() {
    let mut outputs = Vec::new();
    for _ in 0..10 {
        let output = m2sql()
            .args([
                "--dialect",
                "tsql",
                "--stdout",
                "tests/fixtures/multi_step.pq",
            ])
            .output()
            .unwrap();
        outputs.push(String::from_utf8_lossy(&output.stdout).to_string());
    }
    for i in 1..outputs.len() {
        assert_eq!(
            outputs[0], outputs[i],
            "Output should be deterministic across runs"
        );
    }
}

// ── Output file writing ────────────────────────────────────────────

#[test]
fn test_output_files() {
    let dir = tempfile::tempdir().unwrap();
    m2sql()
        .args([
            "--dialect",
            "tsql",
            "--output-dir",
            dir.path().to_str().unwrap(),
            "tests/fixtures/select_rows_simple.pq",
        ])
        .assert()
        .success();
    assert!(
        dir.path().join("select_rows_simple.sql").exists(),
        "Output file should be created"
    );
}

#[test]
fn test_section_output_files() {
    let dir = tempfile::tempdir().unwrap();
    m2sql()
        .args([
            "--dialect",
            "tsql",
            "--output-dir",
            dir.path().to_str().unwrap(),
            "tests/fixtures/section_syntax.pq",
        ])
        .assert()
        .success();
    assert!(
        dir.path().join("Orders.sql").exists(),
        "Orders.sql should be created"
    );
    assert!(
        dir.path().join("Customers.sql").exists(),
        "Customers.sql should be created"
    );
}

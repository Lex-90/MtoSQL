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

// ── Table.RemoveColumns tests ──────────────────────────────────────

#[test]
fn test_remove_cols_bigquery() {
    test_fixture(
        "remove_cols.pq",
        "bigquery",
        &["* EXCEPT (InternalCode, AuditTimestamp)"],
    );
}

#[test]
fn test_remove_cols_duckdb() {
    test_fixture(
        "remove_cols.pq",
        "duckdb",
        &["* EXCEPT (InternalCode, AuditTimestamp)"],
    );
}

#[test]
fn test_remove_cols_tsql_fallback() {
    // T-SQL doesn't support EXCEPT and column list is unknown, so UNTRANSLATABLE
    test_fixture("remove_cols.pq", "tsql", &["UNTRANSLATABLE"]);
}

#[test]
fn test_remove_cols_postgres_fallback() {
    // PostgreSQL doesn't support EXCEPT and column list is unknown, so UNTRANSLATABLE
    test_fixture("remove_cols.pq", "postgres", &["UNTRANSLATABLE"]);
}

#[test]
fn test_remove_cols_all_dialects() {
    for dialect in &["bigquery", "duckdb"] {
        test_fixture("remove_cols.pq", dialect, &["EXCEPT"]);
    }
}

// ── Table.Combine tests ────────────────────────────────────────────

#[test]
fn test_combine_tables_tsql() {
    test_fixture(
        "combine_tables.pq",
        "tsql",
        &["UNION ALL", "FROM Orders", "FROM Returns"],
    );
}

#[test]
fn test_combine_tables_all_dialects() {
    for dialect in &["tsql", "postgres", "bigquery", "snowflake", "duckdb"] {
        test_fixture("combine_tables.pq", dialect, &["UNION ALL"]);
    }
}

#[test]
fn test_combine_tables_stdin_same_let() {
    // Table.Combine with same-let bindings should work from stdin
    m2sql()
        .args(["--dialect", "tsql", "--stdout"])
        .write_stdin(
            r#"let Source = Sql.Database("srv", "db"), A = Source{[Name="A"]}[Data], B = Source{[Name="B"]}[Data], Combined = Table.Combine({A, B}) in Combined"#,
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("UNION ALL"));
}

#[test]
fn test_combine_tables_unresolved_warn() {
    // Table.Combine with an unresolved identifier should emit a warning
    let mut cmd = m2sql();
    cmd.args(["--dialect", "tsql", "--stdout", "--on-error", "warn"])
        .write_stdin(
            r#"let Source = Sql.Database("srv", "db"), Orders = Source{[Name="Orders"]}[Data], Combined = Table.Combine({Orders, ExternalFeed}) in Combined"#,
        );
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("is not a binding or a known input file stem"),
        "Should warn about unresolved table: {}",
        stderr
    );
}

#[test]
fn test_combine_tables_unresolved_fail() {
    // Table.Combine with an unresolved identifier under --on-error fail should exit 1
    m2sql()
        .args(["--dialect", "tsql", "--stdout", "--on-error", "fail"])
        .write_stdin(
            r#"let Source = Sql.Database("srv", "db"), Orders = Source{[Name="Orders"]}[Data], Combined = Table.Combine({Orders, ExternalFeed}) in Combined"#,
        )
        .assert()
        .code(1);
}

// ── Table.ReplaceErrorValues tests ──────────────────────────────────

#[test]
fn test_replace_error_values_tsql() {
    test_fixture(
        "replace_error_values.pq",
        "tsql",
        &["COALESCE", "TRY_CAST", "AS Amount", "AS Qty"],
    );
}

#[test]
fn test_replace_error_values_bigquery() {
    test_fixture(
        "replace_error_values.pq",
        "bigquery",
        &["COALESCE", "SAFE_CAST", "AS Amount", "AS Qty"],
    );
}

#[test]
fn test_replace_error_values_postgres() {
    let mut cmd = m2sql();
    cmd.args([
        "--dialect",
        "postgres",
        "--stdout",
        "tests/fixtures/replace_error_values.pq",
    ]);
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("COALESCE"),
        "Postgres should use COALESCE: {}",
        stdout
    );
    assert!(
        !stdout.contains("TRY_CAST"),
        "Postgres should NOT use TRY_CAST: {}",
        stdout
    );
    assert!(
        stderr.contains("postgres has no TRY_CAST"),
        "Postgres should emit approximate warning: {}",
        stderr
    );
}

#[test]
fn test_replace_error_values_snowflake() {
    test_fixture(
        "replace_error_values.pq",
        "snowflake",
        &["COALESCE", "TRY_CAST", "AS Amount"],
    );
}

#[test]
fn test_replace_error_values_duckdb() {
    test_fixture(
        "replace_error_values.pq",
        "duckdb",
        &["COALESCE", "TRY_CAST", "AS Amount"],
    );
}

#[test]
fn test_replace_error_values_no_coercion_context() {
    // ReplaceErrorValues without a preceding TransformColumnTypes should use CASE WHEN
    let mut cmd = m2sql();
    cmd.args(["--dialect", "tsql", "--stdout"])
        .write_stdin(
            r#"let Source = Sql.Database("srv", "db"), Raw = Source{[Name="Sales"]}[Data], Safe = Table.ReplaceErrorValues(Raw, {{"Amount", 0}}) in Safe"#,
        );
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("CASE WHEN"),
        "Should use CASE WHEN without coercion context: {}",
        stdout
    );
    assert!(
        stderr.contains("no preceding coercion context"),
        "Should emit approximate warning: {}",
        stderr
    );
}

#[test]
fn test_replace_error_values_non_literal() {
    // Non-literal replacement should emit UNTRANSLATABLE
    let mut cmd = m2sql();
    cmd.args(["--dialect", "tsql", "--stdout"])
        .write_stdin(
            r#"let Source = Sql.Database("srv", "db"), Raw = Source{[Name="Sales"]}[Data], Safe = Table.ReplaceErrorValues(Raw, {{"Amount", SomeFunc()}}) in Safe"#,
        );
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("UNTRANSLATABLE"),
        "Non-literal replacement should be untranslatable: {}",
        stdout
    );
}

#[test]
fn test_replace_error_values_all_dialects() {
    for dialect in &["tsql", "postgres", "bigquery", "snowflake", "duckdb"] {
        test_fixture(
            "replace_error_values.pq",
            dialect,
            &["COALESCE", "AS Amount"],
        );
    }
}

// ── Power Query parameter detection tests ───────────────────────────

#[test]
fn test_pq_parameters_tsql() {
    let mut cmd = m2sql();
    cmd.args([
        "--dialect",
        "tsql",
        "--stdout",
        "tests/fixtures/pq_parameters.pq",
    ]);
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Parameter header block should be present
    assert!(
        stdout.contains("POWER QUERY PARAMETERS DETECTED"),
        "Should contain parameter header: {}",
        stdout
    );

    // Parameter placeholders should be in the SQL body for expression-context params
    assert!(
        stdout.contains("/* PARAM: flag_attivi */"),
        "Should contain flag_attivi param in SQL body: {}",
        stdout
    );

    // Data source params (sql_server, sql_db) should be listed in the header
    assert!(
        stdout.contains("sql_server"),
        "Should list sql_server in parameter header: {}",
        stdout
    );
    assert!(
        stdout.contains("sql_db"),
        "Should list sql_db in parameter header: {}",
        stdout
    );

    // PARAM_REFERENCE warnings should be emitted
    assert!(
        stderr.contains("Parameter reference:"),
        "Should emit PARAM_REFERENCE warnings: {}",
        stderr
    );
    assert!(
        stderr.contains("sql_server"),
        "Should mention sql_server in warnings: {}",
        stderr
    );

    // Parameters detected count in summary
    assert!(
        stderr.contains("parameters detected"),
        "Summary should mention parameters detected: {}",
        stderr
    );
}

#[test]
fn test_pq_parameters_all_dialects() {
    for dialect in &["tsql", "postgres", "bigquery", "snowflake", "duckdb"] {
        let mut cmd = m2sql();
        cmd.args([
            "--dialect",
            dialect,
            "--stdout",
            "tests/fixtures/pq_parameters.pq",
        ]);
        let assert = cmd.assert().success();
        let output = assert.get_output();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("POWER QUERY PARAMETERS DETECTED"),
            "Should contain parameter header for {}: {}",
            dialect,
            stdout
        );
        assert!(
            stdout.contains("sql_server"),
            "Should list sql_server in header for {}: {}",
            dialect,
            stdout
        );
        assert!(
            stdout.contains("/* PARAM: flag_attivi */"),
            "Should contain flag_attivi param in SQL body for {}: {}",
            dialect,
            stdout
        );
    }
}

#[test]
fn test_pq_parameters_non_suppressible() {
    // PARAM_REFERENCE warnings should be emitted even with --on-error comment
    for on_error_mode in &["comment", "warn", "fail"] {
        let mut cmd = m2sql();
        cmd.args([
            "--dialect",
            "tsql",
            "--stdout",
            "--on-error",
            on_error_mode,
            "tests/fixtures/pq_parameters.pq",
        ]);
        let output = cmd.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Parameter reference:"),
            "PARAM_REFERENCE warning should be emitted with --on-error {} mode: {}",
            on_error_mode,
            stderr
        );
    }
}

#[test]
fn test_pq_parameters_no_false_positives() {
    // Let bindings, M keywords, and function names should not be flagged as parameters
    let mut cmd = m2sql();
    cmd.args(["--dialect", "tsql", "--stdout"])
        .write_stdin(
            r#"let Source = Sql.Database("srv", "db"), Orders = Source{[Name="Orders"]}[Data], Filtered = Table.SelectRows(Orders, each [Status] = "Active") in Filtered"#,
        );
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains("POWER QUERY PARAMETERS DETECTED"),
        "Should not have parameter header for normal query: {}",
        stdout
    );
    assert!(
        !stderr.contains("PARAM_REFERENCE"),
        "Should not emit PARAM_REFERENCE for normal query: {}",
        stderr
    );
}

#[test]
fn test_pq_parameters_json_summary() {
    let mut cmd = m2sql();
    cmd.args([
        "--dialect",
        "tsql",
        "--stdout",
        "--log-json",
        "tests/fixtures/pq_parameters.pq",
    ]);
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("\"params_detected\""),
        "JSON summary should include params_detected: {}",
        stderr
    );
    assert!(
        stderr.contains("PARAM_REFERENCE"),
        "JSON should include PARAM_REFERENCE entries: {}",
        stderr
    );
}

#[test]
fn test_pq_parameters_header_sorted() {
    let mut cmd = m2sql();
    cmd.args([
        "--dialect",
        "tsql",
        "--stdout",
        "tests/fixtures/pq_parameters.pq",
    ]);
    let assert = cmd.assert().success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parameters should be listed in lexicographic order in the header
    let flag_pos = stdout.find("flag_attivi").unwrap_or(usize::MAX);
    let db_pos = stdout.find("sql_db").unwrap_or(usize::MAX);
    let server_pos = stdout.find("sql_server").unwrap_or(usize::MAX);
    assert!(
        flag_pos < db_pos && db_pos < server_pos,
        "Parameters should be in lexicographic order: flag_attivi < sql_db < sql_server\n{}",
        stdout
    );
}

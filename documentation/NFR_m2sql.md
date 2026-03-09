# NFR: `m2sql` — Non-Functional Requirements

**Version:** 1.5.0
**Status:** Ready for implementation
**Companion doc:** `PRD_m2sql.md`
**Runtime:** Rust (stable toolchain, MSRV 1.75)

> **Changelog from v1.4.0**
> - §2.7: New — Power Query parameter non-suppressibility and run-summary extension requirements.
> - §8.2: Extended run-summary format to include `parameters detected` count.
> - Appendix A: Added `NFR-FEAT-07` traceability entry for Power Query parameter detection.
>
> **Changelog from v1.3.0**
> - §2.6: New — Dialect-specific error-trapping fidelity requirement for `Table.ReplaceErrorValues`: `postgres` approximate-translation warning is a required diagnostic, not optional.
> - Appendix A: Added `NFR-FEAT-06` traceability entry for `Table.ReplaceErrorValues`.
>
> **Changelog from v1.2.0**
> - §2.5: New — Cross-file reference validation for `Table.Combine`: resolver must hold input file stems until all files are parsed before running the combine-table resolution pass.
> - Appendix A: Added `NFR-FEAT-04` (`Table.RemoveColumns`) and `NFR-FEAT-05` (`Table.Combine`) traceability entries.
>
> **Changelog from v1.1.0**
> - §2.2: Added note on TMDL indentation-based source block parsing fidelity.
> - Appendix A: Added `NFR-FEAT-03` traceability entry for indentation-based TMDL parsing.
>
> **Changelog from v1.0.0 → v1.1.0**
> - §2.3: Corrected `--on-error fail` error-isolation semantics to match PRD §7 (report-and-exit after all files, not halt-on-first-error).
> - §7.1: Corrected `rustflags` explanation; clarified actual sources of build reproducibility; added `rust-toolchain.toml` requirement.
> - §8.2: Fixed incorrect TTY check reference (`stdout` → `stderr`).
> - Appendix A: Added traceability entries for `--log-json`, `--query-name`, `--inline-singles`, `Table.RenameColumns`, `Table.ExpandTableColumn`.

---

## 1. Performance

### 1.1 Throughput targets

Measured on the reference hardware profile: a 4-core x86-64 machine with 8 GB RAM and an SSD, running a small workload (≤50 files, ≤500 lines each).

| Scenario | Target |
|---|---|
| Single file, ≤500 lines | ≤ 50 ms wall time (cold binary, warm FS) |
| 50 files, each ≤500 lines | ≤ 1 s wall time |
| Startup overhead (empty run, `--version`) | ≤ 10 ms |

These are upper bounds. The tool must not regress past them in CI benchmarks.

### 1.2 Memory

| Scenario | Target |
|---|---|
| Single file, ≤500 lines | ≤ 32 MB RSS |
| 50 files | ≤ 128 MB RSS |

Files are parsed and translated sequentially; no full corpus is held in memory simultaneously. Each file's AST is dropped before the next file is read.

### 1.3 Parallelism

v1 processes files sequentially. Parallel processing (via Rayon) is a post-v1 optimisation. Sequential processing is sufficient for the small-scale target workload and keeps the error output deterministically ordered.

### 1.4 Benchmark harness

A `benches/` directory must contain at least one Criterion benchmark covering:
- Parse only (50 × 500-line fixture).
- Parse + translate + emit (50 × 500-line fixture).

Run with `cargo bench`. Benchmark results are committed as `benches/baseline.txt` and compared in CI to detect regressions ≥ 20%.

---

## 2. Reliability & Correctness

### 2.1 Determinism (hard requirement)

Given the same input bytes, environment variables, and CLI flags, the tool must produce byte-for-byte identical output across:
- Multiple runs on the same machine.
- Runs on different machines with the same OS and architecture.
- Runs on different supported OSes (Linux, macOS, Windows).

Implementation rules to guarantee this:
- No use of `HashMap` where insertion order affects output — use `IndexMap` (already listed in `Cargo.toml`).
- No use of `std::time`, `SystemTime`, or any timestamp in output content.
- No randomly generated identifiers.
- CTE names derived solely from M binding names, sanitised deterministically (Unicode NFKC normalisation → replace non-`[a-zA-Z0-9_]` chars with `_` → deduplicate with `_2`, `_3` suffix in order of first appearance).
- File processing order follows the order arguments are supplied on the CLI; glob expansion is sorted lexicographically before processing.

### 2.2 Translation fidelity

The SQL output must be semantically equivalent to the M expression for all constructs in the v1 feature surface (§6 of PRD). "Semantically equivalent" means: executing the SQL against the same data source that the M query targets must return the same rows and columns (modulo row ordering, which M does not guarantee either).

Fidelity is validated through the snapshot test suite. Any change that modifies a snapshot requires explicit human approval via `cargo insta review`.

**TMDL parsing fidelity:** The TMDL parser must correctly extract M source from both backtick-delimited (`` source = ``` … ``` ``) and indentation-based (`source =` followed by deeper-indented lines) block formats. Indentation-based blocks terminate when a line at the same or lesser indentation level as the `source` keyword is encountered. Both formats must produce identical downstream translation results for the same M code.

### 2.3 Error isolation

A translation failure in one file must not prevent other files from being translated. Errors are collected across all files and reported together at the end of the run.

When `--on-error fail` is set: all input files are still processed in full, all translation errors are collected, and the process exits with code 1 after reporting every error. No `.sql` file is written for any file that contained at least one translation error; files that translated successfully are written normally. There is no "halt on first error" mode.

### 2.4 No silent data loss

If a CTE step or column cannot be translated, it must either:
- Cause a visible error/warning, or
- Produce a `/* UNTRANSLATABLE: … */` placeholder comment.

Silently dropping steps or columns is never acceptable.

### 2.5 Cross-file reference validation (`Table.Combine`)

`Table.Combine` introduces a resolver-level constraint that is evaluated **after** all input files have been parsed, because the full set of valid table identifiers is only known once every CLI argument has been processed.

**Requirements:**

- The pipeline must collect the file stem of every input file (e.g. `Customers` from `Customers.pq`) into a shared, immutable set before the `resolve_combine_tables` pass begins.
- The `resolve_combine_tables` pass iterates over every `Table.Combine` call site found during parsing and asserts that each member identifier resolves to either a `let`-binding in the same query or a member of the input-file stem set.
- This two-phase approach (parse all → then validate combines) must not break the `§2.3` error-isolation guarantee: an `UNRESOLVED_COMBINE_TABLE` error in one file does not prevent other files from being translated.
- The input-file stem set must be built from the **original CLI arguments** (or glob expansion), not from output filenames or any mutable state. It is read-only during the resolution pass.
- When reading from stdin, the input-file stem set is empty; only same-`let` bindings are valid combine targets.

**Determinism note:** The set of valid file stems must be sorted lexicographically before the validation pass to ensure identical error ordering regardless of filesystem enumeration order (per `§2.1`).

### 2.6 Dialect-specific error-trapping fidelity (`Table.ReplaceErrorValues`)

`Table.ReplaceErrorValues` cannot be translated with equal fidelity across all five dialects because PostgreSQL has no native `TRY_CAST` equivalent. The following rules are **mandatory**, not advisory:

- For `tsql`, `snowflake`, and `duckdb`: the `TRY_CAST`-based translation must be used whenever a preceding coercion context is present. Falling back to a plain `COALESCE(col, replacement)` when a coercion context is available is a translation fidelity failure.
- For `bigquery`: `SAFE_CAST` must be used; `TRY_CAST` is not valid BigQuery syntax and must never be emitted.
- For `postgres`: the plain `COALESCE(col, replacement)` translation is the **only** permitted output. The translator **must** emit a `WARN`-level diagnostic (`REPLACE_ERROR_APPROXIMATE`) for every affected column in the postgres dialect, regardless of `--on-error` mode. This warning cannot be suppressed — it informs the user that the SQL semantics differ from M semantics (non-null error values will not be caught). This is a named exception to the general rule that `warn` mode warnings can be silenced.
- When no preceding coercion context exists (Case 2 in PRD §6.13), the `CASE WHEN … IS NULL` fallback is permitted for all dialects, but the `WARN` diagnostic is still required.

### 2.7 Power Query parameter detection — non-suppressibility and summary integrity

Power Query parameters produce SQL output that is **never directly executable** — the `/* PARAM: … */` placeholders are syntactically invalid in all five target dialects. The following rules are therefore mandatory:

- The `PARAM_REFERENCE` warning must be emitted for each unique parameter name per file regardless of `--on-error` mode, `--no-color`, or any other CLI flag. There is no flag that suppresses it. This is a named exception to the general rule that warning volume can be controlled by the user.
- The parameter header block (PRD §6.14) must be present in every `.sql` output file that contains at least one `/* PARAM: … */` placeholder. Omitting the header block when parameters are present is a reliability failure.
- **Determinism of the header block:** The parameter list in the header block must be sorted lexicographically and must include the file name and 1-indexed line number of the first use of each parameter. Because the order must be independent of AST traversal order, the `detect_parameters` pass must collect all parameter use sites into an `IndexMap<String, (file, line)>` keyed on parameter name, then sort by key before emitting. This satisfies `§2.1` (determinism).
- **No false positives:** The `detect_parameters` pass must maintain a deny-list of all M built-in identifiers, all `let` bindings in scope (including outer scopes for nested `let`), and all input file stems. Any identifier on this list must never be emitted as a `PARAM_REFERENCE`. False positives (classifying a bound variable as a parameter) are a translation fidelity failure per `§2.2`.
- **Run summary integrity:** The `params_detected` count in the run summary (plain text and JSON) must equal the total number of **unique** parameter names found across **all** input files in the run (not the total number of use sites). If a parameter named `sql_server` appears in three different input files, it contributes 3 to the count (one per file-scope detection), not 1. This matches the semantics of the per-file `PARAM_REFERENCE` warning count.

---

## 3. Security

### 3.1 Credential redaction

M source files may contain inline credentials (e.g. passwords in connection strings). The tool must detect and redact the following patterns before they appear in any output (SQL files, stdout, stderr, JSON logs):

| Pattern | Detection heuristic | Replacement in output |
|---|---|---|
| `Password="..."` / `password="..."` | Case-insensitive key `password` in a record literal | `Password=/* REDACTED */` |
| `AccountKey="..."` | Key name `accountkey` (Azure Blob) | `AccountKey=/* REDACTED */` |
| `AccessKey="..."` / `SecretKey="..."` | Key names `accesskey`, `secretkey` | `[Key]=/* REDACTED */` |
| `Credentials=[...]` record fields | Key name `credentials` | `Credentials=/* REDACTED */` |
| Connection string `pwd=...;` / `password=...;` inside a text literal | Regex on string content | value replaced with `***` |

**Server names and database names are never redacted** — they are infrastructure-level references that belong in SQL output.

When a redaction occurs the tool must:
1. Insert `/* SECURITY: credential redacted by m2sql */` on the line immediately above the affected SQL fragment.
2. Emit a `WARN` (or JSON log entry with `"level": "security"`) to stderr identifying the file, line number, and key name — but never the credential value.

Redaction runs before any other output stage. It is not configurable; it cannot be disabled.

### 3.2 No code execution

The tool must never evaluate M expressions — it operates purely as a text transformer. No use of `eval`, shell expansion, or subprocess spawning on user-supplied content.

### 3.3 Path traversal

When writing output files, the tool must resolve all output paths relative to `--output-dir` and reject any path that would write outside that directory (e.g. via `../../` in a query name inferred from a TMDL table name). Emit an error and skip the file if such a path is detected.

### 3.4 Input size guard

Reject any single input file larger than 10 MB with a clear error message. This prevents accidental processing of binary files (e.g. a misidentified `.pbix` passed as `.pq`).

### 3.5 Supply chain

- No network-capable crates (enforced — see §7.2).
- Dependency audit must pass `cargo deny check` in CI (licences, advisories, bans).
- `Cargo.lock` is committed and pinned. Dependency updates require deliberate PRs.

---

## 4. Maintainability

### 4.1 Code style

- `cargo fmt` (default settings) must produce no diff.
- `cargo clippy -- -D warnings` must produce zero warnings on all three target platforms.
- All public items in `src/` must have doc comments (`///`).

### 4.2 Module boundaries

Each stage of the pipeline (parse, resolve, translate, emit) must compile independently. Cross-stage dependencies must flow strictly in one direction: `parser → resolver → translator → emitter`. No reverse imports.

### 4.3 Adding a new dialect

Adding a new SQL dialect must require changes only to:
- A new file in `src/dialect/`.
- A new arm in the `--dialect` enum in `src/cli.rs`.
- New snapshot files in `tests/snapshots/<dialect>/`.

No changes to the parser, resolver, or translator core logic.

### 4.4 Adding a new M function

Adding support for a new M function must require changes only to:
- `src/translator/functions.rs` (one new match arm or handler function).
- A new test fixture + snapshots.

### 4.5 Test coverage

| Scope | Minimum line coverage |
|---|---|
| `src/parser/` | 90% |
| `src/translator/` | 85% |
| `src/dialect/` | 80% |
| `src/resolver/` | 85% |
| Overall | 80% |

Coverage is measured with `cargo llvm-cov` and enforced in CI. PRs that drop coverage below these thresholds must not be merged.

### 4.6 Changelog

A `CHANGELOG.md` following [Keep a Changelog](https://keepachangelog.com) format must be maintained. Every PR must include a changelog entry.

---

## 5. Usability

### 5.1 Error messages

All error and warning messages must include:
- The source file name (or `<stdin>`).
- The 1-indexed line number within that file.
- A plain-English description of what failed and why.
- The original M fragment that could not be translated (truncated to 120 chars if longer).

Example format (plain text):
```
ERROR [Orders.pq:42] Cannot translate: Table.Pivot
  Source: Table.Pivot(Unpivoted, List.Distinct(Unpivoted[Attribute]), "Attribute", "Value")
  Dialect: tsql
  Reason: Table.Pivot is not in the v1 feature set.
```

### 5.2 JSON log format (`--log-json`)

When `--log-json` is set, every diagnostic line to stderr is a newline-delimited JSON object. Field schema:

```json
{"level":"warn","file":"Sales.pq","line":14,"code":"UNTRANSLATABLE","message":"Table.Pivot is not in the v1 feature set.","fragment":"Table.Pivot(Unpivoted, ...)"}
{"level":"error","file":"Sales.pq","line":22,"code":"UNSUPPORTED_TYPE","message":"type duration is not supported in tsql","fragment":"type duration"}
{"level":"security","file":"Creds.pq","line":3,"code":"CREDENTIAL_REDACTED","message":"key 'password' redacted","fragment":null}
{"level":"info","file":null,"line":null,"code":"SUMMARY","message":"12 translated, 0 errors, 2 warnings","duration_ms":1203}
```

Field stability: all field names are stable within a major version (SemVer). New optional fields may be added in minor releases; consumers must ignore unknown fields.

### 5.3 `--help` quality

- Every flag must be listed with its type, default value, and a one-sentence description.
- Enum values must be enumerated inline (e.g. `[tsql|postgres|bigquery|snowflake|duckdb]`).
- The help text must fit in an 80-column terminal without wrapping mid-sentence.

---

## 6. Compatibility

### 6.1 Platform targets

| OS | Architecture | Toolchain | Tier |
|---|---|---|---|
| Linux | x86-64 | glibc ≥ 2.17 | Tier 1 — must pass all tests |
| Windows | x86-64 | MSVC | Tier 1 — must pass all tests |
| macOS | x86-64 (Intel) | system | Tier 1 — must pass all tests |

Tier 1 means: CI runs the full test suite on this target on every PR. Release binaries are provided.

macOS Apple Silicon (arm64) and Linux musl are Tier 2 (best-effort, not gated in CI for v1).

### 6.2 Filesystem

- Output directory is created with `std::fs::create_dir_all` if absent.
- File names are sanitised for all three supported OSes: replace characters illegal on Windows (`\ / : * ? " < > |`) and POSIX (`/`) with `_`.
- Line endings in SQL output: LF (`\n`) on Linux/macOS; CRLF (`\r\n`) on Windows. Controlled by `std::io::Write` on the target platform's default.
- All file paths are handled as UTF-8. Non-UTF-8 paths emit `IO_ERROR` and are skipped.

### 6.3 Locale independence

The tool must produce identical output regardless of the system locale (`LC_ALL`, `LANG`). Number and string literals from M are always rendered in the invariant locale. No use of locale-sensitive formatting functions.

### 6.4 Timezone independence

No timestamp values are generated in output content. The tool does not read the system clock for any output purpose.

---

## 7. Build & Distribution

### 7.1 Build reproducibility

The release binary must be reproducible: building from the same `Cargo.lock` and the same Rust toolchain version on the same OS must produce a byte-for-byte identical binary.

**Required mechanisms:**

A `rust-toolchain.toml` file must be committed to the repository, pinning the exact toolchain channel and version:
```toml
[toolchain]
channel = "1.75.0"
```

The following profile settings contribute to reproducibility by eliminating non-deterministic metadata and path embeddings:
```toml
# .cargo/config.toml
[build]
rustflags = ["-C", "metadata=m2sql"]
# Sets a fixed crate metadata hash suffix, preventing non-deterministic
# symbol mangling across invocations with differing metadata seeds.
# This alone does not guarantee full reproducibility — the rust-toolchain.toml
# pin and the settings below are equally required.
```

```toml
[profile.release]
strip = true          # Removes debug info, which is the primary source of
                      # build-machine path embeddings in binaries.
opt-level = 3
lto = "thin"
codegen-units = 1     # Eliminates non-determinism from parallel codegen scheduling.
```

> **Note:** `strip = true` removes DWARF debug sections, which eliminates the need for `--remap-path-prefix` path scrubbing in this project. If debug builds are ever distributed, `--remap-path-prefix` must be added to `rustflags`.

### 7.2 Dependency constraints

- No crate may make network calls at runtime (enforced via `cargo deny`).
- No crate may execute subprocesses on user-supplied input.
- Banned crates: `openssl`, `reqwest`, `hyper`, `tokio`, `async-std` (async runtime not needed).
- All dependency licences must be MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, or ISC.

`deny.toml` must enumerate all of the above. `cargo deny check` is a required CI gate.

### 7.3 GitHub Releases

On every semver tag (`v*.*.*`), a GitHub Actions workflow builds and uploads the following artefacts:

| Artefact | Target triple |
|---|---|
| `m2sql-linux-x86_64.tar.gz` | `x86_64-unknown-linux-gnu` |
| `m2sql-windows-x86_64.zip` | `x86_64-pc-windows-msvc` |
| `m2sql-macos-x86_64.tar.gz` | `x86_64-apple-darwin` |

Each archive contains: the binary, `README.md`, `LICENSE` (placeholder if licence not yet decided), and `CHANGELOG.md`.

A `SHA256SUMS` file listing the hash of each archive is published alongside the artefacts.

### 7.4 Homebrew formula

A `homebrew-m2sql` tap repository must be maintained. The formula must:
- Download the `macos-x86_64` archive from the GitHub Release.
- Verify the SHA-256 checksum before installing.
- Install the binary to `$(brew --prefix)/bin/m2sql`.
- Include a `test` block: `system "#{bin}/m2sql", "--version"`.

Formula update is automated: the GitHub Release workflow opens a PR against the tap repository with the new version and checksum.

### 7.5 Binary size

Release binary (stripped) must be ≤ 10 MB on all three Tier 1 targets. Enforced as a CI check using `ls -la` on the built binary after `cargo build --release`.

---

## 8. Observability & Diagnostics

### 8.1 Exit codes (restatement from PRD for completeness)

| Code | Meaning |
|---|---|
| 0 | Success (warnings may have been emitted) |
| 1 | Translation failure with `--on-error fail` |
| 2 | Bad CLI arguments or unreadable input |

### 8.2 Run summary

After processing all files, the tool prints a one-line summary to stderr (or a JSON `info` entry if `--log-json`):

Plain text (no parameters detected):
```
m2sql: 12 translated, 0 errors, 2 warnings  [1.2s]
```

Plain text (with parameters detected):
```
m2sql: 3 translated, 0 errors, 0 warnings, 2 parameters detected  [0.8s]
```

JSON (no parameters):
```json
{"level":"info","file":null,"line":null,"code":"SUMMARY","message":"12 translated, 0 errors, 2 warnings","duration_ms":1203}
```

JSON (with parameters):
```json
{"level":"info","file":null,"line":null,"code":"SUMMARY","message":"3 translated, 0 errors, 0 warnings, 2 parameters detected","params_detected":2,"duration_ms":812}
```

The `params_detected` field is omitted from the JSON object when its value is 0 (to keep the common case compact). The plain-text `parameters detected` segment is likewise omitted when the count is 0.

The summary is suppressed if `--no-color` is set and **stderr** is not a TTY, allowing clean pipe usage. (Note: the summary is written to stderr, so the TTY check is correctly performed on stderr, not stdout.)

### 8.3 Timing

Wall-clock time for the entire run (from first byte of input read to last byte of output written) is included in the summary. Measured with `std::time::Instant` (monotonic). Not included in any SQL output.

---

## 9. Versioning & Stability

### 9.1 Semantic versioning

The project follows [SemVer 2.0](https://semver.org):
- **Patch** (`1.0.x`): bug fixes, snapshot updates, performance improvements. No new M constructs.
- **Minor** (`1.x.0`): new M constructs, new SQL dialects, new CLI flags (all additive).
- **Major** (`x.0.0`): breaking changes to CLI flags, output format, or SQL semantics.

### 9.2 CLI stability promise (from v1.0.0)

- Existing flags are never removed or renamed in a minor release.
- Output SQL for a given input is stable within a major version (i.e. a patch release will not change correct output, only fix incorrect output).
- JSON log field names are stable within a major version.

### 9.3 Snapshot lock

The `tests/snapshots/` directory is the source of truth for output stability. Any PR that modifies snapshot content must be labelled `breaking-output` if it changes previously correct output for a supported construct.

---

## 10. CI Pipeline Requirements

The following gates must all pass before a PR can merge:

| Gate | Command | Fail condition |
|---|---|---|
| Format | `cargo fmt -- --check` | Any formatting diff |
| Lint | `cargo clippy -- -D warnings` | Any warning |
| Tests | `cargo test` | Any test failure |
| Snapshots | `cargo insta test` | Any unapproved snapshot change |
| Coverage | `cargo llvm-cov --fail-under-lines 80` | Coverage below threshold |
| Deny | `cargo deny check` | Licence, advisory, or ban violation |
| Bench regression | `cargo bench` vs baseline | ≥ 20% slowdown |
| Binary size | `cargo build --release && ls -la target/release/m2sql*` | Binary > 10 MB |
| Determinism | Run same input 100×, diff all outputs | Any diff |

CI runs on: `ubuntu-latest`, `windows-latest`, `macos-13` (Intel runner).

---

## Appendix A — NFR Traceability Matrix

| NFR ID | Category | PRD section | Enforced by |
|---|---|---|---|
| NFR-PERF-01 | Performance | §3 (CLI) | Criterion bench + CI gate |
| NFR-PERF-02 | Memory | §3 (CLI) | Manual profiling (v1); Valgrind post-v1 |
| NFR-REL-01 | Determinism | §2 (Goals) | Determinism CI gate (100 runs) |
| NFR-REL-02 | Translation fidelity | §6 (M features) | Snapshot tests |
| NFR-REL-03 | Error isolation | §7 (Error handling) | Integration tests (all files processed before exit) |
| NFR-SEC-01 | Credential redaction | §5.1 (Resolver) | Unit tests + security-tagged snapshots |
| NFR-SEC-02 | No code execution | §2 (Goals) | Code review + clippy |
| NFR-SEC-03 | Path traversal | §3.2 (Output) | Integration test with `../../` query name |
| NFR-SEC-04 | Input size guard | §5 (Input) | Unit test with 11 MB dummy file |
| NFR-SEC-05 | Supply chain | §4 (Deps) | `cargo deny check` CI gate |
| NFR-MAINT-01 | Style | §4 (Project) | `cargo fmt` + `clippy` CI gates |
| NFR-MAINT-02 | Coverage | §11 (Testing) | `cargo llvm-cov` CI gate |
| NFR-USE-01 | Error messages | §7 (Errors) | Integration test output assertions |
| NFR-USE-02 | JSON log format | §3.4 (CLI) / §5.2 (NFR) | Integration tests with `--log-json` |
| NFR-USE-03 | `--query-name` flag | §5.3 (Input) / §8 (Output) | Integration tests: stdin + `--stdout` separator |
| NFR-USE-04 | `--inline-singles` flag | §6.2 (CTE chain) | Snapshot diff vs default output |
| NFR-COMPAT-01 | Platform targets | §3 (CLI) | CI matrix (ubuntu, windows, macos-13) |
| NFR-COMPAT-02 | Locale independence | §2 (Goals) | CI runs with `LC_ALL=C` and `LC_ALL=tr_TR` |
| NFR-DIST-01 | GitHub Releases | §3 (Distribution) | Release workflow |
| NFR-DIST-02 | Homebrew formula | §3 (Distribution) | Formula `test` block in CI |
| NFR-DIST-03 | Binary size | §2 (Goals) | Binary size CI gate |
| NFR-DIST-04 | Build reproducibility | §7.1 | `rust-toolchain.toml` pin + CI artefact hash check |
| NFR-VER-01 | SemVer | §2 (Goals) | Changelog review on PR |
| NFR-FEAT-01 | `Table.RenameColumns` | §6.9 (PRD) | Snapshot tests (`rename_cols.pq`) |
| NFR-FEAT-02 | `Table.ExpandTableColumn` | §6.10 (PRD) | Snapshot tests (`nested_join.pq`) |
| NFR-FEAT-03 | TMDL indentation-based source blocks | §5.2 (PRD) | Integration test (`tmdl_indent.tmdl`) |
| NFR-FEAT-04 | `Table.RemoveColumns` | §6.11 (PRD) | Snapshot tests (`remove_cols.pq`); dialect matrix covering `bigquery`/`duckdb` (`EXCEPT`) and `tsql`/`postgres`/`snowflake` (explicit projection); fallback warning test for unknown column set |
| NFR-FEAT-05 | `Table.Combine` cross-file validation | §6.12 (PRD) | Integration tests (`combine_tables.pq`): (a) same-`let` bindings only; (b) valid cross-file reference to `Customers.pq`; (c) unresolved identifier → `UNRESOLVED_COMBINE_TABLE` error; (d) stdin mode → only `let` bindings accepted |
| NFR-FEAT-06 | `Table.ReplaceErrorValues` dialect fidelity | §6.13 (PRD) | Snapshot tests (`replace_error_values.pq`): (a) `tsql`/`snowflake`/`duckdb` → `TRY_CAST` path with coercion context; (b) `bigquery` → `SAFE_CAST` path; (c) `postgres` → `COALESCE` approximation + mandatory `REPLACE_ERROR_APPROXIMATE` warning always present in output; (d) no-coercion-context path → `CASE WHEN IS NULL` + warning for all dialects; (e) non-literal replacement → `UNTRANSLATABLE` |
| NFR-FEAT-07 | Power Query parameter detection | §6.14 (PRD) | Integration tests (`pq_parameters.pq`): (a) server/db args → `/* PARAM: … */` in FROM clause; (b) record-selector name → `/* PARAM: … */` in table reference; (c) filter condition value → `/* PARAM: … */` in WHERE clause; (d) parameter header block present and lexicographically sorted; (e) `PARAM_REFERENCE` warning emitted for each unique parameter regardless of `--on-error` mode (verified with all three modes); (f) no false positives — bound `let` vars, M keywords, and input file stems are not flagged; (g) run summary `params_detected` count is correct; (h) `--log-json` includes `"params_detected"` field only when > 0 |

# AGENTS.md — ddog CLI

## Project Overview
`ddog` is a lightweight Rust CLI for querying Datadog. It replicates the Datadog MCP Server's read-only functionality via direct REST API calls. Uses a custom reqwest-based HTTP client (not the official `datadog-api-client` crate — that's 35MB of auto-generated code with broken docs).

## Build & Run
```bash
cargo build                    # debug build
cargo build --release          # optimized release build (~2.2MB)
cargo fmt --check              # formatting check
cargo clippy --all-targets --all-features -- -D warnings
cargo test                     # run all tests
./target/debug/ddog --help     # show all commands
```

## Architecture
- `src/main.rs` — CLI entry point, clap command definitions
- `src/client.rs` — HTTP client wrapper (reqwest + DD auth headers)
- `src/config.rs` — Environment variable loading (DD_API_KEY, DD_APP_KEY, DD_SITE)
- `src/output.rs` — Output formatting: projected JSON (default), raw JSON (`full`), tables; `--max-tokens` budget (stdout for data, stderr for messages)
- `src/error.rs` — Error types with user-friendly hints (401/403/429 guidance)
- `src/time.rs` — Time parsing (relative/ISO8601/epoch) with safety range limits
- `src/limits.rs` — Safety constants (max time ranges, max result counts per resource)
- `src/log.rs` — User-facing info/warn/error to stderr (colored)
- `src/commands/` — One file per resource
- `src/commands/mod.rs` — shared Datadog request builders/helpers

## Key Patterns
- All commands receive `&DdClient` and return `Result<(), DdError>`
- Time ranges are validated via `time::resolve_range()` which enforces max hours per resource type
- Use `limits::resolve_limit()` for capped result counts and `limits::require_min()` for uncapped page/count validation
- Every command logs to stderr (query info, result count, pagination hints)
- Data goes to stdout (JSON or table) — safe for `| jq` piping
- Default JSON output is projected: `print_output`/`print_object` emit only `columns` per row (keyed by last path segment, full path on collision), compact, under the original wrapper key with `meta`/`metadata` preserved. `--format full` is the raw response. Empty `columns` means pass-through.
- `--max-tokens` (global flag, default 10000, `0` = off) caps projected JSON by dropping trailing rows and warning on stderr. It is set once from `main` via `output::set_max_tokens`.
- Shared Datadog payload builders live in `src/commands/mod.rs` (`search_request_body`, `event_search_body`, `aggregate_request_body`, `measure_sort`, etc.)
- Prefer `output::print_object()` for single-object `data` responses and `output::print_output()` for list responses
- Each command file has `#[cfg(test)]` clap parsing tests; cross-command request-shape tests live in `tests/contract_tests.rs`
- All commands are part of the default build (the `unsupported-commands` feature in Cargo.toml is a no-op)

## Safety Limits
| Resource | Max Time Range | Max Results |
|---|---|---|
| Logs search | 24h | 1000 |
| Log analytics | 7d (168h) | — |
| Spans/Traces | 24h | 1000 |
| Events | 48h | 1000 |
| Metrics query | 30d (720h) | — |
| Hosts | — | 1000 |
| Dashboards | — | 500 |

## Environment Variables
- `DD_API_KEY` — Required. Datadog API key.
- `DD_APP_KEY` — Required. Datadog Application key.
- `DD_SITE` — Optional. Defaults to `datadoghq.com`. Use `datadoghq.eu` for EU.

## Tech Debt

- The column lists for `hosts`, `incidents`, `synthetics`, `notebooks`, `services` and `downtimes` have never been checked against real API responses — none are captured (the app key in use gets 403 on several of those endpoints). Of the four that could be verified against saved responses (logs, spans, monitors, dashboards), all are correct. Since JSON output is projected, a wrong path there is a `null` field in the default output, not just a `-` cell.
- The `unsupported-commands` feature in Cargo.toml is a no-op and can be removed

Invariant to preserve: `columns` drive both the default JSON output and the table, so a column path that never resolves is a permanent `null` field for every user. Every column list needs coverage asserting each column resolves against a realistic response (see `assert_no_dead_columns` in `tests/contract_tests.rs`). A `Format::Full` run cannot catch it.

## Adding a New Command
1. Create `src/commands/<resource>.rs`
2. Define clap Args/Subcommand structs with doc comments including examples
3. Implement handler using `client.get()` or `client.post()`
4. Use `time::resolve_range()` for time-bounded queries
5. Use `limits::resolve_limit()` or `limits::require_min()` for user-controlled counts/pages
6. Use `log::info()` / `log::result_count()` for user feedback
7. Reuse shared request helpers from `src/commands/mod.rs` before adding new ad-hoc JSON builders
8. Add `#[cfg(test)]` tests for clap parsing and add/extend contract tests if request shape matters
9. Register in `src/commands/mod.rs` and `src/main.rs`

# ddog

A single-binary CLI for querying Datadog — logs, metrics, traces, APM, monitors and more — built as a lighter alternative to the Datadog MCP Server. It talks to the Datadog REST API directly and runs on macOS and Linux.

## Install

Prebuilt archives for macOS and Linux are attached to every [GitHub Release](../../releases):

```bash
tar -xzf ddog-<version>-<target>.tar.gz
./ddog --help
```

Or build it yourself with Rust:

```bash
cargo install --path .
```

## Setup

```bash
export DD_API_KEY="your-api-key"
export DD_APP_KEY="your-app-key"
export DD_SITE="datadoghq.eu"   # optional, default datadoghq.com
```

## Quick start

```bash
ddog logs search --query "service:web status:error" --from 1h
ddog logs analyze --query "status:error" --from 1h --group-by service
ddog metrics query --query "avg:system.cpu.user{*}" --from 2h
ddog monitors --query "status:alert"
ddog spans --query "service:web @http.status_code:500" --from 1h
ddog apm bottlenecks --query "service:api" --from 1h
ddog hosts --format table
```

Every command has `--help` with examples. Time flags accept relative (`15m`, `2h`, `3d`), ISO 8601 or Unix epoch values.

## Output

| `--format` | What you get |
|---|---|
| `json` (default) | Compact JSON, one curated set of columns per row (the same ones the table shows), keyed by short name. The response wrapper (`data`, `monitors`, …) and paging `meta` are kept. |
| `full` | The raw API response, compact. Every field, every tag. |
| `table` | Human-readable table, cells truncated to 80 chars. |

```bash
ddog spans --query "service:web"                 # {"data":[{"service":..,"resource_name":..,"duration":..,"status":..,"trace_id":..}],"meta":{..}}
ddog spans --query "service:web" --format full   # raw spans with all attributes and tags
```

`metrics query` returns up to 20 time buckets per series (`min`/`max`/`avg`) plus overall stats instead of the raw pointlist; `--format full` gives every point.

JSON output is capped by `--max-tokens` (default 10000, estimated as bytes/3). Rows past the budget are dropped and stderr says how many were kept. `--max-tokens 0` disables it. The cap does not apply to `full` or `table`.

Data goes to **stdout**, messages to **stderr**, so `| jq` works.

### Tokens per call, compared to the Datadog MCP Server

Measured on 2026-09-04 against a production Datadog org, same query on both sides, MCP tools called with their defaults, counted with `@anthropic-ai/tokenizer`:

| Query | `ddog --format full` | `ddog` (default) | Datadog MCP |
|---|---:|---:|---:|
| Logs, 50 rows requested | 123,548 | 10,029 (11 rows kept by the budget) | 6,073 (4 rows) |
| Metric timeseries, 2h | 5,555 | 1,218 | 1,349 |
| Monitors in alert, 25 rows | 14,781 | 2,554 | 11,525 (39 rows) |
| Spans, 50 rows | 70,744 | 5,630 | 14,608 (12 rows) |
| Dashboards, 2 rows | 360 | 137 | 115 |
| **Total** | **215,093** | **19,656** | **33,699** |

Per row the two cost about the same: neither encodes better, both save tokens by returning fewer fields and fewer rows. The default output gives `ddog` the same knobs the MCP ships with (curated columns, a token budget, a way to ask for everything) while returning more rows for the same budget. Details and raw outputs in `BENCHMARK.md`.

## Safety limits

Built-in caps stop accidental huge requests. The CLI rejects anything past them with a clear error.

| Resource | Max time range | Max results |
|---|---|---|
| Logs search | 24h | 1,000 |
| Log analytics | 7 days | — |
| Spans / traces | 24h | 1,000 |
| Events | 48h | 1,000 |
| RUM | 24h | 1,000 |
| Metrics query | 30 days | — |
| SLO history | 90 days | — |
| Synthetics results | 48h | — |
| Hosts | — | 1,000 |
| Dashboards | — | 500 |

## Commands

| Command | Description |
|---|---|
| `ddog logs search` | Search logs with filter query |
| `ddog logs analyze` | Aggregate logs (count, avg, percentiles) grouped by facet |
| `ddog metrics query` | Query metric timeseries data |
| `ddog metrics context` | Get metric metadata (type, unit, tags) |
| `ddog metrics search` | List configured metrics, optionally filtered by tag |
| `ddog events` | Search events (alerts, deploys, changes) |
| `ddog monitors` | Search monitors |
| `ddog hosts` | Search monitored hosts |
| `ddog dashboards` | Search dashboards |
| `ddog traces` | Get trace spans by trace ID |
| `ddog spans` | Search APM spans |
| `ddog services search` | Search service catalog |
| `ddog services deps` | Get service dependencies |
| `ddog apm spans` | Search APM spans |
| `ddog apm trace` | Explore a trace |
| `ddog apm summary` | Summarize a trace |
| `ddog apm compare` | Compare two traces |
| `ddog apm metrics` | Aggregate span metrics |
| `ddog apm tags` | Discover span tag values (`--facet @http.status_code`) |
| `ddog apm primary-tags` | Get primary tag keys of a trace metric |
| `ddog apm watchdog` | Search Watchdog stories |
| `ddog apm changes` | Search change/deploy events |
| `ddog apm bottlenecks` | Find latency bottlenecks |
| `ddog apm latency-tags` | Compare latency by tag |
| `ddog rum` | Search RUM events (page views, actions, errors) |
| `ddog slos search` | Search SLOs |
| `ddog slos get` | Get SLO details |
| `ddog slos history` | Get SLO history over time |
| `ddog downtimes list` | List scheduled downtimes |
| `ddog downtimes get` | Get downtime details |
| `ddog synthetics list` | List synthetic tests |
| `ddog synthetics results` | Get synthetic test results |
| `ddog incidents search` | Search incidents |
| `ddog incidents get` | Get incident details |
| `ddog notebooks search` | Search notebooks |
| `ddog notebooks get` | Get notebook by ID |

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo coverage            # HTML report in target/llvm-cov/html (needs cargo-llvm-cov)
```

## License

MIT

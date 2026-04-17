# ddog

A lightweight CLI for querying Datadog — logs, metrics, traces, APM, and more.

Built as a fast, single-binary alternative to the Datadog MCP Server, `ddog` talks directly to the Datadog REST API and works on macOS and Linux.

## Install

### From GitHub Releases

Each `v*` tag publishes prebuilt archives for macOS and Linux to GitHub Releases. Download the asset for your target and unpack it:

```bash
tar -xzf ddog-<version>-<target>.tar.gz
./ddog --help
```

### From source (requires Rust)
```bash
cargo install --path .
```

### Build from source
```bash
git clone https://github.com/<you>/dd-cli.git
cd dd-cli
cargo build --release
# Binary at ./target/release/ddog
```

### Coverage

Install the local coverage tool once:

```bash
cargo install cargo-llvm-cov
```

Then run:

```bash
cargo coverage
```

That writes an HTML report to:

```bash
target/llvm-cov/html/index.html
```

For a terminal-only summary:

```bash
cargo coverage-summary
```

## Setup

Export your Datadog credentials:

```bash
export DD_API_KEY="your-api-key"
export DD_APP_KEY="your-app-key"

# Optional: for EU or other regions (default: datadoghq.com)
export DD_SITE="datadoghq.eu"
```

## Usage

```bash
ddog --help
```

### Logs

```bash
# Search logs from the last hour
ddog logs search --query "service:web status:error" --from 1h

# Search logs with table output
ddog logs search --query "service:api" --from 30m --format table

# Aggregate logs — count errors by service
ddog logs analyze --query "status:error" --from 1h --compute count --group-by service
```

### Metrics

```bash
# Query a metric timeseries
ddog metrics query --query "avg:system.cpu.user{host:myhost}" --from 2h

# Get metric metadata
ddog metrics context --name system.cpu.user

# List available metrics
ddog metrics search

# Filter metrics by tags
ddog metrics search --tag env:production service:web
```

### Monitors

```bash
# Find alerting monitors
ddog monitors --query "status:alert"

# Search by type and tag
ddog monitors --query "type:metric tag:env:prod"
```

### Hosts

```bash
# List all hosts
ddog hosts

# Filter by name or tag
ddog hosts --filter "web"
```

### Dashboards

```bash
# List dashboards
ddog dashboards

# Filter by title
ddog dashboards --filter "production"
```

### Traces

```bash
# Get all spans for a trace
ddog traces --trace-id "1234567890abcdef"
```

### Spans

```bash
# Search spans with errors
ddog spans --query "service:web @http.status_code:500" --from 1h
```

### Services

```bash
# List service catalog
ddog services search

# Get service dependencies
ddog services deps --env production
```

### Events

```bash
# Search events from the last 6 hours
ddog events --query "source:deploy" --from 6h

# Search alert events
ddog events --query "source:monitor status:alert" --from 24h
```

### RUM (Real User Monitoring)

```bash
# Search RUM events
ddog rum --query "@type:action" --from 1h

# Search errors in a specific app
ddog rum --query "@application.name:myapp @type:error" --from 30m --format table
```

### SLOs

```bash
# List SLOs
ddog slos search

# Search SLOs by query
ddog slos search --query "service:web" --limit 50

# Get SLO details
ddog slos get --id "abc123def456"

# Get SLO history over the last 30 days
ddog slos history --id "abc123def456" --from 30d
```

### Downtimes

```bash
# List active downtimes
ddog downtimes list

# List all downtimes (including expired)
ddog downtimes list --current-only false

# Get downtime details
ddog downtimes get --id "dt-abc123"
```

### Synthetics

```bash
# List synthetic tests
ddog synthetics list

# Get test results
ddog synthetics results --id "abc-xyz-123" --from 6h
```

### Incidents

```bash
# Search active incidents
ddog incidents search --query "state:active"

# Get incident details
ddog incidents get --id "abc123"
```

### Notebooks

```bash
# Search notebooks
ddog notebooks search --query "postmortem"

# Get notebook by ID
ddog notebooks get --id 12345
```

### APM Deep Analysis

```bash
# Search APM spans
ddog apm spans --query "service:api" --from 1h

# Explore a trace
ddog apm trace --trace-id "abc123"

# Generate trace summary
ddog apm summary --trace-id "abc123"

# Compare two traces
ddog apm compare --trace-a "abc123" --trace-b "def456"

# Analyze trace metrics by service
ddog apm metrics --query "service:web" --compute avg --metric duration --group-by service

# Discover span tags
ddog apm tags --query "service:api" --facet http.status_code

# Show primary APM tag keys for a trace metric
ddog apm primary-tags --metric trace.http.request.duration

# Find latency bottlenecks
ddog apm bottlenecks --query "service:api" --from 1h

# Analyze latency by tag
ddog apm latency-tags --query "service:api" --tag region

# Search Watchdog anomalies
ddog apm watchdog --from 24h

# Search deployment changes
ddog apm changes --from 24h
```

## Output Formats

By default, output is JSON. Use `--format table` for human-readable tables:

```bash
ddog hosts --format table
ddog logs search --query "*" --from 15m --format table
```

Info/warning messages go to **stderr**, data goes to **stdout** — safe for `| jq` piping.

## Safety Limits

Built-in limits prevent accidentally huge API requests:

| Resource | Max Time Range | Max Results |
|---|---|---|
| Logs search | 24h | 1,000 |
| Log analytics | 7 days | — |
| Spans/Traces | 24h | 1,000 |
| Events | 48h | 1,000 |
| RUM | 24h | 1,000 |
| Metrics query | 30 days | — |
| SLO history | 90 days | — |
| Synthetics results | 48h | — |
| Hosts | — | 1,000 |
| Dashboards | — | 500 |

```bash
# This will be rejected with a clear error:
ddog logs search --from 48h
# error: Time range too large: 48h requested, max 24h allowed.
```

## All Commands

| Command | Description |
|---|---|
| `ddog logs search` | Search logs with filter query |
| `ddog logs analyze` | Aggregate/analyze logs (count, avg, sum, etc.) |
| `ddog metrics query` | Query metric timeseries data |
| `ddog metrics context` | Get metric metadata (type, unit, tags) |
| `ddog metrics search` | List configured metrics (optionally filtered by tag) |
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
| `ddog apm summary` | Generate trace summary |
| `ddog apm compare` | Compare two traces |
| `ddog apm metrics` | Analyze trace metrics |
| `ddog apm tags` | Discover span tag keys |
| `ddog apm primary-tags` | Get primary tag keys |
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

Useful local checks:

```bash
cargo fmt --check
cargo test
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

## License

MIT

use crate::client::DdClient;
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_object, print_output, print_table};
use crate::time;
use clap::Subcommand;
use serde_json::{Value, json};

#[derive(Subcommand)]
#[command(verbatim_doc_comment)]
pub enum MetricsCmd {
    /// Query metric timeseries data
    ///
    /// Examples:
    ///   ddog metrics query --query "avg:system.cpu.user{*}" --from 1h
    ///   ddog metrics query --query "sum:http.requests{service:api}.as_count()" --from 6h
    ///   ddog metrics query --query "avg:system.mem.used{host:web-1}" --from 2d
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog metrics query --query \"avg:system.cpu.user{*}\" --from 1h\n  ddog metrics query --query \"sum:http.requests{service:api}.as_count()\" --from 6h\n  ddog metrics query --query \"avg:system.mem.used{host:web-1}\" --from 2d"
    )]
    Query {
        /// Metric query in Datadog syntax (e.g., "avg:system.cpu.user{host:web-*}")
        #[arg(short, long)]
        query: String,

        /// Start time. Max range: 30 days for metrics
        #[arg(long, default_value = "1h")]
        from: String,

        /// End time — defaults to now
        #[arg(long)]
        to: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Get metric metadata (type, description, unit, tags)
    ///
    /// Examples:
    ///   ddog metrics context --name system.cpu.user
    ///   ddog metrics context --name http.requests --format table
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog metrics context --name system.cpu.user\n  ddog metrics context --name http.requests --format table"
    )]
    Context {
        /// Metric name (e.g., "system.cpu.user", "http.requests")
        #[arg(short, long)]
        name: String,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// List configured metrics, optionally filtered by tag
    ///
    /// Note: The Datadog v2 metrics API does not support substring search by name.
    /// Use --tag to filter by tag, or pipe JSON output through jq.
    ///
    /// Examples:
    ///   ddog metrics search
    ///   ddog metrics search --tag env:production
    ///   ddog metrics search --tag service:web --format table
    ///   ddog metrics search | jq '.data[].id | select(contains("cpu"))'
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Note: The Datadog v2 metrics API does not support substring search by name.\nUse --tag to filter by tag, or pipe JSON output through jq.\n\nExamples:\n  ddog metrics search\n  ddog metrics search --tag env:production service:web\n  ddog metrics search --tag env:production --tag service:web --format table\n  ddog metrics search | jq '.data[].id | select(contains(\"cpu\"))'"
    )]
    Search {
        /// Filter by one or more tags (e.g., --tag env:prod service:web)
        #[arg(long, num_args = 1..)]
        tag: Vec<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },
}

pub async fn run(client: &DdClient, cmd: MetricsCmd) -> Result<(), DdError> {
    match cmd {
        MetricsCmd::Query {
            query,
            from,
            to,
            format,
        } => {
            let (from_epoch, to_epoch) =
                time::resolve_range_epoch(&from, &to, limits::MAX_METRIC_HOURS)
                    .map_err(DdError::Validation)?;

            log::info(&format!(
                "Querying metric: \"{query}\" range={}",
                time::format_duration(to_epoch - from_epoch)
            ));

            let result = client
                .get(
                    "/api/v1/query",
                    &[
                        ("from", &from_epoch.to_string()),
                        ("to", &to_epoch.to_string()),
                        ("query", &query),
                    ],
                )
                .await?;

            match format {
                Format::Json => {
                    let count = print_output(&result, &format, &[]);
                    log::result_count(count, "series");
                }
                Format::Table => {
                    let rows = summarize_series(&result);
                    let count = rows.len();
                    print_table(
                        &rows,
                        &["metric", "scope", "latest", "avg", "min", "max", "points"],
                    );
                    log::result_count(count, "series");
                }
            }
            Ok(())
        }
        MetricsCmd::Context { name, format } => {
            log::info(&format!("Fetching metadata for metric: {name}"));
            let result = client.get(&format!("/api/v1/metrics/{name}"), &[]).await?;
            print_object(
                &result,
                &format,
                &["type", "description", "short_name", "unit", "integration"],
            );
            Ok(())
        }
        MetricsCmd::Search { tag, format } => {
            log::info("Listing configured metrics...");
            let mut params: Vec<(&str, &str)> = vec![("filter[configured]", "true")];
            let tag_str = tag.join(",");
            if !tag.is_empty() {
                params.push(("filter[tags]", &tag_str));
            }

            let result = client.get("/api/v2/metrics", &params).await?;
            let count = print_output(&result, &format, &["id", "type", "attributes.metric_type"]);
            log::result_count(count, "metrics");
            Ok(())
        }
    }
}

/// Summarize metric series pointlists into table-friendly rows with latest/avg/min/max.
fn summarize_series(result: &Value) -> Vec<Value> {
    let empty = vec![];
    let series = result
        .get("series")
        .and_then(|s| s.as_array())
        .unwrap_or(&empty);

    series
        .iter()
        .map(|s| {
            let metric = s.get("metric").and_then(|v| v.as_str()).unwrap_or("-");
            let scope = s.get("scope").and_then(|v| v.as_str()).unwrap_or("*");
            let pointlist = s.get("pointlist").and_then(|v| v.as_array());

            match pointlist {
                Some(pts) if !pts.is_empty() => {
                    let values: Vec<f64> = pts
                        .iter()
                        .filter_map(|p| p.as_array().and_then(|arr| arr.get(1)).and_then(|v| v.as_f64()))
                        .collect();

                    if values.is_empty() {
                        return json!({
                            "metric": metric,
                            "scope": scope,
                            "latest": "-",
                            "avg": "-",
                            "min": "-",
                            "max": "-",
                            "points": 0,
                        });
                    }

                    let latest = values.last().copied().unwrap_or(0.0);
                    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let avg = values.iter().sum::<f64>() / values.len() as f64;

                    json!({
                        "metric": metric,
                        "scope": scope,
                        "latest": format!("{latest:.2}"),
                        "avg": format!("{avg:.2}"),
                        "min": format!("{min:.2}"),
                        "max": format!("{max:.2}"),
                        "points": values.len(),
                    })
                }
                _ => json!({
                    "metric": metric,
                    "scope": scope,
                    "latest": "-",
                    "avg": "-",
                    "min": "-",
                    "max": "-",
                    "points": 0,
                }),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: MetricsCmd,
    }

    #[test]
    fn test_parse_query() {
        let cli = TestCli::parse_from([
            "test",
            "query",
            "--query",
            "avg:system.cpu.user{*}",
            "--from",
            "2h",
        ]);
        match cli.cmd {
            MetricsCmd::Query { query, from, .. } => {
                assert_eq!(query, "avg:system.cpu.user{*}");
                assert_eq!(from, "2h");
            }
            _ => panic!("expected Query"),
        }
    }

    #[test]
    fn test_summarize_series() {
        let response = json!({
            "series": [{
                "metric": "system.cpu.user",
                "scope": "host:web-1",
                "pointlist": [[1710000000000.0, 10.0], [1710003600000.0, 20.0], [1710007200000.0, 30.0]]
            }]
        });
        let rows = summarize_series(&response);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["metric"], "system.cpu.user");
        assert_eq!(rows[0]["scope"], "host:web-1");
        assert_eq!(rows[0]["latest"], "30.00");
        assert_eq!(rows[0]["avg"], "20.00");
        assert_eq!(rows[0]["min"], "10.00");
        assert_eq!(rows[0]["max"], "30.00");
        assert_eq!(rows[0]["points"], 3);
    }

    #[test]
    fn test_summarize_series_empty() {
        let response = json!({"series": []});
        let rows = summarize_series(&response);
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn test_parse_context() {
        let cli = TestCli::parse_from(["test", "context", "--name", "system.cpu.user"]);
        match cli.cmd {
            MetricsCmd::Context { name, .. } => {
                assert_eq!(name, "system.cpu.user");
            }
            _ => panic!("expected Context"),
        }
    }

    #[test]
    fn test_parse_search_with_tags() {
        let cli =
            TestCli::parse_from(["test", "search", "--tag", "env:prod", "--tag", "region:us"]);
        match cli.cmd {
            MetricsCmd::Search { tag, .. } => {
                assert_eq!(tag, vec!["env:prod", "region:us"]);
            }
            _ => panic!("expected Search"),
        }
    }

    #[test]
    fn test_parse_search_with_space_separated_tags() {
        let cli = TestCli::parse_from(["test", "search", "--tag", "env:staging", "service:bpg"]);
        match cli.cmd {
            MetricsCmd::Search { tag, .. } => {
                assert_eq!(tag, vec!["env:staging", "service:bpg"]);
            }
            _ => panic!("expected Search"),
        }
    }

    #[test]
    fn test_parse_search_defaults() {
        let cli = TestCli::parse_from(["test", "search"]);
        match cli.cmd {
            MetricsCmd::Search { tag, .. } => {
                assert!(tag.is_empty());
            }
            _ => panic!("expected Search"),
        }
    }
}

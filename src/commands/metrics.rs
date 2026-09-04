use crate::client::DdClient;
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_json, print_object, print_output, print_table};
use crate::time;
use chrono::DateTime;
use clap::Subcommand;
use serde_json::{Value, json};

/// Time buckets emitted per series in the default JSON output.
const BINS: usize = 20;

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
                    let binned = bin_series(&result, BINS);
                    let count = binned["series"].as_array().map_or(0, Vec::len);
                    print_json(&binned);
                    log::result_count(count, "series");
                }
                Format::Full => {
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

struct Stats {
    min: f64,
    max: f64,
    avg: f64,
    last: f64,
}

fn stats(values: &[f64]) -> Option<Stats> {
    let last = *values.last()?;
    Some(Stats {
        min: values.iter().cloned().fold(f64::INFINITY, f64::min),
        max: values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        avg: values.iter().sum::<f64>() / values.len() as f64,
        last,
    })
}

fn series_list(result: &Value) -> &[Value] {
    result
        .get("series")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Pointlist entries as (epoch ms, value), skipping null values.
fn points(series: &Value) -> Vec<(f64, f64)> {
    series
        .get("pointlist")
        .and_then(Value::as_array)
        .map(|pts| {
            pts.iter()
                .filter_map(|p| {
                    let arr = p.as_array()?;
                    Some((arr.first()?.as_f64()?, arr.get(1)?.as_f64()?))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn iso(epoch_ms: f64) -> Value {
    DateTime::from_timestamp_millis(epoch_ms as i64)
        .map(|t| json!(t.format("%Y-%m-%dT%H:%M:%SZ").to_string()))
        .unwrap_or(Value::Null)
}

/// Collapse each series into at most `bins` time buckets (min/max/avg per bucket)
/// plus overall stats, instead of the raw pointlist.
fn bin_series(result: &Value, bins: usize) -> Value {
    let series: Vec<Value> = series_list(result)
        .iter()
        .map(|s| {
            let pts = points(s);
            let values: Vec<f64> = pts.iter().map(|p| p.1).collect();
            let per_bin = pts.len().div_ceil(bins).max(1);
            let binned: Vec<Value> = pts
                .chunks(per_bin)
                .filter_map(|chunk| {
                    let vals: Vec<f64> = chunk.iter().map(|p| p.1).collect();
                    let st = stats(&vals)?;
                    Some(json!({
                        "ts": iso(chunk[0].0),
                        "count": chunk.len(),
                        "min": st.min,
                        "max": st.max,
                        "avg": st.avg,
                    }))
                })
                .collect();
            json!({
                "metric": s.get("metric"),
                "scope": s.get("scope"),
                "expression": s.get("expression"),
                "unit": s.pointer("/unit/0/short_name"),
                "from": pts.first().map_or(Value::Null, |p| iso(p.0)),
                "to": pts.last().map_or(Value::Null, |p| iso(p.0)),
                "points": pts.len(),
                "stats": stats(&values).map_or(Value::Null, |st| json!({
                    "min": st.min, "max": st.max, "avg": st.avg, "last": st.last,
                })),
                "bins": binned,
            })
        })
        .collect();
    json!({ "series": series })
}

/// Summarize metric series pointlists into table-friendly rows with latest/avg/min/max.
fn summarize_series(result: &Value) -> Vec<Value> {
    series_list(result)
        .iter()
        .map(|s| {
            let metric = s.get("metric").and_then(|v| v.as_str()).unwrap_or("-");
            let scope = s.get("scope").and_then(|v| v.as_str()).unwrap_or("*");
            let values: Vec<f64> = points(s).iter().map(|p| p.1).collect();

            match stats(&values) {
                Some(st) => json!({
                    "metric": metric,
                    "scope": scope,
                    "latest": format!("{:.2}", st.last),
                    "avg": format!("{:.2}", st.avg),
                    "min": format!("{:.2}", st.min),
                    "max": format!("{:.2}", st.max),
                    "points": values.len(),
                }),
                None => json!({
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
    fn test_bin_series_collapses_points_into_bins() {
        let pointlist: Vec<Value> = (0..40)
            .map(|i| json!([1_710_000_000_000.0 + i as f64 * 60_000.0, i as f64]))
            .collect();
        let response = json!({
            "series": [{
                "metric": "system.cpu.user",
                "scope": "*",
                "expression": "avg:system.cpu.user{*}",
                "unit": [{"short_name": "%"}, null],
                "pointlist": pointlist
            }]
        });
        let out = bin_series(&response, 20);
        let s = &out["series"][0];
        assert_eq!(s["metric"], "system.cpu.user");
        assert_eq!(s["unit"], "%");
        assert_eq!(s["points"], 40);
        assert_eq!(s["from"], "2024-03-09T16:00:00Z");
        assert_eq!(s["stats"]["min"], 0.0);
        assert_eq!(s["stats"]["max"], 39.0);
        assert_eq!(s["stats"]["last"], 39.0);
        let bins = s["bins"].as_array().unwrap();
        assert_eq!(bins.len(), 20);
        assert_eq!(
            bins[0],
            json!({"ts": "2024-03-09T16:00:00Z", "count": 2, "min": 0.0, "max": 1.0, "avg": 0.5})
        );
        assert!(s.get("pointlist").is_none());
    }

    #[test]
    fn test_bin_series_fewer_points_than_bins_keeps_one_per_point() {
        let response =
            json!({"series": [{"metric": "m", "pointlist": [[0.0, 1.0], [60000.0, 2.0]]}]});
        let out = bin_series(&response, 20);
        assert_eq!(out["series"][0]["bins"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_bin_series_empty_pointlist() {
        let response = json!({"series": [{"metric": "m", "pointlist": []}]});
        let out = bin_series(&response, 20);
        assert_eq!(out["series"][0]["points"], 0);
        assert_eq!(out["series"][0]["stats"], Value::Null);
        assert_eq!(out["series"][0]["bins"], json!([]));
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

use crate::client::DdClient;
use crate::commands::{measure_sort, normalize_facet, normalize_metric};
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_output};
use crate::time;
use clap::Subcommand;
use serde_json::json;

#[derive(Subcommand)]
#[command(verbatim_doc_comment)]
pub enum LogsCmd {
    /// Search logs with filter query
    ///
    /// Examples:
    ///   ddog logs search --query "service:web status:error" --from 1h
    ///   ddog logs search --query "host:prod-*" --from 30m --limit 50
    ///   ddog logs search --query "@http.status_code:500" --from 2h --format table
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog logs search --query \"service:web status:error\" --from 1h\n  ddog logs search --query \"host:prod-*\" --from 30m --limit 50\n  ddog logs search --query \"@http.status_code:500\" --from 2h --format table"
    )]
    Search {
        /// Datadog log query syntax (e.g., "service:web status:error", "@http.url:/api/*")
        #[arg(short, long, default_value = "*")]
        query: String,

        /// Start time — relative (15m, 1h, 2d), ISO8601, or epoch. Max range: 24h
        #[arg(long, default_value = "1h")]
        from: String,

        /// End time — defaults to now
        #[arg(long)]
        to: Option<String>,

        /// Max results (1-1000, default 25). Use --cursor for pagination
        #[arg(short, long, default_value = "25", value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,

        /// Sort: "timestamp" (oldest first) or "-timestamp" (newest first)
        #[arg(long, default_value = "-timestamp", value_parser = ["timestamp", "-timestamp"])]
        sort: String,

        /// Pagination cursor from a previous response's meta.page.after
        #[arg(long)]
        cursor: Option<String>,

        /// Restrict to specific log indexes (can repeat: --indexes main --indexes error)
        #[arg(long)]
        indexes: Vec<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Aggregate and analyze logs (count, avg, percentiles, grouped by facet)
    ///
    /// Examples:
    ///   ddog logs analyze --query "status:error" --from 1h --compute count --group-by service
    ///   ddog logs analyze --query "*" --compute avg --metric response_time --group-by host
    ///   ddog logs analyze --query "service:api" --compute pc95 --metric duration --interval 5m
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog logs analyze --query \"status:error\" --from 1h --compute count --group-by service\n  ddog logs analyze --query \"*\" --compute avg --metric response_time --group-by host\n  ddog logs analyze --query \"service:api\" --compute pc95 --metric duration --interval 5m"
    )]
    Analyze {
        /// Datadog log query
        #[arg(short, long, default_value = "*")]
        query: String,

        /// Start time. Max range: 7 days
        #[arg(long, default_value = "1h")]
        from: String,

        /// End time — defaults to now
        #[arg(long)]
        to: Option<String>,

        /// Aggregation: count, avg, sum, min, max, pc75, pc90, pc95, pc99
        #[arg(long, default_value = "count")]
        compute: String,

        /// Metric to aggregate on (required for avg/sum/min/max/percentile)
        #[arg(long)]
        metric: Option<String>,

        /// Group by facet (repeatable: --group-by service --group-by host)
        #[arg(long)]
        group_by: Vec<String>,

        /// Time bucket interval for timeseries (e.g., "5m", "1h")
        #[arg(long)]
        interval: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },
}

pub async fn run(client: &DdClient, cmd: LogsCmd) -> Result<(), DdError> {
    match cmd {
        LogsCmd::Search {
            query,
            from,
            to,
            limit,
            sort,
            cursor,
            indexes,
            format,
        } => {
            let range = time::resolve_range(&from, &to, limits::MAX_LOG_HOURS)
                .map_err(DdError::Validation)?;
            let limit = limits::resolve_limit(limit, limits::MAX_SEARCH_LIMIT)
                .map_err(DdError::Validation)?;

            if limit > 100 {
                log::warn(&format!(
                    "Requesting {limit} logs — large responses may be slow."
                ));
            }

            log::info(&format!(
                "Searching logs: query=\"{query}\" range={} limit={limit}",
                time::format_duration(range.duration_secs)
            ));

            let mut filter = json!({
                "query": query,
                "from": range.from,
                "to": range.to,
            });
            if !indexes.is_empty() {
                filter["indexes"] = json!(indexes);
            }

            let mut body = json!({
                "filter": filter,
                "sort": sort,
                "page": { "limit": limit },
            });
            if let Some(c) = cursor {
                body["page"]["cursor"] = json!(c);
            }

            let result = client.post("/api/v2/logs/events/search", &body).await?;
            let count = print_output(
                &result,
                &format,
                &[
                    "id",
                    "attributes.timestamp",
                    "attributes.status",
                    "attributes.service",
                    "attributes.message",
                ],
            );
            log::result_count(count, "logs");

            if let Some(cursor) = result
                .get("meta")
                .and_then(|m| m.get("page"))
                .and_then(|p| p.get("after"))
                .and_then(|a| a.as_str())
            {
                log::info(&format!(
                    "More results available. Use --cursor \"{cursor}\""
                ));
            }
            Ok(())
        }
        LogsCmd::Analyze {
            query,
            from,
            to,
            compute,
            metric,
            group_by,
            interval,
            format,
        } => {
            let range = time::resolve_range(&from, &to, limits::MAX_ANALYTICS_HOURS)
                .map_err(DdError::Validation)?;

            if compute != "count" && metric.is_none() {
                return Err(DdError::Validation(format!(
                    "Aggregation '{compute}' requires --metric (e.g., --metric response_time)."
                )));
            }

            log::info(&format!(
                "Analyzing logs: query=\"{query}\" compute={compute} range={}",
                time::format_duration(range.duration_secs)
            ));

            let metric = metric.map(|m| normalize_metric(&m));
            let metric_ref = metric.as_deref();

            let mut compute_obj = json!({ "aggregation": compute });
            if let Some(m) = metric_ref {
                compute_obj["metric"] = json!(m);
            }
            if let Some(iv) = interval {
                compute_obj["interval"] = json!(iv);
                compute_obj["type"] = json!("timeseries");
            }

            let groups: Vec<serde_json::Value> = group_by
                .iter()
                .map(|g| {
                    json!({
                        "facet": normalize_facet(g),
                        "limit": 10,
                        "sort": measure_sort(&compute, metric_ref)
                    })
                })
                .collect();

            let body = json!({
                "filter": {
                    "query": query,
                    "from": range.from,
                    "to": range.to,
                },
                "compute": [compute_obj],
                "group_by": groups,
            });

            let result = client
                .post("/api/v2/logs/analytics/aggregate", &body)
                .await?;
            print_output(&result, &format, &["buckets", "compute", "group_by"]);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // Verify clap parsing works for search
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: LogsCmd,
    }

    #[test]
    fn test_parse_search_defaults() {
        let cli = TestCli::parse_from(["test", "search"]);
        match cli.cmd {
            LogsCmd::Search {
                query,
                from,
                limit,
                sort,
                ..
            } => {
                assert_eq!(query, "*");
                assert_eq!(from, "1h");
                assert_eq!(limit, 25);
                assert_eq!(sort, "-timestamp");
            }
            _ => panic!("expected Search"),
        }
    }

    #[test]
    fn test_parse_search_rejects_invalid_sort() {
        let err = match TestCli::try_parse_from(["test", "search", "--sort", "garbage-value"]) {
            Ok(_) => panic!("expected invalid sort to fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("possible values"));
    }

    #[test]
    fn test_parse_search_rejects_zero_limit() {
        let err = match TestCli::try_parse_from(["test", "search", "--limit", "0"]) {
            Ok(_) => panic!("expected zero limit to fail"),
            Err(err) => err,
        };
        let msg = err.to_string();
        assert!(msg.contains("--limit"));
        assert!(msg.contains('0'));
    }

    #[test]
    fn test_parse_search_custom() {
        let cli = TestCli::parse_from([
            "test",
            "search",
            "-q",
            "service:web",
            "--from",
            "30m",
            "--limit",
            "50",
        ]);
        match cli.cmd {
            LogsCmd::Search { query, limit, .. } => {
                assert_eq!(query, "service:web");
                assert_eq!(limit, 50);
            }
            _ => panic!("expected Search"),
        }
    }

    #[test]
    fn test_parse_analyze_defaults() {
        let cli = TestCli::parse_from(["test", "analyze"]);
        match cli.cmd {
            LogsCmd::Analyze {
                compute, metric, ..
            } => {
                assert_eq!(compute, "count");
                assert!(metric.is_none());
            }
            _ => panic!("expected Analyze"),
        }
    }

    #[test]
    fn test_parse_analyze_with_metric() {
        let cli = TestCli::parse_from([
            "test",
            "analyze",
            "--compute",
            "avg",
            "--metric",
            "response_time",
            "--group-by",
            "service",
        ]);
        match cli.cmd {
            LogsCmd::Analyze {
                compute,
                metric,
                group_by,
                ..
            } => {
                assert_eq!(compute, "avg");
                assert_eq!(metric.unwrap(), "response_time");
                assert_eq!(group_by, vec!["service"]);
            }
            _ => panic!("expected Analyze"),
        }
    }
}

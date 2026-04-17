use crate::client::DdClient;
use crate::commands::events::{self, EventsSearch};
use crate::commands::{
    aggregate_compute, aggregate_request_body, facet_group, measure_sort, normalize_metric,
    require_non_empty, search_request_body, trace_id_query,
};
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_object, print_output};
use crate::time;
use clap::Subcommand;
use serde_json::json;

#[derive(Subcommand)]
#[command(verbatim_doc_comment)]
pub enum ApmCmd {
    /// Search APM spans with query
    ///
    /// Examples:
    ///   ddog apm spans --query "service:api" --from 1h
    ///   ddog apm spans --query "@http.status_code:500" --limit 50 --format table
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog apm spans --query \"service:api\" --from 1h\n  ddog apm spans --query \"@http.status_code:500\" --limit 50 --format table"
    )]
    Spans {
        /// Span query
        #[arg(short, long, default_value = "*")]
        query: String,

        /// Start time. Max range: 24h
        #[arg(long, default_value = "1h")]
        from: String,

        /// End time
        #[arg(long)]
        to: Option<String>,

        /// Max results (1-1000)
        #[arg(short, long, default_value = "25", value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,

        /// Pagination cursor
        #[arg(long)]
        cursor: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Explore a trace — retrieve all spans for a trace ID
    ///
    /// Examples:
    ///   ddog apm trace --trace-id "abc123"
    ///   ddog apm trace --trace-id "abc123" --from 12h --format table
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog apm trace --trace-id \"abc123\"\n  ddog apm trace --trace-id \"abc123\" --from 12h --format table"
    )]
    Trace {
        /// Trace ID
        #[arg(short, long, value_parser = clap::builder::NonEmptyStringValueParser::new())]
        trace_id: String,

        /// Start time. Max range: 24h
        #[arg(long, default_value = "6h")]
        from: String,

        /// End time
        #[arg(long)]
        to: Option<String>,

        /// Max spans (1-1000)
        #[arg(short, long, default_value = "200", value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Generate a summary of a trace (span count, services, errors)
    ///
    /// Examples:
    ///   ddog apm summary --trace-id "abc123"
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog apm summary --trace-id \"abc123\""
    )]
    Summary {
        /// Trace ID
        #[arg(short, long, value_parser = clap::builder::NonEmptyStringValueParser::new())]
        trace_id: String,

        /// Start time. Max range: 24h
        #[arg(long, default_value = "6h")]
        from: String,

        /// End time
        #[arg(long)]
        to: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Compare two traces side-by-side
    ///
    /// Examples:
    ///   ddog apm compare --trace-a "abc123" --trace-b "def456"
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog apm compare --trace-a \"abc123\" --trace-b \"def456\""
    )]
    Compare {
        /// First trace ID (e.g., the slow one)
        #[arg(long, value_parser = clap::builder::NonEmptyStringValueParser::new())]
        trace_a: String,

        /// Second trace ID (e.g., the fast one)
        #[arg(long, value_parser = clap::builder::NonEmptyStringValueParser::new())]
        trace_b: String,

        /// Start time. Max range: 24h
        #[arg(long, default_value = "6h")]
        from: String,

        /// End time
        #[arg(long)]
        to: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Analyze trace metrics with aggregation (avg, p95, p99 by service/resource)
    ///
    /// Examples:
    ///   ddog apm metrics --query "service:web" --compute avg --metric duration --group-by service
    ///   ddog apm metrics --query "*" --compute pc99 --metric duration --group-by resource_name
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog apm metrics --query \"service:web\" --compute avg --metric duration --group-by service\n  ddog apm metrics --query \"*\" --compute pc99 --metric duration --group-by resource_name"
    )]
    Metrics {
        /// Span query to scope the analysis
        #[arg(short, long, default_value = "*")]
        query: String,

        /// Start time. Max range: 24h
        #[arg(long, default_value = "1h")]
        from: String,

        /// End time
        #[arg(long)]
        to: Option<String>,

        /// Aggregation: count, avg, sum, min, max, pc75, pc90, pc95, pc99
        #[arg(long, default_value = "avg")]
        compute: String,

        /// Metric to aggregate (default: duration)
        #[arg(long, default_value = "duration")]
        metric: String,

        /// Group by fields (repeatable: --group-by service --group-by resource_name)
        #[arg(long)]
        group_by: Vec<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Discover available span tag keys and their top values
    ///
    /// Examples:
    ///   ddog apm tags --facet http.status_code
    ///   ddog apm tags --query "service:api" --facet resource_name
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog apm tags --facet http.status_code\n  ddog apm tags --query \"service:api\" --facet resource_name"
    )]
    Tags {
        /// Span query to scope tag discovery
        #[arg(short, long, default_value = "*")]
        query: String,

        /// Start time. Max range: 24h
        #[arg(long, default_value = "1h")]
        from: String,

        /// End time
        #[arg(long)]
        to: Option<String>,

        /// Tag facet to discover values for (e.g., "service", "http.status_code")
        #[arg(long, default_value = "service")]
        facet: String,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Get primary tag keys for a trace metric
    ///
    /// Examples:
    ///   ddog apm primary-tags
    ///   ddog apm primary-tags --metric trace.http.request.duration
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog apm primary-tags\n  ddog apm primary-tags --metric trace.http.request.duration"
    )]
    PrimaryTags {
        /// Trace metric name
        #[arg(short, long, default_value = "trace.http.request.duration")]
        metric: String,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Search Watchdog anomaly detection stories
    ///
    /// Examples:
    ///   ddog apm watchdog --from 24h
    ///   ddog apm watchdog --id "story123"
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog apm watchdog --from 24h\n  ddog apm watchdog --id \"story123\""
    )]
    Watchdog {
        /// Specific story ID to retrieve
        #[arg(long)]
        id: Option<String>,

        /// Start time. Max range: 48h
        #[arg(long, default_value = "24h")]
        from: String,

        /// End time
        #[arg(long)]
        to: Option<String>,

        /// Max results (1-1000)
        #[arg(short, long, default_value = "25", value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Search for deployment/infrastructure change events
    ///
    /// Examples:
    ///   ddog apm changes --from 24h
    ///   ddog apm changes --query "source:github" --from 12h
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog apm changes --from 24h\n  ddog apm changes --query \"source:github\" --from 12h"
    )]
    Changes {
        /// Query for change events
        #[arg(short, long, default_value = "source:(deploy OR deployment OR github)")]
        query: String,

        /// Start time. Max range: 48h
        #[arg(long, default_value = "24h")]
        from: String,

        /// End time
        #[arg(long)]
        to: Option<String>,

        /// Max results (1-1000)
        #[arg(short, long, default_value = "25", value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Identify latency bottleneck spans by p99 self-time
    ///
    /// Examples:
    ///   ddog apm bottlenecks --query "service:api" --from 1h
    ///   ddog apm bottlenecks --group-by service --format table
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog apm bottlenecks --query \"service:api\" --from 1h\n  ddog apm bottlenecks --group-by service --format table"
    )]
    Bottlenecks {
        /// Span query
        #[arg(short, long, default_value = "*")]
        query: String,

        /// Start time. Max range: 24h
        #[arg(long, default_value = "1h")]
        from: String,

        /// End time
        #[arg(long)]
        to: Option<String>,

        /// Group by field (e.g., "service", "resource_name")
        #[arg(long, default_value = "resource_name")]
        group_by: String,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Compare tag distributions to find latency correlation
    ///
    /// Examples:
    ///   ddog apm latency-tags --query "service:api" --tag region
    ///   ddog apm latency-tags --query "service:web" --tag http.status_code
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog apm latency-tags --query \"service:api\" --tag region\n  ddog apm latency-tags --query \"service:web\" --tag http.status_code"
    )]
    LatencyTags {
        /// Span query
        #[arg(short, long, default_value = "*")]
        query: String,

        /// Start time. Max range: 24h
        #[arg(long, default_value = "1h")]
        from: String,

        /// End time
        #[arg(long)]
        to: Option<String>,

        /// Tag to analyze (e.g., "region", "http.status_code", "version")
        #[arg(long)]
        tag: String,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },
}

const AGGREGATE_COLUMNS: &[&str] = &["attributes.by", "attributes.compute"];
const SPAN_COLUMNS: &[&str] = &[
    "attributes.service",
    "attributes.resource_name",
    "attributes.custom.duration",
    "attributes.status",
];
const TRACE_COLUMNS: &[&str] = &[
    "attributes.service",
    "attributes.resource_name",
    "attributes.custom.duration",
    "attributes.span_id",
    "attributes.parent_id",
];

pub async fn run(client: &DdClient, cmd: ApmCmd) -> Result<(), DdError> {
    match cmd {
        ApmCmd::Spans {
            query,
            from,
            to,
            limit,
            cursor,
            format,
        } => {
            let range = time::resolve_range(&from, &to, limits::MAX_SPAN_HOURS)
                .map_err(DdError::Validation)?;
            let limit = limits::resolve_limit(limit, limits::MAX_SEARCH_LIMIT)
                .map_err(DdError::Validation)?;

            log::info(&format!(
                "Searching spans: query=\"{query}\" range={} limit={limit}",
                time::format_duration(range.duration_secs)
            ));

            let body = search_request_body(
                &query,
                &range.from,
                &range.to,
                limit,
                Some("-timestamp"),
                cursor.as_deref(),
            );
            let result = client.post("/api/v2/spans/events/search", &body).await?;
            let count = print_output(&result, &format, SPAN_COLUMNS);
            log::result_count(count, "spans");
            Ok(())
        }

        ApmCmd::Trace {
            trace_id,
            from,
            to,
            limit,
            format,
        } => {
            require_non_empty("Trace ID", &trace_id).map_err(DdError::Validation)?;
            let range = time::resolve_range(&from, &to, limits::MAX_SPAN_HOURS)
                .map_err(DdError::Validation)?;
            let limit = limits::resolve_limit(limit, limits::MAX_SEARCH_LIMIT)
                .map_err(DdError::Validation)?;

            log::info(&format!("Exploring trace: {trace_id}"));

            let query = trace_id_query(&trace_id);
            let body = search_request_body(
                &query,
                &range.from,
                &range.to,
                limit,
                Some("timestamp"),
                None,
            );
            let result = client.post("/api/v2/spans/events/search", &body).await?;
            let count = print_output(&result, &format, TRACE_COLUMNS);
            log::result_count(count, "spans in trace");
            Ok(())
        }

        ApmCmd::Summary {
            trace_id,
            from,
            to,
            format,
        } => {
            require_non_empty("Trace ID", &trace_id).map_err(DdError::Validation)?;
            let range = time::resolve_range(&from, &to, limits::MAX_SPAN_HOURS)
                .map_err(DdError::Validation)?;

            log::info(&format!("Generating summary for trace: {trace_id}"));

            let query = trace_id_query(&trace_id);
            let body = search_request_body(
                &query,
                &range.from,
                &range.to,
                1000,
                Some("timestamp"),
                None,
            );
            let result = client.post("/api/v2/spans/events/search", &body).await?;

            let spans = result
                .get("data")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();

            let span_count = spans.len();
            let mut services: Vec<String> = spans
                .iter()
                .filter_map(|s| {
                    s.get("attributes")
                        .and_then(|a| a.get("service"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect();
            services.sort();
            services.dedup();

            let error_count = spans
                .iter()
                .filter(|s| {
                    s.get("attributes")
                        .and_then(|a| a.get("status"))
                        .and_then(|v| v.as_str())
                        == Some("error")
                })
                .count();

            let summary = json!({
                "trace_id": trace_id,
                "span_count": span_count,
                "services": services,
                "service_count": services.len(),
                "error_count": error_count,
            });

            print_output(
                &summary,
                &format,
                &[
                    "trace_id",
                    "span_count",
                    "service_count",
                    "error_count",
                    "services",
                ],
            );
            Ok(())
        }

        ApmCmd::Compare {
            trace_a,
            trace_b,
            from,
            to,
            format,
        } => {
            require_non_empty("Trace A", &trace_a).map_err(DdError::Validation)?;
            require_non_empty("Trace B", &trace_b).map_err(DdError::Validation)?;
            let range = time::resolve_range(&from, &to, limits::MAX_SPAN_HOURS)
                .map_err(DdError::Validation)?;

            log::info(&format!("Comparing traces: {trace_a} vs {trace_b}"));

            let query_a = trace_id_query(&trace_a);
            let query_b = trace_id_query(&trace_b);
            let body_a = search_request_body(&query_a, &range.from, &range.to, 1000, None, None);
            let body_b = search_request_body(&query_b, &range.from, &range.to, 1000, None, None);

            let (result_a, result_b) = tokio::join!(
                client.post("/api/v2/spans/events/search", &body_a),
                client.post("/api/v2/spans/events/search", &body_b),
            );
            let result_a = result_a?;
            let result_b = result_b?;

            let count_a = result_a
                .get("data")
                .and_then(|d| d.as_array())
                .map_or(0, |a| a.len());
            let count_b = result_b
                .get("data")
                .and_then(|d| d.as_array())
                .map_or(0, |a| a.len());

            log::info(&format!(
                "Trace A: {count_a} spans, Trace B: {count_b} spans"
            ));

            let comparison = json!({
                "trace_a": { "trace_id": trace_a, "span_count": count_a, "data": result_a.get("data") },
                "trace_b": { "trace_id": trace_b, "span_count": count_b, "data": result_b.get("data") },
            });
            print_output(&comparison, &format, &["trace_a", "trace_b"]);
            Ok(())
        }

        ApmCmd::Metrics {
            query,
            from,
            to,
            compute,
            metric,
            group_by,
            format,
        } => {
            let range = time::resolve_range(&from, &to, limits::MAX_SPAN_HOURS)
                .map_err(DdError::Validation)?;

            log::info(&format!(
                "Analyzing trace metrics: compute={compute} metric={metric}"
            ));

            let metric_ref = normalize_metric(&metric);
            let compute_metric = (compute != "count").then_some(metric_ref.as_str());

            let groups: Vec<serde_json::Value> = group_by
                .iter()
                .map(|g| facet_group(g, 25, measure_sort(&compute, compute_metric)))
                .collect();

            let body = aggregate_request_body(
                &query,
                &range.from,
                &range.to,
                vec![aggregate_compute(&compute, compute_metric)],
                groups,
            );

            let result = client
                .post("/api/v2/spans/analytics/aggregate", &body)
                .await?;
            print_output(&result, &format, AGGREGATE_COLUMNS);
            Ok(())
        }

        ApmCmd::Tags {
            query,
            from,
            to,
            facet,
            format,
        } => {
            let range = time::resolve_range(&from, &to, limits::MAX_SPAN_HOURS)
                .map_err(DdError::Validation)?;

            log::info(&format!("Discovering tag values for: {facet}"));

            let body = aggregate_request_body(
                &query,
                &range.from,
                &range.to,
                vec![aggregate_compute("count", None)],
                vec![facet_group(&facet, 100, measure_sort("count", None))],
            );

            let result = client
                .post("/api/v2/spans/analytics/aggregate", &body)
                .await?;
            print_output(&result, &format, AGGREGATE_COLUMNS);
            Ok(())
        }

        ApmCmd::PrimaryTags { metric, format } => {
            log::info(&format!("Fetching primary tags for metric: {metric}"));
            let result = client
                .get(&format!("/api/v2/metrics/{metric}/all-tags"), &[])
                .await?;
            print_object(
                &result,
                &format,
                &["data.id", "data.type", "data.attributes.tags"],
            );
            Ok(())
        }

        ApmCmd::Watchdog {
            id,
            from,
            to,
            limit,
            format,
        } => {
            let query = match &id {
                Some(story_id) => {
                    format!("source:watchdog {story_id}")
                }
                None => "source:watchdog".to_string(),
            };
            events::search_with_result_label(
                client,
                EventsSearch {
                    query,
                    from,
                    to,
                    limit,
                    cursor: None,
                    format,
                },
                "watchdog stories",
            )
            .await
        }

        ApmCmd::Changes {
            query,
            from,
            to,
            limit,
            format,
        } => {
            events::search_with_result_label(
                client,
                EventsSearch {
                    query,
                    from,
                    to,
                    limit,
                    cursor: None,
                    format,
                },
                "change events",
            )
            .await
        }

        ApmCmd::Bottlenecks {
            query,
            from,
            to,
            group_by,
            format,
        } => {
            let range = time::resolve_range(&from, &to, limits::MAX_SPAN_HOURS)
                .map_err(DdError::Validation)?;

            log::info(&format!("Finding latency bottlenecks: group_by={group_by}"));

            let body = aggregate_request_body(
                &query,
                &range.from,
                &range.to,
                vec![
                    aggregate_compute("pc99", Some("@duration")),
                    aggregate_compute("count", None),
                ],
                vec![facet_group(
                    &group_by,
                    25,
                    measure_sort("pc99", Some("@duration")),
                )],
            );

            let result = client
                .post("/api/v2/spans/analytics/aggregate", &body)
                .await?;
            print_output(&result, &format, AGGREGATE_COLUMNS);
            Ok(())
        }

        ApmCmd::LatencyTags {
            query,
            from,
            to,
            tag,
            format,
        } => {
            let range = time::resolve_range(&from, &to, limits::MAX_SPAN_HOURS)
                .map_err(DdError::Validation)?;

            log::info(&format!("Analyzing latency by tag: {tag}"));

            let body = aggregate_request_body(
                &query,
                &range.from,
                &range.to,
                vec![
                    aggregate_compute("avg", Some("@duration")),
                    aggregate_compute("pc95", Some("@duration")),
                    aggregate_compute("count", None),
                ],
                vec![facet_group(
                    &tag,
                    50,
                    measure_sort("avg", Some("@duration")),
                )],
            );

            let result = client
                .post("/api/v2/spans/analytics/aggregate", &body)
                .await?;
            print_output(&result, &format, AGGREGATE_COLUMNS);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: ApmCmd,
    }

    #[test]
    fn test_parse_spans() {
        let cli = TestCli::parse_from(["test", "spans", "--query", "service:api"]);
        match cli.cmd {
            ApmCmd::Spans { query, limit, .. } => {
                assert_eq!(query, "service:api");
                assert_eq!(limit, 25);
            }
            _ => panic!("expected Spans"),
        }
    }

    #[test]
    fn test_parse_trace() {
        let cli = TestCli::parse_from(["test", "trace", "--trace-id", "abc123"]);
        match cli.cmd {
            ApmCmd::Trace { trace_id, from, .. } => {
                assert_eq!(trace_id, "abc123");
                assert_eq!(from, "6h");
            }
            _ => panic!("expected Trace"),
        }
    }

    #[test]
    fn test_parse_trace_rejects_empty_trace_id() {
        let err = TestCli::try_parse_from(["test", "trace", "--trace-id", ""])
            .err()
            .expect("expected clap validation error");
        assert!(err.to_string().contains("--trace-id"));
    }

    #[test]
    fn test_parse_summary() {
        let cli = TestCli::parse_from(["test", "summary", "--trace-id", "def456"]);
        match cli.cmd {
            ApmCmd::Summary { trace_id, .. } => assert_eq!(trace_id, "def456"),
            _ => panic!("expected Summary"),
        }
    }

    #[test]
    fn test_parse_compare() {
        let cli = TestCli::parse_from(["test", "compare", "--trace-a", "aaa", "--trace-b", "bbb"]);
        match cli.cmd {
            ApmCmd::Compare {
                trace_a, trace_b, ..
            } => {
                assert_eq!(trace_a, "aaa");
                assert_eq!(trace_b, "bbb");
            }
            _ => panic!("expected Compare"),
        }
    }

    #[test]
    fn test_parse_compare_rejects_empty_trace_id() {
        let err = TestCli::try_parse_from(["test", "compare", "--trace-a", "", "--trace-b", "bbb"])
            .err()
            .expect("expected clap validation error");
        assert!(err.to_string().contains("--trace-a"));
    }

    #[test]
    fn test_parse_metrics() {
        let cli = TestCli::parse_from([
            "test",
            "metrics",
            "--compute",
            "pc99",
            "--metric",
            "duration",
            "--group-by",
            "service",
        ]);
        match cli.cmd {
            ApmCmd::Metrics {
                compute,
                metric,
                group_by,
                ..
            } => {
                assert_eq!(compute, "pc99");
                assert_eq!(metric, "duration");
                assert_eq!(group_by, vec!["service"]);
            }
            _ => panic!("expected Metrics"),
        }
    }

    #[test]
    fn test_parse_tags() {
        let cli = TestCli::parse_from(["test", "tags", "--facet", "http.status_code"]);
        match cli.cmd {
            ApmCmd::Tags { facet, .. } => assert_eq!(facet, "http.status_code"),
            _ => panic!("expected Tags"),
        }
    }

    #[test]
    fn test_parse_watchdog() {
        let cli = TestCli::parse_from(["test", "watchdog", "--from", "12h"]);
        match cli.cmd {
            ApmCmd::Watchdog { id, from, .. } => {
                assert!(id.is_none());
                assert_eq!(from, "12h");
            }
            _ => panic!("expected Watchdog"),
        }
    }

    #[test]
    fn test_parse_bottlenecks() {
        let cli = TestCli::parse_from(["test", "bottlenecks", "--group-by", "service"]);
        match cli.cmd {
            ApmCmd::Bottlenecks { group_by, .. } => assert_eq!(group_by, "service"),
            _ => panic!("expected Bottlenecks"),
        }
    }

    #[test]
    fn test_parse_latency_tags() {
        let cli = TestCli::parse_from(["test", "latency-tags", "--tag", "region"]);
        match cli.cmd {
            ApmCmd::LatencyTags { tag, .. } => assert_eq!(tag, "region"),
            _ => panic!("expected LatencyTags"),
        }
    }

    #[test]
    fn test_parse_changes() {
        let cli = TestCli::parse_from(["test", "changes", "--from", "12h"]);
        match cli.cmd {
            ApmCmd::Changes { from, .. } => assert_eq!(from, "12h"),
            _ => panic!("expected Changes"),
        }
    }
}

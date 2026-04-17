use crate::client::DdClient;
use crate::commands::search_request_body;
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_output};
use crate::time;
use clap::Args;

const SPAN_COLUMNS: &[&str] = &[
    "attributes.service",
    "attributes.resource_name",
    "attributes.custom.duration",
    "attributes.status",
    "attributes.trace_id",
];

/// Search APM spans
///
/// Examples:
///   ddog spans --query "service:web @http.status_code:500" --from 1h
///   ddog spans --query "service:api @duration:>1000000000" --from 30m --format table
///   ddog spans --query "@error.message:*timeout*" --limit 50
#[derive(Args)]
pub struct SpansSearch {
    /// Span query using Datadog syntax (e.g., "service:web @http.status_code:500")
    #[arg(short, long, default_value = "*")]
    pub query: String,

    /// Start time. Max range: 24h
    #[arg(long, default_value = "1h")]
    pub from: String,

    /// End time — defaults to now
    #[arg(long)]
    pub to: Option<String>,

    /// Max results (1-1000)
    #[arg(short, long, default_value = "25", value_parser = clap::value_parser!(u32).range(1..))]
    pub limit: u32,

    /// Sort: "timestamp" or "-timestamp"
    #[arg(long, default_value = "-timestamp", value_parser = ["timestamp", "-timestamp"])]
    pub sort: String,

    /// Pagination cursor
    #[arg(long)]
    pub cursor: Option<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "json")]
    pub format: Format,
}

pub async fn search(client: &DdClient, args: SpansSearch) -> Result<(), DdError> {
    let range = time::resolve_range(&args.from, &args.to, limits::MAX_SPAN_HOURS)
        .map_err(DdError::Validation)?;
    let limit =
        limits::resolve_limit(args.limit, limits::MAX_SEARCH_LIMIT).map_err(DdError::Validation)?;

    log::info(&format!(
        "Searching spans: query=\"{}\" range={} limit={limit}",
        args.query,
        time::format_duration(range.duration_secs)
    ));

    let body = search_request_body(
        &args.query,
        &range.from,
        &range.to,
        limit,
        Some(&args.sort),
        args.cursor.as_deref(),
    );

    let result = client.post("/api/v2/spans/events/search", &body).await?;
    let count = print_output(&result, &args.format, SPAN_COLUMNS);
    log::result_count(count, "spans");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: SpansSearch,
    }

    #[test]
    fn test_parse_defaults() {
        let cli = TestCli::parse_from(["test"]);
        assert_eq!(cli.args.query, "*");
        assert_eq!(cli.args.from, "1h");
        assert_eq!(cli.args.limit, 25);
        assert_eq!(cli.args.sort, "-timestamp");
    }

    #[test]
    fn test_parse_custom() {
        let cli = TestCli::parse_from([
            "test",
            "--query",
            "service:web",
            "--from",
            "30m",
            "--limit",
            "100",
        ]);
        assert_eq!(cli.args.query, "service:web");
        assert_eq!(cli.args.limit, 100);
    }

    #[test]
    fn test_parse_rejects_invalid_sort() {
        let err = match TestCli::try_parse_from(["test", "--sort", "garbage-value"]) {
            Ok(_) => panic!("expected invalid sort to fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("possible values"));
    }

    #[test]
    fn test_parse_rejects_zero_limit() {
        let err = match TestCli::try_parse_from(["test", "--limit", "0"]) {
            Ok(_) => panic!("expected zero limit to fail"),
            Err(err) => err,
        };
        let msg = err.to_string();
        assert!(msg.contains("--limit"));
        assert!(msg.contains('0'));
    }
}

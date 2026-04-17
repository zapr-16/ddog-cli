use crate::client::DdClient;
use crate::commands::{require_non_empty, search_request_body, trace_id_query};
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_output};
use crate::time;
use clap::Args;

const TRACE_COLUMNS: &[&str] = &[
    "attributes.service",
    "attributes.resource_name",
    "attributes.custom.duration",
    "attributes.status",
    "attributes.span_id",
];

/// Get all spans for a trace by trace ID
///
/// Examples:
///   ddog traces --trace-id "1234567890abcdef"
///   ddog traces --trace-id "abc123" --from 6h --format table
///   ddog traces --trace-id "abc123" --limit 200
#[derive(Args)]
pub struct TracesGet {
    /// Trace ID to retrieve
    #[arg(short, long, value_parser = clap::builder::NonEmptyStringValueParser::new())]
    pub trace_id: String,

    /// Start time to search within. Max range: 24h
    #[arg(long, default_value = "6h")]
    pub from: String,

    /// End time — defaults to now
    #[arg(long)]
    pub to: Option<String>,

    /// Max spans to return (1-1000)
    #[arg(short, long, default_value = "100", value_parser = clap::value_parser!(u32).range(1..))]
    pub limit: u32,

    /// Output format
    #[arg(short, long, value_enum, default_value = "json")]
    pub format: Format,
}

pub async fn get(client: &DdClient, args: TracesGet) -> Result<(), DdError> {
    require_non_empty("Trace ID", &args.trace_id).map_err(DdError::Validation)?;
    let range = time::resolve_range(&args.from, &args.to, limits::MAX_SPAN_HOURS)
        .map_err(DdError::Validation)?;
    let limit =
        limits::resolve_limit(args.limit, limits::MAX_SEARCH_LIMIT).map_err(DdError::Validation)?;

    log::info(&format!(
        "Fetching trace: {} (range={}, limit={limit})",
        args.trace_id,
        time::format_duration(range.duration_secs)
    ));

    let query = trace_id_query(&args.trace_id);
    let body = search_request_body(
        &query,
        &range.from,
        &range.to,
        limit,
        Some("timestamp"),
        None,
    );

    let result = client.post("/api/v2/spans/events/search", &body).await?;
    let count = print_output(&result, &args.format, TRACE_COLUMNS);
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
        args: TracesGet,
    }

    #[test]
    fn test_parse_trace_id() {
        let cli = TestCli::parse_from(["test", "--trace-id", "abc123"]);
        assert_eq!(cli.args.trace_id, "abc123");
        assert_eq!(cli.args.from, "6h");
        assert_eq!(cli.args.limit, 100);
    }

    #[test]
    fn test_parse_rejects_empty_trace_id() {
        let err = TestCli::try_parse_from(["test", "--trace-id", ""])
            .err()
            .expect("expected clap validation error");
        assert!(err.to_string().contains("--trace-id"));
    }
}

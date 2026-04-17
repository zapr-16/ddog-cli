use crate::client::DdClient;
use crate::commands::event_search_body;
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_output};
use crate::time;
use clap::Args;

pub(crate) const EVENT_COLUMNS: &[&str] = &[
    "id",
    "attributes.timestamp",
    "attributes.message",
    "attributes.attributes.evt.name",
    "attributes.attributes.status",
];

/// Search events (monitor alerts, deploys, infrastructure changes)
///
/// Examples:
///   ddog events --query "source:deploy" --from 6h
///   ddog events --query "priority:normal" --from 1h --format table
///   ddog events --query "source:monitor status:alert" --from 24h
#[derive(Args)]
pub struct EventsSearch {
    /// Event query (e.g., "source:deploy", "priority:normal", "status:alert")
    #[arg(short, long, default_value = "*")]
    pub query: String,

    /// Start time. Max range: 48h
    #[arg(long, default_value = "1h")]
    pub from: String,

    /// End time — defaults to now
    #[arg(long)]
    pub to: Option<String>,

    /// Max results (1-1000)
    #[arg(short, long, default_value = "25", value_parser = clap::value_parser!(u32).range(1..))]
    pub limit: u32,

    /// Pagination cursor from previous response
    #[arg(long)]
    pub cursor: Option<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "json")]
    pub format: Format,
}

pub async fn search(client: &DdClient, args: EventsSearch) -> Result<(), DdError> {
    search_with_result_label(client, args, "events").await
}

pub(crate) async fn search_with_result_label(
    client: &DdClient,
    args: EventsSearch,
    result_label: &str,
) -> Result<(), DdError> {
    let range = time::resolve_range(&args.from, &args.to, limits::MAX_EVENT_HOURS)
        .map_err(DdError::Validation)?;
    let limit =
        limits::resolve_limit(args.limit, limits::MAX_SEARCH_LIMIT).map_err(DdError::Validation)?;

    log::info(&format!(
        "Searching {result_label}: query=\"{}\" range={} limit={limit}",
        args.query,
        time::format_duration(range.duration_secs)
    ));

    let body = event_search_body(
        &args.query,
        &range.from,
        &range.to,
        limit,
        args.cursor.as_deref(),
    );

    let result = client.post("/api/v2/events/search", &body).await?;
    let count = print_output(&result, &args.format, EVENT_COLUMNS);
    log::result_count(count, result_label);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: EventsSearch,
    }

    #[test]
    fn test_parse_defaults() {
        let cli = TestCli::parse_from(["test"]);
        assert_eq!(cli.args.query, "*");
        assert_eq!(cli.args.from, "1h");
        assert_eq!(cli.args.limit, 25);
    }

    #[test]
    fn test_parse_custom() {
        let cli = TestCli::parse_from([
            "test",
            "--query",
            "source:deploy",
            "--from",
            "6h",
            "--limit",
            "50",
        ]);
        assert_eq!(cli.args.query, "source:deploy");
        assert_eq!(cli.args.limit, 50);
    }
}

use crate::client::DdClient;
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_output};
use clap::Args;

/// Search monitors by status, type, or tags
///
/// Examples:
///   ddog monitors --query "status:alert"
///   ddog monitors --query "type:metric tag:env:prod"
///   ddog monitors --query "status:warn" --per-page 50 --format table
#[derive(Args)]
pub struct MonitorsSearch {
    /// Monitor search query (e.g., "status:alert", "type:metric tag:env:prod")
    #[arg(short, long, default_value = "")]
    pub query: String,

    /// Page number (0-indexed)
    #[arg(long, default_value = "0")]
    pub page: u32,

    /// Results per page (max 100)
    #[arg(long, default_value = "25", value_parser = clap::value_parser!(u32).range(1..))]
    pub per_page: u32,

    /// Sort field (e.g., "name", "status", "type")
    #[arg(long)]
    pub sort: Option<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "json")]
    pub format: Format,
}

pub async fn search(client: &DdClient, args: MonitorsSearch) -> Result<(), DdError> {
    let requested_per_page =
        limits::require_min("Per page", args.per_page, 1).map_err(DdError::Validation)?;
    let per_page = requested_per_page.min(100);
    log::info(&format!(
        "Searching monitors: query=\"{}\" page={} per_page={per_page}",
        args.query, args.page
    ));

    let page_str = args.page.to_string();
    let per_page_str = per_page.to_string();
    let mut params: Vec<(&str, &str)> = vec![
        ("query", &args.query),
        ("page", &page_str),
        ("per_page", &per_page_str),
    ];
    let sort_val;
    if let Some(s) = &args.sort {
        sort_val = s.clone();
        params.push(("sort", &sort_val));
    }

    let result = client.get("/api/v1/monitor/search", &params).await?;
    let count = print_output(
        &result,
        &args.format,
        &["id", "name", "type", "overall_state", "query"],
    );
    log::result_count(count, "monitors");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: MonitorsSearch,
    }

    #[test]
    fn test_parse_defaults() {
        let cli = TestCli::parse_from(["test"]);
        assert_eq!(cli.args.query, "");
        assert_eq!(cli.args.page, 0);
        assert_eq!(cli.args.per_page, 25);
    }

    #[test]
    fn test_parse_alert_query() {
        let cli = TestCli::parse_from(["test", "--query", "status:alert"]);
        assert_eq!(cli.args.query, "status:alert");
    }

    #[test]
    fn test_parse_rejects_zero_per_page() {
        let err = TestCli::try_parse_from(["test", "--per-page", "0"])
            .err()
            .expect("expected clap validation error");
        assert!(err.to_string().contains("0"));
    }
}

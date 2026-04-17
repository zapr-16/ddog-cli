use crate::client::DdClient;
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_output};
use clap::Args;
use serde_json::json;

/// Search dashboards
///
/// Examples:
///   ddog dashboards
///   ddog dashboards --filter "production"
///   ddog dashboards --filter "api" --format table
#[derive(Args)]
pub struct DashboardsSearch {
    /// Filter dashboards by title (client-side substring match, pages through all results)
    #[arg(long)]
    pub filter: Option<String>,

    /// Max dashboards to return (max 500)
    #[arg(short, long, default_value = "100", value_parser = clap::value_parser!(u32).range(1..))]
    pub count: u32,

    /// Output format
    #[arg(short, long, value_enum, default_value = "json")]
    pub format: Format,
}

const PAGE_SIZE: u32 = 100;

pub async fn search(client: &DdClient, args: DashboardsSearch) -> Result<(), DdError> {
    let requested_count =
        limits::require_min("Count", args.count, 1).map_err(DdError::Validation)?;
    let max_results = requested_count.min(limits::MAX_DASHBOARDS);
    log::info(&format!(
        "Searching dashboards: filter={} max={max_results}",
        args.filter.as_deref().unwrap_or("(all)")
    ));

    if args.filter.is_some() {
        // Page through all dashboards to find matches
        let filter_lower = args
            .filter
            .as_ref()
            .map(|f| f.to_lowercase())
            .unwrap_or_default();
        let mut matched = Vec::new();
        let mut offset: u32 = 0;

        loop {
            let offset_str = offset.to_string();
            let page_str = PAGE_SIZE.to_string();
            let params: Vec<(&str, &str)> = vec![("start", &offset_str), ("count", &page_str)];

            let result = client.get("/api/v1/dashboard", &params).await?;
            let page_dashboards = result
                .get("dashboards")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();

            let page_len = page_dashboards.len() as u32;

            for d in page_dashboards {
                if d.get("title")
                    .and_then(|t| t.as_str())
                    .is_some_and(|t| t.to_lowercase().contains(&filter_lower))
                {
                    matched.push(d);
                    if matched.len() as u32 >= max_results {
                        break;
                    }
                }
            }

            if matched.len() as u32 >= max_results || page_len < PAGE_SIZE {
                break;
            }
            offset += page_len;
        }

        let result = json!({ "dashboards": matched });
        let n = print_output(
            &result,
            &args.format,
            &["id", "title", "description", "layout_type", "url"],
        );
        log::result_count(n, "dashboards");
    } else {
        // No filter: single page fetch
        let count_str = max_results.to_string();
        let params: Vec<(&str, &str)> = vec![("count", &count_str)];
        let result = client.get("/api/v1/dashboard", &params).await?;
        let n = print_output(
            &result,
            &args.format,
            &["id", "title", "description", "layout_type", "url"],
        );
        log::result_count(n, "dashboards");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: DashboardsSearch,
    }

    #[test]
    fn test_parse_defaults() {
        let cli = TestCli::parse_from(["test"]);
        assert!(cli.args.filter.is_none());
        assert_eq!(cli.args.count, 100);
    }

    #[test]
    fn test_parse_count() {
        let cli = TestCli::parse_from(["test", "--count", "50"]);
        assert_eq!(cli.args.count, 50);
    }

    #[test]
    fn test_parse_with_filter() {
        let cli = TestCli::parse_from(["test", "--filter", "prod"]);
        assert_eq!(cli.args.filter.unwrap(), "prod");
    }

    #[test]
    fn test_parse_rejects_zero_count() {
        let err = TestCli::try_parse_from(["test", "--count", "0"])
            .err()
            .expect("expected clap validation error");
        assert!(err.to_string().contains("0"));
    }
}

use crate::client::DdClient;
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_output};
use clap::Args;

/// Search monitored hosts
///
/// Examples:
///   ddog hosts
///   ddog hosts --filter "web"
///   ddog hosts --filter "env:production" --count 50 --format table
///   ddog hosts --sort-field cpu --sort-dir desc
#[derive(Args)]
pub struct HostsSearch {
    /// Filter by host name, alias, or tag
    #[arg(long, default_value = "")]
    pub filter: String,

    /// Sort field: "apps", "cpu", "iowait", "load"
    #[arg(long, value_parser = ["apps", "cpu", "iowait", "load"])]
    pub sort_field: Option<String>,

    /// Sort direction: "asc" or "desc"
    #[arg(long, value_parser = ["asc", "desc"])]
    pub sort_dir: Option<String>,

    /// Offset for pagination
    #[arg(long, default_value = "0")]
    pub start: u32,

    /// Max hosts to return (max 1000)
    #[arg(short, long, default_value = "100", value_parser = clap::value_parser!(u32).range(1..))]
    pub count: u32,

    /// Include muted hosts data
    #[arg(long, default_value = "false")]
    pub include_muted: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value = "json")]
    pub format: Format,
}

pub async fn search(client: &DdClient, args: HostsSearch) -> Result<(), DdError> {
    let requested_count =
        limits::require_min("Count", args.count, 1).map_err(DdError::Validation)?;
    let count = requested_count.min(limits::MAX_HOSTS);
    let filter_display = if args.filter.is_empty() {
        "(all)".to_string()
    } else {
        format!("\"{}\"", args.filter)
    };
    log::info(&format!(
        "Searching hosts: filter={filter_display} count={count}"
    ));

    let start_str = args.start.to_string();
    let count_str = count.to_string();
    let muted_str = args.include_muted.to_string();

    let mut params: Vec<(&str, &str)> = vec![
        ("filter", &args.filter),
        ("start", &start_str),
        ("count", &count_str),
        ("include_muted_hosts_data", &muted_str),
    ];

    let sort_field_val;
    if let Some(sf) = &args.sort_field {
        sort_field_val = sf.clone();
        params.push(("sort_field", &sort_field_val));
    }
    let sort_dir_val;
    if let Some(sd) = &args.sort_dir {
        sort_dir_val = sd.clone();
        params.push(("sort_dir", &sort_dir_val));
    }

    let result = client.get("/api/v1/hosts", &params).await?;
    let n = print_output(
        &result,
        &args.format,
        &["name", "aliases", "apps", "up", "meta.platform"],
    );
    log::result_count(n, "hosts");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: HostsSearch,
    }

    #[test]
    fn test_parse_defaults() {
        let cli = TestCli::parse_from(["test"]);
        assert_eq!(cli.args.filter, "");
        assert_eq!(cli.args.count, 100);
        assert!(!cli.args.include_muted);
    }

    #[test]
    fn test_parse_with_filter() {
        let cli = TestCli::parse_from(["test", "--filter", "web", "--count", "50"]);
        assert_eq!(cli.args.filter, "web");
        assert_eq!(cli.args.count, 50);
    }

    #[test]
    fn test_parse_sort() {
        let cli = TestCli::parse_from(["test", "--sort-field", "cpu", "--sort-dir", "desc"]);
        assert_eq!(cli.args.sort_field.unwrap(), "cpu");
        assert_eq!(cli.args.sort_dir.unwrap(), "desc");
    }

    #[test]
    fn test_parse_rejects_zero_count() {
        let err = TestCli::try_parse_from(["test", "--count", "0"])
            .err()
            .expect("expected clap validation error");
        assert!(err.to_string().contains("0"));
    }

    #[test]
    fn test_parse_rejects_invalid_sort_field() {
        let err = TestCli::try_parse_from(["test", "--sort-field", "name"])
            .err()
            .expect("expected clap validation error");
        assert!(err.to_string().contains("name"));
    }
}

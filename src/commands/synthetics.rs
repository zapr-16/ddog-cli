use clap::Subcommand;
use serde_json::Value;

use crate::client::DdClient;
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_output};
use crate::time;

#[derive(Subcommand)]
#[command(verbatim_doc_comment)]
pub enum SyntheticsCmd {
    /// List synthetic tests
    ///
    /// Examples:
    ///   ddog synthetics list
    ///   ddog synthetics list --page-size 50
    ///   ddog synthetics list --page-number 2 --format table
    List {
        /// Page size (default 25)
        #[arg(long, default_value = "25")]
        page_size: u32,

        /// Page number (default 0)
        #[arg(long, default_value = "0")]
        page_number: u32,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Get results for a synthetic test
    ///
    /// Examples:
    ///   ddog synthetics results --id abc-xyz-123 --from 6h
    ///   ddog synthetics results --id abc-xyz-123 --from 24h --format table
    ///   ddog synthetics results --id abc-xyz-123 --from 2h --to 1h
    Results {
        /// Synthetic test public ID
        #[arg(short, long)]
        id: String,

        /// Start time. Max range: 48h
        #[arg(long, default_value = "1h")]
        from: String,

        /// End time — defaults to now
        #[arg(long)]
        to: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },
}

pub async fn run(client: &DdClient, cmd: SyntheticsCmd) -> Result<(), DdError> {
    match cmd {
        SyntheticsCmd::List {
            page_size,
            page_number,
            format,
        } => {
            log::info(&format!(
                "Listing synthetic tests: page={page_number} size={page_size}"
            ));

            let page_size_str = page_size.to_string();
            let page_number_str = page_number.to_string();

            let params: Vec<(&str, &str)> = vec![
                ("page_size", &page_size_str),
                ("page_number", &page_number_str),
            ];

            let result = client.get("/api/v1/synthetics/tests", &params).await?;

            // Datadog returns { "tests": [...] } — extract the array for print_output
            let tests = result.get("tests").cloned().unwrap_or(Value::Array(vec![]));

            let count = print_output(
                &tests,
                &format,
                &["public_id", "name", "type", "status", "locations"],
            );
            log::result_count(count, "synthetic tests");
            Ok(())
        }
        SyntheticsCmd::Results {
            id,
            from,
            to,
            format,
        } => {
            let (from_epoch, to_epoch) =
                time::resolve_range_epoch(&from, &to, limits::MAX_SYNTHETICS_HOURS)
                    .map_err(DdError::Validation)?;

            log::info(&format!("Fetching results for test: {id}"));

            let from_str = from_epoch.to_string();
            let to_str = to_epoch.to_string();

            let params: Vec<(&str, &str)> = vec![("from_ts", &from_str), ("to_ts", &to_str)];

            let result = client
                .get(&format!("/api/v1/synthetics/tests/{id}/results"), &params)
                .await?;

            // Datadog returns { "results": [...] } — extract the array for print_output
            let results = result
                .get("results")
                .cloned()
                .unwrap_or(Value::Array(vec![]));

            let count = print_output(
                &results,
                &format,
                &["result_id", "status", "check_time", "dc_id"],
            );
            log::result_count(count, "results");
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
        cmd: SyntheticsCmd,
    }

    #[test]
    fn test_parse_list_defaults() {
        let cli = TestCli::parse_from(["test", "list"]);
        match cli.cmd {
            SyntheticsCmd::List {
                page_size,
                page_number,
                ..
            } => {
                assert_eq!(page_size, 25);
                assert_eq!(page_number, 0);
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_parse_list_custom() {
        let cli = TestCli::parse_from(["test", "list", "--page-size", "50", "--page-number", "3"]);
        match cli.cmd {
            SyntheticsCmd::List {
                page_size,
                page_number,
                ..
            } => {
                assert_eq!(page_size, 50);
                assert_eq!(page_number, 3);
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_parse_results() {
        let cli = TestCli::parse_from(["test", "results", "--id", "abc-xyz-123", "--from", "6h"]);
        match cli.cmd {
            SyntheticsCmd::Results { id, from, to, .. } => {
                assert_eq!(id, "abc-xyz-123");
                assert_eq!(from, "6h");
                assert!(to.is_none());
            }
            _ => panic!("expected Results"),
        }
    }

    #[test]
    fn test_parse_results_with_to() {
        let cli = TestCli::parse_from([
            "test", "results", "--id", "test-123", "--from", "2h", "--to", "1h",
        ]);
        match cli.cmd {
            SyntheticsCmd::Results { id, from, to, .. } => {
                assert_eq!(id, "test-123");
                assert_eq!(from, "2h");
                assert_eq!(to.unwrap(), "1h");
            }
            _ => panic!("expected Results"),
        }
    }
}

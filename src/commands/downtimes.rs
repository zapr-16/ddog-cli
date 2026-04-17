use clap::Subcommand;

use crate::client::DdClient;
use crate::error::DdError;
use crate::log;
use crate::output::{Format, print_object, print_output};

#[derive(Subcommand)]
#[command(verbatim_doc_comment)]
pub enum DowntimesCmd {
    /// List scheduled downtimes
    ///
    /// Examples:
    ///   ddog downtimes list
    ///   ddog downtimes list --current-only false --limit 50
    ///   ddog downtimes list --format table
    List {
        /// Show only currently active downtimes
        #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
        current_only: bool,

        /// Page size
        #[arg(short, long, default_value = "25")]
        limit: u32,

        /// Page offset
        #[arg(long, default_value = "0")]
        offset: u32,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Get downtime details by ID
    ///
    /// Examples:
    ///   ddog downtimes get --id abc123
    ///   ddog downtimes get --id abc123 --include "created_by,monitor"
    ///   ddog downtimes get --id abc123 --format table
    Get {
        /// Downtime ID
        #[arg(short, long)]
        id: String,

        /// Include related resources (comma-separated, e.g., "created_by,monitor")
        #[arg(long)]
        include: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },
}

pub async fn run(client: &DdClient, cmd: DowntimesCmd) -> Result<(), DdError> {
    match cmd {
        DowntimesCmd::List {
            current_only,
            limit,
            offset,
            format,
        } => {
            log::info(&format!(
                "Listing downtimes: current_only={current_only} limit={limit}"
            ));

            let current_only_str = current_only.to_string();
            let limit_str = limit.to_string();
            let offset_str = offset.to_string();

            let params: Vec<(&str, &str)> = vec![
                ("current_only", &current_only_str),
                ("page[limit]", &limit_str),
                ("page[offset]", &offset_str),
            ];

            let result = client.get("/api/v2/downtime", &params).await?;
            let count = print_output(
                &result,
                &format,
                &[
                    "id",
                    "attributes.display_name",
                    "attributes.status",
                    "attributes.scope",
                    "attributes.schedule.start",
                ],
            );
            log::result_count(count, "downtimes");
            Ok(())
        }
        DowntimesCmd::Get {
            id,
            include,
            format,
        } => {
            log::info(&format!("Fetching downtime: {id}"));
            let mut params: Vec<(&str, &str)> = Vec::new();
            let inc_val;
            if let Some(inc) = &include {
                inc_val = inc.clone();
                params.push(("include", &inc_val));
            }

            let result = client
                .get(&format!("/api/v2/downtime/{id}"), &params)
                .await?;
            print_object(
                &result,
                &format,
                &[
                    "data.id",
                    "data.attributes.display_name",
                    "data.attributes.status",
                    "data.attributes.scope",
                    "data.attributes.schedule",
                ],
            );
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
        cmd: DowntimesCmd,
    }

    #[test]
    fn test_parse_list_defaults() {
        let cli = TestCli::parse_from(["test", "list"]);
        match cli.cmd {
            DowntimesCmd::List {
                current_only,
                limit,
                offset,
                ..
            } => {
                assert!(current_only);
                assert_eq!(limit, 25);
                assert_eq!(offset, 0);
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_parse_get() {
        let cli = TestCli::parse_from(["test", "get", "--id", "abc123"]);
        match cli.cmd {
            DowntimesCmd::Get { id, include, .. } => {
                assert_eq!(id, "abc123");
                assert!(include.is_none());
            }
            _ => panic!("expected Get"),
        }
    }

    #[test]
    fn test_parse_list_all() {
        let cli = TestCli::parse_from(["test", "list", "--current-only", "false", "--limit", "50"]);
        match cli.cmd {
            DowntimesCmd::List {
                current_only,
                limit,
                ..
            } => {
                assert!(!current_only);
                assert_eq!(limit, 50);
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_parse_get_with_include() {
        let cli = TestCli::parse_from([
            "test",
            "get",
            "--id",
            "dt-789",
            "--include",
            "created_by,monitor",
        ]);
        match cli.cmd {
            DowntimesCmd::Get { id, include, .. } => {
                assert_eq!(id, "dt-789");
                assert_eq!(include.unwrap(), "created_by,monitor");
            }
            _ => panic!("expected Get"),
        }
    }
}

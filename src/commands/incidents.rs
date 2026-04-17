use clap::Subcommand;

use crate::client::DdClient;
use crate::error::DdError;
use crate::log;
use crate::output::{Format, print_object, print_output};

#[derive(Subcommand)]
#[command(verbatim_doc_comment)]
pub enum IncidentsCmd {
    /// Get incident details by ID
    ///
    /// Examples:
    ///   ddog incidents get --id "abc123"
    ///   ddog incidents get --id "abc123" --format table
    Get {
        /// Incident ID
        #[arg(short, long)]
        id: String,

        /// Include related resources (comma-separated, e.g., "users,attachments")
        #[arg(long)]
        include: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Search incidents by query
    ///
    /// Examples:
    ///   ddog incidents search --query "state:active"
    ///   ddog incidents search --query "severity:SEV-1 state:active"
    ///   ddog incidents search --query "state:resolved" --limit 50 --format table
    Search {
        /// Search query (e.g., "state:active", "severity:SEV-1")
        #[arg(short, long, default_value = "")]
        query: String,

        /// Sort field (prefix "-" for descending, e.g., "-created")
        #[arg(long, default_value = "-created")]
        sort: String,

        /// Page size (max 100)
        #[arg(short, long, default_value = "25")]
        limit: u32,

        /// Page offset
        #[arg(long, default_value = "0")]
        offset: u32,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },
}

pub async fn run(client: &DdClient, cmd: IncidentsCmd) -> Result<(), DdError> {
    match cmd {
        IncidentsCmd::Get {
            id,
            include,
            format,
        } => {
            log::info(&format!("Fetching incident: {id}"));
            let mut params: Vec<(&str, &str)> = Vec::new();
            let inc_val;
            if let Some(inc) = &include {
                inc_val = inc.clone();
                params.push(("include", &inc_val));
            }

            let result = client
                .get(&format!("/api/v2/incidents/{id}"), &params)
                .await?;
            print_object(
                &result,
                &format,
                &[
                    "data.id",
                    "data.attributes.title",
                    "data.attributes.severity",
                    "data.attributes.state",
                    "data.attributes.created",
                ],
            );
            Ok(())
        }
        IncidentsCmd::Search {
            query,
            sort,
            limit,
            offset,
            format,
        } => {
            let limit = limit.min(100);
            log::info(&format!(
                "Searching incidents: query=\"{query}\" limit={limit}"
            ));

            let limit_str = limit.to_string();
            let offset_str = offset.to_string();

            let params: Vec<(&str, &str)> = vec![
                ("query", &query),
                ("sort", &sort),
                ("page[size]", &limit_str),
                ("page[offset]", &offset_str),
            ];

            let result = client.get("/api/v2/incidents/search", &params).await?;
            let count = print_output(
                &result,
                &format,
                &[
                    "id",
                    "attributes.title",
                    "attributes.severity",
                    "attributes.state",
                ],
            );
            log::result_count(count, "incidents");
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
        cmd: IncidentsCmd,
    }

    #[test]
    fn test_parse_get() {
        let cli = TestCli::parse_from(["test", "get", "--id", "abc123"]);
        match cli.cmd {
            IncidentsCmd::Get { id, .. } => assert_eq!(id, "abc123"),
            _ => panic!("expected Get"),
        }
    }

    #[test]
    fn test_parse_search() {
        let cli = TestCli::parse_from(["test", "search", "--query", "state:active"]);
        match cli.cmd {
            IncidentsCmd::Search { query, limit, .. } => {
                assert_eq!(query, "state:active");
                assert_eq!(limit, 25);
            }
            _ => panic!("expected Search"),
        }
    }
}

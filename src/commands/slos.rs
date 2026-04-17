use clap::Subcommand;

use crate::client::DdClient;
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_object, print_output};
use crate::time;

#[derive(Subcommand)]
#[command(verbatim_doc_comment)]
pub enum SlosCmd {
    /// Search SLOs by query
    ///
    /// Examples:
    ///   ddog slos search
    ///   ddog slos search --query "service:web"
    ///   ddog slos search --ids "abc123,def456"
    ///   ddog slos search --query "env:prod" --limit 50 --format table
    Search {
        /// Filter by SLO IDs (comma-separated)
        #[arg(long)]
        ids: Option<String>,

        /// Search query (e.g., "service:web", "env:prod")
        #[arg(short, long, default_value = "")]
        query: String,

        /// Max results (1-1000)
        #[arg(short, long, default_value = "25", value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,

        /// Result offset for pagination
        #[arg(long, default_value = "0")]
        offset: u32,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Get SLO details by ID
    ///
    /// Examples:
    ///   ddog slos get --id "abc123def456"
    ///   ddog slos get --id "abc123def456" --format table
    Get {
        /// SLO ID
        #[arg(short, long)]
        id: String,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Get SLO history (status over time)
    ///
    /// Examples:
    ///   ddog slos history --id "abc123def456" --from 7d
    ///   ddog slos history --id "abc123def456" --from 30d --to "2026-03-01T00:00:00Z"
    ///   ddog slos history --id "abc123def456" --from 90d --format table
    History {
        /// SLO ID
        #[arg(short, long)]
        id: String,

        /// Start time. Max range: 90 days
        #[arg(long, default_value = "7d")]
        from: String,

        /// End time — defaults to now
        #[arg(long)]
        to: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },
}

pub async fn run(client: &DdClient, cmd: SlosCmd) -> Result<(), DdError> {
    match cmd {
        SlosCmd::Search {
            ids,
            query,
            limit,
            offset,
            format,
        } => {
            let limit = limits::resolve_limit(limit, limits::MAX_SEARCH_LIMIT)
                .map_err(DdError::Validation)?;
            log::info(&format!(
                "Searching SLOs: query=\"{query}\" limit={limit} offset={offset}"
            ));

            let limit_str = limit.to_string();
            let offset_str = offset.to_string();
            let mut params: Vec<(&str, &str)> = vec![
                ("query", &query),
                ("limit", &limit_str),
                ("offset", &offset_str),
            ];
            let ids_val;
            if let Some(ids_ref) = &ids {
                ids_val = ids_ref.clone();
                params.push(("ids", &ids_val));
            }

            let result = client.get("/api/v1/slo", &params).await?;
            let count = print_output(
                &result,
                &format,
                &[
                    "id",
                    "name",
                    "type",
                    "overall_status.0.state",
                    "thresholds.0.target",
                ],
            );
            log::result_count(count, "SLOs");
            Ok(())
        }
        SlosCmd::Get { id, format } => {
            log::info(&format!("Fetching SLO: {id}"));
            let result = client.get(&format!("/api/v1/slo/{id}"), &[]).await?;
            print_object(
                &result,
                &format,
                &["data.id", "data.name", "data.type", "data.thresholds"],
            );
            Ok(())
        }
        SlosCmd::History {
            id,
            from,
            to,
            format,
        } => {
            let (from_epoch, to_epoch) =
                time::resolve_range_epoch(&from, &to, limits::MAX_SLO_HOURS)
                    .map_err(DdError::Validation)?;

            log::info(&format!(
                "Fetching SLO history: id=\"{id}\" range={}",
                time::format_duration(to_epoch - from_epoch)
            ));

            let from_str = from_epoch.to_string();
            let to_str = to_epoch.to_string();
            let result = client
                .get(
                    &format!("/api/v1/slo/{id}/history"),
                    &[("from_ts", &from_str), ("to_ts", &to_str)],
                )
                .await?;
            print_object(
                &result,
                &format,
                &[
                    "data.overall.sli_value",
                    "data.overall.span_precision",
                    "data.overall.name",
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
        cmd: SlosCmd,
    }

    #[test]
    fn test_parse_search_defaults() {
        let cli = TestCli::parse_from(["test", "search"]);
        match cli.cmd {
            SlosCmd::Search {
                query,
                limit,
                offset,
                ids,
                ..
            } => {
                assert_eq!(query, "");
                assert_eq!(limit, 25);
                assert_eq!(offset, 0);
                assert!(ids.is_none());
            }
            _ => panic!("expected Search"),
        }
    }

    #[test]
    fn test_parse_search_with_args() {
        let cli = TestCli::parse_from([
            "test",
            "search",
            "--query",
            "service:web",
            "--limit",
            "50",
            "--ids",
            "a,b,c",
        ]);
        match cli.cmd {
            SlosCmd::Search {
                query, limit, ids, ..
            } => {
                assert_eq!(query, "service:web");
                assert_eq!(limit, 50);
                assert_eq!(ids.as_deref(), Some("a,b,c"));
            }
            _ => panic!("expected Search"),
        }
    }

    #[test]
    fn test_parse_get() {
        let cli = TestCli::parse_from(["test", "get", "--id", "abc123def456"]);
        match cli.cmd {
            SlosCmd::Get { id, .. } => assert_eq!(id, "abc123def456"),
            _ => panic!("expected Get"),
        }
    }

    #[test]
    fn test_parse_history() {
        let cli = TestCli::parse_from(["test", "history", "--id", "abc123", "--from", "30d"]);
        match cli.cmd {
            SlosCmd::History { id, from, to, .. } => {
                assert_eq!(id, "abc123");
                assert_eq!(from, "30d");
                assert!(to.is_none());
            }
            _ => panic!("expected History"),
        }
    }

    #[test]
    fn test_parse_history_with_to() {
        let cli = TestCli::parse_from([
            "test",
            "history",
            "--id",
            "abc123",
            "--from",
            "7d",
            "--to",
            "2026-03-01T00:00:00Z",
        ]);
        match cli.cmd {
            SlosCmd::History { id, from, to, .. } => {
                assert_eq!(id, "abc123");
                assert_eq!(from, "7d");
                assert_eq!(to.as_deref(), Some("2026-03-01T00:00:00Z"));
            }
            _ => panic!("expected History"),
        }
    }
}

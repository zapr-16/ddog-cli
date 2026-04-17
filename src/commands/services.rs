use crate::client::DdClient;
use crate::error::DdError;
use crate::limits;
use crate::log;
use crate::output::{Format, print_output};
use clap::Subcommand;

#[derive(Subcommand)]
#[command(verbatim_doc_comment)]
pub enum ServicesCmd {
    /// Search service catalog definitions
    ///
    /// Examples:
    ///   ddog services search
    ///   ddog services search --limit 50 --format table
    ///   ddog services search --schema v2.2
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog services search\n  ddog services search --limit 50 --format table\n  ddog services search --schema v2.2"
    )]
    Search {
        /// Page size
        #[arg(short, long, default_value = "20", value_parser = clap::value_parser!(u32).range(1..))]
        limit: u32,

        /// Page number (1-indexed)
        #[arg(long, default_value = "1", value_parser = clap::value_parser!(u32).range(1..))]
        page: u32,

        /// Schema version (v2, v2.1, v2.2)
        #[arg(long, default_value = "v2.1")]
        schema: String,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Get upstream/downstream service dependencies
    ///
    /// Examples:
    ///   ddog services deps --env production
    ///   ddog services deps --env staging --format table
    #[command(
        long_about = None,
        next_line_help = false,
        after_help = "Examples:\n  ddog services deps --env production\n  ddog services deps --env staging --format table"
    )]
    Deps {
        /// Environment name (required)
        #[arg(short, long)]
        env: String,

        /// Primary tag (optional)
        #[arg(long)]
        primary_tag: Option<String>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },
}

pub async fn run(client: &DdClient, cmd: ServicesCmd) -> Result<(), DdError> {
    match cmd {
        ServicesCmd::Search {
            limit,
            page,
            schema,
            format,
        } => {
            let limit = limits::require_min("Limit", limit, 1).map_err(DdError::Validation)?;
            let page = limits::require_min("Page", page, 1).map_err(DdError::Validation)?;
            log::info(&format!(
                "Searching service catalog: page={page} limit={limit}"
            ));
            let limit_str = limit.to_string();
            let page_str = page.to_string();

            let params: Vec<(&str, &str)> = vec![
                ("page[size]", &limit_str),
                ("page[number]", &page_str),
                ("schema_version", &schema),
            ];

            let result = client.get("/api/v2/services/definitions", &params).await?;
            let count = print_output(
                &result,
                &format,
                &[
                    "attributes.schema.dd-service",
                    "attributes.schema.team",
                    "attributes.schema.description",
                ],
            );
            log::result_count(count, "services");
            Ok(())
        }
        ServicesCmd::Deps {
            env,
            primary_tag,
            format,
        } => {
            log::info(&format!("Fetching service dependencies: env={env}"));
            let mut params: Vec<(&str, &str)> = vec![("env", &env)];
            let ptag;
            if let Some(pt) = &primary_tag {
                ptag = pt.clone();
                params.push(("primary_tag", &ptag));
            }

            let result = client.get("/api/v1/service_dependencies", &params).await?;
            print_output(&result, &format, &["service_name", "dependencies"]);
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
        cmd: ServicesCmd,
    }

    #[test]
    fn test_parse_search() {
        let cli = TestCli::parse_from(["test", "search"]);
        match cli.cmd {
            ServicesCmd::Search { limit, page, .. } => {
                assert_eq!(limit, 20);
                assert_eq!(page, 1);
            }
            _ => panic!("expected Search"),
        }
    }

    #[test]
    fn test_parse_search_rejects_zero_limit() {
        let err = TestCli::try_parse_from(["test", "search", "--limit", "0"])
            .err()
            .expect("expected clap validation error");
        assert!(err.to_string().contains("0"));
    }

    #[test]
    fn test_parse_search_rejects_zero_page() {
        let err = TestCli::try_parse_from(["test", "search", "--page", "0"])
            .err()
            .expect("expected clap validation error");
        assert!(err.to_string().contains("0"));
    }

    #[test]
    fn test_parse_deps() {
        let cli = TestCli::parse_from(["test", "deps", "--env", "production"]);
        match cli.cmd {
            ServicesCmd::Deps { env, .. } => {
                assert_eq!(env, "production");
            }
            _ => panic!("expected Deps"),
        }
    }
}

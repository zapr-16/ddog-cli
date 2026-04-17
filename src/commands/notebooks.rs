use clap::Subcommand;

use crate::client::DdClient;
use crate::error::DdError;
use crate::log;
use crate::output::{Format, print_object, print_output};

#[derive(Subcommand)]
#[command(verbatim_doc_comment)]
pub enum NotebooksCmd {
    /// Get notebook by ID
    ///
    /// Examples:
    ///   ddog notebooks get --id 12345
    ///   ddog notebooks get --id 12345 --format table
    Get {
        /// Notebook ID
        #[arg(short, long)]
        id: u64,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },

    /// Search notebooks by title, author, or content
    ///
    /// Examples:
    ///   ddog notebooks search --query "postmortem"
    ///   ddog notebooks search --author "user@company.com"
    ///   ddog notebooks search --query "deploy" --format table
    Search {
        /// Search query (matches title and content)
        #[arg(short, long, default_value = "")]
        query: String,

        /// Filter by author handle (email)
        #[arg(long)]
        author: Option<String>,

        /// Sort field: "modified" or "name"
        #[arg(long, default_value = "modified")]
        sort_field: String,

        /// Sort direction: "asc" or "desc"
        #[arg(long, default_value = "desc")]
        sort_dir: String,

        /// Offset for pagination
        #[arg(long, default_value = "0")]
        start: u32,

        /// Max results
        #[arg(short, long, default_value = "25")]
        count: u32,

        /// Output format
        #[arg(short, long, value_enum, default_value = "json")]
        format: Format,
    },
}

pub async fn run(client: &DdClient, cmd: NotebooksCmd) -> Result<(), DdError> {
    match cmd {
        NotebooksCmd::Get { id, format } => {
            log::info(&format!("Fetching notebook: {id}"));
            let result = client.get(&format!("/api/v1/notebooks/{id}"), &[]).await?;
            print_object(
                &result,
                &format,
                &[
                    "data.id",
                    "data.attributes.name",
                    "data.attributes.author.handle",
                    "data.attributes.modified",
                ],
            );
            Ok(())
        }
        NotebooksCmd::Search {
            query,
            author,
            sort_field,
            sort_dir,
            start,
            count,
            format,
        } => {
            log::info(&format!("Searching notebooks: query=\"{query}\""));
            let start_str = start.to_string();
            let count_str = count.to_string();

            let mut params: Vec<(&str, &str)> = vec![
                ("query", &query),
                ("sort_field", &sort_field),
                ("sort_dir", &sort_dir),
                ("start", &start_str),
                ("count", &count_str),
            ];
            let author_val;
            if let Some(a) = &author {
                author_val = a.clone();
                params.push(("author_handle", &author_val));
            }

            let result = client.get("/api/v1/notebooks", &params).await?;
            let n = print_output(
                &result,
                &format,
                &[
                    "id",
                    "attributes.name",
                    "attributes.author.handle",
                    "attributes.modified",
                ],
            );
            log::result_count(n, "notebooks");
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
        cmd: NotebooksCmd,
    }

    #[test]
    fn test_parse_get() {
        let cli = TestCli::parse_from(["test", "get", "--id", "12345"]);
        match cli.cmd {
            NotebooksCmd::Get { id, .. } => assert_eq!(id, 12345),
            _ => panic!("expected Get"),
        }
    }

    #[test]
    fn test_parse_search() {
        let cli = TestCli::parse_from([
            "test",
            "search",
            "--query",
            "postmortem",
            "--author",
            "user@co.com",
        ]);
        match cli.cmd {
            NotebooksCmd::Search { query, author, .. } => {
                assert_eq!(query, "postmortem");
                assert_eq!(author.unwrap(), "user@co.com");
            }
            _ => panic!("expected Search"),
        }
    }
}

use std::process;

use clap::{CommandFactory, Parser, Subcommand};

use ddog::{client, commands, config, log};

#[derive(Parser)]
#[command(
    name = "ddog",
    version,
    about = "A lightweight CLI for querying Datadog — logs, metrics, traces, APM, and more",
    long_about = "Query Datadog resources from the command line.\n\n\
        Authentication:\n  \
        DD_API_KEY  — Required. Your Datadog API key.\n  \
        DD_APP_KEY  — Required. Your Datadog Application key.\n  \
        DD_SITE     — Optional. Defaults to datadoghq.com (use datadoghq.eu for EU).\n\n\
        Time flags accept: relative (15m, 1h, 2d), ISO8601, or Unix epoch.\n\n\
        Output is JSON by default. Use --format table for human-readable tables.\n\
        Info messages go to stderr, data goes to stdout — safe for piping.",
    after_help = "Examples:\n  \
        ddog logs search --query \"service:web status:error\" --from 1h\n  \
        ddog metrics query --query \"avg:system.cpu.user{*}\" --from 2h\n  \
        ddog hosts --format table\n  \
        ddog apm bottlenecks --query \"service:api\" --from 1h\n  \
        ddog monitors --query \"status:alert\""
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search and analyze logs
    Logs {
        #[command(subcommand)]
        cmd: commands::logs::LogsCmd,
    },

    /// Query, search, and inspect metrics
    Metrics {
        #[command(subcommand)]
        cmd: commands::metrics::MetricsCmd,
    },

    /// Search events (monitor alerts, deploys, infrastructure changes)
    Events {
        #[command(flatten)]
        args: commands::events::EventsSearch,
    },

    /// Search monitors by status, type, or tags
    Monitors {
        #[command(flatten)]
        args: commands::monitors::MonitorsSearch,
    },

    /// Search monitored hosts
    Hosts {
        #[command(flatten)]
        args: commands::hosts::HostsSearch,
    },

    /// Search dashboards
    Dashboards {
        #[command(flatten)]
        args: commands::dashboards::DashboardsSearch,
    },

    /// Get a trace by trace ID
    Traces {
        #[command(flatten)]
        args: commands::traces::TracesGet,
    },

    /// Search APM spans
    Spans {
        #[command(flatten)]
        args: commands::spans::SpansSearch,
    },

    /// Search service catalog and dependencies
    Services {
        #[command(subcommand)]
        cmd: commands::services::ServicesCmd,
    },

    /// APM deep analysis — traces, metrics, watchdog, bottlenecks
    Apm {
        #[command(subcommand)]
        cmd: commands::apm::ApmCmd,
    },

    /// Search RUM events (page views, actions, errors, resources)
    Rum {
        #[command(flatten)]
        args: commands::rum::RumSearch,
    },

    /// Manage SLOs (Service Level Objectives)
    Slos {
        #[command(subcommand)]
        cmd: commands::slos::SlosCmd,
    },

    /// List and inspect scheduled downtimes
    Downtimes {
        #[command(subcommand)]
        cmd: commands::downtimes::DowntimesCmd,
    },

    /// List and inspect synthetic tests
    Synthetics {
        #[command(subcommand)]
        cmd: commands::synthetics::SyntheticsCmd,
    },

    /// Search and inspect incidents
    Incidents {
        #[command(subcommand)]
        cmd: commands::incidents::IncidentsCmd,
    },

    /// Search and inspect notebooks
    Notebooks {
        #[command(subcommand)]
        cmd: commands::notebooks::NotebooksCmd,
    },
}

#[tokio::main]
async fn main() {
    if std::env::args_os().len() == 1 {
        Cli::command()
            .print_help()
            .expect("failed to print ddog help");
        println!();
        process::exit(0);
    }

    let cli = Cli::parse();

    let config = match config::Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            log::error(&e.to_string());
            process::exit(1);
        }
    };

    let client = match client::DdClient::new(&config) {
        Ok(c) => c,
        Err(e) => {
            log::error(&e.to_string());
            process::exit(1);
        }
    };

    let result = match cli.command {
        Commands::Logs { cmd } => commands::logs::run(&client, cmd).await,
        Commands::Metrics { cmd } => commands::metrics::run(&client, cmd).await,
        Commands::Events { args } => commands::events::search(&client, args).await,
        Commands::Monitors { args } => commands::monitors::search(&client, args).await,
        Commands::Hosts { args } => commands::hosts::search(&client, args).await,
        Commands::Dashboards { args } => commands::dashboards::search(&client, args).await,
        Commands::Traces { args } => commands::traces::get(&client, args).await,
        Commands::Spans { args } => commands::spans::search(&client, args).await,
        Commands::Services { cmd } => commands::services::run(&client, cmd).await,
        Commands::Apm { cmd } => commands::apm::run(&client, cmd).await,
        Commands::Rum { args } => commands::rum::search(&client, args).await,
        Commands::Slos { cmd } => commands::slos::run(&client, cmd).await,
        Commands::Downtimes { cmd } => commands::downtimes::run(&client, cmd).await,
        Commands::Synthetics { cmd } => commands::synthetics::run(&client, cmd).await,
        Commands::Incidents { cmd } => commands::incidents::run(&client, cmd).await,
        Commands::Notebooks { cmd } => commands::notebooks::run(&client, cmd).await,
    };

    if let Err(e) = result {
        log::error(&e.to_string());
        process::exit(1);
    }
}

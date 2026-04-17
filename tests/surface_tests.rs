use std::fs;
use std::process::Command;

fn top_level_commands(help: &str) -> Vec<&str> {
    let mut commands = Vec::new();
    let mut in_commands = false;

    for line in help.lines() {
        if line == "Commands:" {
            in_commands = true;
            continue;
        }
        if !in_commands {
            continue;
        }
        if line.is_empty() || line == "Options:" {
            break;
        }

        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }

        if let Some((name, _)) = trimmed.split_once(char::is_whitespace) {
            commands.push(name);
        }
    }

    commands
}

#[test]
fn top_level_help_lists_only_supported_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_ddog"))
        .arg("--help")
        .output()
        .expect("failed to run ddog --help");
    assert!(output.status.success());

    let help = String::from_utf8(output.stdout).expect("help output was not valid UTF-8");
    let commands = top_level_commands(&help);

    assert_eq!(
        commands,
        vec![
            "logs",
            "metrics",
            "events",
            "monitors",
            "hosts",
            "dashboards",
            "traces",
            "spans",
            "services",
            "apm",
            "rum",
            "slos",
            "downtimes",
            "synthetics",
            "incidents",
            "notebooks",
            "help",
        ]
    );
}

#[test]
fn no_args_prints_help_and_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_ddog"))
        .output()
        .expect("failed to run ddog");
    assert!(output.status.success());

    let help = String::from_utf8(output.stdout).expect("help output was not valid UTF-8");
    assert!(help.contains("Usage: ddog <COMMAND>"));
    assert!(help.contains("Commands:"));
}

#[test]
fn metrics_search_help_keeps_examples_multiline() {
    let output = Command::new(env!("CARGO_BIN_EXE_ddog"))
        .args(["metrics", "search", "--help"])
        .output()
        .expect("failed to run ddog metrics search --help");
    assert!(output.status.success());

    let help = String::from_utf8(output.stdout).expect("help output was not valid UTF-8");
    assert!(
        help.contains(
            "Examples:\n  ddog metrics search\n  ddog metrics search --tag env:production"
        )
    );
    assert!(
        !help.contains("Examples: ddog metrics search ddog metrics search --tag env:production")
    );
}

#[test]
fn readme_advertises_all_commands() {
    let readme = fs::read_to_string("README.md").expect("failed to read README.md");

    assert!(readme.contains("ddog apm metrics"));
    assert!(readme.contains("ddog dashboards"));
    assert!(readme.contains("ddog incidents"));
    assert!(readme.contains("ddog notebooks"));
    assert!(readme.contains("ddog rum"));
    assert!(readme.contains("ddog slos"));
    assert!(readme.contains("ddog downtimes"));
    assert!(readme.contains("ddog synthetics"));
}

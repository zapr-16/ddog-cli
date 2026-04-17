//! User-facing logging utilities.
//! All info/warning messages go to stderr so stdout stays clean for piping JSON.

pub fn info(msg: &str) {
    eprintln!("\x1b[36minfo:\x1b[0m {msg}");
}

pub fn warn(msg: &str) {
    eprintln!("\x1b[33mwarn:\x1b[0m {msg}");
}

pub fn error(msg: &str) {
    eprintln!("\x1b[31merror:\x1b[0m {msg}");
}

/// Print a summary line after a successful query (e.g., "Returned 25 results")
pub fn result_count(count: usize, resource: &str) {
    if count == 0 {
        info(&format!("No {resource} found."));
    } else {
        info(&format!("Returned {count} {resource}."));
    }
}

#[cfg(test)]
mod tests {
    // These functions write to stderr which we can't easily capture in unit tests,
    // but we verify they don't panic.
    use super::*;

    #[test]
    fn test_info_does_not_panic() {
        info("test message");
    }

    #[test]
    fn test_warn_does_not_panic() {
        warn("test warning");
    }

    #[test]
    fn test_error_does_not_panic() {
        error("test error");
    }

    #[test]
    fn test_result_count_does_not_panic() {
        result_count(0, "logs");
        result_count(5, "hosts");
    }
}

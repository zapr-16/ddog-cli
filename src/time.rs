use chrono::{DateTime, Duration, Utc};

/// Maximum allowed time range per resource type.
#[derive(Debug)]
pub struct TimeRange {
    pub from: String,
    pub to: String,
    pub duration_secs: i64,
}

/// Parse a time string that is either:
/// - A relative duration like "15m", "1h", "2d", "30s"
/// - An ISO8601 timestamp like "2024-01-15T10:00:00Z"
/// - A Unix epoch in seconds
///
/// Returns an RFC3339 string for the Datadog API.
pub fn parse_time(input: &str) -> Result<String, String> {
    if let Some(dur) = parse_relative(input) {
        let t = Utc::now() - dur;
        return Ok(t.to_rfc3339());
    }
    if let Ok(dt) = input.parse::<DateTime<Utc>>() {
        return Ok(dt.to_rfc3339());
    }
    if let Ok(epoch) = input.parse::<i64>()
        && let Some(dt) = DateTime::from_timestamp(epoch, 0)
    {
        return Ok(dt.to_rfc3339());
    }
    Err(format!(
        "Invalid time format: '{input}'. Expected relative (15m, 1h, 2d), ISO8601, or epoch seconds."
    ))
}

/// Parse a time string and return epoch seconds (for v1 API endpoints).
#[cfg(test)]
pub fn parse_time_epoch(input: &str) -> Result<i64, String> {
    if let Some(dur) = parse_relative(input) {
        let t = Utc::now() - dur;
        return Ok(t.timestamp());
    }
    if let Ok(dt) = input.parse::<DateTime<Utc>>() {
        return Ok(dt.timestamp());
    }
    if let Ok(epoch) = input.parse::<i64>() {
        return Ok(epoch);
    }
    Err(format!(
        "Invalid time format: '{input}'. Expected relative (15m, 1h, 2d), ISO8601, or epoch seconds."
    ))
}

/// Resolve --from/--to into a TimeRange, enforcing a maximum duration.
/// `max_hours` is the safety cap — if the range exceeds it, return an error.
pub fn resolve_range(from: &str, to: &Option<String>, max_hours: u32) -> Result<TimeRange, String> {
    let from_ts = parse_time(from)?;
    let to_ts = match to {
        Some(t) => parse_time(t)?,
        None => now_rfc3339(),
    };

    let from_dt: DateTime<Utc> = from_ts
        .parse()
        .map_err(|e| format!("Failed to parse 'from' time: {e}"))?;
    let to_dt: DateTime<Utc> = to_ts
        .parse()
        .map_err(|e| format!("Failed to parse 'to' time: {e}"))?;

    if from_dt >= to_dt {
        return Err("'from' must be before 'to'.".into());
    }

    let duration_secs = (to_dt - from_dt).num_seconds();
    let max_secs = i64::from(max_hours) * 3600;

    if duration_secs > max_secs {
        return Err(format!(
            "Time range too large: {} requested, max {}h allowed. Use a shorter --from or add --to to narrow the window.",
            format_duration(duration_secs),
            max_hours
        ));
    }

    Ok(TimeRange {
        from: from_ts,
        to: to_ts,
        duration_secs,
    })
}

/// Same as resolve_range but returns epoch seconds (for v1 API endpoints).
pub fn resolve_range_epoch(
    from: &str,
    to: &Option<String>,
    max_hours: u32,
) -> Result<(i64, i64), String> {
    let range = resolve_range(from, to, max_hours)?;
    let from_dt: DateTime<Utc> = range.from.parse().unwrap();
    let to_dt: DateTime<Utc> = range.to.parse().unwrap();
    Ok((from_dt.timestamp(), to_dt.timestamp()))
}

pub fn format_duration(duration_secs: i64) -> String {
    if duration_secs % 86_400 == 0 {
        return format!("{}d", duration_secs / 86_400);
    }
    if duration_secs % 3_600 == 0 {
        return format!("{}h", duration_secs / 3_600);
    }
    if duration_secs % 60 == 0 {
        return format!("{}m", duration_secs / 60);
    }

    format!("{duration_secs}s")
}

fn parse_relative(input: &str) -> Option<Duration> {
    let input = input.trim();
    if input.len() < 2 {
        return None;
    }
    let (num_str, unit) = input.split_at(input.len() - 1);
    let num: i64 = num_str.parse().ok()?;
    if num <= 0 || num > 365 * 24 * 3600 {
        // Cap at 1 year in seconds to prevent overflow
        return None;
    }
    let dur = match unit {
        "s" => Duration::seconds(num),
        "m" => Duration::minutes(num),
        "h" => Duration::hours(num),
        "d" if num <= 365 => Duration::days(num),
        "w" if num <= 52 => Duration::weeks(num),
        _ => return None,
    };
    Some(dur)
}

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_relative_minutes() {
        let result = parse_relative("15m");
        assert!(result.is_some());
        assert_eq!(result.unwrap().num_seconds(), 900);
    }

    #[test]
    fn test_parse_relative_hours() {
        assert_eq!(parse_relative("2h").unwrap().num_seconds(), 7200);
    }

    #[test]
    fn test_parse_relative_days() {
        assert_eq!(parse_relative("3d").unwrap().num_seconds(), 259200);
    }

    #[test]
    fn test_parse_relative_weeks() {
        assert_eq!(parse_relative("1w").unwrap().num_seconds(), 604800);
    }

    #[test]
    fn test_parse_relative_seconds() {
        assert_eq!(parse_relative("30s").unwrap().num_seconds(), 30);
    }

    #[test]
    fn test_parse_relative_invalid() {
        assert!(parse_relative("abc").is_none());
        assert!(parse_relative("").is_none());
        assert!(parse_relative("5").is_none());
        assert!(parse_relative("0m").is_none());
        assert!(parse_relative("-5m").is_none());
    }

    #[test]
    fn test_parse_relative_overflow_protection() {
        // Days > 365 rejected
        assert!(parse_relative("9999d").is_none());
        assert!(parse_relative("366d").is_none());
        // Weeks > 52 rejected
        assert!(parse_relative("53w").is_none());
        assert!(parse_relative("9999w").is_none());
        // Absurdly large seconds rejected
        assert!(parse_relative("99999999999s").is_none());
        // Boundary values work
        assert!(parse_relative("365d").is_some());
        assert!(parse_relative("52w").is_some());
    }

    #[test]
    fn test_parse_time_relative() {
        let result = parse_time("1h");
        assert!(result.is_ok());
        let ts = result.unwrap();
        // Should be a valid RFC3339 timestamp roughly 1h ago
        let dt: DateTime<Utc> = ts.parse().unwrap();
        let diff = (Utc::now() - dt).num_seconds();
        assert!((3590..=3610).contains(&diff)); // ~1 hour +/- 10s
    }

    #[test]
    fn test_parse_time_iso8601() {
        let result = parse_time("2024-01-15T10:00:00Z");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("2024-01-15"));
    }

    #[test]
    fn test_parse_time_epoch() {
        let result = parse_time("1700000000");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_time_invalid() {
        let result = parse_time("not-a-time");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid time format"));
    }

    #[test]
    fn test_parse_time_epoch_fn_relative() {
        let result = parse_time_epoch("30m");
        assert!(result.is_ok());
        let epoch = result.unwrap();
        let diff = Utc::now().timestamp() - epoch;
        assert!((1790..=1810).contains(&diff));
    }

    #[test]
    fn test_resolve_range_within_limit() {
        let result = resolve_range("1h", &None, 24);
        assert!(result.is_ok());
        let range = result.unwrap();
        assert!(range.duration_secs <= 3610); // ~1h with margin
    }

    #[test]
    fn test_resolve_range_exceeds_limit() {
        let result = resolve_range("48h", &None, 24);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Time range too large"));
    }

    #[test]
    fn test_resolve_range_reports_precise_requested_duration() {
        let result = resolve_range(
            "2025-01-15T00:00:00Z",
            &Some("2025-01-16T00:01:00Z".into()),
            24,
        );
        let err = result.expect_err("expected range to exceed max");
        assert!(err.contains("1441m requested"));
        assert!(err.contains("max 24h allowed"));
    }

    #[test]
    fn test_resolve_range_from_after_to() {
        let result = resolve_range(
            "2025-01-15T10:00:00Z",
            &Some("2025-01-15T09:00:00Z".into()),
            24,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("'from' must be before 'to'"));
    }

    #[test]
    fn test_format_duration_prefers_human_units() {
        assert_eq!(format_duration(300), "5m");
        assert_eq!(format_duration(1800), "30m");
        assert_eq!(format_duration(3600), "1h");
        assert_eq!(format_duration(172800), "2d");
        assert_eq!(format_duration(45), "45s");
    }
}

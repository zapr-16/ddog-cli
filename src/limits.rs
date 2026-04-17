//! Safety limits to prevent accidental massive API requests.
//! These are deliberately conservative — users can override with explicit flags.

/// Max results per page for log/span/event search queries.
pub const MAX_SEARCH_LIMIT: u32 = 1000;

/// Max time range (hours) for log search — logs can be enormous.
pub const MAX_LOG_HOURS: u32 = 24;

/// Max time range (hours) for span/trace search.
pub const MAX_SPAN_HOURS: u32 = 24;

/// Max time range (hours) for event search.
pub const MAX_EVENT_HOURS: u32 = 48;

/// Max time range (hours) for RUM event search.
pub const MAX_RUM_HOURS: u32 = 24;

/// Max time range (hours) for metric queries — these are pre-aggregated so wider is OK.
pub const MAX_METRIC_HOURS: u32 = 720; // 30 days

/// Max time range (hours) for log analytics/aggregation — pre-aggregated, wider OK.
pub const MAX_ANALYTICS_HOURS: u32 = 168; // 7 days

/// Max hosts per request (API cap).
pub const MAX_HOSTS: u32 = 1000;

/// Max dashboards per request.
pub const MAX_DASHBOARDS: u32 = 500;

/// Max time range (hours) for SLO history queries — 90 days.
pub const MAX_SLO_HOURS: u32 = 2160;

/// Max time range (hours) for synthetic test results.
pub const MAX_SYNTHETICS_HOURS: u32 = 48;

/// Clamp a user-provided limit to the safety maximum.
pub fn clamp_limit(requested: u32, max: u32) -> u32 {
    if requested > max {
        requested.min(max)
    } else {
        requested
    }
}

/// Validate a user-provided limit and clamp it to the safety maximum.
pub fn resolve_limit(requested: u32, max: u32) -> Result<u32, String> {
    require_min("Limit", requested, 1)?;

    Ok(clamp_limit(requested, max))
}

pub fn require_min(label: &str, requested: u32, min: u32) -> Result<u32, String> {
    if requested < min {
        return Err(format!("{label} must be at least {min}."));
    }

    Ok(requested)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp_limit_within() {
        assert_eq!(clamp_limit(25, 1000), 25);
    }

    #[test]
    fn test_clamp_limit_exceeds() {
        assert_eq!(clamp_limit(5000, 1000), 1000);
    }

    #[test]
    fn test_clamp_limit_at_boundary() {
        assert_eq!(clamp_limit(1000, 1000), 1000);
    }

    #[test]
    fn test_resolve_limit_rejects_zero() {
        assert_eq!(
            resolve_limit(0, 1000).unwrap_err(),
            "Limit must be at least 1."
        );
    }

    #[test]
    fn test_resolve_limit_clamps_large_values() {
        assert_eq!(resolve_limit(5000, 1000).unwrap(), 1000);
    }

    #[test]
    fn test_require_min_rejects_small_values() {
        assert_eq!(
            require_min("Page", 0, 1).unwrap_err(),
            "Page must be at least 1."
        );
    }
}

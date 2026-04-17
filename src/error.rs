use std::fmt;

#[derive(Debug)]
pub enum DdError {
    MissingEnv(String),
    Http(reqwest::Error),
    Api { status: u16, body: String },
    Json(serde_json::Error),
    Validation(String),
}

impl fmt::Display for DdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DdError::MissingEnv(var) => {
                write!(
                    f,
                    "Missing environment variable: {var}\n\
                     \n\
                     Set your Datadog credentials:\n  \
                     export {var}=\"your-key-here\"\n\
                     \n\
                     Get your keys at: https://app.datadoghq.com/organization-settings/api-keys"
                )
            }
            DdError::Http(e) => {
                if e.is_connect() {
                    write!(
                        f,
                        "Connection failed: {e}\n\
                         Check your network connection and DD_SITE setting."
                    )
                } else if e.is_timeout() {
                    write!(
                        f,
                        "Request timed out: {e}\n\
                         Try a shorter time range or smaller --limit."
                    )
                } else {
                    write!(f, "HTTP error: {e}")
                }
            }
            DdError::Api { status, body } => {
                let hint = match status {
                    401 => "\nYour API key is invalid or expired. Check DD_API_KEY.",
                    403 => "\nPermission denied. Check DD_APP_KEY has the required scopes.",
                    404 => "\nResource not found. Check the ID or query parameters.",
                    429 => "\nRate limited by Datadog. Wait a moment and retry.",
                    _ => "",
                };
                write!(f, "Datadog API error (HTTP {status}): {body}{hint}")
            }
            DdError::Json(e) => write!(f, "Failed to parse API response: {e}"),
            DdError::Validation(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for DdError {}

impl From<reqwest::Error> for DdError {
    fn from(e: reqwest::Error) -> Self {
        DdError::Http(e)
    }
}

impl From<serde_json::Error> for DdError {
    fn from(e: serde_json::Error) -> Self {
        DdError::Json(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_env_display() {
        let err = DdError::MissingEnv("DD_API_KEY".into());
        let msg = err.to_string();
        assert!(msg.contains("DD_API_KEY"));
        assert!(msg.contains("export"));
        assert!(msg.contains("api-keys"));
    }

    #[test]
    fn test_api_error_401_hint() {
        let err = DdError::Api {
            status: 401,
            body: "Unauthorized".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("401"));
        assert!(msg.contains("invalid or expired"));
    }

    #[test]
    fn test_api_error_403_hint() {
        let err = DdError::Api {
            status: 403,
            body: "Forbidden".into(),
        };
        assert!(err.to_string().contains("Permission denied"));
    }

    #[test]
    fn test_api_error_429_hint() {
        let err = DdError::Api {
            status: 429,
            body: "Too Many Requests".into(),
        };
        assert!(err.to_string().contains("Rate limited"));
    }

    #[test]
    fn test_validation_error() {
        let err = DdError::Validation("bad input".into());
        assert_eq!(err.to_string(), "bad input");
    }
}

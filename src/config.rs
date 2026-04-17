use crate::error::DdError;

pub struct Config {
    pub api_key: String,
    pub app_key: String,
    pub site: String,
}

impl Config {
    pub fn from_env() -> Result<Self, DdError> {
        let api_key = required_env("DD_API_KEY")?;
        let app_key = required_env("DD_APP_KEY")?;
        let site = std::env::var("DD_SITE")
            .ok()
            .map(|site| site.trim().to_string())
            .filter(|site| !site.is_empty())
            .unwrap_or_else(|| "datadoghq.com".into());
        Ok(Config {
            api_key,
            app_key,
            site,
        })
    }

    pub fn base_url(&self) -> String {
        format!("https://api.{}", self.site)
    }
}

fn required_env(name: &str) -> Result<String, DdError> {
    let value = std::env::var(name).map_err(|_| DdError::MissingEnv(name.into()))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(DdError::MissingEnv(name.into()));
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{LazyLock, Mutex};

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct EnvRestore {
        api_key: Option<OsString>,
        app_key: Option<OsString>,
        site: Option<OsString>,
    }

    impl EnvRestore {
        fn capture() -> Self {
            Self {
                api_key: std::env::var_os("DD_API_KEY"),
                app_key: std::env::var_os("DD_APP_KEY"),
                site: std::env::var_os("DD_SITE"),
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            unsafe {
                restore_var("DD_API_KEY", self.api_key.take());
                restore_var("DD_APP_KEY", self.app_key.take());
                restore_var("DD_SITE", self.site.take());
            }
        }
    }

    unsafe fn restore_var(name: &str, value: Option<OsString>) {
        if let Some(value) = value {
            unsafe {
                std::env::set_var(name, value);
            }
        } else {
            unsafe {
                std::env::remove_var(name);
            }
        }
    }

    #[test]
    fn from_env_reads_required_vars_and_defaults_site() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::capture();

        unsafe {
            std::env::set_var("DD_API_KEY", "api-key");
            std::env::set_var("DD_APP_KEY", "app-key");
            std::env::remove_var("DD_SITE");
        }

        let config = Config::from_env().expect("expected config to load");
        assert_eq!(config.api_key, "api-key");
        assert_eq!(config.app_key, "app-key");
        assert_eq!(config.site, "datadoghq.com");
        assert_eq!(config.base_url(), "https://api.datadoghq.com");
    }

    #[test]
    fn from_env_uses_custom_site() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::capture();

        unsafe {
            std::env::set_var("DD_API_KEY", "api-key");
            std::env::set_var("DD_APP_KEY", "app-key");
            std::env::set_var("DD_SITE", "datadoghq.eu");
        }

        let config = Config::from_env().expect("expected config to load");
        assert_eq!(config.site, "datadoghq.eu");
        assert_eq!(config.base_url(), "https://api.datadoghq.eu");
    }

    #[test]
    fn from_env_defaults_empty_site() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::capture();

        unsafe {
            std::env::set_var("DD_API_KEY", "api-key");
            std::env::set_var("DD_APP_KEY", "app-key");
            std::env::set_var("DD_SITE", "   ");
        }

        let config = Config::from_env().expect("expected config to load");
        assert_eq!(config.site, "datadoghq.com");
    }

    #[test]
    fn from_env_requires_api_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::capture();

        unsafe {
            std::env::remove_var("DD_API_KEY");
            std::env::set_var("DD_APP_KEY", "app-key");
            std::env::remove_var("DD_SITE");
        }

        match Config::from_env() {
            Err(DdError::MissingEnv(name)) => assert_eq!(name, "DD_API_KEY"),
            Ok(_) => panic!("expected missing DD_API_KEY"),
            Err(other) => panic!("expected missing DD_API_KEY, got {other}"),
        }
    }

    #[test]
    fn from_env_requires_app_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::capture();

        unsafe {
            std::env::set_var("DD_API_KEY", "api-key");
            std::env::remove_var("DD_APP_KEY");
            std::env::remove_var("DD_SITE");
        }

        match Config::from_env() {
            Err(DdError::MissingEnv(name)) => assert_eq!(name, "DD_APP_KEY"),
            Ok(_) => panic!("expected missing DD_APP_KEY"),
            Err(other) => panic!("expected missing DD_APP_KEY, got {other}"),
        }
    }

    #[test]
    fn from_env_rejects_empty_api_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::capture();

        unsafe {
            std::env::set_var("DD_API_KEY", "   ");
            std::env::set_var("DD_APP_KEY", "app-key");
            std::env::remove_var("DD_SITE");
        }

        match Config::from_env() {
            Err(DdError::MissingEnv(name)) => assert_eq!(name, "DD_API_KEY"),
            Ok(_) => panic!("expected empty DD_API_KEY to be rejected"),
            Err(other) => panic!("expected missing DD_API_KEY, got {other}"),
        }
    }

    #[test]
    fn from_env_rejects_empty_app_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::capture();

        unsafe {
            std::env::set_var("DD_API_KEY", "api-key");
            std::env::set_var("DD_APP_KEY", "");
            std::env::remove_var("DD_SITE");
        }

        match Config::from_env() {
            Err(DdError::MissingEnv(name)) => assert_eq!(name, "DD_APP_KEY"),
            Ok(_) => panic!("expected empty DD_APP_KEY to be rejected"),
            Err(other) => panic!("expected missing DD_APP_KEY, got {other}"),
        }
    }
}

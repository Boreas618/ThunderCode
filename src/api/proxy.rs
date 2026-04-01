//! HTTP/HTTPS proxy configuration for reqwest.
//!
//! Reads proxy settings from standard environment variables (`HTTP_PROXY`,
//! `HTTPS_PROXY`, `NO_PROXY`) and applies them to a reqwest client builder.

use reqwest::ClientBuilder;

// ---------------------------------------------------------------------------
// ProxyConfig
// ---------------------------------------------------------------------------

/// Proxy configuration read from the environment or settings.
#[derive(Debug, Clone, Default)]
pub struct ProxyConfig {
    /// HTTP proxy URL (e.g. `http://proxy:8080`).
    pub http_proxy: Option<String>,
    /// HTTPS proxy URL.
    pub https_proxy: Option<String>,
    /// Comma-separated list of hosts to bypass the proxy.
    pub no_proxy: Option<String>,
}

impl ProxyConfig {
    /// Read proxy config from standard environment variables.
    pub fn from_env() -> Self {
        Self {
            http_proxy: std::env::var("HTTP_PROXY")
                .ok()
                .or_else(|| std::env::var("http_proxy").ok()),
            https_proxy: std::env::var("HTTPS_PROXY")
                .ok()
                .or_else(|| std::env::var("https_proxy").ok()),
            no_proxy: std::env::var("NO_PROXY")
                .ok()
                .or_else(|| std::env::var("no_proxy").ok()),
        }
    }

    /// Whether any proxy is configured.
    pub fn has_proxy(&self) -> bool {
        self.http_proxy.is_some() || self.https_proxy.is_some()
    }
}

// ---------------------------------------------------------------------------
// configure_proxy
// ---------------------------------------------------------------------------

/// Apply proxy settings to a reqwest client builder.
///
/// If no proxy is configured, the builder is returned unchanged (reqwest will
/// use system proxy settings by default).
pub fn configure_proxy(mut builder: ClientBuilder, config: &ProxyConfig) -> ClientBuilder {
    if !config.has_proxy() {
        return builder;
    }

    if let Some(ref url) = config.http_proxy {
        match reqwest::Proxy::http(url) {
            Ok(proxy) => {
                builder = builder.proxy(maybe_with_no_proxy(proxy, config));
            }
            Err(e) => {
                tracing::warn!("Invalid HTTP_PROXY URL '{}': {}", url, e);
            }
        }
    }

    if let Some(ref url) = config.https_proxy {
        match reqwest::Proxy::https(url) {
            Ok(proxy) => {
                builder = builder.proxy(maybe_with_no_proxy(proxy, config));
            }
            Err(e) => {
                tracing::warn!("Invalid HTTPS_PROXY URL '{}': {}", url, e);
            }
        }
    }

    builder
}

/// Attach the `NO_PROXY` bypass list to a proxy object if configured.
fn maybe_with_no_proxy(proxy: reqwest::Proxy, config: &ProxyConfig) -> reqwest::Proxy {
    if let Some(ref no_proxy) = config.no_proxy {
        let no_proxy = no_proxy.to_owned();
        proxy.no_proxy(reqwest::NoProxy::from_string(&no_proxy))
    } else {
        proxy
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_no_proxy() {
        let cfg = ProxyConfig::default();
        assert!(!cfg.has_proxy());
    }

    #[test]
    fn with_http_proxy() {
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".to_owned()),
            https_proxy: None,
            no_proxy: None,
        };
        assert!(cfg.has_proxy());
    }

    #[test]
    fn configure_no_proxy_noop() {
        let builder = reqwest::Client::builder();
        let cfg = ProxyConfig::default();
        // Should not panic.
        let _builder = configure_proxy(builder, &cfg);
    }

    #[test]
    fn configure_with_proxy() {
        let builder = reqwest::Client::builder();
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".to_owned()),
            https_proxy: Some("https://proxy:8443".to_owned()),
            no_proxy: Some("localhost,127.0.0.1".to_owned()),
        };
        // Should not panic.
        let _builder = configure_proxy(builder, &cfg);
    }
}

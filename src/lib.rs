use actix_web::ResponseError;
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;
use std::io;
use std::net::SocketAddr;
use thiserror::Error;

pub mod calendar;
mod markdown;
pub mod metrics;

/// Result type used throughout this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The error type returned by all fallible operations within this crate.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("config error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("Client error: {0}")]
    GoogleCalendar(#[from] calendar::google::ClientError),
    #[error("Prometheus error: {0}")]
    Prometheus(#[from] prometheus::Error),
}

impl ResponseError for Error {}

/// A link configuration.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Link {
    /// The link title.
    pub title: String,
    /// The URL that it points to.
    pub href: String,
    /// Whether the doors on the homepage should link here or not. If this is `true` for multiple
    /// links the first one wins.
    #[serde(default)]
    pub doors: bool,
    /// Whether to add `target="_blank"` to the generated `a` tag or not.
    #[serde(default)]
    pub blank: bool,
}

/// Calendar configuration.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct CalendarConfig {
    /// Source for calendar events.
    pub event_source: calendar::EventSourceKind,
    /// Mapping of event date to event title.
    #[serde(default)]
    pub events: Vec<calendar::Event>,
    /// Period for calendar synchronization.
    pub sync_period_seconds: Option<u64>,
}

/// Website specific configuration.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct SiteConfig {
    /// The site title.
    pub title: String,
    /// The tagline displayed next to the site title.
    pub tagline: String,
    /// Optional site description. This is used in the description meta tag.
    pub description: Option<String>,
    /// Optional canonical URL of the site. This is used in the canonical meta tag.
    pub canonical_url: Option<String>,
    /// Links to display in the site footer.
    #[serde(default)]
    pub links: Vec<Link>,
}

/// Global application configuration.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ServerConfig {
    /// Address on which the web server will listen.
    pub listen_addr: SocketAddr,
    /// Automatically reload templates when they are modified.
    pub template_autoreload: bool,
}

/// Global application configuration.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct AppConfig {
    /// Server configuration section.
    pub server: ServerConfig,
    /// Website configuration section.
    pub site: SiteConfig,
    /// Calendar configuration section.
    pub calendar: CalendarConfig,
    /// Metrics configuration section.
    pub metrics: MetricsConfig,
}

/// Global metrics configuration.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct MetricsConfig {
    /// Whether to enable the metrics server or not.
    pub enabled: bool,
    /// Token to use for Bearer authentication. If `None`, the metrics endpoint will be
    /// unauthenticated.
    pub token: Option<String>,
}

impl AppConfig {
    /// Loads the application configuration from files in the `config/` directory and environment
    /// variables.
    pub fn load() -> Result<AppConfig> {
        let app_env = env::var("APP_ENV").unwrap_or_else(|_| "development".into());

        log::info!("loading configuration using {} environment", app_env);

        let config = Config::builder()
            // Configuration defaults from `config/default.toml`.
            .add_source(File::with_name("config/default"))
            // Optional environment specific config overrides, e.g. `config/production.toml`.
            .add_source(File::with_name(&format!("config/{}", app_env)).required(false))
            // Optional local config overrides from `config/local.toml` (on .gitignore).
            .add_source(File::with_name("config/local").required(false))
            // Config from environment variables.
            .add_source(Environment::default().separator("__"))
            // Config from environment variables prefixed with `WZ_`.
            .add_source(
                Environment::with_prefix("WZ")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()?
            .try_deserialize()?;

        log::debug!("loaded configuration: {:?}", config);

        Ok(config)
    }
}

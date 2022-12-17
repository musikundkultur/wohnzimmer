use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;
use std::io;
use std::net::SocketAddr;
use thiserror::Error;

pub mod calendar;

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
}

/// A link configuration.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Link {
    /// The link title.
    pub title: String,
    /// The URL that it points to.
    pub href: String,
}

/// Calendar configuration.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct CalendarConfig {
    /// Source for calendar events.
    pub event_source: calendar::EventSourceKind,
    /// Mapping of event date to event title.
    #[serde(default)]
    pub events: Vec<calendar::Event>,
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

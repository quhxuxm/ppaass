use std::str::FromStr;

use common::ProxyServerLogTimer;
use config::ProxyServerLogConfig;

use manager::ProxyServerManager;

use anyhow::{Context, Result};
use tracing::{metadata::LevelFilter, subscriber};
use tracing_subscriber::{fmt::Layer, prelude::__tracing_subscriber_SubscriberExt, Registry};
mod arguments;
mod common;
mod config;
mod constant;
mod crypto;

mod processor;

mod manager;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    let log_configuration_file_content = tokio::fs::read_to_string(config::DEFAULT_PROXY_LOG_CONFIG_FILE)
        .await
        .context(format!("Fail to read proxy log configuration file: {}", config::DEFAULT_PROXY_LOG_CONFIG_FILE))?;
    let proxy_server_log_config: ProxyServerLogConfig = toml::from_str(&log_configuration_file_content).context(format!(
        "Fail to parse proxy server log configuration file: {}",
        config::DEFAULT_PROXY_LOG_CONFIG_FILE
    ))?;
    let log_dir = proxy_server_log_config
        .get_dir()
        .as_ref()
        .expect("Fail to get log directory from configuration file.");
    let log_file = proxy_server_log_config
        .get_file()
        .as_ref()
        .expect("Fail to get log file from configuration file.");
    let default_log_level = "info".to_string();
    let log_level = proxy_server_log_config.get_level().as_ref().unwrap_or(&default_log_level);
    let log_file_appender = tracing_appender::rolling::daily(log_dir, log_file);
    let log_level_filter = LevelFilter::from_str(log_level).expect("Fail to initialize log filter");
    let (non_blocking, _appender_guard) = tracing_appender::non_blocking(log_file_appender);
    let subscriber = Registry::default()
        .with(
            Layer::default()
                .with_level(true)
                .with_target(true)
                .with_timer(ProxyServerLogTimer)
                .with_thread_ids(true)
                .with_file(true)
                .with_ansi(false)
                .with_line_number(true)
                .with_writer(non_blocking),
        )
        .with(log_level_filter);
    subscriber::set_global_default(subscriber).expect("Fail to initialize tracing subscriber");
    let proxy_server_manager = ProxyServerManager::new()?;
    proxy_server_manager.start().await?;
    Ok(())
}

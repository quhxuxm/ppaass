use std::{str::FromStr, sync::Arc};

use anyhow::Context;
use anyhow::Result;
use common::AgentServerLogTimer;
use config::AgentServerLogConfig;
use ppaass_agent_desktop_lib::{config::AgentServerConfig, server::AgentServer};
use tracing::{metadata::LevelFilter, subscriber};
use tracing_subscriber::{fmt::Layer, prelude::__tracing_subscriber_SubscriberExt, Registry};

mod common;
mod config;
mod constant;
#[tokio::main]
async fn main() -> Result<()> {
    let log_configuration_file_content = tokio::fs::read_to_string(config::DEFAULT_AGENT_LOG_CONFIG_FILE)
        .await
        .context(format!("fail to read agent log configuration file: {}", config::DEFAULT_AGENT_LOG_CONFIG_FILE))?;
    let agent_server_log_config: AgentServerLogConfig = toml::from_str(&log_configuration_file_content).context(format!(
        "fail to parse agent server log configuration file: {}",
        config::DEFAULT_AGENT_LOG_CONFIG_FILE
    ))?;
    let log_dir = agent_server_log_config
        .get_dir()
        .as_ref()
        .expect("Fail to get log directory from configuration file.");
    let log_file = agent_server_log_config
        .get_file()
        .as_ref()
        .expect("Fail to get log file from configuration file.");
    let default_log_level = "info".to_string();
    let log_level = agent_server_log_config.get_level().as_ref().unwrap_or(&default_log_level);
    let log_file_appender = tracing_appender::rolling::daily(log_dir, log_file);
    let log_level_filter = LevelFilter::from_str(&log_level).expect("Fail to initialize log filter");
    let (non_blocking, _appender_guard) = tracing_appender::non_blocking(log_file_appender);
    let subscriber = Registry::default()
        .with(
            Layer::default()
                .with_level(true)
                .with_target(true)
                .with_timer(AgentServerLogTimer)
                .with_thread_ids(true)
                .with_file(true)
                .with_ansi(false)
                .with_line_number(true)
                .with_writer(non_blocking),
        )
        .with(log_level_filter);
    subscriber::set_global_default(subscriber).expect("Fail to initialize tracing subscriber");

    let agent_configuration_file = tokio::fs::read_to_string(constant::DEFAULT_AGENT_CONFIG_FILE_PATH)
        .await
        .context(format!("fail to read agent configuration file: {}", constant::DEFAULT_AGENT_CONFIG_FILE_PATH))?;
    let agent_server_config: AgentServerConfig = toml::from_str(&agent_configuration_file).context(format!(
        "fail to parse agent server configuration file: {}",
        constant::DEFAULT_AGENT_CONFIG_FILE_PATH
    ))?;
    let mut agent_server = AgentServer::new(Arc::new(agent_server_config));
    agent_server.start().await?;
    Ok(())
}

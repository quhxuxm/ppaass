pub(crate) mod config;
pub(crate) mod server;

pub(crate) mod connection;
pub(crate) mod crypto;
pub(crate) mod error;
pub(crate) mod processor;

use anyhow::Result;
use chrono::Local;
use config::AGENT_CONFIG;
use ppaass_common::PpaassMessagePayloadEncryptionSelector;
use tracing::{error, Level};

use server::AgentServer;
use tokio::runtime::Builder;

use tracing_subscriber::{
    filter::Targets,
    fmt::{format::Writer, time::FormatTime, Layer},
    prelude::__tracing_subscriber_SubscriberExt,
    util::SubscriberInitExt,
};

pub const SOCKS_V5: u8 = 5;
pub const SOCKS_V4: u8 = 4;

pub struct AgentServerPayloadEncryptionTypeSelector;

impl PpaassMessagePayloadEncryptionSelector for AgentServerPayloadEncryptionTypeSelector {}

pub struct LogTimer;

impl FormatTime for LogTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%FT%T%.3f"))
    }
}

fn main() -> Result<()> {
    let console_subscriber = console_subscriber::spawn();
    let log_dir_path = "log";
    let log_file_path = "ppaass-agent.log";
    let file_appender = tracing_appender::rolling::daily(log_dir_path, log_file_path);
    let (non_blocking, _appender_guard) = tracing_appender::non_blocking(file_appender);
    let event_filter = Targets::new().with_targets(vec![("tokio", Level::ERROR), ("runtime", Level::ERROR), ("ppaass-agent", Level::ERROR)]);
    tracing_subscriber::registry()
        .with(console_subscriber)
        .with(
            Layer::default()
                // .json()
                .with_level(true)
                .with_target(true)
                .with_timer(LogTimer)
                .with_thread_ids(true)
                .with_file(true)
                .with_ansi(false)
                .with_line_number(true)
                .with_writer(non_blocking),
        )
        .with(event_filter)
        .init();

    let agent_server_runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(AGENT_CONFIG.get_worker_thread_number())
        .build()?;
    agent_server_runtime.block_on(async move {
        let mut agent_server = AgentServer::default();
        if let Err(e) = agent_server.start().await {
            error!("Fail to start agent server because of error: {e:?}");
        };
    });
    Ok(())
}

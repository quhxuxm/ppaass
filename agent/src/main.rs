pub(crate) mod config;
pub(crate) mod server;

pub(crate) mod crypto;
pub(crate) mod error;
pub(crate) mod pool;
pub(crate) mod processor;

use chrono::Local;
use config::AGENT_CONFIG;
use ppaass_common::PpaassMessagePayloadEncryptionSelector;

use anyhow::Result;
use log::{error, LevelFilter as LogLevelFilter};
use server::AgentServer;
use tokio::runtime::Builder;
use tracing::metadata::LevelFilter as TracingLevelFilter;
use tracing_log::LogTracer;
use tracing_subscriber::{
    fmt::{format::Writer, time::FormatTime, Layer},
    prelude::__tracing_subscriber_SubscriberExt,
    Registry,
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
    let log_dir_path = "log";
    let log_file_path = "ppaass-agent.log";
    let file_appender = tracing_appender::rolling::daily(log_dir_path, log_file_path);
    let (non_blocking, _appender_guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = Registry::default()
        .with(
            Layer::default()
                .with_level(true)
                .with_target(true)
                .with_timer(LogTimer)
                .with_thread_ids(true)
                .with_file(true)
                .with_ansi(false)
                .with_line_number(true)
                .with_writer(non_blocking),
        )
        .with(TracingLevelFilter::ERROR);
    tracing::subscriber::set_global_default(subscriber).expect("Fail to initialize tracing subscriber");
    LogTracer::builder().with_max_level(LogLevelFilter::Error).init()?;

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

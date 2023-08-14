mod common;
mod config;
mod crypto;
mod error;
mod processor;
mod server;

use chrono::Local;
use config::PROXY_CONFIG;
use log::info;
use log::{error, LevelFilter as LogLevelFilter};

use anyhow::Result;
use tokio::runtime::Builder;

use tracing_log::LogTracer;
use tracing_subscriber::{
    fmt::{format::Writer, time::FormatTime, Layer},
    prelude::__tracing_subscriber_SubscriberExt,
    Registry,
};

use tracing::metadata::LevelFilter as TracingLevelFilter;

use crate::server::ProxyServer;

pub struct LogTimer;

impl FormatTime for LogTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%FT%T%.3f"))
    }
}

fn main() -> Result<()> {
    let log_dir_path = "log";
    let log_file_path = "ppaass-proxy.log";
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

    let proxy_server_runtime = Builder::new_multi_thread()
        .enable_all()
        .thread_name("proxy-server-runtime")
        .worker_threads(PROXY_CONFIG.get_worker_thread_number())
        .build()?;

    proxy_server_runtime.block_on(async {
        info!("Begin to start proxy server.");
        let mut proxy_server = ProxyServer::default();
        if let Err(e) = proxy_server.start().await {
            error!("Fail to start proxy server because of error: {e:?}");
            panic!("Fail to start proxy server because of error: {e:?}")
        }
    });
    Ok(())
}

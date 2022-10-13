use std::time::Duration;

use anyhow::Result;
use tokio::runtime::{Builder, Runtime};
use tracing::info;
pub(crate) struct ProxyServer {
    runtime: Runtime,
}

impl ProxyServer {
    pub(crate) fn new() -> Result<Self> {
        let mut runtime_builder = Builder::new_multi_thread();
        let runtime = runtime_builder.build()?;
        Ok(Self { runtime })
    }

    pub(crate) fn start(&self) -> Result<()> {
        self.runtime.spawn(async {
            info!("Proxy server start to serve request.");
        });
        Ok(())
    }

    pub(crate) fn shutdown(self) -> Result<()> {
        self.runtime.shutdown_timeout(Duration::from_secs(30));
        Ok(())
    }
}

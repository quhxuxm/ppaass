use std::{sync::Arc, time::Duration};

use anyhow::Result;
use tokio::runtime::{Builder, Runtime};
use tracing::info;

use crate::config::ProxyServerConfig;
pub(crate) struct ProxyServer {
    configuration: Arc<ProxyServerConfig>,
    runtime: Runtime,
}

impl ProxyServer {
    pub(crate) fn new(configuration: Arc<ProxyServerConfig>) -> Result<Self> {
        let mut runtime_builder = Builder::new_multi_thread();
        let runtime = runtime_builder.build()?;
        Ok(Self { runtime, configuration })
    }

    pub(crate) fn start(&self) -> Result<()> {
        self.runtime.spawn(async {
            info!("Proxy server start to serve request.");
        });
        Ok(())
    }

    pub(crate) fn shutdown(self) {
        info!("Going to shutdown proxy server in 3o seconds.");
        self.runtime.shutdown_timeout(Duration::from_secs(30));
    }
}

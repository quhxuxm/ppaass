use std::collections::HashMap;

use tokio::sync::Mutex;

use super::ProxyTcpTunnel;

pub(crate) struct ProxyTcpTunnelRepository {
    inner: Mutex<HashMap<String, ProxyTcpTunnel>>,
}

impl ProxyTcpTunnelRepository {
    pub(crate) fn new() -> Self {
        let inner = Mutex::new(HashMap::new());
        Self { inner }
    }

    pub(crate) async fn insert(&mut self, proxy_tunnel_id: String, proxy_tunnel: ProxyTcpTunnel) {
        let mut repository = self.inner.lock().await;
        repository.insert(proxy_tunnel_id, proxy_tunnel);
    }
}

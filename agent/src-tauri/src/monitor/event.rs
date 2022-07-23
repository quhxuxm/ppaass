pub(crate) enum MonitorEvent {
    ClientConnectionConnected {
        client_connection_id: String,
        timestamp: u64,
    },
    ClientConnectionConnectedToProxy {
        client_connection_id: String,
        proxy_connection_id: String,
        timestamp: u64,
    },
    ClientConnectionRelayToProxy {
        client_connection_id: String,
        proxy_connection_id: String,
        timestamp: u64,
    },
    ProxyConnectionCreate {
        proxy_connection_id: String,
        timestamp: u64,
    },
    ProxyConnectionConnected {
        proxy_connection_id: String,
        timestamp: u64,
    },
    ProxyConnectionClosed {
        proxy_connection_id: String,
        timestamp: u64,
    },
    ProxyConnectionTimeout {
        proxy_connection_id: String,
        timestamp: u64,
    },
    ProxyConnectionInitialized {
        proxy_connection_id: String,
        timestamp: u64,
    },
    ProxyConnectionRelay {
        proxy_connection_id: String,
        timestamp: u64,
    },
}

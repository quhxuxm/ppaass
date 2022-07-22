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
        connection_id: String,
        timestamp: u64,
    },
    ProxyConnectionConnected {
        connection_id: String,
        timestamp: u64,
    },
    ProxyConnectionClosed {
        connection_id: String,
        timestamp: u64,
    },
    ProxyConnectionTimeout {
        connection_id: String,
        timestamp: u64,
    },
    ProxyConnectionInitialized {
        connection_id: String,
        timestamp: u64,
    },
    ProxyConnectionRelay {
        connection_id: String,
        timestamp: u64,
    },
}

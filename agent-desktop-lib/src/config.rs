use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct AgentServerConfig {
    /// Whehter use ip v6
    ipv6: Option<bool>,
    /// Port of the ppaass proxy
    port: Option<u16>,
    /// The root directory used to store the rsa
    /// files for each user
    rsa_dir: Option<String>,
    /// The threads number
    thread_number: Option<usize>,
    /// Whether enable compressing
    compress: Option<bool>,
    /// The client connection pool size.
    client_max_connection_number: Option<usize>,
    /// The timeout to accept client connection pool size.
    client_tcp_connection_accept_timout_seconds: Option<u64>,
    /// The proxy addresses
    proxy_addresses: Option<Vec<String>>,
}

impl AgentServerConfig {
    pub(crate) fn set_proxy_addresses(&mut self, proxy_addresses: Vec<String>) {
        self.proxy_addresses = Some(proxy_addresses)
    }

    pub(crate) fn get_proxy_addresses(&self) -> Option<&Vec<String>> {
        self.proxy_addresses.as_ref()
    }
    pub(crate) fn set_ipv6(&mut self, ipv6: bool) {
        self.ipv6 = Some(ipv6)
    }

    pub(crate) fn get_ipv6(&self) -> bool {
        self.ipv6.unwrap_or(false)
    }

    pub(crate) fn set_port(&mut self, port: u16) {
        self.port = Some(port)
    }

    pub(crate) fn get_port(&self) -> &Option<u16> {
        &self.port
    }

    pub(crate) fn set_rsa_dir(&mut self, rsa_dir: &str) {
        self.rsa_dir = Some(rsa_dir.to_string())
    }

    pub(crate) fn get_rsa_dir(&self) -> Option<&String> {
        self.rsa_dir.as_ref()
    }

    pub(crate) fn set_thread_number(&mut self, thread_number: usize) {
        self.thread_number = Some(thread_number)
    }

    pub(crate) fn get_thread_number(&self) -> usize {
        self.thread_number.unwrap_or(128)
    }

    pub(crate) fn set_compress(&mut self, compress: bool) {
        self.compress = Some(compress)
    }

    pub(crate) fn get_compress(&self) -> bool {
        self.compress.unwrap_or(false)
    }

    pub(crate) fn get_client_max_connection_number(&self) -> usize {
        self.client_max_connection_number.unwrap_or(1024)
    }

    pub(crate) fn get_client_tcp_connection_accept_timout_seconds(&self) -> u64 {
        self.client_tcp_connection_accept_timout_seconds.unwrap_or(20)
    }
}

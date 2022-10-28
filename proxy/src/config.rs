use serde_derive::{Deserialize, Serialize};

use crate::constant::{DEFAULT_PROXY_SERVER_PORT, DEFAULT_RSA_DIR, DEFAULT_THREAD_NUMBER};

pub const DEFAULT_PROXY_LOG_CONFIG_FILE: &str = "./ppaass-proxy-log.toml";

#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct ProxyServerConfig {
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
    /// The buffer size for one agent connection
    agent_connection_buffer_size: Option<usize>,
    /// The agent connection pool size.
    agent_max_connection_number: Option<usize>,
    agent_tcp_connection_accept_timout_seconds: Option<u64>,
}

impl ProxyServerConfig {
    pub(crate) fn set_ipv6(&mut self, ipv6: bool) {
        self.ipv6 = Some(ipv6)
    }

    pub(crate) fn get_ipv6(&self) -> bool {
        self.ipv6.unwrap_or(false)
    }

    pub(crate) fn set_port(&mut self, port: u16) {
        self.port = Some(port)
    }

    pub(crate) fn get_port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PROXY_SERVER_PORT)
    }

    pub(crate) fn set_rsa_dir(&mut self, rsa_dir: &str) {
        self.rsa_dir = Some(rsa_dir.to_string())
    }

    pub(crate) fn get_rsa_dir(&self) -> String {
        self.rsa_dir.as_ref().unwrap_or(&DEFAULT_RSA_DIR.to_string()).to_string()
    }

    pub(crate) fn set_thread_number(&mut self, thread_number: usize) {
        self.thread_number = Some(thread_number)
    }

    pub(crate) fn get_thread_number(&self) -> usize {
        self.thread_number.unwrap_or(DEFAULT_THREAD_NUMBER)
    }

    pub(crate) fn set_compress(&mut self, compress: bool) {
        self.compress = Some(compress)
    }

    pub(crate) fn get_compress(&self) -> bool {
        self.compress.unwrap_or(false)
    }

    pub(crate) fn get_agent_connection_buffer_size(&self) -> usize {
        self.agent_connection_buffer_size.unwrap_or(1024 * 64)
    }

    pub(crate) fn get_agent_max_connection_number(&self) -> usize {
        self.agent_max_connection_number.unwrap_or(1024)
    }

    pub(crate) fn get_agent_tcp_connection_accept_timout_seconds(&self) -> u64 {
        self.agent_tcp_connection_accept_timout_seconds.unwrap_or(20)
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct ProxyServerLogConfig {
    /// The log directory
    dir: Option<String>,
    /// The log file name prefix
    file: Option<String>,
    /// The max log level
    level: Option<String>,
}

impl ProxyServerLogConfig {
    pub(crate) fn get_dir(&self) -> &Option<String> {
        &self.dir
    }

    pub(crate) fn get_file(&self) -> &Option<String> {
        &self.file
    }

    pub(crate) fn get_level(&self) -> &Option<String> {
        &self.level
    }
}

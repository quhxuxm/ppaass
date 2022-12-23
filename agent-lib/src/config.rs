use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct AgentServerConfig {
    //The user token
    user_token: Option<String>,
    /// Whehter use ip v6
    ipv6: Option<bool>,
    /// Port of the ppaass proxy
    port: Option<u16>,
    /// The root directory used to store the rsa
    /// files for each user
    rsa_dir: Option<String>,
    /// The threads number
    agent_server_worker_thread_number: Option<usize>,
    /// Whether enable compressing
    compress: Option<bool>,
    /// The client connection pool size.
    proxy_connection_pool_size: Option<usize>,

    /// The proxy addresses
    proxy_addresses: Option<Vec<String>>,
    message_framed_buffer_size: Option<usize>,
    idle_proxy_heartbeat_interval: Option<u64>,
    client_connection_accept_timeout: Option<u64>,
    connect_to_proxy_timeout: Option<u64>,
    client_io_buffer_size: Option<usize>,
    max_client_connection_number: Option<usize>,
}

impl AgentServerConfig {
    pub fn get_user_token(&self) -> &Option<String> {
        &self.user_token
    }
    pub fn set_user_token(&mut self, user_token: String) {
        self.user_token = Some(user_token);
    }

    pub fn set_proxy_addresses(&mut self, proxy_addresses: Vec<String>) {
        self.proxy_addresses = Some(proxy_addresses)
    }

    pub fn get_proxy_addresses(&self) -> Option<&Vec<String>> {
        self.proxy_addresses.as_ref()
    }
    pub fn set_ipv6(&mut self, ipv6: bool) {
        self.ipv6 = Some(ipv6)
    }

    pub fn get_ipv6(&self) -> bool {
        self.ipv6.unwrap_or(false)
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = Some(port)
    }

    pub fn get_port(&self) -> &Option<u16> {
        &self.port
    }

    pub fn set_rsa_dir(&mut self, rsa_dir: &str) {
        self.rsa_dir = Some(rsa_dir.to_string())
    }

    pub fn get_rsa_dir(&self) -> Option<&String> {
        self.rsa_dir.as_ref()
    }

    pub fn set_agent_server_worker_thread_number(&mut self, thread_number: usize) {
        self.agent_server_worker_thread_number = Some(thread_number)
    }

    pub fn get_agent_server_worker_thread_number(&self) -> usize {
        self.agent_server_worker_thread_number.unwrap_or(128)
    }

    pub fn set_compress(&mut self, compress: bool) {
        self.compress = Some(compress)
    }

    pub fn get_compress(&self) -> bool {
        self.compress.unwrap_or(false)
    }

    pub fn get_proxy_connection_pool_size(&self) -> usize {
        self.proxy_connection_pool_size.unwrap_or(1024)
    }

    pub fn get_message_framed_buffer_size(&self) -> usize {
        self.message_framed_buffer_size.unwrap_or(65536)
    }

    pub fn get_idle_proxy_heartbeat_interval(&self) -> u64 {
        self.idle_proxy_heartbeat_interval.unwrap_or(20)
    }
    pub fn get_client_connection_accept_timeout(&self) -> u64 {
        self.client_connection_accept_timeout.unwrap_or(5)
    }

    pub fn get_client_io_buffer_size(&self) -> usize {
        self.client_io_buffer_size.unwrap_or(1024 * 64)
    }

    pub fn get_connect_to_proxy_timeout(&self) -> u64 {
        self.connect_to_proxy_timeout.unwrap_or(20)
    }

    pub fn get_max_client_connection_number(&self) -> usize {
        let default_max_client_connection_number = self.get_proxy_connection_pool_size();
        self.max_client_connection_number.unwrap_or(default_max_client_connection_number)
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct AgentServerLogConfig {
    /// The log directory
    dir: Option<String>,
    /// The log file name prefix
    file: Option<String>,
    /// The max log level
    level: Option<String>,
}

impl AgentServerLogConfig {
    pub fn get_dir(&self) -> &Option<String> {
        &self.dir
    }

    pub fn get_file(&self) -> &Option<String> {
        &self.file
    }

    pub fn get_level(&self) -> &Option<String> {
        &self.level
    }
}

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
    proxy_connection_number: Option<usize>,
    /// The timeout to accept client connection pool size.
    client_tcp_connection_accept_timout_seconds: Option<u64>,
    /// The proxy addresses
    proxy_addresses: Option<Vec<String>>,
    message_framed_buffer_size: Option<usize>,
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

    pub fn get_proxy_connection_number(&self) -> usize {
        self.proxy_connection_number.unwrap_or(1024)
    }

    pub fn get_client_tcp_connection_accept_timout_seconds(&self) -> u64 {
        self.client_tcp_connection_accept_timout_seconds.unwrap_or(20)
    }

    pub fn get_message_framed_buffer_size(&self) -> usize {
        self.message_framed_buffer_size.unwrap_or(65536)
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

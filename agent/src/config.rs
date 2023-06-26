use std::fs::read_to_string;

use lazy_static::lazy_static;
use serde_derive::{Deserialize, Serialize};

lazy_static! {
    pub(crate) static ref AGENT_CONFIG: AgentConfig = {
        let agent_configuration_file = read_to_string("resources/config/ppaass-agent.toml").expect("Fail to read agent configuration file.");
        toml::from_str(&agent_configuration_file).expect("Fail to parse agent configuration file content.")
    };
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct AgentConfig {
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
    /// The proxy addresses
    proxy_addresses: Option<Vec<String>>,
    client_receive_buffer_size: Option<usize>,
    proxy_send_buffer_size: Option<usize>,
    connect_to_proxy_timeout: Option<u64>,

    proxy_relay_timeout: Option<u64>,
    client_relay_timeout: Option<u64>,
}

impl AgentConfig {
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

    pub fn get_port(&self) -> u16 {
        self.port.unwrap_or(80)
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

    pub fn get_proxy_send_buffer_size(&self) -> usize {
        self.proxy_send_buffer_size.unwrap_or(1024 * 512)
    }

    pub fn get_connect_to_proxy_timeout(&self) -> u64 {
        self.connect_to_proxy_timeout.unwrap_or(20)
    }
    pub fn get_client_receive_buffer_size(&self) -> usize {
        self.client_receive_buffer_size.unwrap_or(1024 * 512)
    }

    pub fn get_proxy_relay_timeout(&self) -> u64 {
        self.proxy_relay_timeout.unwrap_or(20)
    }

    pub fn get_client_relay_timeout(&self) -> u64 {
        self.client_relay_timeout.unwrap_or(20)
    }
}

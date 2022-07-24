#![allow(unused)]
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;

use clap::Parser;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use tracing::error;

pub(crate) const DEFAULT_AGENT_LOG_CONFIG_FILE: &str = "ppaass-agent-log.toml";
pub(crate) const DEFAULT_AGENT_CONFIGURATION_FILE: &str = "ppaass-agent.toml";

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentLogConfig {
    log_dir: Option<String>,
    log_file: Option<String>,
    max_log_level: Option<String>,
}

impl AgentLogConfig {
    pub fn log_dir(&self) -> &Option<String> {
        &self.log_dir
    }
    pub fn set_log_dir(&mut self, log_dir: String) {
        self.log_dir = Some(log_dir);
    }
    pub fn log_file(&self) -> &Option<String> {
        &self.log_file
    }
    pub fn set_log_file(&mut self, log_file: String) {
        self.log_file = Some(log_file)
    }
    pub fn max_log_level(&self) -> &Option<String> {
        &self.max_log_level
    }
    pub fn set_max_log_level(&mut self, max_log_level: String) {
        self.max_log_level = Some(max_log_level)
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentConfig {
    port: Option<u16>,
    so_recv_buffer_size: Option<u32>,
    so_send_buffer_size: Option<u32>,
    user_token: Option<String>,
    proxy_addresses: Option<Vec<String>>,
    client_buffer_size: Option<usize>,
    message_framed_buffer_size: Option<usize>,
    thread_number: Option<usize>,
    max_blocking_threads: Option<usize>,
    thread_timeout: Option<u64>,
    compress: Option<bool>,
    client_stream_so_linger: Option<u64>,
    proxy_stream_so_linger: Option<u64>,
    so_backlog: Option<u32>,
    agent_private_key_file: Option<String>,
    proxy_public_key_file: Option<String>,
    init_proxy_connection_number: Option<usize>,
    min_proxy_connection_number: Option<usize>,
    proxy_connection_number_incremental: Option<usize>,
    proxy_connection_check_interval_seconds: Option<u64>,
    proxy_connection_check_timeout: Option<u64>,
}

impl AgentConfig {
    pub fn port(&self) -> Option<u16> {
        self.port
    }
    pub fn set_port(&mut self, port: u16) {
        self.port = Some(port);
    }
    pub fn user_token(&self) -> &Option<String> {
        &self.user_token
    }
    pub fn set_user_token(&mut self, user_token: String) {
        self.user_token = Some(user_token);
    }
    pub fn proxy_addresses(&self) -> &Option<Vec<String>> {
        &self.proxy_addresses
    }
    pub fn set_proxy_addresses(&mut self, proxy_addresses: Vec<String>) {
        self.proxy_addresses = Some(proxy_addresses);
    }
    pub fn client_buffer_size(&self) -> Option<usize> {
        self.client_buffer_size
    }
    pub fn set_client_buffer_size(&mut self, client_buffer_size: usize) {
        self.client_buffer_size = Some(client_buffer_size)
    }
    pub fn message_framed_buffer_size(&self) -> Option<usize> {
        self.message_framed_buffer_size
    }
    pub fn set_message_framed_buffer_size(&mut self, message_framed_buffer_size: usize) {
        self.message_framed_buffer_size = Some(message_framed_buffer_size)
    }
    pub fn thread_number(&self) -> Option<usize> {
        self.thread_number
    }
    pub fn set_thread_number(&mut self, thread_number: usize) {
        self.thread_number = Some(thread_number);
    }
    pub fn max_blocking_threads(&self) -> Option<usize> {
        self.max_blocking_threads
    }
    pub fn thread_timeout(&self) -> Option<u64> {
        self.thread_timeout
    }

    pub fn compress(&self) -> Option<bool> {
        self.compress
    }
    pub fn set_compress(&mut self, compress: bool) {
        self.compress = Some(compress);
    }
    pub fn client_stream_so_linger(&self) -> Option<u64> {
        self.client_stream_so_linger
    }
    pub fn proxy_stream_so_linger(&self) -> Option<u64> {
        self.proxy_stream_so_linger
    }
    pub fn so_backlog(&self) -> Option<u32> {
        self.so_backlog
    }
    pub fn set_so_backlog(&mut self, so_backlog: u32) {
        self.so_backlog = Some(so_backlog)
    }
    pub fn so_recv_buffer_size(&self) -> Option<u32> {
        self.so_recv_buffer_size
    }
    pub fn so_send_buffer_size(&self) -> Option<u32> {
        self.so_send_buffer_size
    }
    pub fn agent_private_key_file(&self) -> &Option<String> {
        &self.agent_private_key_file
    }
    pub fn set_agent_private_key_file(&mut self, agent_private_key_file: String) {
        self.agent_private_key_file = Some(agent_private_key_file)
    }
    pub fn proxy_public_key_file(&self) -> &Option<String> {
        &self.proxy_public_key_file
    }
    pub fn set_proxy_public_key_file(&mut self, proxy_public_key_file: String) {
        self.proxy_public_key_file = Some(proxy_public_key_file)
    }
    pub fn init_proxy_connection_number(&self) -> Option<usize> {
        self.init_proxy_connection_number
    }
    pub fn set_init_proxy_connection_number(&mut self, init_proxy_connection_number: usize) {
        self.init_proxy_connection_number = Some(init_proxy_connection_number)
    }
    pub fn set_min_proxy_connection_number(&mut self, min_proxy_connection_number: usize) {
        self.min_proxy_connection_number = Some(min_proxy_connection_number);
    }
    pub fn min_proxy_connection_number(&self) -> Option<usize> {
        self.min_proxy_connection_number
    }
    pub fn proxy_connection_check_interval_seconds(&self) -> Option<u64> {
        self.proxy_connection_check_interval_seconds
    }
    pub fn set_proxy_connection_check_interval_seconds(&mut self, proxy_connection_check_interval_seconds: u64) {
        self.proxy_connection_check_interval_seconds = Some(proxy_connection_check_interval_seconds)
    }
    pub fn proxy_connection_number_increasement(&self) -> Option<usize> {
        self.proxy_connection_number_incremental
    }
    pub fn set_proxy_connection_number_incremental(&mut self, proxy_connection_number_increasement: usize) {
        self.proxy_connection_number_incremental = Some(proxy_connection_number_increasement)
    }
    pub fn proxy_connection_check_timeout(&self) -> Option<u64> {
        self.proxy_connection_check_timeout
    }
    pub fn set_proxy_connection_check_timeout(&mut self, proxy_connection_check_timeout: u64) {
        self.proxy_connection_check_timeout = Some(proxy_connection_check_timeout)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UiConfiguration {
    pub user_token: Option<String>,
    pub proxy_addresses: Option<Vec<String>>,
    pub port: Option<String>,
    pub client_buffer_size: Option<usize>,
    pub message_framed_buffer_size: Option<usize>,
    pub thread_number: Option<usize>,
    pub compress: Option<bool>,
    pub init_proxy_connection_number: Option<usize>,
    pub min_proxy_connection_number: Option<usize>,
    pub proxy_connection_number_incremental: Option<usize>,
    pub proxy_connection_check_interval_seconds: Option<u64>,
    pub proxy_connection_check_timeout: Option<u64>,
}

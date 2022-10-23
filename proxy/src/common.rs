use chrono::Local;
use ppaass_io::PpaassMessageFramed;
use ppaass_protocol::PpaassMessagePayloadEncryptionSelector;
use tokio::net::TcpStream;
use tracing_subscriber::fmt::{format::Writer, time::FormatTime};

use crate::crypto::ProxyServerRsaCryptoFetcher;

pub(crate) type AgentMessageFramed = PpaassMessageFramed<TcpStream, ProxyServerRsaCryptoFetcher>;

pub(crate) struct ProxyServerPayloadEncryptionSelector {}

impl PpaassMessagePayloadEncryptionSelector for ProxyServerPayloadEncryptionSelector {}

pub struct ProxyServerLogTimer;

impl FormatTime for ProxyServerLogTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%FT%T%.3f"))
    }
}

use chrono::Local;

use ppaass_common::PpaassMessagePayloadEncryptionSelector;

use tracing_subscriber::fmt::{format::Writer, time::FormatTime};

pub(crate) struct ProxyServerPayloadEncryptionSelector {}

impl PpaassMessagePayloadEncryptionSelector for ProxyServerPayloadEncryptionSelector {}

pub struct ProxyServerLogTimer;

impl FormatTime for ProxyServerLogTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%FT%T%.3f"))
    }
}

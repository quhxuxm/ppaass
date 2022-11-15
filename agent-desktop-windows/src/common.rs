use chrono::Local;

use tracing_subscriber::fmt::{format::Writer, time::FormatTime};

pub(crate) struct AgentServerLogTimer;

impl FormatTime for AgentServerLogTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%FT%T%.3f"))
    }
}

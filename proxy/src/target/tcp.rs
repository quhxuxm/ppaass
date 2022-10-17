use anyhow::Result;
use tokio::sync::mpsc::Receiver;

pub(crate) struct TcpMessage {}
pub(crate) struct TargetTcpLoop {
    tcp_message_receiver: Receiver<TcpMessage>,
}

impl TargetTcpLoop {
    pub(crate) fn new(tcp_message_receiver: Receiver<TcpMessage>) -> Self {
        Self { tcp_message_receiver }
    }
    pub(crate) async fn exec(&mut self) -> Result<()> {
        loop {
            let tcp_message = match self.tcp_message_receiver.recv().await {
                None => {
                    return Ok(());
                },
                Some(v) => v,
            };
        }
    }
}

use anyhow::Result;
use tokio::sync::mpsc::{channel, Receiver, Sender};

pub(crate) struct TargetInboundMessage {}
pub(crate) struct TargetOutboundMessage {}
pub(crate) struct TargetTcpTransport {
    agent_tcp_message_inbound_receiver: Receiver<TargetOutboundMessage>,
    agent_tcp_message_outbound_sender: Sender<TargetInboundMessage>,
}

impl TargetTcpTransport {
    pub(crate) fn new(agent_tcp_message_inbound_receiver: Receiver<TargetOutboundMessage>) -> (Self, Receiver<TargetInboundMessage>) {
        let (agent_tcp_message_outbound_sender, agent_tcp_message_outbound_receiver) = channel::<TargetInboundMessage>(1024);
        (
            Self {
                agent_tcp_message_inbound_receiver,
                agent_tcp_message_outbound_sender,
            },
            agent_tcp_message_outbound_receiver,
        )
    }
    pub(crate) async fn exec(self) -> Result<()> {
        let mut agent_tcp_message_inbound_receiver = self.agent_tcp_message_inbound_receiver;
        tokio::spawn(async move {
            loop {
                let agent_tcp_message = match agent_tcp_message_inbound_receiver.recv().await {
                    None => {
                        return;
                    },
                    Some(v) => v,
                };
            }
        });
        Ok(())
    }
}

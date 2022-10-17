use anyhow::Result;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use super::{TargetTransportInboundValue, TargetTransportOutboundValue};

pub(crate) struct TargetTransport {
    inbound_receiver: Receiver<TargetTransportInboundValue>,
    outbound_sender: Sender<TargetTransportOutboundValue>,
}

impl TargetTransport {
    pub(crate) fn new(inbound_receiver: Receiver<TargetTransportInboundValue>) -> (Self, Receiver<TargetTransportOutboundValue>) {
        let (outbound_sender, outbound_receiver) = channel::<TargetTransportOutboundValue>(1024);
        (
            Self {
                outbound_sender,
                inbound_receiver,
            },
            outbound_receiver,
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

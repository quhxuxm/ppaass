use tokio::{
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
};

use super::{AgentToTargetData, AgentToTargetDataType, TargetToAgentData};

#[derive(Debug)]
pub(crate) struct TargetEdge {
    transport_id: String,
    target_tcp_stream: Option<TcpStream>,
    agent_to_target_data_receiver: Receiver<AgentToTargetData>,
    target_to_agent_data_sender: Sender<TargetToAgentData>,
}

impl TargetEdge {
    pub(crate) fn new(
        transport_id: String, agent_to_target_data_receiver: Receiver<AgentToTargetData>, target_to_agent_data_sender: Sender<TargetToAgentData>,
    ) -> Self {
        Self {
            transport_id,
            target_tcp_stream: None,
            agent_to_target_data_receiver,
            target_to_agent_data_sender,
        }
    }

    pub(crate) async fn exec(self) {
        let mut target_edge_request_receiver = self.target_edge_request_receiver;
        tokio::spawn(async move {
            loop {
                let AgentToTargetData { data_type: request_type } = match target_edge_request_receiver.recv().await {
                    None => {
                        return;
                    },
                    Some(v) => v,
                };
                match request_type {
                    AgentToTargetDataType::TcpInitialize { target_address } => {
                        // let target_address = target_address.ok_or(PpaassError::CodecError)?;
                        // let target_socket_addrs = target_address.to_socket_addrs()?;
                        // let target_socket_addrs = target_socket_addrs.collect::<Vec<SocketAddr>>();
                        // let target_tcp_stream = match TcpStream::connect(target_socket_addrs.as_slice()).await {
                        //     Err(e) => {
                        //         return Err(anyhow!(PpaassError::IoError { source: e }));
                        //     },
                        //     Ok(v) => v,
                        // };
                        // let connection_id = generate_uuid();
                        // let tcp_initialize_success_message_payload =
                        //     PpaassMessagePayload::new(Some(connection_id), source_address, Some(target_address), payload_type, data);
                        // let payload_bytes: Vec<u8> = tcp_initialize_success_message_payload.try_into()?;
                        // let tcp_initialize_success_message = PpaassMessage::new(
                        //     user_token,
                        //     ppaass_protocol::PpaassMessagePayloadEncryption::Aes("".as_bytes().to_vec()),
                        //     payload_bytes,
                        // );
                    },
                    TargetTcpTransportInputType::Relay { data } => {},
                }
            }
        });
    }
}

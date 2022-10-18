use anyhow::Result;
use ppaass_common::generate_uuid;
use tokio::{
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
};

use super::{TargetTcpTransportInput, TargetTcpTransportInputType, TargetTcpTransportOutput};

pub(crate) struct TargetTcpTransport {
    id: String,
    input_receiver: Receiver<TargetTcpTransportInput>,
    output_sender: Sender<TargetTcpTransportOutput>,
    target_tcp_stream: Option<TcpStream>,
}

impl TargetTcpTransport {
    pub(crate) fn new(input_receiver: Receiver<TargetTcpTransportInput>, output_sender: Sender<TargetTcpTransportOutput>) -> Self {
        Self {
            id: generate_uuid(),
            input_receiver,
            output_sender,
            target_tcp_stream: None,
        }
    }

    pub(crate) fn get_id(&self) -> &str {
        &self.id
    }

    pub(crate) async fn exec(self) -> Result<()> {
        let mut input_receiver = self.input_receiver;
        tokio::spawn(async move {
            loop {
                let TargetTcpTransportInput { input_type } = match input_receiver.recv().await {
                    None => {
                        return;
                    },
                    Some(v) => v,
                };
                match input_type {
                    TargetTcpTransportInputType::Connect { target_address } => {
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
        Ok(())
    }
}

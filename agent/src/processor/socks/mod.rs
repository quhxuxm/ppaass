use bytes::BytesMut;

use futures::{SinkExt, StreamExt};
use ppaass_common::{
    tcp::{TcpInitResponse, TcpInitResponseType},
    PpaassMessage, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassMessageProxyPayload, PpaassMessageProxyPayloadType, PpaassNetAddress,
};

use log::{debug, error};

use tokio::net::TcpStream;
use tokio_util::codec::{Framed, FramedParts};

use self::message::Socks5InitCommandResultStatus;

use crate::{
    config::AGENT_CONFIG,
    connection::PROXY_CONNECTION_FACTORY,
    error::{AgentError, DecoderError, EncoderError, NetworkError},
    processor::{
        socks::{
            codec::{Socks5AuthCommandContentCodec, Socks5InitCommandContentCodec},
            message::{Socks5AuthCommandResult, Socks5InitCommandResult, Socks5InitCommandType},
        },
        ClientDataRelayInfo, ClientProtocolProcessor,
    },
    AgentServerPayloadEncryptionTypeSelector,
};

use ppaass_common::generate_uuid;

mod codec;
mod message;

pub(crate) struct Socks5ClientProcessor {
    client_tcp_stream: TcpStream,
    src_address: PpaassNetAddress,
}

impl Socks5ClientProcessor {
    pub(crate) fn new(client_tcp_stream: TcpStream, src_address: PpaassNetAddress) -> Self {
        Self {
            client_tcp_stream,
            src_address,
        }
    }

    pub(crate) async fn exec(self, initial_buf: BytesMut) -> Result<(), AgentError> {
        let client_tcp_stream = self.client_tcp_stream;
        let src_address = self.src_address;
        let mut auth_framed_parts = FramedParts::new(client_tcp_stream, Socks5AuthCommandContentCodec);
        auth_framed_parts.read_buf = initial_buf;
        let mut auth_framed = Framed::from_parts(auth_framed_parts);
        let auth_message = auth_framed
            .next()
            .await
            .ok_or(NetworkError::ConnectionExhausted)?
            .map_err(DecoderError::Socks5)?;
        debug!(
            "Client tcp connection [{src_address}] start socks5 authenticate process, authenticate methods in request: {:?}",
            auth_message.methods
        );
        let auth_response = Socks5AuthCommandResult::new(message::Socks5AuthMethod::NoAuthenticationRequired);
        auth_framed.send(auth_response).await.map_err(EncoderError::Socks5)?;
        let FramedParts { io: client_tcp_stream, .. } = auth_framed.into_parts();
        let mut init_framed = Framed::new(client_tcp_stream, Socks5InitCommandContentCodec);
        let init_message = init_framed
            .next()
            .await
            .ok_or(NetworkError::ConnectionExhausted)?
            .map_err(DecoderError::Socks5)?;
        debug!(
            "Client tcp connection [{src_address}] start socks5 init process, command type: {:?}, destination address: {:?}",
            init_message.request_type, init_message.dst_address
        );

        match init_message.request_type {
            Socks5InitCommandType::Bind => todo!(),
            Socks5InitCommandType::UdpAssociate => todo!(),
            Socks5InitCommandType::Connect => {
                Self::handle_connect_command(src_address, init_message.dst_address.into(), init_framed).await?;
            },
        }

        Ok(())
    }

    async fn handle_connect_command(
        src_address: PpaassNetAddress, dst_address: PpaassNetAddress, mut init_framed: Framed<TcpStream, Socks5InitCommandContentCodec>,
    ) -> Result<(), AgentError> {
        let user_token = AGENT_CONFIG
            .get_user_token()
            .ok_or(AgentError::Configuration("User token not configured.".to_string()))?;

        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(user_token, Some(generate_uuid().into_bytes()));
        let tcp_init_request =
            PpaassMessageGenerator::generate_tcp_init_request(user_token, src_address.clone(), dst_address.clone(), payload_encryption.clone())?;
        let mut proxy_connection = PROXY_CONNECTION_FACTORY.create_connection().await?;

        debug!(
            "Client tcp connection [{src_address}] take proxy connectopn [{}] to do proxy.",
            proxy_connection.get_connection_id()
        );
        proxy_connection.send(tcp_init_request).await?;
        let proxy_message = proxy_connection.next().await.ok_or(NetworkError::ConnectionExhausted)??;
        let PpaassMessage { payload, user_token, .. } = proxy_message;
        let PpaassMessageProxyPayload { payload_type, data } = payload.as_slice().try_into()?;
        let tcp_init_response = match payload_type {
            PpaassMessageProxyPayloadType::TcpInit => data.as_slice().try_into()?,
            _ => {
                error!("Client tcp connection [{src_address}] receive invalid message from proxy, payload type: {payload_type:?}");
                return Err(AgentError::InvalidProxyResponse("Not a tcp init response.".to_string()));
            },
        };
        let TcpInitResponse {
            id: tcp_loop_key,
            dst_address,
            response_type,
            ..
        } = tcp_init_response;
        match response_type {
            TcpInitResponseType::Success => {
                debug!("Client tcp connection [{src_address}] receive init tcp loop init response: {tcp_loop_key}");
            },
            TcpInitResponseType::Fail => {
                error!("Client tcp connection [{src_address}] fail to do tcp loop init, tcp loop key: [{tcp_loop_key}]");
                return Err(AgentError::InvalidProxyResponse("Proxy tcp init fail.".to_string()));
            },
        }
        let socks5_init_success_result = Socks5InitCommandResult::new(Socks5InitCommandResultStatus::Succeeded, Some(dst_address.clone().try_into()?));
        init_framed.send(socks5_init_success_result).await.map_err(EncoderError::Socks5)?;
        let FramedParts { io: client_tcp_stream, .. } = init_framed.into_parts();
        debug!("Client tcp connection [{src_address}] success to do sock5 handshake begin to relay, tcp loop key: [{tcp_loop_key}].");
        ClientProtocolProcessor::relay(ClientDataRelayInfo {
            client_tcp_stream,
            src_address: src_address.clone(),
            dst_address,
            user_token,
            payload_encryption,
            proxy_connection,
            init_data: None,
        })
        .await?;
        debug!("Client tcp connection [{src_address}] complete sock5 relay, tcp loop key: [{tcp_loop_key}].");
        Ok(())
    }
}

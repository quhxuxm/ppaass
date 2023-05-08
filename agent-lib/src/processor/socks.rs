use bytes::BytesMut;

use futures::{SinkExt, StreamExt};
use ppaass_common::{
    tcp::{TcpInitResponse, TcpInitResponseType},
    PpaassConnectionParts, PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryptionSelector, PpaassMessageProxyPayload,
    PpaassMessageProxyPayloadParts, PpaassMessageProxyPayloadType, PpaassNetAddress,
};

use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, FramedParts};
use tracing::{debug, error};

use self::message::Socks5InitCommandResultStatus;

use crate::{
    config::AgentServerConfig,
    error::{AgentError, DecoderError, EncoderError, NetworkError},
    pool::ProxyConnectionPool,
    processor::{
        socks::{
            codec::{Socks5AuthCommandContentCodec, Socks5InitCommandContentCodec},
            message::{
                Socks5AuthCommandContentParts, Socks5AuthCommandResultContent, Socks5InitCommandContentParts, Socks5InitCommandResultContent,
                Socks5InitCommandType,
            },
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

    pub(crate) async fn exec(
        self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>, initial_buf: BytesMut,
    ) -> Result<(), AgentError> {
        let client_tcp_stream = self.client_tcp_stream;
        let src_address = self.src_address;
        let mut auth_framed_parts = FramedParts::new(client_tcp_stream, Socks5AuthCommandContentCodec::default());
        auth_framed_parts.read_buf = initial_buf;
        let mut auth_framed = Framed::from_parts(auth_framed_parts);
        let auth_message = auth_framed
            .next()
            .await
            .ok_or(NetworkError::ConnectionExhausted)?
            .map_err(DecoderError::Socks5)?;
        let Socks5AuthCommandContentParts { methods } = auth_message.split();
        debug!("Client tcp connection [{src_address}] start socks5 authenticate process, authenticate methods in request: {methods:?}");
        let auth_response = Socks5AuthCommandResultContent::new(message::Socks5AuthMethod::NoAuthenticationRequired);
        auth_framed.send(auth_response).await.map_err(EncoderError::Socks5)?;
        let FramedParts { io: client_tcp_stream, .. } = auth_framed.into_parts();
        let mut init_framed = Framed::new(client_tcp_stream, Socks5InitCommandContentCodec::default());
        let init_message = init_framed
            .next()
            .await
            .ok_or(NetworkError::ConnectionExhausted)?
            .map_err(DecoderError::Socks5)?;
        let Socks5InitCommandContentParts { command_type, dst_address } = init_message.split();
        debug!("Client tcp connection [{src_address}] start socks5 init process, command type: {command_type:?}, destination address: {dst_address:?}");

        match command_type {
            Socks5InitCommandType::Bind => todo!(),
            Socks5InitCommandType::UdpAssociate => todo!(),
            Socks5InitCommandType::Connect => {
                Self::handle_connect_command(src_address, dst_address.into(), proxy_connection_pool, init_framed, configuration).await?;
            },
        }

        Ok(())
    }

    async fn handle_connect_command(
        src_address: PpaassNetAddress, dst_address: PpaassNetAddress, proxy_connection_pool: Arc<ProxyConnectionPool>,
        mut init_framed: Framed<TcpStream, Socks5InitCommandContentCodec>, configuration: Arc<AgentServerConfig>,
    ) -> Result<(), AgentError> {
        let user_token = configuration
            .get_user_token()
            .clone()
            .ok_or(AgentError::Configuration("User token not configured.".to_string()))?;

        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let tcp_init_request =
            PpaassMessageGenerator::generate_tcp_init_request(&user_token, src_address.clone(), dst_address.clone(), payload_encryption.clone())?;
        let proxy_connection = proxy_connection_pool.take_connection().await?;
        let PpaassConnectionParts {
            read_part: mut proxy_connection_read,
            write_part: mut proxy_connection_write,
            id: proxy_connection_id,
        } = proxy_connection.split();

        debug!("Client tcp connection [{src_address}] take proxy connectopn [{proxy_connection_id}] to do proxy");
        proxy_connection_write.send(tcp_init_request).await?;
        let proxy_message = proxy_connection_read.next().await.ok_or(NetworkError::ConnectionExhausted)??;
        let PpaassMessageParts {
            payload: proxy_message_payload_bytes,
            user_token,
            ..
        } = proxy_message.split();
        let PpaassMessageProxyPayloadParts { payload_type, data } = TryInto::<PpaassMessageProxyPayload>::try_into(proxy_message_payload_bytes)?.split();
        let tcp_init_response = match payload_type {
            PpaassMessageProxyPayloadType::TcpInit => TryInto::<TcpInitResponse>::try_into(data)?,
            _ => {
                error!("Client tcp connection [{src_address}] receive invalid message from proxy, payload type: {payload_type:?}");
                return Err(AgentError::InvalidProxyResponse("Not a tcp init response.".to_string()));
            },
        };
        let TcpInitResponse {
            unique_key: tcp_loop_key,
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
        let socks5_init_success_result = Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Succeeded, Some(dst_address.clone().into()));
        init_framed.send(socks5_init_success_result).await.map_err(EncoderError::Socks5)?;
        let FramedParts { io: client_tcp_stream, .. } = init_framed.into_parts();
        debug!("Client tcp connection [{src_address}] success to do sock5 handshake begin to relay, tcp loop key: [{tcp_loop_key}].");
        ClientProtocolProcessor::relay(ClientDataRelayInfo {
            client_tcp_stream,
            src_address: src_address.clone(),
            dst_address,
            user_token,
            payload_encryption,
            proxy_connection_read,
            proxy_connection_write,
            configuration,
            init_data: None,
        })
        .await?;
        debug!("Client tcp connection [{src_address}] complete sock5 relay, tcp loop key: [{tcp_loop_key}].");
        Ok(())
    }
}

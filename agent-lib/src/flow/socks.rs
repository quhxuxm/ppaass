use bytes::BytesMut;

use futures::{SinkExt, StreamExt};
use ppaass_common::{
    tcp::{TcpInitResponse, TcpInitResponseType},
    PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryptionSelector, PpaassMessageProxyPayload, PpaassMessageProxyPayloadParts,
    PpaassMessageProxyPayloadType, PpaassNetAddress, RsaCryptoFetcher,
};

use std::{fmt::Display, sync::Arc};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, FramedParts};
use tracing::{debug, error};

use self::message::Socks5InitCommandResultStatus;

use crate::{
    config::AgentServerConfig,
    crypto::AgentServerRsaCryptoFetcher,
    flow::{
        socks::{
            codec::{Socks5AuthCommandContentCodec, Socks5InitCommandContentCodec},
            message::{
                Socks5AuthCommandContentParts, Socks5AuthCommandResultContent, Socks5InitCommandContentParts, Socks5InitCommandResultContent,
                Socks5InitCommandType,
            },
        },
        ClientDataRelayInfo, ClientFlow,
    },
    pool::ProxyConnectionPool,
    AgentServerPayloadEncryptionTypeSelector,
};
use anyhow::{Context, Result};
use ppaass_common::generate_uuid;

mod codec;
mod message;

pub(crate) struct Socks5Flow {
    client_io: TcpStream,
    src_address: PpaassNetAddress,
}

impl Socks5Flow {
    pub(crate) fn new(client_io: TcpStream, src_address: PpaassNetAddress) -> Self {
        Self { client_io, src_address }
    }

    pub(crate) async fn exec<R, I>(
        self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>, initial_buf: BytesMut,
    ) -> Result<()>
    where
        R: RsaCryptoFetcher + Send + Sync + 'static,
        I: AsRef<str> + Send + Sync + Clone + Display + 'static,
    {
        let client_io = self.client_io;
        let src_address = self.src_address;
        let mut auth_framed_parts = FramedParts::new(client_io, Socks5AuthCommandContentCodec::default());
        auth_framed_parts.read_buf = initial_buf;
        let mut auth_framed = Framed::from_parts(auth_framed_parts);
        let auth_message = auth_framed
            .next()
            .await
            .context(format!(
                "Client tcp connection [{src_address}] nothing to read from socks5 client in authenticate phase"
            ))?
            .context(format!(
                "Client tcp connection [{src_address}] error happen when read socks5 client data in authenticate phase"
            ))?;
        let Socks5AuthCommandContentParts { methods } = auth_message.split();
        debug!("Client tcp connection [{src_address}] start socks5 authenticate process, authenticate methods in request: {methods:?}");
        let auth_response = Socks5AuthCommandResultContent::new(message::Socks5AuthMethod::NoAuthenticationRequired);
        if let Err(e) = auth_framed.send(auth_response).await {
            error!("Client tcp connection [{src_address}] fail reply auth success in socks5 flow.");
            return Err(e);
        };

        let FramedParts { io: client_io, .. } = auth_framed.into_parts();
        let mut init_framed = Framed::new(client_io, Socks5InitCommandContentCodec::default());
        let init_message = init_framed
            .next()
            .await
            .context(format!(
                "Client tcp connection [{src_address}] nothing to read from socks5 client in init phase"
            ))?
            .context(format!(
                "Client tcp connection [{src_address}] error happen when read socks5 client data in init phase"
            ))?;
        let Socks5InitCommandContentParts { command_type, dst_address } = init_message.split();
        debug!("Client tcp connection [{src_address}] start socks5 init process, command type: {command_type:?}, destination address: {dst_address:?}");

        match command_type {
            Socks5InitCommandType::Bind => todo!(),
            Socks5InitCommandType::UdpAssociate => todo!(),
            Socks5InitCommandType::Connect => {
                Self::handle_connect_command::<AgentServerRsaCryptoFetcher, String>(
                    src_address,
                    dst_address.into(),
                    proxy_connection_pool,
                    init_framed,
                    configuration,
                )
                .await?;
            },
        }

        Ok(())
    }

    async fn handle_connect_command<R, I>(
        src_address: PpaassNetAddress, dst_address: PpaassNetAddress, proxy_connection_pool: Arc<ProxyConnectionPool>,
        mut init_framed: Framed<TcpStream, Socks5InitCommandContentCodec>, configuration: Arc<AgentServerConfig>,
    ) -> Result<(), anyhow::Error>
    where
        R: RsaCryptoFetcher + Send + Sync + 'static,
        I: AsRef<str> + Send + Sync + Clone + Display + 'static,
    {
        let user_token = configuration
            .get_user_token()
            .as_ref()
            .context(format!("Client tcp connection [{src_address}] can not get user token form configuration file"))?
            .clone();

        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let tcp_init_request =
            PpaassMessageGenerator::generate_tcp_init_request(&user_token, src_address.clone(), dst_address.clone(), payload_encryption.clone())?;
        let proxy_connection = proxy_connection_pool.take_connection().await.context(format!(
            "Client tcp connection [{src_address}] fail to take proxy connection from connection poool because of error"
        ))?;
        let proxy_connection_id = proxy_connection.connection_id.clone();
        let (mut proxy_connection_read, mut proxy_connection_write) = proxy_connection.split()?;
        debug!("Client tcp connection [{src_address}] take proxy connectopn [{proxy_connection_id}] to do proxy");
        if let Err(e) = proxy_connection_write.send(tcp_init_request).await {
            error!("Client tcp connection [{src_address}] fail to send tcp loop init to proxy because of error: {e:?}");
            return Err(anyhow::anyhow!(format!(
                "Client tcp connection [{src_address}] fail to send tcp loop init request to proxy because of error: {e:?}"
            )));
        };
        let proxy_message = proxy_connection_read
            .next()
            .await
            .context(format!(
                "Client tcp connection [{src_address}] nothing to read from proxy for init tcp loop response"
            ))?
            .context(format!(
                "Client tcp connection [{src_address}] error happen when read proxy message for init tcp loop response"
            ))?;
        let PpaassMessageParts {
            payload: proxy_message_payload_bytes,
            user_token,
            ..
        } = proxy_message.split();
        let PpaassMessageProxyPayloadParts { payload_type, data } = TryInto::<PpaassMessageProxyPayload>::try_into(proxy_message_payload_bytes)?.split();
        let tcp_loop_init_response = match payload_type {
            PpaassMessageProxyPayloadType::TcpInit => TryInto::<TcpInitResponse>::try_into(data)?,
            _ => {
                error!("Client tcp connection [{src_address}] receive invalid message from proxy, payload type: {payload_type:?}");
                return Err(anyhow::anyhow!(format!(
                    "Client tcp connection [{src_address}] receive invalid message from proxy, payload type: {payload_type:?}"
                )));
            },
        };
        let TcpInitResponse {
            loop_key: tcp_loop_key,
            dst_address,
            response_type,
            ..
        } = tcp_loop_init_response;
        match response_type {
            TcpInitResponseType::Success => {
                debug!("Client tcp connection [{src_address}] receive init tcp loop init response: {tcp_loop_key}");
            },
            TcpInitResponseType::Fail => {
                error!("Client tcp connection [{src_address}] fail to do tcp loop init, tcp loop key: [{tcp_loop_key}]");
                return Err(anyhow::anyhow!(format!(
                    "Client tcp connection [{src_address}] fail to do tcp loop init, tcp loop key: [{tcp_loop_key}]"
                )));
            },
        }
        let socks5_init_success_result = Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Succeeded, Some(dst_address.clone().into()));
        if let Err(e) = init_framed.send(socks5_init_success_result).await {
            error!("Client tcp connection [{src_address}] fail reply init success in socks5 flow, tcp loop key: [{tcp_loop_key}].");
            return Err(e);
        };
        let FramedParts { io: client_io, .. } = init_framed.into_parts();
        debug!("Client tcp connection [{src_address}] success to do sock5 handshake begin to relay, tcp loop key: [{tcp_loop_key}].");
        ClientFlow::relay(ClientDataRelayInfo {
            client_io,
            src_address: src_address.clone(),
            dst_address,
            tcp_loop_key: tcp_loop_key.clone(),
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

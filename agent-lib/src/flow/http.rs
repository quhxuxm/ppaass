pub(crate) mod codec;

use std::net::SocketAddr;
use std::sync::Arc;

use bytecodec::{bytes::BytesEncoder, EncodeExt};

use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use httpcodec::{BodyEncoder, HttpVersion, ReasonPhrase, RequestEncoder, Response, StatusCode};
use ppaass_common::{
    generate_uuid,
    tcp_loop::{TcpLoopInitResponsePayload, TcpLoopInitResponseType},
    PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryptionSelector, PpaassMessageProxyPayload, PpaassMessageProxyPayloadParts,
    PpaassMessageProxyPayloadType, PpaassNetAddress,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, FramedParts};
use tracing::{debug, error};
use url::Url;

use crate::{
    config::AgentServerConfig,
    flow::{http::codec::HttpCodec, ClientDataRelayInfo, ClientFlow},
    pool::ProxyConnectionPool,
    AgentServerPayloadEncryptionTypeSelector,
};
use anyhow::{Context, Result};

const HTTPS_SCHEMA: &str = "https";
const SCHEMA_SEP: &str = "://";
const CONNECT_METHOD: &str = "connect";
const HTTPS_DEFAULT_PORT: u16 = 443;
const HTTP_DEFAULT_PORT: u16 = 80;
const OK_CODE: u16 = 200;
const ERROR_CODE: u16 = 400;
const ERROR_REASON: &str = " ";
const CONNECTION_ESTABLISHED: &str = "Connection Established";

pub(crate) struct HttpFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    client_io: T,
    client_socket_address: SocketAddr,
}

impl<T> HttpFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub(crate) fn new(client_io: T, client_socket_address: SocketAddr) -> Self {
        Self {
            client_io,
            client_socket_address,
        }
    }

    pub(crate) async fn exec(
        self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>, initial_buf: BytesMut,
    ) -> Result<()> {
        let client_io = self.client_io;
        let client_socket_address = self.client_socket_address;
        let mut framed_parts = FramedParts::new(client_io, HttpCodec::default());
        framed_parts.read_buf = initial_buf;
        let mut http_framed = Framed::from_parts(framed_parts);
        let http_message = http_framed
            .next()
            .await
            .context(format!(
                "Client tcp connection [{client_socket_address}] nothing to read from http client data in init phase"
            ))?
            .context(format!(
                "Client tcp connection [{client_socket_address}] error happen when read http client data in init phase"
            ))?;
        let http_method = http_message.method().to_string().to_lowercase();
        let (request_url, init_data) = if http_method == CONNECT_METHOD {
            (format!("{}{}{}", HTTPS_SCHEMA, SCHEMA_SEP, http_message.request_target()), None)
        } else {
            let request_url = http_message.request_target().to_string();
            let mut http_data_encoder = RequestEncoder::<BodyEncoder<BytesEncoder>>::default();
            let encode_result = match http_data_encoder.encode_into_bytes(http_message) {
                Err(e) => {
                    error!("Fail to encode http data because of error: {:#?} ", e);
                    return Err(anyhow::anyhow!(e));
                },
                Ok(v) => v,
            };
            (request_url, Some(encode_result))
        };

        let parsed_request_url = Url::parse(request_url.as_str()).map_err(|e| {
            error!("Fail to parse request url because of error: {:#?}", e);
            e
        })?;
        let target_port = match parsed_request_url.port() {
            None => match parsed_request_url.scheme() {
                HTTPS_SCHEMA => HTTPS_DEFAULT_PORT,
                _ => HTTP_DEFAULT_PORT,
            },
            Some(v) => v,
        };
        let target_host = match parsed_request_url.host() {
            None => {
                return Err(anyhow::anyhow!("Fail to parse target host from request url"));
            },
            Some(v) => v.to_string(),
        };
        let dest_address = PpaassNetAddress::Domain {
            host: target_host,
            port: target_port,
        };

        let user_token = configuration
            .get_user_token()
            .as_ref()
            .context(format!(
                "Client tcp connection [{client_socket_address}] can not get user token form configuration file"
            ))?
            .clone();
        let src_address: PpaassNetAddress = self.client_socket_address.into();

        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let tcp_loop_init_request =
            PpaassMessageGenerator::generate_tcp_loop_init_request(&user_token, src_address.clone(), dest_address.clone(), payload_encryption.clone())?;

        let proxy_connection = proxy_connection_pool.take_connection().await.context(format!(
            "Client tcp connection [{client_socket_address}] fail to take proxy connection from connection poool because of error"
        ))?;

        let proxy_connection_id = proxy_connection.id.clone();
        let (mut proxy_connection_read, mut proxy_connection_write) = proxy_connection.split()?;

        debug!("Client tcp connection [{client_socket_address}] take proxy connectopn [{proxy_connection_id}] to do proxy");

        if let Err(e) = proxy_connection_write.send(tcp_loop_init_request).await {
            error!("Client tcp connection [{client_socket_address}] fail to send tcp loop init to proxy because of error: {e:?}");
            return Err(anyhow::anyhow!(format!(
                "Client tcp connection [{client_socket_address}] fail to send tcp loop init request to proxy because of error: {e:?}"
            )));
        };

        let proxy_message = proxy_connection_read
            .next()
            .await
            .context(format!(
                "Client tcp connection [{client_socket_address}] nothing to read from proxy for init tcp loop response"
            ))?
            .context(format!(
                "Client tcp connection [{client_socket_address}] error happen when read proxy message for init tcp loop response"
            ))?;

        let PpaassMessageParts {
            payload_bytes: proxy_message_payload_bytes,
            user_token,
            ..
        } = proxy_message.split();
        let PpaassMessageProxyPayloadParts { payload_type, data } = TryInto::<PpaassMessageProxyPayload>::try_into(proxy_message_payload_bytes)?.split();
        let tcp_loop_init_response = match payload_type {
            PpaassMessageProxyPayloadType::TcpLoopInit => TryInto::<TcpLoopInitResponsePayload>::try_into(data)?,
            _ => {
                error!("Client tcp connection [{client_socket_address}] receive invalid message from proxy, payload type: {payload_type:?}");
                return Err(anyhow::anyhow!(format!(
                    "Client tcp connection [{client_socket_address}] receive invalid message from proxy, payload type: {payload_type:?}"
                )));
            },
        };

        let TcpLoopInitResponsePayload {
            loop_key: tcp_loop_key,
            response_type,
            ..
        } = tcp_loop_init_response;

        match response_type {
            TcpLoopInitResponseType::Success => {
                debug!("Client tcp connection [{client_socket_address}] receive init tcp loop init response: {tcp_loop_key}");
            },
            TcpLoopInitResponseType::Fail => {
                error!("Client tcp connection [{client_socket_address}] fail to do tcp loop init, tcp loop key: [{tcp_loop_key}]");
                return Err(anyhow::anyhow!(format!(
                    "Client tcp connection [{client_socket_address}] fail to do tcp loop init, tcp loop key: [{tcp_loop_key}]"
                )));
            },
        }
        if init_data.is_none() {
            //For https proxy
            let http_connect_success_response = Response::new(
                HttpVersion::V1_1,
                StatusCode::new(OK_CODE).unwrap(),
                ReasonPhrase::new(CONNECTION_ESTABLISHED).unwrap(),
                vec![],
            );
            http_framed.send(http_connect_success_response).await?;
        }

        let FramedParts { io: client_io, .. } = http_framed.into_parts();
        ClientFlow::relay(ClientDataRelayInfo {
            client_io,
            client_socket_address,
            tcp_loop_key,
            user_token,
            payload_encryption,
            proxy_connection_read,
            proxy_connection_write,
            configuration,
            init_data,
        })
        .await?;

        Ok(())
    }
}

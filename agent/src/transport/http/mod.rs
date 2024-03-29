pub(crate) mod codec;

use anyhow::anyhow;
use async_trait::async_trait;
use bytecodec::{bytes::BytesEncoder, EncodeExt};

use bytes::Bytes;
use derive_more::Constructor;
use futures::{SinkExt, StreamExt};
use httpcodec::{BodyEncoder, HttpVersion, ReasonPhrase, RequestEncoder, Response, StatusCode};
use log::{debug, error};
use ppaass_common::{
    random_32_bytes, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassProxyMessage, PpaassProxyMessagePayload, PpaassUnifiedAddress,
};

use ppaass_common::tcp::{ProxyTcpInitResult, ProxyTcpPayload};
use tokio_util::codec::{Framed, FramedParts};
use url::Url;

use crate::{
    config::AGENT_CONFIG,
    connection::PROXY_CONNECTION_FACTORY,
    error::{AgentError, ConversionError, DecoderError, EncoderError, NetworkError},
    transport::{http::codec::HttpCodec, ClientTransportDataRelayInfo, ClientTransportTcpDataRelay},
    AgentServerPayloadEncryptionTypeSelector,
};

use super::{dispatcher::ClientTransportHandshakeInfo, ClientTransportHandshake, ClientTransportRelay};

const HTTPS_SCHEMA: &str = "https";
const SCHEMA_SEP: &str = "://";
const CONNECT_METHOD: &str = "connect";
const HTTPS_DEFAULT_PORT: u16 = 443;
const HTTP_DEFAULT_PORT: u16 = 80;
const OK_CODE: u16 = 200;
const CONNECTION_ESTABLISHED: &str = "Connection Established";

#[derive(Debug, Constructor)]
pub(crate) struct HttpClientTransport;

impl ClientTransportRelay for HttpClientTransport {}

#[async_trait]
impl ClientTransportHandshake for HttpClientTransport {
    async fn handshake(
        &self, handshake_info: ClientTransportHandshakeInfo,
    ) -> Result<(ClientTransportDataRelayInfo, Box<dyn ClientTransportRelay + Send + Sync>), AgentError> {
        let ClientTransportHandshakeInfo {
            initial_buf,
            src_address,
            client_tcp_stream,
        } = handshake_info;
        let mut framed_parts = FramedParts::new(client_tcp_stream, HttpCodec::default());
        framed_parts.read_buf = initial_buf;
        let mut http_framed = Framed::from_parts(framed_parts);
        let http_message = http_framed.next().await.ok_or(NetworkError::ConnectionExhausted)?.map_err(DecoderError::Http)?;
        let http_method = http_message.method().to_string().to_lowercase();
        let (request_url, init_data) = if http_method == CONNECT_METHOD {
            (format!("{}{}{}", HTTPS_SCHEMA, SCHEMA_SEP, http_message.request_target()), None)
        } else {
            let request_url = http_message.request_target().to_string();
            let mut http_data_encoder = RequestEncoder::<BodyEncoder<BytesEncoder>>::default();
            let encode_result: Bytes = http_data_encoder
                .encode_into_bytes(http_message)
                .map_err(|e| AgentError::Encode(EncoderError::Http(e.into())))?
                .into();
            (request_url, Some(encode_result))
        };

        let parsed_request_url = Url::parse(request_url.as_str()).map_err(ConversionError::UrlFormat)?;
        let target_port = parsed_request_url.port().unwrap_or_else(|| match parsed_request_url.scheme() {
            HTTPS_SCHEMA => HTTPS_DEFAULT_PORT,
            _ => HTTP_DEFAULT_PORT,
        });
        let target_host = parsed_request_url
            .host()
            .ok_or(ConversionError::NoHost(parsed_request_url.to_string()))?
            .to_string();
        if target_host.eq("0.0.0.1") || target_host.eq("127.0.0.1") {
            return Err(AgentError::Other(anyhow!("0.0.0.1 or 127.0.0.1 is not a valid destination address")));
        }
        let dst_address = PpaassUnifiedAddress::Domain {
            host: target_host,
            port: target_port,
        };

        let user_token = AGENT_CONFIG.get_user_token();
        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(user_token, Some(random_32_bytes()));
        let tcp_init_request = PpaassMessageGenerator::generate_agent_tcp_init_message(
            user_token.to_string(),
            src_address.clone(),
            dst_address.clone(),
            payload_encryption.clone(),
        )?;

        let mut proxy_connection = PROXY_CONNECTION_FACTORY.create_connection().await?;
        debug!(
            "Client tcp connection [{src_address}] take proxy connection [{}] to do proxy",
            proxy_connection.get_connection_id()
        );
        proxy_connection.send(tcp_init_request).await?;

        let proxy_message = proxy_connection.next().await.ok_or(NetworkError::ConnectionExhausted)??;

        let PpaassProxyMessage {
            payload: proxy_message_payload,
            ..
        } = proxy_message;

        let PpaassProxyMessagePayload::Tcp(ProxyTcpPayload::Init { result, .. }) = proxy_message_payload else {
            return Err(AgentError::InvalidProxyResponse("Not a tcp init response.".to_string()));
        };
        let tunnel_id = match result {
            ProxyTcpInitResult::Success(tunnel_id) => tunnel_id,
            ProxyTcpInitResult::Fail(reason) => {
                error!("Client http tcp connection [{src_address}] fail to initialize tcp connection with proxy because of reason: {reason:?}");
                return Err(AgentError::InvalidProxyResponse(format!(
                    "Client http tcp connection [{src_address}] fail to initialize tcp connection with proxy because of reason: {reason:?}"
                )));
            },
        };
        debug!("Client http tcp connection [{src_address}] success to initialize tcp connection with proxy on tunnel: {tunnel_id}");
        if init_data.is_none() {
            //For https proxy
            let http_connect_success_response = Response::new(
                HttpVersion::V1_1,
                StatusCode::new(OK_CODE).unwrap(),
                ReasonPhrase::new(CONNECTION_ESTABLISHED).unwrap(),
                vec![],
            );
            http_framed.send(http_connect_success_response).await.map_err(EncoderError::Http)?;
        }
        let FramedParts { io: client_tcp_stream, .. } = http_framed.into_parts();
        Ok((
            ClientTransportDataRelayInfo::Tcp(ClientTransportTcpDataRelay {
                tunnel_id,
                client_tcp_stream,
                src_address,
                dst_address,
                proxy_connection,
                init_data,
            }),
            Box::new(Self),
        ))
    }
}

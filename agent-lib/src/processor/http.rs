pub(crate) mod codec;

use std::sync::Arc;

use bytecodec::{bytes::BytesEncoder, EncodeExt};

use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use httpcodec::{BodyEncoder, HttpVersion, ReasonPhrase, RequestEncoder, Response, StatusCode};
use ppaass_common::{
    generate_uuid,
    tcp::{TcpInitResponse, TcpInitResponseType},
    PpaassConnectionParts, PpaassMessage, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassMessageProxyPayload,
    PpaassMessageProxyPayloadType, PpaassNetAddress,
};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, FramedParts};
use tracing::{debug, error};
use url::Url;

use crate::{
    config::AgentServerConfig,
    error::{AgentError, ConversionError, DecoderError, EncoderError, NetworkError},
    pool::ProxyConnectionPool,
    processor::{http::codec::HttpCodec, ClientDataRelayInfo, ClientProtocolProcessor},
    AgentServerPayloadEncryptionTypeSelector,
};

const HTTPS_SCHEMA: &str = "https";
const SCHEMA_SEP: &str = "://";
const CONNECT_METHOD: &str = "connect";
const HTTPS_DEFAULT_PORT: u16 = 443;
const HTTP_DEFAULT_PORT: u16 = 80;
const OK_CODE: u16 = 200;
const CONNECTION_ESTABLISHED: &str = "Connection Established";

pub(crate) struct HttpClientProcessor {
    client_tcp_stream: TcpStream,
    src_address: PpaassNetAddress,
}

impl HttpClientProcessor {
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
            let encode_result = http_data_encoder
                .encode_into_bytes(http_message)
                .map_err(|e| AgentError::Encode(EncoderError::Http(e.into())))?;
            (request_url, Some(encode_result))
        };

        let parsed_request_url = Url::parse(request_url.as_str()).map_err(ConversionError::UrlFormat)?;
        let target_port = match parsed_request_url.port() {
            None => match parsed_request_url.scheme() {
                HTTPS_SCHEMA => HTTPS_DEFAULT_PORT,
                _ => HTTP_DEFAULT_PORT,
            },
            Some(v) => v,
        };
        let target_host = parsed_request_url
            .host()
            .ok_or(ConversionError::NoHost(parsed_request_url.to_string()))?
            .to_string();
        let dst_address = PpaassNetAddress::Domain {
            host: target_host,
            port: target_port,
        };

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
        ClientProtocolProcessor::relay(ClientDataRelayInfo {
            client_tcp_stream,
            src_address,
            dst_address,
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

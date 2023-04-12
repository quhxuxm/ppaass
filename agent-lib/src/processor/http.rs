pub(crate) mod codec;

use std::fmt::Display;
use std::sync::Arc;

use bytecodec::{bytes::BytesEncoder, EncodeExt};

use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use httpcodec::{BodyEncoder, HttpVersion, ReasonPhrase, RequestEncoder, Response, StatusCode};
use ppaass_common::{
    generate_uuid,
    tcp::{TcpInitResponse, TcpInitResponseType},
    PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryptionSelector, PpaassMessageProxyPayload, PpaassMessageProxyPayloadParts,
    PpaassMessageProxyPayloadType, PpaassNetAddress, RsaCryptoFetcher,
};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, FramedParts};
use tracing::{debug, error};
use url::Url;

use crate::{
    config::AgentServerConfig,
    pool::ProxyConnectionPool,
    processor::{http::codec::HttpCodec, ClientDataRelayInfo, ClientProcessor},
    AgentServerPayloadEncryptionTypeSelector,
};
use anyhow::{Context, Result};

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

    pub(crate) async fn exec<R, I>(
        self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>, initial_buf: BytesMut,
    ) -> Result<()>
    where
        R: RsaCryptoFetcher + Send + Sync + 'static,
        I: AsRef<str> + Send + Sync + Clone + Display + 'static,
    {
        let client_tcp_stream = self.client_tcp_stream;
        let src_address = self.src_address;
        let mut framed_parts = FramedParts::new(client_tcp_stream, HttpCodec::default());
        framed_parts.read_buf = initial_buf;
        let mut http_framed = Framed::from_parts(framed_parts);
        let http_message = http_framed
            .next()
            .await
            .context(format!(
                "Client tcp connection [{src_address}] nothing to read from http client data in init phase"
            ))?
            .context(format!(
                "Client tcp connection [{src_address}] error happen when read http client data in init phase"
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
        let dst_address = PpaassNetAddress::Domain {
            host: target_host,
            port: target_port,
        };

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
        let tcp_init_response = match payload_type {
            PpaassMessageProxyPayloadType::TcpInit => TryInto::<TcpInitResponse>::try_into(data)?,
            _ => {
                error!("Client tcp connection [{src_address}] receive invalid message from proxy, payload type: {payload_type:?}");
                return Err(anyhow::anyhow!(format!(
                    "Client tcp connection [{src_address}] receive invalid message from proxy, payload type: {payload_type:?}"
                )));
            },
        };

        let TcpInitResponse {
            unique_key: tcp_loop_key,
            response_type,
            ..
        } = tcp_init_response;

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

        let FramedParts { io: client_tcp_stream, .. } = http_framed.into_parts();
        ClientProcessor::relay(ClientDataRelayInfo {
            client_tcp_stream,
            src_address,
            dst_address,
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

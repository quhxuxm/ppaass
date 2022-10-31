use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytecodec::{bytes::BytesEncoder, EncodeExt};
use bytes::{BufMut, BytesMut};
use error::Error;

use error::HttpCodecGeneralFailError;
use error::HttpCodecParseTargetHostFailError;
use error::HttpCodecParseUrlFailError;
use error::IoError;
use error::Socks5CodecError;
use futures::{ready, Stream};

use httpcodec::{BodyEncoder, RequestEncoder};
use pin_project::pin_project;
use ppaass_protocol::PpaassProtocolAddress;
use snafu::{OptionExt, ResultExt};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use url::Url;

use crate::{
    error,
    http::framed::HttpFramed,
    socks::{
        framed::{Socks5AuthFramed, Socks5InitFramed},
        message::{
            auth::Socks5AuthCommandContent,
            init::{Socks5InitCommandContent, Socks5InitCommandType},
        },
    },
};

use super::message::ClientInputMessage;

const HTTPS_SCHEMA: &str = "https";
const SCHEMA_SEP: &str = "://";
const CONNECT_METHOD: &str = "connect";
const HTTPS_DEFAULT_PORT: u16 = 443;
const HTTP_DEFAULT_PORT: u16 = 80;

enum ClientTcpConnectionStatus<T>
where
    T: AsyncRead + AsyncWrite,
{
    New,
    Relay,
    Http(Pin<Box<HttpFramed<T>>>),
    Socks5Auth(Pin<Box<Socks5AuthFramed<T>>>),
    Socks5Init(Pin<Box<Socks5InitFramed<T>>>),
}

#[pin_project]
pub(crate) struct ClientTcpConnectionFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    next_status: ClientTcpConnectionStatus<T>,
    #[pin]
    stream: T,
}

impl<T> ClientTcpConnectionFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    fn new(stream: T) -> Self {
        ClientTcpConnectionFramed {
            next_status: ClientTcpConnectionStatus::New,
            stream,
        }
    }
}

impl<T> Stream for ClientTcpConnectionFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    type Item = Result<ClientInputMessage, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        let stream = this.stream;
        match this.next_status {
            ClientTcpConnectionStatus::New => {
                let mut protocol_buf = [0u8; 1];
                let mut protocol_read_buf = ReadBuf::new(&mut protocol_buf);
                if let Err(e) = ready!(this.stream.poll_read(cx, &mut protocol_read_buf)) {
                    return Poll::Ready(Some(Err(e).context(IoError { message: format!("{e}") })));
                }
                match protocol_buf[0] {
                    5 => {
                        // For socks5 protocol
                        let mut initial_read_buf = BytesMut::new();
                        initial_read_buf.put_u8(5);
                        this.next_status = &mut ClientTcpConnectionStatus::Socks5Auth(Box::pin(Socks5AuthFramed::new(stream, initial_read_buf)));
                        return Poll::Pending;
                    },
                    4 => {
                        // For socks4 protocol
                        return Poll::Ready(Some(
                            Socks5CodecError {
                                message: "socks4 unsupported.",
                            }
                            .fail(),
                        ));
                    },
                    v => {
                        // For http protocol
                        let mut initial_read_buf = BytesMut::new();
                        initial_read_buf.put_u8(v);
                        this.next_status = &mut ClientTcpConnectionStatus::Http(Box::pin(HttpFramed::new(stream, initial_read_buf)));
                        return Poll::Pending;
                    },
                }
            },
            ClientTcpConnectionStatus::Http(http_codec) => {
                let http_codec_poll_result = ready!(http_codec.as_mut().poll_next(cx));
                match http_codec_poll_result {
                    None => return Poll::Ready(None),
                    Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                    Some(Ok(http_message)) => {
                        let request_url = http_message.request_target().to_string();
                        let parsed_request_url = Url::parse(request_url.as_str()).context(HttpCodecParseUrlFailError { url: request_url })?;
                        let target_port = parsed_request_url.port().unwrap_or_else(|| match parsed_request_url.scheme() {
                            HTTPS_SCHEMA => HTTPS_DEFAULT_PORT,
                            _ => HTTP_DEFAULT_PORT,
                        });
                        let target_host = parsed_request_url
                            .host()
                            .with_context(|| HttpCodecParseTargetHostFailError { url: request_url })?
                            .to_string();
                        let dest_address = PpaassProtocolAddress::Domain {
                            host: target_host,
                            port: target_port,
                        };
                        let http_method = http_message.method();
                        if CONNECT_METHOD.eq_ignore_ascii_case(http_method.as_str()) {
                            // Handle https connect method.
                            this.next_status = &mut ClientTcpConnectionStatus::Relay;
                            return Poll::Ready(Some(Ok(ClientInputMessage::HttpsConnect { dest_address })));
                        }
                        // Handle http request.
                        let mut http_body_data_encoder = RequestEncoder::<BodyEncoder<BytesEncoder>>::default();
                        let http_initial_body_data = http_body_data_encoder.encode_into_bytes(http_message).context(HttpCodecGeneralFailError {
                            message: "parse http request body fail",
                        })?;
                        this.next_status = &mut ClientTcpConnectionStatus::Relay;
                        return Poll::Ready(Some(Ok(ClientInputMessage::HttpInitial {
                            dest_address,
                            initial_data: http_initial_body_data,
                        })));
                    },
                }
            },
            ClientTcpConnectionStatus::Socks5Auth(socks5_auth_codec) => {
                let socks5_auth_poll_result = ready!(socks5_auth_codec.as_mut().poll_next(cx));
                match socks5_auth_poll_result {
                    None => return Poll::Ready(None),
                    Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                    Some(Ok(auth_message)) => {
                        let Socks5AuthCommandContent { method_number, methods, .. } = auth_message;
                        this.next_status = &mut ClientTcpConnectionStatus::Socks5Init(Box::pin(Socks5InitFramed::new(stream)));
                        return Poll::Ready(Some(Ok(ClientInputMessage::Socks5Auth { method_number, methods })));
                    },
                }
            },
            ClientTcpConnectionStatus::Socks5Init(socks5_init_codec) => {
                let socks5_init_poll_result = ready!(socks5_init_codec.as_mut().poll_next(cx));
                match socks5_init_poll_result {
                    None => return Poll::Ready(None),
                    Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                    Some(Ok(init_message)) => {
                        let Socks5InitCommandContent {
                            request_type: socks5_init_request_type,
                            dest_address,
                            ..
                        } = init_message;
                        let client_input_message = match socks5_init_request_type {
                            Socks5InitCommandType::Connect => ClientInputMessage::Socks5InitConnect { dest_address },
                            Socks5InitCommandType::Bind => ClientInputMessage::Socks5InitBind { dest_address },
                            Socks5InitCommandType::UdpAssociate => ClientInputMessage::Socks5InitUdpAssociate { dest_address },
                        };
                        this.next_status = &mut ClientTcpConnectionStatus::<T>::Relay;
                        return Poll::Ready(Some(Ok(client_input_message)));
                    },
                }
            },
            ClientTcpConnectionStatus::Relay => {
                let mut relay_buf = [0u8; 1024 * 64];
                let mut relay_read_buf = ReadBuf::new(&mut relay_buf);
                let initial_fill_length = relay_read_buf.filled().len();
                let relay_read_result = ready!(stream.poll_read(cx, &mut relay_read_buf)).context(IoError {
                    message: "fail to poll data from stream",
                })?;
                let current_fill_length = relay_read_buf.filled().len();
                if current_fill_length <= initial_fill_length {
                    return Poll::Ready(None);
                }
                return Poll::Ready(Some(Ok(ClientInputMessage::Raw(relay_buf.into()))));
            },
        }
    }
}

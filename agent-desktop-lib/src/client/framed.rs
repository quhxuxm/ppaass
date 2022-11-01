use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytecodec::{bytes::BytesEncoder, EncodeExt};
use bytes::{BufMut, BytesMut};
use error::Error;

use error::CodecNoBaseTcpStreamError;
use error::HttpCodecGeneralFailError;
use error::HttpCodecParseTargetHostFailError;
use error::HttpCodecParseUrlFailError;
use error::IoError;
use error::Socks5CodecError;
use futures::{ready, Stream};

use httpcodec::{BodyEncoder, RequestEncoder};

use ppaass_protocol::PpaassProtocolAddress;
use snafu::{OptionExt, ResultExt};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_util::codec::{Framed, FramedParts};
use url::Url;

use crate::{
    error,
    http::codec::HttpCodec,
    socks::{
        codec::{Socks5AuthCommandContentCodec, Socks5InitCommandContentCodec},
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
    T: AsyncRead + AsyncWrite + Unpin,
{
    New(Option<T>),
    Relay(T),
    Http(Option<Framed<T, HttpCodec>>),
    Socks5Auth(Option<Framed<T, Socks5AuthCommandContentCodec>>),
    Socks5Init(Option<Framed<T, Socks5InitCommandContentCodec>>),
}

pub(crate) struct ClientTcpConnectionFramed<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    next_status: ClientTcpConnectionStatus<T>,
}

impl<T> ClientTcpConnectionFramed<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn new(stream: T) -> Self {
        ClientTcpConnectionFramed {
            next_status: ClientTcpConnectionStatus::New(Some(stream)),
        }
    }
}

impl<T> Stream for ClientTcpConnectionFramed<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = Result<ClientInputMessage, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.next_status {
            ClientTcpConnectionStatus::New(ref mut stream) => {
                let mut stream = match stream.take() {
                    None => {
                        return Poll::Ready(Some(CodecNoBaseTcpStreamError {}.fail()));
                    },
                    Some(v) => v,
                };
                let mut protocol_buf = [0u8; 1];
                let mut protocol_read_buf = ReadBuf::new(&mut protocol_buf);

                if let Err(e) = ready!(Pin::new(&mut stream).poll_read(cx, &mut protocol_read_buf)) {
                    return Poll::Ready(Some(Err(e).context(IoError { message: format!("io error") })));
                }
                match protocol_buf[0] {
                    5 => {
                        // For socks5 protocol
                        let mut initial_read_buf = BytesMut::new();
                        initial_read_buf.put_u8(5);
                        let mut socks5_framed_parts = FramedParts::new(stream, Socks5AuthCommandContentCodec);
                        socks5_framed_parts.read_buf = initial_read_buf;
                        let socks5_framed = Framed::from_parts(socks5_framed_parts);
                        self.next_status = ClientTcpConnectionStatus::Socks5Auth(Some(socks5_framed));
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
                        let mut http_framed_parts = FramedParts::new(stream, HttpCodec::default());
                        http_framed_parts.read_buf = initial_read_buf;
                        let http_framed = Framed::from_parts(http_framed_parts);
                        self.next_status = ClientTcpConnectionStatus::Http(Some(http_framed));
                        return Poll::Pending;
                    },
                }
            },
            ClientTcpConnectionStatus::Http(ref mut http_framed) => {
                let mut http_framed = match http_framed.take() {
                    None => {
                        return Poll::Ready(Some(CodecNoBaseTcpStreamError {}.fail()));
                    },
                    Some(v) => v,
                };
                let http_codec_poll_result = ready!(Pin::new(&mut http_framed).poll_next(cx));
                match http_codec_poll_result {
                    None => return Poll::Ready(None),
                    Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                    Some(Ok(http_message)) => {
                        let request_url = http_message.request_target().to_string();
                        let parsed_request_url = Url::parse(request_url.as_str()).context(HttpCodecParseUrlFailError { url: &request_url })?;
                        let target_port = parsed_request_url.port().unwrap_or_else(|| match parsed_request_url.scheme() {
                            HTTPS_SCHEMA => HTTPS_DEFAULT_PORT,
                            _ => HTTP_DEFAULT_PORT,
                        });
                        let target_host = parsed_request_url
                            .host()
                            .with_context(|| HttpCodecParseTargetHostFailError { url: &request_url })?
                            .to_string();
                        let dest_address = PpaassProtocolAddress::Domain {
                            host: target_host,
                            port: target_port,
                        };
                        let http_method = http_message.method();
                        let FramedParts { io, .. } = http_framed.into_parts();
                        if CONNECT_METHOD.eq_ignore_ascii_case(http_method.as_str()) {
                            // Handle https connect method.
                            self.next_status = ClientTcpConnectionStatus::Relay(io);
                            return Poll::Ready(Some(Ok(ClientInputMessage::HttpsConnect { dest_address })));
                        }
                        // Handle http request.
                        let mut http_body_data_encoder = RequestEncoder::<BodyEncoder<BytesEncoder>>::default();
                        let http_initial_body_data = http_body_data_encoder.encode_into_bytes(http_message).context(HttpCodecGeneralFailError {
                            message: "parse http request body fail",
                        })?;
                        self.next_status = ClientTcpConnectionStatus::Relay(io);
                        return Poll::Ready(Some(Ok(ClientInputMessage::HttpInitial {
                            dest_address,
                            initial_data: http_initial_body_data,
                        })));
                    },
                }
            },
            ClientTcpConnectionStatus::Socks5Auth(ref mut socks5_auth_framed) => {
                let mut socks5_auth_framed = match socks5_auth_framed.take() {
                    None => {
                        return Poll::Ready(Some(CodecNoBaseTcpStreamError {}.fail()));
                    },
                    Some(v) => v,
                };
                let socks5_auth_poll_result = ready!(Pin::new(&mut socks5_auth_framed).poll_next(cx));
                match socks5_auth_poll_result {
                    None => return Poll::Ready(None),
                    Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                    Some(Ok(auth_message)) => {
                        let Socks5AuthCommandContent { method_number, methods, .. } = auth_message;
                        let FramedParts { io, .. } = socks5_auth_framed.into_parts();
                        let socks5_init_framed = Framed::new(io, Socks5InitCommandContentCodec);
                        self.next_status = ClientTcpConnectionStatus::Socks5Init(Some(socks5_init_framed));
                        return Poll::Ready(Some(Ok(ClientInputMessage::Socks5Auth { method_number, methods })));
                    },
                }
            },
            ClientTcpConnectionStatus::Socks5Init(ref mut socks5_init_framed) => {
                let mut socks5_init_framed = match socks5_init_framed.take() {
                    None => {
                        return Poll::Ready(Some(CodecNoBaseTcpStreamError {}.fail()));
                    },
                    Some(v) => v,
                };
                let socks5_init_poll_result = ready!(Pin::new(&mut socks5_init_framed).poll_next(cx));
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
                        let FramedParts { io, .. } = socks5_init_framed.into_parts();
                        self.next_status = ClientTcpConnectionStatus::<T>::Relay(io);
                        return Poll::Ready(Some(Ok(client_input_message)));
                    },
                }
            },
            ClientTcpConnectionStatus::Relay(ref mut stream) => {
                let mut relay_buf = [0u8; 1024 * 64];
                let mut relay_read_buf = ReadBuf::new(&mut relay_buf);
                let initial_fill_length = relay_read_buf.filled().len();
                ready!(Pin::new(stream).poll_read(cx, &mut relay_read_buf)).context(IoError {
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

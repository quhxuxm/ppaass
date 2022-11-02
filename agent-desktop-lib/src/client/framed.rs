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
use error::InvalidStatusError;
use error::IoError;
use error::Socks5CodecError;
use futures::{ready, Sink, Stream};

use httpcodec::{BodyEncoder, RequestEncoder};

use ppaass_protocol::PpaassProtocolAddress;
use snafu::{OptionExt, ResultExt};

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
use core::fmt::Debug;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_util::codec::{Framed, FramedParts};
use url::Url;

use super::message::{ClientInputMessage, ClientOutputMessage};

const HTTPS_SCHEMA: &str = "https";
const CONNECT_METHOD: &str = "connect";
const HTTPS_DEFAULT_PORT: u16 = 443;
const HTTP_DEFAULT_PORT: u16 = 80;
const SOCKS_V5: u8 = 5;
const SOCKS_V4: u8 = 4;
const CONNECTION_ESTABLISHED: &str = "Connection Established";
enum ClientInboundStatus<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    New(Option<T>),
    Relay(T),
    HttpInitialize(Option<Framed<T, HttpCodec>>),
    Socks5Authenticate(Option<Framed<T, Socks5AuthCommandContentCodec>>),
    Socks5Initialize(Option<Framed<T, Socks5InitCommandContentCodec>>),
}

impl<T> Debug for ClientInboundStatus<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New(_) => f.debug_tuple("New").finish(),
            Self::Relay(_) => f.debug_tuple("Relay").finish(),
            Self::HttpInitialize(_) => f.debug_tuple("HttpInitialize").finish(),
            Self::Socks5Authenticate(_) => f.debug_tuple("Socks5Authenticate").finish(),
            Self::Socks5Initialize(_) => f.debug_tuple("Socks5Initialize").finish(),
        }
    }
}

pub(crate) struct ClientInboundStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    current_status: ClientInboundStatus<T>,
}

impl<T> ClientInboundStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub(crate) fn new(stream: T) -> Self {
        ClientInboundStream {
            current_status: ClientInboundStatus::New(Some(stream)),
        }
    }
}

impl<T> Stream for ClientInboundStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = Result<ClientInputMessage, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.current_status {
            ClientInboundStatus::New(ref mut stream) => {
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
                    SOCKS_V5 => {
                        // For socks5 protocol
                        let mut initial_read_buf = BytesMut::new();
                        initial_read_buf.put_u8(5);
                        let mut socks5_framed_parts = FramedParts::new(stream, Socks5AuthCommandContentCodec);
                        socks5_framed_parts.read_buf = initial_read_buf;
                        let socks5_framed = Framed::from_parts(socks5_framed_parts);
                        self.current_status = ClientInboundStatus::Socks5Authenticate(Some(socks5_framed));
                        return Poll::Pending;
                    },
                    SOCKS_V4 => {
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
                        self.current_status = ClientInboundStatus::HttpInitialize(Some(http_framed));
                        return Poll::Pending;
                    },
                }
            },
            ClientInboundStatus::HttpInitialize(ref mut http_framed) => {
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
                            self.current_status = ClientInboundStatus::Relay(io);
                            return Poll::Ready(Some(Ok(ClientInputMessage::HttpsConnect { dest_address })));
                        }
                        // Handle http request.
                        let mut http_body_data_encoder = RequestEncoder::<BodyEncoder<BytesEncoder>>::default();
                        let http_initial_body_data = http_body_data_encoder.encode_into_bytes(http_message).context(HttpCodecGeneralFailError {
                            message: "parse http request body fail",
                        })?;

                        self.current_status = ClientInboundStatus::Relay(io);
                        return Poll::Ready(Some(Ok(ClientInputMessage::HttpConnect {
                            dest_address,
                            initial_data: http_initial_body_data,
                        })));
                    },
                }
            },
            ClientInboundStatus::Socks5Authenticate(ref mut socks5_auth_framed) => {
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
                        self.current_status = ClientInboundStatus::Socks5Initialize(Some(socks5_init_framed));
                        return Poll::Ready(Some(Ok(ClientInputMessage::Socks5Authenticate { method_number, methods })));
                    },
                }
            },
            ClientInboundStatus::Socks5Initialize(ref mut socks5_init_framed) => {
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
                            Socks5InitCommandType::Connect => ClientInputMessage::Socks5Connect { dest_address },
                            Socks5InitCommandType::Bind => ClientInputMessage::Socks5Bind { dest_address },
                            Socks5InitCommandType::UdpAssociate => ClientInputMessage::Socks5UdpAssociate { dest_address },
                        };
                        let FramedParts { io, .. } = socks5_init_framed.into_parts();
                        self.current_status = ClientInboundStatus::<T>::Relay(io);
                        return Poll::Ready(Some(Ok(client_input_message)));
                    },
                }
            },
            ClientInboundStatus::Relay(ref mut stream) => {
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
                return Poll::Ready(Some(Ok(ClientInputMessage::Relay(relay_buf.into()))));
            },
            ref invalid_status => {
                return Poll::Ready(Some(
                    InvalidStatusError {
                        message: format!("{invalid_status:?}"),
                    }
                    .fail(),
                ))
            },
        }
    }
}

pub(crate) struct ClientOutboundSink<'a, T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    base_stream: T,
    http_framed: Framed<&'a T, HttpCodec>,
    socks5_auth_framed: Framed<&'a T, Socks5AuthCommandContentCodec>,
    socks5_init_framed: Framed<&'a T, Socks5InitCommandContentCodec>,
}

impl<'a, T> ClientOutboundSink<'a, T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub(crate) fn new(stream: T) -> Self {
        Self {
            base_stream: stream,
            http_framed: Framed::new(&mut stream, HttpCodec::default()),
            socks5_auth_framed: Framed::new(&stream, Socks5AuthCommandContentCodec),
            socks5_init_framed: Framed::new(&stream, Socks5InitCommandContentCodec),
        }
    }
}

impl<T> Sink<ClientOutputMessage> for ClientOutboundSink<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        todo!()
    }

    fn start_send(self: Pin<&mut Self>, item: ClientOutputMessage) -> Result<(), Self::Error> {
        todo!()
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {}

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        todo!()
    }
}

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytes::{BufMut, BytesMut};
use error::Error;
use error::IoError;
use error::Socks5CodecError;
use futures::{ready, Stream};

use httpcodec::Method;
use pin_project::pin_project;
use snafu::ResultExt;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::{
    error,
    http::framed::HttpFramed,
    socks::{
        framed::{Socks5AuthFramed, Socks5InitFramed},
        message::{auth::Socks5AuthCommandContent, init::Socks5InitCommandContent},
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
    status: ClientTcpConnectionStatus<T>,
    #[pin]
    stream: T,
}

impl<T> ClientTcpConnectionFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    fn new(stream: T) -> Self {
        ClientTcpConnectionFramed {
            status: ClientTcpConnectionStatus::New,
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
        let stream = *this.stream;
        match this.status {
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
                        this.status = &mut ClientTcpConnectionStatus::Socks5Auth(Box::pin(Socks5AuthFramed::new(stream, initial_read_buf)));
                        return Poll::Pending;
                    },
                    4 => {
                        // For socks5 protocol
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
                        this.status = &mut ClientTcpConnectionStatus::Http(Box::pin(HttpFramed::new(stream, initial_read_buf)));
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
                        let http_method = http_message.method();
                        if CONNECT_METHOD.eq_ignore_ascii_case(http_method.as_str()){
                            return Poll::Ready(Some(Err(e)));
                        }

                        return Poll::Ready(Some(Ok(ClientInputMessage::HttpConnect)));
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
                            crate::socks::message::init::Socks5InitCommandType::Connect => ClientInputMessage::Socks5InitConnect { dest_address },
                            crate::socks::message::init::Socks5InitCommandType::Bind => ClientInputMessage::Socks5InitBind { dest_address },
                            crate::socks::message::init::Socks5InitCommandType::UdpAssociate => ClientInputMessage::Socks5InitUdpAssociate { dest_address },
                        };
                        this.status = &mut ClientTcpConnectionStatus::<T>::Relay;
                        return Poll::Ready(Some(Ok(client_input_message)));
                    },
                }
            },
            ClientTcpConnectionStatus::Relay => {
                let mut relay_buf = [0u8; 1024 * 64];
                let mut relay_read_buf = ReadBuf::new(&mut relay_buf);
                let relay_read_result = ready!(this.stream.poll_read(cx, &mut relay_read_buf));
                return Poll::Ready(Some(Ok(ClientInputMessage::Raw(relay_buf.into()))));
            },
        }
    }
}

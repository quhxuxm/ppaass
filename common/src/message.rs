#![allow(unused)]
use std::{
    collections::vec_deque::Iter,
    fmt::{Debug, Display, Formatter},
    io::{Error, ErrorKind},
    mem::size_of,
    pin::Pin,
    task::{Context, Poll},
};
use std::{
    io::Cursor,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};
use std::{net::ToSocketAddrs, sync::Arc};
use std::{ops::Deref, str::FromStr};

use bytes::{Buf, BufMut, Bytes, BytesMut};

use futures::{Stream, TryStream};
use pin_project::pin_project;
use pretty_hex::*;
use rsa::pkcs8::der::bigint::generic_array::typenum::PowerOfTwo;
use serde_derive::{Deserialize, Serialize};
use tracing::error;

use crate::NetAddress::IpV4;
use crate::{error::PpaassError, util::generate_uuid};

const ENCRYPTION_TYPE_PLAIN: u8 = 0;
const ENCRYPTION_TYPE_AES: u8 = 2;

const IPV4_TYPE: u8 = 0;
const IPV6_TYPE: u8 = 1;
const DOMAIN_TYPE: u8 = 2;

/// The net address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetAddress {
    /// Ip v4 net address
    IpV4([u8; 4], u16),
    /// Ip v6 net address
    IpV6([u8; 16], u16),
    /// Domain net address
    Domain(String, u16),
}

impl Default for NetAddress {
    fn default() -> Self {
        IpV4([0, 0, 0, 0], 0)
    }
}

pub struct SocketAddrIter {
    elements: Vec<SocketAddr>,
    index: usize,
}

impl SocketAddrIter {
    pub fn new(elements: Vec<SocketAddr>) -> Self {
        Self { elements, index: 0 }
    }
}

impl Iterator for SocketAddrIter {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        let mut result = self.elements.get(self.index);
        self.index += 1;
        result.map(|item| *item)
    }
}

impl ToSocketAddrs for NetAddress {
    type Iter = SocketAddrIter;

    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        match self {
            Self::IpV4(ip, port) => {
                let socket_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), *port));
                let elements = vec![socket_addr];
                Ok(SocketAddrIter::new(elements))
            },
            Self::IpV6(ip, port) => {
                let mut cursor = Cursor::new(ip);
                let socket_addr = SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::new(
                        cursor.get_u16(),
                        cursor.get_u16(),
                        cursor.get_u16(),
                        cursor.get_u16(),
                        cursor.get_u16(),
                        cursor.get_u16(),
                        cursor.get_u16(),
                        cursor.get_u16(),
                    ),
                    *port,
                    0,
                    0,
                ));
                let elements = vec![socket_addr];
                Ok(SocketAddrIter::new(elements))
            },
            Self::Domain(host, port) => {
                let addresses = format!("{}:{}", host, port).to_socket_addrs()?.collect::<Vec<_>>();
                Ok(SocketAddrIter::new(addresses))
            },
        }
    }
}

impl ToString for NetAddress {
    fn to_string(&self) -> String {
        match self {
            Self::IpV4(ip_content, port) => {
                format!("{}.{}.{}.{}:{}", ip_content[0], ip_content[1], ip_content[2], ip_content[3], port)
            },
            Self::IpV6(ip_content, port) => {
                let mut cursor = Cursor::new(ip_content);
                format!(
                    "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{}",
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                    port
                )
            },
            Self::Domain(host, port) => {
                format!("{}:{}", host, port)
            },
        }
    }
}

impl TryFrom<NetAddress> for Vec<SocketAddr> {
    type Error = PpaassError;
    fn try_from(net_address: NetAddress) -> Result<Self, PpaassError> {
        match net_address {
            NetAddress::IpV4(ip, port) => {
                let socket_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), port));
                Ok(vec![socket_addr])
            },
            NetAddress::IpV6(ip, port) => {
                let mut ip_cursor = Cursor::new(ip);
                let socket_addr = SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::new(
                        ip_cursor.get_u16(),
                        ip_cursor.get_u16(),
                        ip_cursor.get_u16(),
                        ip_cursor.get_u16(),
                        ip_cursor.get_u16(),
                        ip_cursor.get_u16(),
                        ip_cursor.get_u16(),
                        ip_cursor.get_u16(),
                    ),
                    port,
                    0,
                    0,
                ));
                Ok(vec![socket_addr])
            },
            NetAddress::Domain(host, port) => {
                let addresses = format!("{}:{}", host, port).to_socket_addrs()?.collect::<Vec<_>>();
                Ok(addresses)
            },
        }
    }
}

impl From<SocketAddr> for NetAddress {
    fn from(value: SocketAddr) -> Self {
        let ip_address = value.ip();
        match ip_address {
            IpAddr::V4(addr) => Self::IpV4(addr.octets(), value.port()),
            IpAddr::V6(addr) => Self::IpV6(addr.octets(), value.port()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PayloadEncryptionType {
    Plain,
    Aes(Vec<u8>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AgentMessagePayloadTypeValue {
    TcpConnect,
    TcpData,
    UdpAssociate,
    UdpData,
    DomainResolve,
    Heartbeat,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ProxyMessagePayloadTypeValue {
    TcpConnectSuccess,
    TcpConnectFail,
    TcpData,
    UdpAssociateSuccess,
    UdpAssociateFail,
    UdpData,
    UdpDataRelayFail,
    DomainResolveSuccess,
    DomainResolveFail,
    HeartbeatSuccess,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PayloadType {
    AgentPayload(AgentMessagePayloadTypeValue),
    ProxyPayload(ProxyMessagePayloadTypeValue),
}

#[derive(Serialize, Deserialize)]
pub struct MessagePayload {
    /// The source address
    pub source_address: Option<NetAddress>,
    /// The target address
    pub target_address: Option<NetAddress>,
    /// The payload type
    pub payload_type: PayloadType,
    /// The data
    pub data: Option<Vec<u8>>,
}

impl Debug for MessagePayload {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessagePayload")
            .field("source_address", &self.source_address)
            .field("target_address", &self.target_address)
            .field("payload_type", &self.payload_type)
            .field("data", &format!("\n\n{}\n", pretty_hex(self.data.as_ref().unwrap_or(&vec![]))))
            .finish()
    }
}

impl TryFrom<MessagePayload> for Vec<u8> {
    type Error = PpaassError;

    fn try_from(value: MessagePayload) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).map_err(|e| {
            error!("Fail to convert message payload object to bytes because of error: {e:#?}");
            PpaassError::CodecError
        })?;
        Ok(result)
    }
}

impl TryFrom<Vec<u8>> for MessagePayload {
    type Error = PpaassError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(value.as_ref()).map_err(|e| {
            error!("Fail to convert bytes to message payload object because of error: {e:#?}");
            PpaassError::CodecError
        })?;
        Ok(result)
    }
}

#[derive(Serialize, Deserialize)]
/// The message
pub struct Message {
    /// The message id
    pub id: String,
    /// The message id that this message reference to
    pub ref_id: Option<String>,
    /// The connection id that initial this message
    pub connection_id: Option<String>,
    /// The user token
    pub user_token: String,
    /// The payload encryption type
    pub payload_encryption_type: PayloadEncryptionType,
    /// The payload
    pub payload: Option<Vec<u8>>,
}

impl Debug for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Message")
            .field("id", &self.id)
            .field("ref_id", &self.ref_id)
            .field("connection_id", &self.connection_id)
            .field("user_token", &self.user_token)
            .field("payload_encryption_type", &"[...omit...]")
            .field("payload", &"[... omit ...]")
            .finish()
    }
}

impl TryFrom<Vec<u8>> for Message {
    type Error = PpaassError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(value.as_ref()).map_err(|e| {
            error!("Fail to convert bytes to message object because of error: {e:#?}");
            PpaassError::CodecError
        })?;
        Ok(result)
    }
}

impl TryFrom<Message> for Vec<u8> {
    type Error = PpaassError;

    fn try_from(value: Message) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).map_err(|e| {
            error!("Fail to convert message object to bytes because of error: {e:#?}");
            PpaassError::CodecError
        })?;
        Ok(result)
    }
}

#[pin_project]
pub struct MessageStream {
    #[pin]
    inner: Vec<Option<Message>>,
    index: usize,
}

impl MessageStream {
    fn new(messages: Vec<Message>) -> Self {
        let inner = messages.into_iter().map(|item| Some(item)).collect::<Vec<_>>();
        Self { inner, index: 0 }
    }
}

impl Stream for MessageStream {
    type Item = Result<Message, PpaassError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        let mut index = *this.index;
        let mut inner_item = this.inner.get_mut().get_mut(index);
        inner_item.map_or(Poll::Ready(None), |value| {
            let message = value.take();
            index += 1;
            *this.index = index;
            let result = message.ok_or(PpaassError::IoError {
                source: Error::new(ErrorKind::InvalidData, "Fail to convert message stream because of item is a none value."),
            });
            return Poll::Ready(Some(result));
        })
    }
}

impl From<Vec<Message>> for MessageStream {
    fn from(messages: Vec<Message>) -> Self {
        MessageStream::new(messages)
    }
}

#[derive(Serialize, Deserialize)]
pub struct DomainResolveRequest {
    pub name: String,
    pub id: i32,
    pub port: Option<u16>,
}

#[derive(Serialize, Deserialize)]
pub struct DomainResolveResponse {
    pub id: i32,
    pub name: String,
    pub port: Option<u16>,
    pub addresses: Vec<[u8; 4]>,
}

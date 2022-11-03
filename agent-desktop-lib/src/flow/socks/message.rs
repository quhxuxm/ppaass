use std::{
    fmt::Debug,
    io::Cursor,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use ppaass_protocol::PpaassProtocolAddress;
use snafu::{OptionExt, ResultExt};

use crate::error::Error;
use crate::error::IoError;
use crate::error::Socks5CodecError;

mod auth;
mod init;
mod udp;

pub(crate) use auth::*;
pub(crate) use init::*;
pub(crate) use udp::*;

#[derive(Debug, Clone)]
pub(crate) enum Socks5Address {
    IpV4([u8; 4], u16),
    IpV6([u8; 16], u16),
    Domain(String, u16),
}

impl TryFrom<Socks5Address> for SocketAddr {
    type Error = Error;

    fn try_from(socks5_addr: Socks5Address) -> Result<Self, Self::Error> {
        match socks5_addr {
            Socks5Address::IpV4(ip, port) => Ok(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), port))),
            Socks5Address::IpV6(ip, port) => {
                let mut ip_cursor = Cursor::new(ip);
                Ok(SocketAddr::V6(SocketAddrV6::new(
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
                )))
            },
            Socks5Address::Domain(host, port) => {
                let addresses = format!("{host}:{port}")
                    .to_socket_addrs()
                    .context(IoError {
                        message: format!("{host}:{port}"),
                    })?
                    .collect::<Vec<_>>();
                let result = addresses.get(0).context(Socks5CodecError {
                    message: format!("none socket address parsed from {host}:{port}"),
                })?;
                Ok(*result)
            },
        }
    }
}
impl From<SocketAddr> for Socks5Address {
    fn from(socket_addr: SocketAddr) -> Self {
        match socket_addr {
            SocketAddr::V4(addr) => Socks5Address::IpV4(addr.ip().octets(), addr.port()),
            SocketAddr::V6(addr) => Socks5Address::IpV6(addr.ip().octets(), addr.port()),
        }
    }
}

impl ToString for Socks5Address {
    fn to_string(&self) -> String {
        match self {
            Self::IpV4(ip_content, port) => {
                format!("{}.{}.{}.{}:{}", ip_content[0], ip_content[1], ip_content[2], ip_content[3], port)
            },
            Self::IpV6(ip_content, port) => {
                let mut ip_content_bytes = Bytes::from(ip_content.to_vec());
                format!(
                    "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{}",
                    ip_content_bytes.get_u16(),
                    ip_content_bytes.get_u16(),
                    ip_content_bytes.get_u16(),
                    ip_content_bytes.get_u16(),
                    ip_content_bytes.get_u16(),
                    ip_content_bytes.get_u16(),
                    ip_content_bytes.get_u16(),
                    ip_content_bytes.get_u16(),
                    port
                )
            },
            Self::Domain(host, port) => {
                format!("{}:{}", host, port)
            },
        }
    }
}

impl TryFrom<&mut Bytes> for Socks5Address {
    type Error = Error;
    fn try_from(value: &mut Bytes) -> Result<Self, Self::Error> {
        if !value.has_remaining() {
            return Socks5CodecError {
                message: "no remaing bytes to parse socks5 address",
            }
            .fail();
        }
        let address_type = value.get_u8();
        let address = match address_type {
            1 => {
                if value.remaining() < 6 {
                    return Socks5CodecError {
                        message: format!(
                            "no enough remaing bytes to parse socks5 IPV4 address, remaining is {}, require 6",
                            value.remaining()
                        ),
                    }
                    .fail();
                }
                let mut addr_content = [0u8; 4];
                addr_content.iter_mut().for_each(|item| {
                    *item = value.get_u8();
                });
                let port = value.get_u16();
                Socks5Address::IpV4(addr_content, port)
            },
            4 => {
                if value.remaining() < 18 {
                    return Socks5CodecError {
                        message: format!(
                            "no enough remaing bytes to parse socks5 IPV6 address, remaining is {}, require 18",
                            value.remaining()
                        ),
                    }
                    .fail();
                }
                let mut addr_content = [0u8; 16];
                addr_content.iter_mut().for_each(|item| {
                    *item = value.get_u8();
                });
                let port = value.get_u16();
                Socks5Address::IpV6(addr_content, port)
            },
            3 => {
                if value.remaining() < 1 {
                    return Socks5CodecError {
                        message: format!(
                            "no enough remaing bytes to parse socks5 Domain address, remaining is {}, require 1",
                            value.remaining()
                        ),
                    }
                    .fail();
                }
                let domain_name_length = value.get_u8() as usize;
                if value.remaining() < domain_name_length + 2 {
                    return Socks5CodecError {
                        message: format!(
                            "no enough remaing bytes to parse socks5 Domain address, remaining is {}, require {}",
                            value.remaining(),
                            domain_name_length + 2
                        ),
                    }
                    .fail();
                }
                let domain_name_bytes = value.copy_to_bytes(domain_name_length);
                let domain_name = match String::from_utf8_lossy(domain_name_bytes.chunk()).to_string().as_str() {
                    "0" => "127.0.0.1".to_string(),
                    v => v.to_string(),
                };
                let port = value.get_u16();
                Socks5Address::Domain(domain_name, port)
            },
            unknown_addr_type => {
                return Socks5CodecError {
                    message: format!("unknown address type: {unknown_addr_type}"),
                }
                .fail();
            },
        };
        Ok(address)
    }
}

impl TryFrom<Bytes> for Socks5Address {
    type Error = Error;

    fn try_from(mut value: Bytes) -> Result<Self, Self::Error> {
        let value_mut_ref = &mut value;
        value_mut_ref.try_into()
    }
}

impl TryFrom<&mut BytesMut> for Socks5Address {
    type Error = Error;

    fn try_from(value: &mut BytesMut) -> Result<Self, Self::Error> {
        let value = value.copy_to_bytes(value.len());
        value.try_into()
    }
}

impl From<Socks5Address> for Bytes {
    fn from(address: Socks5Address) -> Self {
        let mut result = BytesMut::new();
        match address {
            Socks5Address::IpV4(addr_content, port) => {
                result.put_u8(1);
                result.put_slice(&addr_content);
                result.put_u16(port);
            },
            Socks5Address::IpV6(addr_content, port) => {
                result.put_u8(4);
                result.put_slice(&addr_content);
                result.put_u16(port);
            },
            Socks5Address::Domain(addr_content, port) => {
                result.put_u8(3);
                result.put_u8(addr_content.len() as u8);
                result.put_slice(&addr_content.as_bytes());
                result.put_u16(port);
            },
        }
        result.into()
    }
}

impl From<Socks5Address> for PpaassProtocolAddress {
    fn from(value: Socks5Address) -> Self {
        match value {
            Socks5Address::IpV4(host, port) => PpaassProtocolAddress::IpV4 { ip: host, port },
            Socks5Address::IpV6(host, port) => PpaassProtocolAddress::IpV6 { ip: host, port },
            Socks5Address::Domain(host, port) => PpaassProtocolAddress::Domain { host, port },
        }
    }
}

impl From<PpaassProtocolAddress> for Socks5Address {
    fn from(net_addr: PpaassProtocolAddress) -> Self {
        match net_addr {
            PpaassProtocolAddress::IpV4 { ip, port } => Socks5Address::IpV4(ip, port),
            PpaassProtocolAddress::IpV6 { ip, port } => Socks5Address::IpV6(ip, port),
            PpaassProtocolAddress::Domain { host, port } => Socks5Address::Domain(host, port),
        }
    }
}

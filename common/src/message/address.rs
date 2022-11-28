use crate::serializer::array_u8_l16_to_base64;
use crate::serializer::array_u8_l4_to_base64;
use anyhow::Context;
use bytes::Buf;
use serde_derive::{Deserialize, Serialize};

use std::fmt::{Display, Formatter};
use std::net::{IpAddr, SocketAddr};
use std::{
    io::Cursor,
    net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PpaassNetAddress {
    IpV4 {
        #[serde(with = "array_u8_l4_to_base64")]
        ip: [u8; 4],
        port: u16,
    },
    IpV6 {
        #[serde(with = "array_u8_l16_to_base64")]
        ip: [u8; 16],
        port: u16,
    },
    Domain {
        host: String,
        port: u16,
    },
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
        let result = self.elements.get(self.index);
        self.index += 1;
        result.map(|item| *item)
    }
}

impl ToSocketAddrs for PpaassNetAddress {
    type Iter = SocketAddrIter;

    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        let socket_addr_vec: Vec<SocketAddr> = self.try_into().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(SocketAddrIter::new(socket_addr_vec))
    }
}

impl TryFrom<&PpaassNetAddress> for Vec<SocketAddr> {
    type Error = anyhow::Error;

    fn try_from(value: &PpaassNetAddress) -> Result<Self, Self::Error> {
        match value {
            PpaassNetAddress::IpV4 { ip, port } => {
                let socket_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), *port));
                Ok(vec![socket_addr])
            },
            PpaassNetAddress::IpV6 { ip, port } => {
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
                    *port,
                    0,
                    0,
                ));
                Ok(vec![socket_addr])
            },
            PpaassNetAddress::Domain { host, port } => {
                let address_string = format!("{}:{}", host, port);
                let addresses = address_string
                    .to_socket_addrs()
                    .context(format!("fail to parse domain name, host: {host}, port: {port}"))?
                    .collect::<Vec<_>>();
                Ok(addresses)
            },
        }
    }
}

impl TryFrom<PpaassNetAddress> for Vec<SocketAddr> {
    type Error = anyhow::Error;
    fn try_from(value: PpaassNetAddress) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl From<&SocketAddr> for PpaassNetAddress {
    fn from(value: &SocketAddr) -> Self {
        let ip_address = value.ip();
        match ip_address {
            IpAddr::V4(addr) => Self::IpV4 {
                ip: addr.octets(),
                port: value.port(),
            },
            IpAddr::V6(addr) => Self::IpV6 {
                ip: addr.octets(),
                port: value.port(),
            },
        }
    }
}

impl From<SocketAddr> for PpaassNetAddress {
    fn from(value: SocketAddr) -> Self {
        (&value).into()
    }
}

impl Display for PpaassNetAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PpaassNetAddress::IpV4 { ip, port } => {
                let ipv4_addr = Ipv4Addr::from(ip.clone());
                write!(f, "{ipv4_addr}:{port}")
            },
            PpaassNetAddress::IpV6 { ip, port } => {
                let ipv6_addr = Ipv6Addr::from(ip.clone());
                write!(f, "{ipv6_addr}:{port}")
            },
            PpaassNetAddress::Domain { host, port } => {
                write!(f, "{host}:{port}")
            },
        }
    }
}
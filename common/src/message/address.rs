use crate::CommonError;

use bytes::Buf;

use derive_more::Display;
use serde_derive::{Deserialize, Serialize};

use std::net::{IpAddr, SocketAddr};
use std::{
    io::Cursor,
    net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Display)]
pub enum PpaassNetAddress {
    #[display(fmt = "{:?}:{}", ip, port)]
    IpV4 { ip: [u8; 4], port: u16 },
    #[display(fmt = "{:?}:{}", ip, port)]
    IpV6 { ip: [u8; 16], port: u16 },
    #[display(fmt = "{}:{}", host, port)]
    Domain { host: String, port: u16 },
}

impl PartialEq for PpaassNetAddress {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::IpV4 { ip: l_ip, port: l_port }, Self::IpV4 { ip: r_ip, port: r_port }) => l_ip == r_ip && l_port == r_port,
            (Self::IpV4 { ip: l_ip, port: l_port }, Self::Domain { host: r_host, port: r_port }) => {
                format!("{}.{}.{}.{}", l_ip[0], l_ip[1], l_ip[2], l_ip[3]).eq(r_host) && l_port == r_port
            },
            (Self::IpV6 { ip: l_ip, port: l_port }, Self::IpV6 { ip: r_ip, port: r_port }) => l_ip == r_ip && l_port == r_port,
            (Self::Domain { host: l_host, port: l_port }, Self::Domain { host: r_host, port: r_port }) => l_host == r_host && l_port == r_port,
            (Self::Domain { host: l_host, port: l_port }, Self::IpV4 { ip: r_ip, port: r_port }) => {
                format!("{}.{}.{}.{}", r_ip[0], r_ip[1], r_ip[2], r_ip[3]).eq(l_host) && l_port == r_port
            },
            _ => false,
        }
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
        let result = self.elements.get(self.index);
        self.index += 1;
        result.copied()
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
    type Error = CommonError;

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
                let address_string = format!("{host}:{port}");
                let addresses = address_string.to_socket_addrs()?;
                let addresses = addresses.collect::<Vec<_>>();
                Ok(addresses)
            },
        }
    }
}

impl TryFrom<PpaassNetAddress> for Vec<SocketAddr> {
    type Error = CommonError;
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

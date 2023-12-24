use crate::CommonError;
use derive_more::Display;
use serde_derive::{Deserialize, Serialize};

use log::trace;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, Display)]
pub enum PpaassUnifiedAddress {
    #[display(fmt = "{_0:?}")]
    Ip(SocketAddr),
    #[display(fmt = "{}:{}", host, port)]
    Domain { host: String, port: u16 },
}

impl PartialEq for PpaassUnifiedAddress {
    fn eq(&self, other: &Self) -> bool {
        match self {
            PpaassUnifiedAddress::Ip(self_ip_addr) => match other {
                PpaassUnifiedAddress::Ip(other_ip_addr) => self_ip_addr.eq(other_ip_addr),
                PpaassUnifiedAddress::Domain {
                    host: other_host,
                    port: other_port,
                } => {
                    trace!("Fail to compare ip address and domain address, ip address=[{self_ip_addr}], domain address=[{other_host}:{other_port}]");
                    false
                },
            },
            PpaassUnifiedAddress::Domain {
                host: self_host,
                port: self_port,
            } => match other {
                PpaassUnifiedAddress::Ip(other_ip_addr) => {
                    trace!("Fail to compare domain address and ip address, domain address=[{self_host}:{self_port}], ip address=[{other_ip_addr}], ");
                    false
                },
                PpaassUnifiedAddress::Domain {
                    host: other_host,
                    port: other_port,
                } => self_host == other_host && self_port == other_port,
            },
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

impl ToSocketAddrs for PpaassUnifiedAddress {
    type Iter = SocketAddrIter;

    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        let socket_addr_vec: Vec<SocketAddr> = self.clone().try_into().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(SocketAddrIter::new(socket_addr_vec))
    }
}

impl TryFrom<PpaassUnifiedAddress> for Vec<SocketAddr> {
    type Error = CommonError;

    fn try_from(value: PpaassUnifiedAddress) -> Result<Self, Self::Error> {
        match value {
            PpaassUnifiedAddress::Ip(socket_addr) => Ok(vec![socket_addr]),
            PpaassUnifiedAddress::Domain { host, port } => {
                let address_string = format!("{host}:{port}");
                let addresses = address_string.to_socket_addrs()?;
                let addresses = addresses.collect::<Vec<_>>();
                Ok(addresses)
            },
        }
    }
}

impl From<SocketAddr> for PpaassUnifiedAddress {
    fn from(value: SocketAddr) -> Self {
        PpaassUnifiedAddress::Ip(value)
    }
}

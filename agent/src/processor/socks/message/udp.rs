use crate::error::Socks5DecodeError;

use super::Socks5Address;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::mem::size_of;

/// Socks5 udp data request
#[derive(Debug)]
pub(crate) struct Socks5UdpDataPacket {
    pub frag: u8,
    pub address: Socks5Address,
    pub data: Bytes,
}

impl TryFrom<Bytes> for Socks5UdpDataPacket {
    type Error = Socks5DecodeError;

    fn try_from(mut src: Bytes) -> Result<Self, Self::Error> {
        // Check the buffer
        if !src.has_remaining() {
            return Err(Socks5DecodeError::NoRemaining("No remaining to convert socks5 udp packet".to_string()));
        }
        // Check and skip the revision
        if src.remaining() < size_of::<u16>() {
            return Err(Socks5DecodeError::NoRemaining("No remaining to convert socks5 udp packet".to_string()));
        }
        src.get_u16();
        if src.remaining() < size_of::<u8>() {
            return Err(Socks5DecodeError::NoRemaining("No remaining to convert socks5 udp packet".to_string()));
        }
        let frag = src.get_u8();
        let address = Socks5Address::parse(&mut src)?;
        Ok(Socks5UdpDataPacket { frag, address, data: src })
    }
}

impl From<Socks5UdpDataPacket> for Bytes {
    fn from(packet: Socks5UdpDataPacket) -> Self {
        let mut result = BytesMut::new();
        result.put_u16(0);
        result.put_u8(packet.frag);
        result.put::<Bytes>(packet.address.into());
        result.put(packet.data.as_ref());
        result.freeze()
    }
}

#[derive(Debug)]
pub(crate) struct UdpDiagram {
    pub source_port: u16,
    pub target_port: u16,
    pub length: u16,
    pub checksum: u16,
    pub data: Bytes,
}

impl From<Bytes> for UdpDiagram {
    fn from(mut bytes: Bytes) -> Self {
        let source_port = bytes.get_u16();
        let target_port = bytes.get_u16();
        let length = bytes.get_u16();
        let checksum = bytes.get_u16();
        let data: Bytes = bytes.copy_to_bytes(length as usize);
        Self {
            source_port,
            target_port,
            length,
            checksum,
            data,
        }
    }
}

impl From<UdpDiagram> for Vec<u8> {
    fn from(value: UdpDiagram) -> Self {
        let mut result = BytesMut::new();
        result.put_u16(value.source_port);
        result.put_u16(value.target_port);
        result.put_u16(value.length);
        result.put_u16(value.checksum);
        result.put_slice(value.data.chunk());
        result.to_vec()
    }
}

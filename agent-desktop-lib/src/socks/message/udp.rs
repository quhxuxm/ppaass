use super::Socks5Address;
use crate::error::Error;
use crate::error::Socks5CodecError;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::mem::size_of;

/// Socks5 udp data request
#[derive(Debug)]
pub(crate) struct Socks5UdpDataPacket {
    pub frag: u8,
    pub address: Socks5Address,
    pub data: Vec<u8>,
}

impl TryFrom<Bytes> for Socks5UdpDataPacket {
    type Error = Error;
    fn try_from(mut src: Bytes) -> Result<Self, Self::Error> {
        // Check the buffer
        if !src.has_remaining() {
            return Socks5CodecError {
                message: "incoming bytes has no remaining",
            }
            .fail();
        }
        // Check and skip the revision
        if src.remaining() < size_of::<u16>() {
            return Socks5CodecError {
                message: format!("incoming bytes remaing < {}", size_of::<u16>()),
            }
            .fail();
        }
        src.get_u16();
        if src.remaining() < size_of::<u8>() {
            return Socks5CodecError {
                message: format!("incoming bytes remaing < {}", size_of::<u8>()),
            }
            .fail();
        }
        let frag = src.get_u8();
        let address: Socks5Address = (&mut src).try_into()?;
        let data = src.copy_to_bytes(src.remaining());
        Ok(Socks5UdpDataPacket {
            frag,
            address,
            data: data.to_vec(),
        })
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
    fn from(bytes: Bytes) -> Self {
        let mut bytes = Bytes::from(bytes);
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

mod codec;
mod protocol;

use std::{
    fmt::Debug,
    io::Cursor,
    mem::size_of,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use ppaass_protocol::PpaassProtocolAddress;
use tracing::error;



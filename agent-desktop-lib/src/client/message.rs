use ppaass_protocol::PpaassProtocolAddress;

use crate::socks::message::{auth::Socks5AuthMethod, Socks5Address};

pub(crate) enum ClientInputMessage {
    HttpsConnect {
        dest_address: PpaassProtocolAddress,
    },
    HttpConnect {
        dest_address: PpaassProtocolAddress,
        initial_data: Vec<u8>,
    },
    Socks5Authenticate {
        method_number: u8,
        methods: Vec<Socks5AuthMethod>,
    },
    Socks5Connect {
        dest_address: Socks5Address,
    },
    Socks5Bind {
        dest_address: Socks5Address,
    },
    Socks5UdpAssociate {
        dest_address: Socks5Address,
    },
    Relay(Vec<u8>),
}

pub(crate) struct ClientOutputMessage {}

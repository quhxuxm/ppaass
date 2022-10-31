use ppaass_protocol::PpaassProtocolAddress;

use crate::socks::message::{auth::Socks5AuthMethod, Socks5Address};

pub(crate) enum ClientInputMessage {
    HttpsConnect {
        dest_address: PpaassProtocolAddress,
    },
    HttpInitial {
        dest_address: PpaassProtocolAddress,
        initial_data: Vec<u8>,
    },
    Socks5Auth {
        method_number: u8,
        methods: Vec<Socks5AuthMethod>,
    },
    Socks5InitConnect {
        dest_address: Socks5Address,
    },
    Socks5InitBind {
        dest_address: Socks5Address,
    },
    Socks5InitUdpAssociate {
        dest_address: Socks5Address,
    },
    Raw(Vec<u8>),
}

pub(crate) struct ClientOutputMessage {}

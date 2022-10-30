use crate::socks::message::{auth::Socks5AuthMethod, Socks5Address};

pub(crate) enum ClientInputMessage {
    HttpConnect { dest_address: Socks5Address },
    Socks5Auth { method_number: u8, methods: Vec<Socks5AuthMethod> },
    Socks5InitConnect { dest_address: Socks5Address },
    Socks5InitBind { dest_address: Socks5Address },
    Socks5InitUdpAssociate { dest_address: Socks5Address },
    Raw(Vec<u8>),
}

pub(crate) struct ClientOutputMessage {}

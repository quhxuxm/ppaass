#[derive(Debug)]
pub(crate) enum Socks5AuthMethod {
    NoAuthenticationRequired,
    GssApi,
    UsernameAndPassword,
    IanaAssigned,
    ReservedForPrivateMethods,
    NoAcceptableMethods,
}

impl From<u8> for Socks5AuthMethod {
    fn from(v: u8) -> Self {
        match v {
            0 => Socks5AuthMethod::NoAuthenticationRequired,
            1 => Socks5AuthMethod::GssApi,
            2 => Socks5AuthMethod::UsernameAndPassword,
            3 => Socks5AuthMethod::IanaAssigned,
            8 => Socks5AuthMethod::ReservedForPrivateMethods,
            16 => Socks5AuthMethod::NoAcceptableMethods,
            _ => Socks5AuthMethod::NoAuthenticationRequired,
        }
    }
}

impl From<Socks5AuthMethod> for u8 {
    fn from(value: Socks5AuthMethod) -> Self {
        match value {
            Socks5AuthMethod::NoAuthenticationRequired => 0,
            Socks5AuthMethod::GssApi => 1,
            Socks5AuthMethod::UsernameAndPassword => 2,
            Socks5AuthMethod::IanaAssigned => 3,
            Socks5AuthMethod::ReservedForPrivateMethods => 8,
            Socks5AuthMethod::NoAcceptableMethods => 16,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Socks5AuthCommandContent {
    pub version: u8,
    pub method_number: u8,
    pub methods: Vec<Socks5AuthMethod>,
}

impl Socks5AuthCommandContent {
    pub fn new(method_number: u8, methods: Vec<Socks5AuthMethod>) -> Self {
        Socks5AuthCommandContent {
            version: 5,
            method_number,
            methods,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Socks5AuthCommandResultContent {
    pub version: u8,
    pub method: Socks5AuthMethod,
}

impl Socks5AuthCommandResultContent {
    pub fn new(method: Socks5AuthMethod) -> Self {
        Socks5AuthCommandResultContent { version: 5u8, method }
    }
}

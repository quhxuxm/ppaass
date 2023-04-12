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

pub(crate) struct Socks5AuthCommandContentParts {
    pub methods: Vec<Socks5AuthMethod>,
}
#[derive(Debug)]
pub(crate) struct Socks5AuthCommandContent {
    methods: Vec<Socks5AuthMethod>,
}

impl Socks5AuthCommandContent {
    pub fn new(methods: Vec<Socks5AuthMethod>) -> Self {
        Socks5AuthCommandContent { methods }
    }

    pub(crate) fn split(self) -> Socks5AuthCommandContentParts {
        Socks5AuthCommandContentParts { methods: self.methods }
    }
}

pub(crate) struct Socks5AuthCommandResultContentParts {
    pub(crate) method: Socks5AuthMethod,
}
#[derive(Debug)]
pub(crate) struct Socks5AuthCommandResultContent {
    method: Socks5AuthMethod,
}

impl Socks5AuthCommandResultContent {
    pub(crate) fn new(method: Socks5AuthMethod) -> Self {
        Socks5AuthCommandResultContent { method }
    }

    pub(crate) fn split(self) -> Socks5AuthCommandResultContentParts {
        Socks5AuthCommandResultContentParts { method: self.method }
    }
}

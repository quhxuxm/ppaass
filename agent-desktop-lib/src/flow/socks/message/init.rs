use crate::error::Error;

use super::Socks5Address;
use crate::error::Socks5CodecError;

#[derive(Debug)]
pub(crate) enum Socks5InitCommandType {
    Connect,
    Bind,
    UdpAssociate,
}

impl TryFrom<u8> for Socks5InitCommandType {
    type Error = Error;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            1 => Ok(Socks5InitCommandType::Connect),
            2 => Ok(Socks5InitCommandType::Bind),
            3 => Ok(Socks5InitCommandType::UdpAssociate),
            unknown_type => Socks5CodecError {
                message: format!("unknown init command type: {unknown_type}"),
            }
            .fail(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Socks5InitCommandResultStatus {
    Succeeded,
    Failure,
    ConnectionNotAllowedByRuleSet,
    NetworkUnReachable,
    HostUnReachable,
    ConnectionRefused,
    TtlExpired,
    CommandNotSupported,
    AddressTypeNotSupported,
    Unassigned,
}

impl TryFrom<u8> for Socks5InitCommandResultStatus {
    type Error = Error;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Socks5InitCommandResultStatus::Succeeded),
            1 => Ok(Socks5InitCommandResultStatus::Failure),
            2 => Ok(Socks5InitCommandResultStatus::ConnectionNotAllowedByRuleSet),
            3 => Ok(Socks5InitCommandResultStatus::NetworkUnReachable),
            4 => Ok(Socks5InitCommandResultStatus::HostUnReachable),
            5 => Ok(Socks5InitCommandResultStatus::ConnectionRefused),
            6 => Ok(Socks5InitCommandResultStatus::TtlExpired),
            7 => Ok(Socks5InitCommandResultStatus::CommandNotSupported),
            8 => Ok(Socks5InitCommandResultStatus::AddressTypeNotSupported),
            9 => Ok(Socks5InitCommandResultStatus::Unassigned),
            unknown_status => Socks5CodecError {
                message: format!("unknown init command status: {unknown_status}"),
            }
            .fail(),
        }
    }
}

impl From<Socks5InitCommandResultStatus> for u8 {
    fn from(value: Socks5InitCommandResultStatus) -> Self {
        match value {
            Socks5InitCommandResultStatus::Succeeded => 0,
            Socks5InitCommandResultStatus::Failure => 1,
            Socks5InitCommandResultStatus::ConnectionNotAllowedByRuleSet => 2,
            Socks5InitCommandResultStatus::NetworkUnReachable => 3,
            Socks5InitCommandResultStatus::HostUnReachable => 4,
            Socks5InitCommandResultStatus::ConnectionRefused => 5,
            Socks5InitCommandResultStatus::TtlExpired => 6,
            Socks5InitCommandResultStatus::CommandNotSupported => 7,
            Socks5InitCommandResultStatus::AddressTypeNotSupported => 8,
            Socks5InitCommandResultStatus::Unassigned => 9,
        }
    }
}

pub(crate) struct Socks5InitCommandContentParts {
    pub request_type: Socks5InitCommandType,
    pub dest_address: Socks5Address,
}

#[derive(Debug)]
pub(crate) struct Socks5InitCommandContent {
    request_type: Socks5InitCommandType,
    dest_address: Socks5Address,
}

impl Socks5InitCommandContent {
    pub fn new(request_type: Socks5InitCommandType, dest_address: Socks5Address) -> Self {
        Socks5InitCommandContent { request_type, dest_address }
    }

    pub(crate) fn split(self) -> Socks5InitCommandContentParts {
        Socks5InitCommandContentParts {
            request_type: self.request_type,
            dest_address: self.dest_address,
        }
    }
}

pub(crate) struct Socks5InitCommandResultContentParts {
    pub status: Socks5InitCommandResultStatus,
    pub bind_address: Option<Socks5Address>,
}

#[derive(Debug)]
pub(crate) struct Socks5InitCommandResultContent {
    status: Socks5InitCommandResultStatus,
    bind_address: Option<Socks5Address>,
}

impl Socks5InitCommandResultContent {
    pub fn new(status: Socks5InitCommandResultStatus, bind_address: Option<Socks5Address>) -> Self {
        Socks5InitCommandResultContent { status, bind_address }
    }

    pub fn split(self) -> Socks5InitCommandResultContentParts {
        Socks5InitCommandResultContentParts {
            status: self.status,
            bind_address: self.bind_address,
        }
    }
}

use bytes::Bytes;
use serde_derive::{Deserialize, Serialize};

use crate::{make_as_bytes, CommonError, PpaassUnifiedAddress};

make_as_bytes! {
    #[derive(Serialize, Deserialize, Debug)]
    pub enum AgentTcpPayload {
        Init {
            src_address: PpaassUnifiedAddress,
            dst_address: PpaassUnifiedAddress,
        },
        Data {
            content: Bytes
        },
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ProxyTcpInitFailureReason {
    CanNotConnectToDestination,
    General,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ProxyTcpInitResult {
    Success(String),
    Fail(ProxyTcpInitFailureReason),
}

make_as_bytes! {
    #[derive(Serialize, Deserialize, Debug)]
    pub enum ProxyTcpPayload {
        Init {
            src_address: PpaassUnifiedAddress,
            dst_address: PpaassUnifiedAddress,
            result: ProxyTcpInitResult,
        },
        Data {
            content: Bytes
        }
    }
}

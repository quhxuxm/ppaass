use bytes::Bytes;
use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

use crate::{make_as_bytes, CommonError, PpaassNetAddress};

make_as_bytes! {
    #[derive(Serialize, Deserialize, Constructor)]
    pub struct AgentTcpInit {
        pub src_address: PpaassNetAddress,
        pub dst_address: PpaassNetAddress,
    }
}

#[derive(Serialize, Deserialize)]
pub enum ProxyTcpInitResultType {
    Success,
    Fail,
    ConnectToDstFail,
}

make_as_bytes! {
    #[derive(Serialize, Deserialize, Constructor)]
    pub struct ProxyTcpInit {
        pub id: String,
        pub src_address: PpaassNetAddress,
        pub dst_address: PpaassNetAddress,
        pub result_type: ProxyTcpInitResultType,
    }
}

make_as_bytes! {
    #[non_exhaustive]
    #[derive(Serialize, Deserialize, Constructor)]
    pub struct AgentTcpData {
        pub src_address: PpaassNetAddress,
        pub dst_address: PpaassNetAddress,
        pub data: Bytes,
    }
}

make_as_bytes! {
    #[non_exhaustive]
    #[derive(Serialize, Deserialize, Constructor)]
    pub struct ProxyTcpData {
        pub src_address: PpaassNetAddress,
        pub dst_address: PpaassNetAddress,
        pub data: Bytes,
    }
}

use crate::{make_as_bytes, CommonError, PpaassNetAddress};
use bytes::Bytes;
use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

make_as_bytes! {
    #[non_exhaustive]
    #[derive(Serialize, Deserialize, Constructor)]
    pub struct AgentUdpData {
        pub src_address: PpaassNetAddress,
        pub dst_address: PpaassNetAddress,
        pub data: Bytes,
        pub need_response: bool,
    }
}

make_as_bytes! {
    #[non_exhaustive]
    #[derive(Serialize, Deserialize, Constructor)]
    pub struct ProxyUdpData {
        pub src_address: PpaassNetAddress,
        pub dst_address: PpaassNetAddress,
        pub data: Bytes,
    }
}

use crate::{make_as_bytes, CommonError, PpaassUnifiedAddress};
use bytes::Bytes;
use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

make_as_bytes! {
    #[derive(Serialize, Deserialize, Debug, Constructor)]
    struct AgentUdpData {
        src_address: PpaassUnifiedAddress,
        dst_address: PpaassUnifiedAddress,
        data: Bytes,
        need_response: bool,
    }
}

make_as_bytes! {
    #[derive(Serialize, Deserialize, Debug, Constructor)]
    struct ProxyUdpData {
        src_address: PpaassUnifiedAddress,
        dst_address: PpaassUnifiedAddress,
        data: Bytes,
    }
}

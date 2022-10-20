use serde_derive::{Deserialize, Serialize};

use crate::serializer::vec_array_u8_l4_to_base64;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainResolveRequest {
    pub name: String,
    pub id: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainResolveResponse {
    pub id: i32,
    pub name: String,
    #[serde(with = "vec_array_u8_l4_to_base64")]
    pub ip_addresses: Vec<[u8; 4]>,
}

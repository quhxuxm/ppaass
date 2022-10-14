use crate::serializer::vec_u8_to_base64;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PpaassMessagePayloadEncryption {
    Plain,
    Aes(#[serde(with = "vec_u8_to_base64")] Vec<u8>),
    Bloofish(#[serde(with = "vec_u8_to_base64")] Vec<u8>),
}

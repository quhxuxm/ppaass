use crate::serializer::convert_vecu8_to_base64;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PpaassMessagePayloadEncryption {
    Pain,
    Aes(#[serde(with = "convert_vecu8_to_base64")] Vec<u8>),
    Bloofish(#[serde(with = "convert_vecu8_to_base64")] Vec<u8>),
}

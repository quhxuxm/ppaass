use crate::serializer::vec_u8_to_base64;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PpaassMessagePayloadEncryption {
    Plain,
    Aes(#[serde(with = "vec_u8_to_base64")] Vec<u8>),
    Blowfish(#[serde(with = "vec_u8_to_base64")] Vec<u8>),
}

pub trait PpaassMessagePayloadEncryptionSelector {
    fn select(_user_token: &str, encryption_token: Option<Vec<u8>>) -> PpaassMessagePayloadEncryption {
        match encryption_token {
            None => PpaassMessagePayloadEncryption::Plain,
            Some(encryption_token) => PpaassMessagePayloadEncryption::Aes(encryption_token),
        }
    }
}

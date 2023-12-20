use bytes::Bytes;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PpaassMessagePayloadEncryption {
    Plain,
    Aes(Bytes),
}

pub trait PpaassMessagePayloadEncryptionSelector {
    fn select(_user_token: &str, encryption_token: Option<Bytes>) -> PpaassMessagePayloadEncryption {
        match encryption_token {
            None => PpaassMessagePayloadEncryption::Plain,
            Some(encryption_token) => PpaassMessagePayloadEncryption::Aes(encryption_token),
        }
    }
}

use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PpaassMessagePayloadEncryption {
    Plain,
    Aes(Vec<u8>),
    Blowfish(Vec<u8>),
}

pub trait PpaassMessagePayloadEncryptionSelector {
    fn select(_user_token: impl AsRef<str>, encryption_token: Option<Vec<u8>>) -> PpaassMessagePayloadEncryption {
        match encryption_token {
            None => PpaassMessagePayloadEncryption::Plain,
            Some(encryption_token) => PpaassMessagePayloadEncryption::Aes(encryption_token),
        }
    }
}

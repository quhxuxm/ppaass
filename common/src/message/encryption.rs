use std::borrow::Cow;

use crate::serializer::caw_u8_slince_to_base64;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "token")]
pub enum PpaassMessagePayloadEncryption<'a> {
    Plain,
    Aes(#[serde(with = "caw_u8_slince_to_base64")] Cow<'a, [u8]>),
    Blowfish(#[serde(with = "caw_u8_slince_to_base64")] Cow<'a, [u8]>),
}

pub trait PpaassMessagePayloadEncryptionSelector {
    fn select<'a>(_user_token: impl AsRef<str>, encryption_token: Option<Cow<'a, [u8]>>) -> PpaassMessagePayloadEncryption<'a> {
        match encryption_token {
            None => PpaassMessagePayloadEncryption::Plain,
            Some(encryption_token) => PpaassMessagePayloadEncryption::Aes(encryption_token),
        }
    }
}

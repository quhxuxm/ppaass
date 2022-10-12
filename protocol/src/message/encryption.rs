use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PpaassMessagePayloadEncryption {
    Pain,
    Aes(Vec<u8>),
    Bloofish(Vec<u8>),
}

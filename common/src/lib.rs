pub mod codec;
mod connection;
mod crypto;
mod error;
mod message;

use bytes::Bytes;
pub use connection::*;
pub use crypto::*;
pub use error::*;
pub use message::*;
use rand::random;

pub fn random_32_bytes() -> Bytes {
    let random_32_bytes = random::<[u8; 32]>();
    Bytes::from(random_32_bytes.to_vec())
}

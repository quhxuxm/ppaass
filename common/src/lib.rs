pub mod codec;
mod connection;
mod crypto;
mod error;
mod message;

pub use connection::*;
pub use crypto::*;
pub use error::*;
pub use message::*;
use uuid::Uuid;

pub fn generate_uuid() -> String {
    let uuid_str = Uuid::new_v4().to_string();
    uuid_str.replace('-', "")
}

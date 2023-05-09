mod crypto;
pub use crypto::*;

use uuid::Uuid;
pub mod codec;

mod connection;
mod error;
mod message;

pub use connection::*;
pub use error::*;
pub use message::*;

pub fn generate_uuid() -> String {
    let uuid_str = Uuid::new_v4().to_string();
    uuid_str.replace('-', "")
}

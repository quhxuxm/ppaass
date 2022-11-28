mod crypto;
mod framed;
pub use crypto::*;

use uuid::Uuid;
pub(crate) mod codec;

pub use framed::*;

mod message;
mod serializer;

pub use message::*;

pub fn generate_uuid() -> String {
    let uuid_str = Uuid::new_v4().to_string();
    uuid_str.replace('-', "")
}

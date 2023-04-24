use std::borrow::Cow;

use base64::{engine::general_purpose, Engine};
use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};

fn serialize_byte_array<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
    let base64 = general_purpose::STANDARD.encode(v);
    String::serialize(&base64, s)
}

fn deserialize_byte_array<'de, D: Deserializer<'de>>(d: D) -> Result<Cow<'de, [u8]>, D::Error> {
    let base64 = String::deserialize(d)?;
    let decode_result = general_purpose::STANDARD.decode(base64.as_bytes()).map_err(serde::de::Error::custom)?;
    Ok(Cow::Owned(decode_result))
}

fn deserialize_byte_cow_slince<'de, D: Deserializer<'de>>(d: D) -> Result<Cow<'de, [u8]>, D::Error> {
    let base64 = String::deserialize(d)?;
    let result = general_purpose::STANDARD.decode(base64.as_bytes()).map_err(serde::de::Error::custom)?;
    Ok(result.into())
}

pub(crate) mod caw_u8_slince_to_base64 {

    use std::borrow::Cow;

    use serde::{Deserializer, Serializer};

    use super::{deserialize_byte_cow_slince, serialize_byte_array};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        serialize_byte_array(v, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Cow<'de, [u8]>, D::Error> {
        deserialize_byte_cow_slince(d)
    }
}

pub(crate) mod array_u8_l4_to_base64 {

    use serde::{Deserializer, Serializer};

    use super::{deserialize_byte_array, serialize_byte_array};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        serialize_byte_array(v, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 4], D::Error> {
        let bytes = deserialize_byte_array::<'de, D>(d)?;
        if bytes.len() != 4 {
            return Err(serde::de::Error::custom("Can not deserialize byte array with length 4"));
        }
        let mut bytes_l4 = [0u8; 4];
        bytes_l4.copy_from_slice(&bytes);
        Ok(bytes_l4)
    }
}

pub(crate) mod array_u8_l16_to_base64 {

    use serde::{Deserializer, Serializer};

    use super::{deserialize_byte_array, serialize_byte_array};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        serialize_byte_array(v, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 16], D::Error> {
        let bytes = deserialize_byte_array::<'de, D>(d)?;
        if bytes.len() != 16 {
            return Err(serde::de::Error::custom("Can not deserialize byte array with length 16"));
        }
        let mut bytes_l16 = [0u8; 16];
        bytes_l16.copy_from_slice(&bytes);
        Ok(bytes_l16)
    }
}

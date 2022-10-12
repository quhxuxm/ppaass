use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};

fn serialize_byte_array<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
    let base64 = base64::encode(v);
    String::serialize(&base64, s)
}

fn deserialize_byte_array<'de, D: Deserializer<'de>, const N: usize>(d: D) -> Result<[u8; N], D::Error> {
    let base64 = String::deserialize(d)?;
    let decode_result = base64::decode(base64.as_bytes()).map_err(|e| serde::de::Error::custom(e))?;
    if decode_result.len() != N {
        return Err(serde::de::Error::custom("The length of the result is not equale to 4."));
    }
    let mut result = [0u8; N];
    result.copy_from_slice(&decode_result);
    Ok(result)
}

fn deserialize_byte_vec<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    let base64 = String::deserialize(d)?;
    let result = base64::decode(base64.as_bytes()).map_err(|e| serde::de::Error::custom(e))?;
    Ok(result)
}

pub(crate) mod vec_u8_to_base64 {

    use serde::{Deserializer, Serializer};

    use super::{deserialize_byte_vec, serialize_byte_array};

    pub fn serialize<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        serialize_byte_array(v, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        deserialize_byte_vec(d)
    }
}

pub(crate) mod array_u8_l4_to_base64 {

    use serde::{Deserializer, Serializer};

    use super::{deserialize_byte_array, serialize_byte_array};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        serialize_byte_array(v, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 4], D::Error> {
        deserialize_byte_array::<'de, D, 4>(d)
    }
}

pub(crate) mod array_u8_l16_to_base64 {

    use serde::{Deserializer, Serializer};

    use super::{deserialize_byte_array, serialize_byte_array};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        serialize_byte_array(v, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 16], D::Error> {
        deserialize_byte_array::<'de, D, 16>(d)
    }
}

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

pub(crate) mod option_vec_array_u8_l4_to_base64 {

    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use tracing::error;

    pub fn serialize<S: Serializer>(v: &Option<Vec<[u8; 4]>>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            None => None::<Vec<[u8; 4]>>.serialize(s),
            Some(v) => {
                let mut base64_container = vec![];
                v.iter().for_each(|v| {
                    let base64 = base64::encode(v);
                    base64_container.push(base64);
                });
                Vec::serialize(&base64_container, s)
            },
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<[u8; 4]>>, D::Error> {
        let base64_vec = Vec::<String>::deserialize(d)?;
        let mut result = vec![];
        base64_vec.iter().for_each(|base64| {
            let decode_result = match base64::decode(base64.as_bytes()) {
                Ok(v) => v,
                Err(e) => {
                    error!("Fail to decode base64 bytes because of error: {e:?}");
                    return;
                },
            };
            if decode_result.len() != 4 {
                return;
            }
            let mut ipv4 = [0u8; 4];
            ipv4.copy_from_slice(&decode_result);
            result.push(ipv4);
        });
        Ok(Some(result))
    }
}

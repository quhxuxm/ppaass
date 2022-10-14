use std::{
    fmt::{Debug, Formatter},
    mem::size_of,
    sync::Arc,
};

use bytes::{Buf, BufMut, BytesMut};
use lz4::block::{compress, decompress};
use ppaass_protocol::{PpaassMessage, PpaassMessageParts, PpaassMessagePayloadEncryption};
use pretty_hex::*;
use tokio_util::codec::{Decoder, Encoder};
use tracing::{debug, error, trace};

use ppaass_common::{decrypt_with_aes, decrypt_with_blowfish, encrypt_with_aes, encrypt_with_blowfish, PpaassError, RsaCryptoFetcher};

const PPAASS_FLAG: &[u8] = "__PPAASS__".as_bytes();

enum DecodeStatus {
    Head,
    Data(bool, u64),
}

pub struct PpaassMessageCodec<T: RsaCryptoFetcher> {
    rsa_crypto_fetcher: Arc<T>,
    compress: bool,
    status: DecodeStatus,
}

impl<T> Debug for PpaassMessageCodec<T>
where
    T: RsaCryptoFetcher,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "PpaassMessageCodec: compress={}", self.compress)
    }
}

impl<T> PpaassMessageCodec<T>
where
    T: RsaCryptoFetcher,
{
    pub fn new(compress: bool, rsa_crypto_fetcher: Arc<T>) -> Self {
        Self {
            rsa_crypto_fetcher,
            compress,
            status: DecodeStatus::Head,
        }
    }
}

/// Decode the input bytes buffer to ppaass message
impl<T> Decoder for PpaassMessageCodec<T>
where
    T: RsaCryptoFetcher,
{
    type Item = PpaassMessage;
    type Error = PpaassError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let header_length = PPAASS_FLAG.len() + size_of::<u8>() + size_of::<u64>();
        let (body_is_compressed, body_length) = match self.status {
            DecodeStatus::Head => {
                if src.len() < header_length {
                    debug!("Input message is not enough to decode header.");
                    src.reserve(header_length);
                    return Ok(None);
                }
                let ppaass_flag = src.split_to(PPAASS_FLAG.len());
                if !PPAASS_FLAG.eq(&ppaass_flag) {
                    error!(
                        "Fail to decode input ppaass message because of it dose not begin with ppaass flag, hex data:\n\n{}\n\n",
                        pretty_hex(src)
                    );
                    return Err(PpaassError::CodecError);
                }
                let compressed = src.get_u8() == 1;
                let body_length = src.get_u64();
                src.reserve(body_length as usize);
                debug!("The body length of the input message is {}", body_length);
                self.status = DecodeStatus::Data(compressed, body_length);
                (compressed, body_length)
            },
            DecodeStatus::Data(body_is_compressed, body_length) => (body_is_compressed, body_length),
        };
        if src.remaining() < body_length as usize {
            debug!(
                "Input message is not enough to decode body, buffer remaining: {}, body length: {}.",
                src.remaining(),
                body_length
            );
            return Ok(None);
        }
        self.status = DecodeStatus::Data(body_is_compressed, body_length);
        let body_bytes = src.split_to(body_length as usize);
        trace!("Input message body bytes:\n\n{}\n\n", pretty_hex(&body_bytes));
        let encrypted_message: PpaassMessage = if body_is_compressed {
            debug!("Input message body is compressed.");
            let decompress_result = decompress(body_bytes.chunk(), None)?;
            decompress_result.try_into()?
        } else {
            body_bytes.to_vec().try_into()?
        };

        let PpaassMessageParts {
            id,
            user_token,
            payload_encryption,
            payload_bytes,
        } = encrypted_message.split();

        let rsa_crypto = self.rsa_crypto_fetcher.fetch(&user_token)?.ok_or_else(|| {
            error!("Fail to get user rsa crypto because of not exist, user token: {}", &user_token);
            PpaassError::CodecError
        })?;

        let decrypt_payload_bytes = match payload_encryption {
            PpaassMessagePayloadEncryption::Plain => payload_bytes,
            PpaassMessagePayloadEncryption::Aes(ref encryption_token) => {
                let original_encryption_token = rsa_crypto.decrypt(&encryption_token)?;
                decrypt_with_aes(&original_encryption_token, &payload_bytes)?
            },
            PpaassMessagePayloadEncryption::Bloofish(ref encryption_token) => {
                let original_encryption_token = rsa_crypto.decrypt(&encryption_token)?;
                decrypt_with_blowfish(&original_encryption_token, &payload_bytes)?
            },
        };
        self.status = DecodeStatus::Head;
        src.reserve(header_length);
        let new_message_parts = PpaassMessageParts {
            id,
            user_token,
            payload_encryption,
            payload_bytes: decrypt_payload_bytes,
        };
        Ok(Some(new_message_parts.into()))
    }
}

/// Encode the ppaass message to bytes buffer
impl<T> Encoder<PpaassMessage> for PpaassMessageCodec<T>
where
    T: RsaCryptoFetcher,
{
    type Error = PpaassError;

    fn encode(&mut self, original_message: PpaassMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        debug!("Encode message to output(decrypted): {:?}", original_message);
        dst.put(PPAASS_FLAG);
        if self.compress {
            dst.put_u8(1);
        } else {
            dst.put_u8(0);
        }
        let PpaassMessageParts {
            id,
            user_token,
            payload_encryption,
            payload_bytes,
        } = original_message.split();

        let rsa_crypto = self.rsa_crypto_fetcher.fetch(&user_token)?.ok_or(PpaassError::CodecError)?;
        let (encrypted_payload_bytes, encrypted_payload_encryption_type) = match payload_encryption {
            PpaassMessagePayloadEncryption::Plain => (payload_bytes, PpaassMessagePayloadEncryption::Plain),
            PpaassMessagePayloadEncryption::Aes(ref original_token) => {
                let encrypted_payload_encryption_token = rsa_crypto.encrypt(original_token)?;
                let encrypted_payload_bytes = encrypt_with_aes(original_token, &payload_bytes);
                (encrypted_payload_bytes, PpaassMessagePayloadEncryption::Aes(encrypted_payload_encryption_token))
            },
            PpaassMessagePayloadEncryption::Bloofish(ref original_token) => {
                let encrypted_payload_encryption_token = rsa_crypto.encrypt(original_token)?;
                let encrypted_payload_bytes = encrypt_with_blowfish(original_token, &payload_bytes);
                (
                    encrypted_payload_bytes,
                    PpaassMessagePayloadEncryption::Bloofish(encrypted_payload_encryption_token),
                )
            },
        };
        let message_parts_to_encode = PpaassMessageParts {
            id,
            user_token,
            payload_encryption: encrypted_payload_encryption_type,
            payload_bytes: encrypted_payload_bytes,
        };
        let message_to_encode: PpaassMessage = message_parts_to_encode.into();
        let result_bytes: Vec<u8> = message_to_encode.try_into()?;
        let result_bytes = if self.compress {
            compress(&result_bytes, None, true)?
        } else {
            result_bytes
        };
        let result_bytes_length = result_bytes.len();
        dst.put_u64(result_bytes_length as u64);
        dst.put(result_bytes.as_ref());
        Ok(())
    }
}

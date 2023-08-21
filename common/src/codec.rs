use std::{
    fmt::{Debug, Formatter},
    io::{Read, Write},
    mem::size_of,
};

use crate::{
    decrypt_with_aes, decrypt_with_blowfish, encrypt_with_aes, encrypt_with_blowfish, CommonError, CryptoError, DecoderError, PpaassMessagePayload,
    RsaCryptoFetcher, RsaError,
};
use crate::{PpaassMessage, PpaassMessagePayloadEncryption};
use anyhow::anyhow;
use bytes::{Buf, BufMut, BytesMut};

use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use pretty_hex::*;

use log::{error, trace};
use tokio_util::codec::{Decoder, Encoder};

const PPAASS_FLAG: &[u8] = "__PPAASS__".as_bytes();
const HEADER_LENGTH: usize = PPAASS_FLAG.len() + size_of::<u8>() + size_of::<u64>();
const COMPRESS_FLAG: u8 = 1;
const UNCOMPRESS_FLAG: u8 = 1;
enum DecodeStatus {
    Head,
    Data(bool, u64),
}

pub(crate) struct PpaassMessageCodec<'r, T>
where
    T: RsaCryptoFetcher + 'static,
{
    rsa_crypto_fetcher: &'r T,
    compress: bool,
    status: DecodeStatus,
}

impl<T> Debug for PpaassMessageCodec<'_, T>
where
    T: RsaCryptoFetcher + 'static,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "PpaassMessageCodec: compress={}", self.compress)
    }
}

impl<'r, T> PpaassMessageCodec<'r, T>
where
    T: RsaCryptoFetcher + 'static,
{
    pub fn new<'a>(compress: bool, rsa_crypto_fetcher: &'a T) -> PpaassMessageCodec<'r, T>
    where
        'a: 'r,
    {
        Self {
            rsa_crypto_fetcher,
            compress,
            status: DecodeStatus::Head,
        }
    }
}

/// Decode the input bytes buffer to ppaass message
impl<T> Decoder for PpaassMessageCodec<'_, T>
where
    T: RsaCryptoFetcher + 'static,
{
    type Item = PpaassMessage;
    type Error = CommonError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let (compressed, body_length) = match self.status {
            DecodeStatus::Head => {
                if src.len() < HEADER_LENGTH {
                    trace!("Input message is not enough to decode header, header length: {}", src.len());
                    src.reserve(HEADER_LENGTH);
                    return Ok(None);
                }
                let ppaass_flag = src.split_to(PPAASS_FLAG.len());
                if !PPAASS_FLAG.eq(&ppaass_flag) {
                    return Err(DecoderError::InvalidMessageFlag(anyhow!("The incoming message is not begin with {:?}", PPAASS_FLAG)).into());
                }
                let compressed = src.get_u8() == 1;
                let body_length = src.get_u64();
                src.reserve(body_length as usize);
                trace!("The body length of the input message is {}", body_length);
                self.status = DecodeStatus::Data(compressed, body_length);
                (compressed, body_length)
            },
            DecodeStatus::Data(compressed, body_length) => (compressed, body_length),
        };
        if src.remaining() < body_length as usize {
            trace!(
                "Input message is not enough to decode body, continue read, buffer remaining: {}, body length: {body_length}.",
                src.remaining(),
            );
            src.reserve(body_length as usize);
            return Ok(None);
        }
        trace!(
            "Input message has enough bytes to decode body, buffer remaining: {}, body length: {body_length}.",
            src.remaining(),
        );
        self.status = DecodeStatus::Data(compressed, body_length);
        let body_bytes = src.split_to(body_length as usize);
        trace!("Input message body bytes(compressed={compressed}):\n\n{}\n\n", pretty_hex(&body_bytes));
        let encrypted_message: PpaassMessage = if compressed {
            let mut gzip_decoder = GzDecoder::new(body_bytes.chunk());
            let mut decompressed_bytes = Vec::new();
            if let Err(e) = gzip_decoder.read_to_end(&mut decompressed_bytes) {
                error!("Fail to decompress incoming message bytes because of error: {e:?}");
                return Err(DecoderError::Io(e).into());
            };
            trace!(
                "Decompressed bytes will convert to PpaassMessage:\n{}\n",
                pretty_hex::pretty_hex(&decompressed_bytes)
            );
            let encrypted_message: PpaassMessage = decompressed_bytes.try_into()?;
            encrypted_message
        } else {
            trace!("Raw bytes will convert to PpaassMessage:\n{}\n", pretty_hex::pretty_hex(&body_bytes));
            body_bytes.as_ref().try_into()?
        };

        let PpaassMessage { id, user_token, payload } = encrypted_message;

        let rsa_crypto = self
            .rsa_crypto_fetcher
            .fetch(&user_token)
            .map_err(CryptoError::Rsa)?
            .ok_or(CryptoError::Rsa(RsaError::NotFound(user_token.clone())))?;
        let decrypt_payload_bytes = match payload_encryption {
            PpaassMessagePayloadEncryption::Plain => payload,
            PpaassMessagePayloadEncryption::Aes(ref encryption_token) => {
                let original_encryption_token = rsa_crypto.decrypt(encryption_token).map_err(CryptoError::Rsa)?;
                decrypt_with_aes(&original_encryption_token, &payload).map_err(CryptoError::Aes)?
            },
            PpaassMessagePayloadEncryption::Blowfish(ref encryption_token) => {
                let original_encryption_token = rsa_crypto.decrypt(encryption_token).map_err(CryptoError::Rsa)?;
                decrypt_with_blowfish(&original_encryption_token, &payload).map_err(CryptoError::Blowfish)?
            },
        };
        self.status = DecodeStatus::Head;
        src.reserve(HEADER_LENGTH);
        let message_framed = PpaassMessage::new(id, user_token, payload_encryption, decrypt_payload_bytes);
        Ok(Some(message_framed))
    }
}

/// Encode the ppaass message to bytes buffer
impl<T> Encoder<PpaassMessage> for PpaassMessageCodec<'_, T>
where
    T: RsaCryptoFetcher + 'static,
{
    type Error = CommonError;

    fn encode(&mut self, original_message: PpaassMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        trace!("Encode message to output(decrypted): {:?}", original_message);
        dst.put(PPAASS_FLAG);
        if self.compress {
            dst.put_u8(COMPRESS_FLAG);
        } else {
            dst.put_u8(UNCOMPRESS_FLAG);
        }
        let PpaassMessage {
            id,
            user_token,
            payload_encryption,
            payload,
        } = original_message;

        let rsa_crypto = self
            .rsa_crypto_fetcher
            .fetch(&user_token)
            .map_err(CryptoError::Rsa)?
            .ok_or(CryptoError::Rsa(RsaError::NotFound(user_token.clone())))?;

        let (encrypted_payload_bytes, encrypted_payload_encryption_type) = match payload_encryption {
            PpaassMessagePayloadEncryption::Plain => (payload, PpaassMessagePayloadEncryption::Plain),
            PpaassMessagePayloadEncryption::Aes(ref original_token) => {
                let encrypted_payload_encryption_token = rsa_crypto.encrypt(original_token).map_err(CryptoError::Rsa)?;
                let encrypted_payload_bytes = encrypt_with_aes(original_token, &payload);
                (encrypted_payload_bytes, PpaassMessagePayloadEncryption::Aes(encrypted_payload_encryption_token))
            },
            PpaassMessagePayloadEncryption::Blowfish(ref original_token) => {
                let encrypted_payload_encryption_token = rsa_crypto.encrypt(original_token).map_err(CryptoError::Rsa)?;
                let encrypted_payload_bytes = encrypt_with_blowfish(original_token, &payload);
                (
                    encrypted_payload_bytes,
                    PpaassMessagePayloadEncryption::Blowfish(encrypted_payload_encryption_token),
                )
            },
        };

        let message_to_encode = PpaassMessage::new(id, user_token, encrypted_payload_encryption_type, encrypted_payload_bytes);
        let result_bytes: Vec<u8> = message_to_encode.try_into()?;
        let result_bytes = if self.compress {
            let mut gzip_encoder = GzEncoder::new(Vec::new(), Compression::fast());
            gzip_encoder.write_all(&result_bytes)?;
            gzip_encoder.finish()?
        } else {
            result_bytes
        };
        let result_bytes_length = result_bytes.len();
        dst.put_u64(result_bytes_length as u64);
        dst.put(result_bytes.as_ref());
        Ok(())
    }
}

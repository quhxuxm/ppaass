use aes::Aes256;
use anyhow::anyhow;
use bytes::{Bytes, BytesMut};
use cipher::{block_padding::Pkcs7, BlockEncryptMut};
use cipher::{BlockDecryptMut, KeyInit};

use crate::{generate_uuid, AesError};

type PaddingMode = Pkcs7;

type AesEncryptor = ecb::Encryptor<Aes256>;
type AesDecryptor = ecb::Decryptor<Aes256>;

const BLOCK_SIZE: usize = 32;

pub fn generate_aes_encryption_token() -> Bytes {
    Bytes::from(format!("{}{}", generate_uuid(), generate_uuid()).as_bytes().to_vec())
}

pub fn encrypt_with_aes(encryption_token: &Bytes, target: &mut BytesMut) -> Result<BytesMut, AesError> {
    let original_len = target.len();
    let padding_len = (original_len / BLOCK_SIZE + 1) * BLOCK_SIZE;
    target.extend(vec![0u8; padding_len - original_len]);
    let encryptor = AesEncryptor::new(encryption_token[..].into());
    encryptor
        .encrypt_padded_mut::<PaddingMode>(target.as_mut(), original_len)
        .map(BytesMut::from)
        .map_err(|e| AesError::from(anyhow!(e)))
}

pub fn decrypt_with_aes(encryption_token: &Bytes, target: &mut BytesMut) -> Result<BytesMut, AesError> {
    let decryptor = AesDecryptor::new(encryption_token[..].into());
    decryptor
        .decrypt_padded_mut::<PaddingMode>(target.as_mut())
        .map(BytesMut::from)
        .map_err(|e| AesError::from(anyhow!(e)))
}

#[test]
fn test() -> Result<(), AesError> {
    let encryption_token = Bytes::from_iter(generate_uuid().as_bytes().to_vec());
    let mut target = BytesMut::from_iter("hello world! this is my plaintext888888888888888.".as_bytes().to_vec());
    encrypt_with_aes(&encryption_token, &mut target)?;
    println!("Encrypt result: [{:?}]", String::from_utf8_lossy(&target));
    let mut encrypted_target = BytesMut::from_iter(target.to_vec());
    let descrypted_result = decrypt_with_aes(&encryption_token, &mut encrypted_target)?;
    println!("Decrypted result: [{:?}]", String::from_utf8_lossy(&descrypted_result));
    Ok(())
}

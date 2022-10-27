use aes::Aes256;

use cipher::{block_padding::Pkcs7, BlockEncryptMut};
use cipher::{BlockDecryptMut, KeyInit};
use snafu::ResultExt;

use crate::error::{CryptoUnpadError, Error};

type PaddingMode = Pkcs7;

type AesEncryptor = ecb::Encryptor<Aes256>;
type AesDecryptor = ecb::Decryptor<Aes256>;

pub fn encrypt_with_aes(encryption_token: &[u8], target: &[u8]) -> Vec<u8> {
    let encryptor = AesEncryptor::new(encryption_token.into());
    encryptor.encrypt_padded_vec_mut::<PaddingMode>(target)
}

pub fn decrypt_with_aes(encryption_token: &[u8], target: &[u8]) -> Result<Vec<u8>, Error> {
    let decryptor = AesDecryptor::new(encryption_token.into());
    Ok(decryptor.decrypt_padded_vec_mut::<PaddingMode>(target).context(CryptoUnpadError {
        message: "Fail to decrypt aes bytes because of unpad.",
    })?)
}
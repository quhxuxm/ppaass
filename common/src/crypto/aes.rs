use aes::Aes256;

use cipher::{block_padding::Pkcs7, BlockEncryptMut};
use cipher::{BlockDecryptMut, KeyInit};

use anyhow::{Context, Result};

type PaddingMode = Pkcs7;

type AesEncryptor = ecb::Encryptor<Aes256>;
type AesDecryptor = ecb::Decryptor<Aes256>;

pub fn encrypt_with_aes(encryption_token: &[u8], target: &[u8]) -> Vec<u8> {
    let encryptor = AesEncryptor::new(encryption_token.into());
    encryptor.encrypt_padded_vec_mut::<PaddingMode>(target)
}

pub fn decrypt_with_aes(encryption_token: &[u8], target: &[u8]) -> Result<Vec<u8>> {
    let decryptor = AesDecryptor::new(encryption_token.into());
    decryptor
        .decrypt_padded_vec_mut::<PaddingMode>(target)
        .context("padding error happen when decrypt aes data")
}

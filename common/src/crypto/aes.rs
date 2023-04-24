use std::borrow::Cow;

use aes::Aes256;

use cipher::{block_padding::Pkcs7, BlockEncryptMut};
use cipher::{BlockDecryptMut, KeyInit};

use anyhow::{anyhow, Result};

type PaddingMode = Pkcs7;

type AesEncryptor = ecb::Encryptor<Aes256>;
type AesDecryptor = ecb::Decryptor<Aes256>;

pub fn encrypt_with_aes<'a>(encryption_token: &'a [u8], target: &'a [u8]) -> Cow<'a, [u8]> {
    let encryptor = AesEncryptor::new(encryption_token.into());
    encryptor.encrypt_padded_vec_mut::<PaddingMode>(target).into()
}

pub fn decrypt_with_aes<'a>(encryption_token: &'a [u8], target: &'a [u8]) -> Result<Cow<'a, [u8]>> {
    let decryptor = AesDecryptor::new(encryption_token.into());
    Ok(decryptor
        .decrypt_padded_vec_mut::<PaddingMode>(target)
        .map_err(|e| anyhow!("Padding error happen when decrypt aes data, error:{e:?}"))?
        .into())
}

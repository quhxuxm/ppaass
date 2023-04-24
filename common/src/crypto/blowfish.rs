use std::borrow::Cow;

use anyhow::anyhow;
use anyhow::Result;
use blowfish::Blowfish;
use cipher::{block_padding::Pkcs7, BlockEncryptMut};
use cipher::{BlockDecryptMut, KeyInit};

type PaddingMode = Pkcs7;

type BlowfishEncryptor = ecb::Encryptor<Blowfish>;
type BlowfishDecryptor = ecb::Decryptor<Blowfish>;

pub fn encrypt_with_blowfish<'a>(encryption_token: &'a [u8], target: &'a [u8]) -> Cow<'a, [u8]> {
    let encryptor = BlowfishEncryptor::new(encryption_token.into());
    encryptor.encrypt_padded_vec_mut::<PaddingMode>(target).into()
}

pub fn decrypt_with_blowfish(encryption_token: &[u8], target: &[u8]) -> Result<Vec<u8>> {
    let decryptor = BlowfishDecryptor::new(encryption_token.into());
    Ok(decryptor
        .decrypt_padded_vec_mut::<PaddingMode>(target)
        .map_err(|e| anyhow!("Padding error happen when decrypt blowfish data, error: {e:?}"))?
        .into())
}

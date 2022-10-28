use blowfish::Blowfish;

use cipher::{block_padding::Pkcs7, BlockEncryptMut};
use cipher::{BlockDecryptMut, KeyInit};
use snafu::ResultExt;

use crate::error::{CryptoUnpadError, Error};

type PaddingMode = Pkcs7;

type BlowfishEncryptor = ecb::Encryptor<Blowfish>;
type BlowfishDecryptor = ecb::Decryptor<Blowfish>;

pub fn encrypt_with_blowfish(encryption_token: &[u8], target: &[u8]) -> Vec<u8> {
    let encryptor = BlowfishEncryptor::new(encryption_token.into());
    encryptor.encrypt_padded_vec_mut::<PaddingMode>(target)
}

pub fn decrypt_with_blowfish(encryption_token: &[u8], target: &[u8]) -> Result<Vec<u8>, Error> {
    let decryptor = BlowfishDecryptor::new(encryption_token.into());
    Ok(decryptor.decrypt_padded_vec_mut::<PaddingMode>(target).context(CryptoUnpadError {
        message: "padding error happen.",
    })?)
}

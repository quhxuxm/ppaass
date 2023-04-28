use anyhow::Context;
use anyhow::Result;
use blowfish::Blowfish;
use cipher::{block_padding::Pkcs7, BlockEncryptMut};
use cipher::{BlockDecryptMut, KeyInit};

type PaddingMode = Pkcs7;

type BlowfishEncryptor = ecb::Encryptor<Blowfish>;
type BlowfishDecryptor = ecb::Decryptor<Blowfish>;

pub fn encrypt_with_blowfish(encryption_token: &[u8], target: &[u8]) -> Vec<u8> {
    let encryptor = BlowfishEncryptor::new(encryption_token.into());
    encryptor.encrypt_padded_vec_mut::<PaddingMode>(target)
}

pub fn decrypt_with_blowfish<'a, 'b>(encryption_token: &'a [u8], target: &'b [u8]) -> Result<Cow<'b, [u8]>> {
    let decryptor = BlowfishDecryptor::new(encryption_token.into());
    decryptor
        .decrypt_padded_vec_mut::<PaddingMode>(target)
        .context("padding error happen when decrypt blowfish data")
}

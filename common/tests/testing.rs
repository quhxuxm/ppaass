#[cfg(test)]
mod testing {
    use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyInit};
    use cipher::block_padding::NoPadding;
    use common::{decrypt_with_aes, encrypt_with_aes, generate_uuid, PpaassError};
    use hex_literal::hex;

    #[test]
    fn test1() -> Result<(), PpaassError> {
        let uid = generate_uuid();
        let key = uid.as_bytes();
        let mut plaintext = *b"hello world! this is my plaintext.dhello world! this is my plaintext.dhello world! this is my plaintext.dhello world! this is my plaintext.dhello world! this is my plaintext.dhello world! this is my plaintext.dhello world! this is my plaintext.dhello world! this is my plaintext.dhello world! this is my plaintext.dhello world! this is my plaintext.dswssd";
        // let mut plaintext = *b"";
        let mut encrypt_result = encrypt_with_aes(&key, &mut plaintext)?;
        println!("{:?}", encrypt_result);
        let decrypt_result = decrypt_with_aes(&key, encrypt_result.as_mut_slice());
        println!("{:?}", plaintext);
        println!("{:?}", decrypt_result);
        Ok(())
    }
    #[test]
    fn test2() {
        type Aes128EcbEnc = ecb::Encryptor<aes::Aes128>;
        type Aes128EcbDec = ecb::Decryptor<aes::Aes128>;

        let key = [0x42; 16];
        let plaintext = *b"hello world! this is my plaintext.";
        let ciphertext = hex!(
            "42b153410851a931eb3e6c048867ae5f"
            "95eb20b42e176b07840db75688be9c70"
            "e4670ea0d87a71be5f9f3099b4fff3dc"
        );

        // encrypt/decrypt in-place
        // buffer must be big enough for padded plaintext
        let mut buf = [1u8; 48];
        let pt_len = plaintext.len();
        buf[..pt_len].copy_from_slice(&plaintext);
        let ct = Aes128EcbEnc::new(&key.into()).encrypt_padded_mut::<Pkcs7>(&mut buf, pt_len).unwrap();
        assert_eq!(ct, &ciphertext[..]);

        let pt = Aes128EcbDec::new(&key.into()).decrypt_padded_mut::<Pkcs7>(&mut buf).unwrap();
        assert_eq!(pt, &plaintext);
    }
}

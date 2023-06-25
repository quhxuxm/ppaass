use bytes::{Buf, BufMut, Bytes, BytesMut};

use tokio_util::codec::{Decoder, Encoder};
use tracing::error;

use crate::{
    error::{Socks5DecodeError, Socks5EncodeError},
    SOCKS_V5,
};

use super::message::{
    Socks5Address, Socks5AuthCommandContent, Socks5AuthCommandResultContent, Socks5AuthMethod, Socks5InitCommandContent, Socks5InitCommandResultContent,
    Socks5InitCommandType,
};

#[derive(Debug, Default)]
pub(crate) struct Socks5AuthCommandContentCodec;

impl Decoder for Socks5AuthCommandContentCodec {
    type Item = Socks5AuthCommandContent;
    type Error = Socks5DecodeError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 2 {
            return Ok(None);
        }
        let version = src.get_u8();
        if version != SOCKS_V5 {
            error!("The incoming protocol is not for socks 5: {version}.");
            return Err(Socks5DecodeError::InvalidVersion(version));
        }
        let methods_number = src.get_u8();
        let mut methods = Vec::<Socks5AuthMethod>::new();
        (0..methods_number).for_each(|_| {
            methods.push(Socks5AuthMethod::from(src.get_u8()));
        });
        Ok(Some(Socks5AuthCommandContent::new(methods)))
    }
}

impl Encoder<Socks5AuthCommandResultContent> for Socks5AuthCommandContentCodec {
    type Error = Socks5EncodeError;

    fn encode(&mut self, item: Socks5AuthCommandResultContent, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.put_u8(SOCKS_V5);
        dst.put_u8(item.method.into());
        Ok(())
    }
}

#[derive(Debug, Default)]
pub(crate) struct Socks5InitCommandContentCodec;

impl Decoder for Socks5InitCommandContentCodec {
    type Item = Socks5InitCommandContent;
    type Error = Socks5DecodeError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            return Ok(None);
        }
        let version = src.get_u8();
        if version != SOCKS_V5 {
            error!("The incoming protocol is not for socks 5: {version}.");
            return Err(Socks5DecodeError::InvalidVersion(version));
        }
        let request_type: Socks5InitCommandType = src.get_u8().try_into()?;
        src.get_u8();
        let dest_address: Socks5Address = src.try_into()?;
        Ok(Some(Socks5InitCommandContent::new(request_type, dest_address)))
    }
}

impl Encoder<Socks5InitCommandResultContent> for Socks5InitCommandContentCodec {
    type Error = Socks5EncodeError;

    fn encode(&mut self, item: Socks5InitCommandResultContent, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.put_u8(5);
        dst.put_u8(item.status.into());
        dst.put_u8(0);
        if let Some(bind_address) = item.bind_address {
            dst.put::<Bytes>(bind_address.into());
        }
        Ok(())
    }
}

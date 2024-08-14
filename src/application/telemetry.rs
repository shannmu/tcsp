use std::time::Duration;

use async_trait::async_trait;
use tokio::time::timeout;

use super::{Application, Fallback, Frame};

pub struct TeleMetry<F> {
    fallback: F,
}

#[async_trait]
impl<F: Fallback> Application for TeleMetry<F> {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut response = Frame::new(Self::APPLICATION_ID);
        response.set_meta(frame.meta());
        response.meta_mut().dest_id = 0;
        response.set_len(100)?;

        let send_future = self.fallback.fallback("60000".to_owned().into_bytes());
        let reply = timeout(Duration::from_millis(100), send_future).await??;
        let buf = response.data_mut();

        for (i, byte) in (0..100).zip(reply) {
            buf[i] = byte;
        }
        Ok(Some(response))
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }
}

impl<F: Fallback> TeleMetry<F> {
    pub(crate) const APPLICATION_ID: u8 = 0;
}


impl<F> TeleMetry<F> {
    pub fn request(src_id: u8, dst_id: u8) -> std::io::Result<Frame> {
        let mut frame = Frame::new(0);
        frame.meta_mut().src_id = src_id;
        frame.meta_mut().dest_id = dst_id;
        Ok(frame)
    }
}

impl<F: Fallback> TeleMetry<F> {
    pub fn new(fallback: F) -> Self {
        Self { fallback }
    }
}

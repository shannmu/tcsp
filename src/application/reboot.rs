use async_trait::async_trait;

use super::{Application, Frame};

pub struct Reboot {}

#[async_trait]
impl Application for Reboot {
    async fn handle(&self, _frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let response = Frame::new_from_slice(Self::APPLICATION_ID, "ok".as_bytes())?;
        log::info!("receive reboot",);
        Ok(Some(response))
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }
}

impl Reboot {
    pub(crate) const APPLICATION_ID: u8 = 3;
    pub(crate) fn request(&self) -> std::io::Result<Frame> {
        Ok(Frame::new(Self::APPLICATION_ID))
    }
}

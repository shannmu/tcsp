use async_trait::async_trait;

use super::{Application, Frame};

pub struct Reboot {}

#[async_trait]
impl Application for Reboot {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut response = Frame::new_from_slice(Self::APPLICATION_ID, "ok".as_bytes())?;
        response.set_meta_from_request(frame.meta());

        log::info!("receive reboot");
        Ok(Some(response))
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }

    fn application_name(&self) -> &'static str{
        "Reboot"
    }
}

impl Reboot {
    pub(crate) const APPLICATION_ID: u8 = 3;
    pub(crate) fn request(&self) -> std::io::Result<Frame> {
        Ok(Frame::new(Self::APPLICATION_ID))
    }
}

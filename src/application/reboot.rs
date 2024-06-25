use super::{Application, Frame};


pub struct Reboot {}

impl Application for Reboot {
    fn handle(&self, _frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        #[allow(clippy::unwrap_used)]
        let response = Frame::new_from_slice(Self::APPLICATION_ID,"ok".as_bytes()).unwrap();
        log::info!("receive reboot",);
        Ok(Some(response))
      
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }
}


impl Reboot{
    pub(crate) const APPLICATION_ID: u8 = 3;
    pub(crate) fn request(&self) -> std::io::Result<Frame>{
        Ok(Frame::new(Self::APPLICATION_ID))
    }
}
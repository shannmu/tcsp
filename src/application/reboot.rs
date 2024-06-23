use super::{Application, Frame};

pub struct Reboot {}

impl Application for Reboot {
    fn handle(&self, _frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        #[allow(clippy::unwrap_used)]
        let response = Frame::new_from_slice(1,"ok".as_bytes()).unwrap();
        log::info!("receive reboot",);
        Ok(Some(response))
      
    }

    fn application_id(&self) -> u8 {
        3
    }
}


use super::{Application,Frame};

pub struct EchoCommand;


impl Application for EchoCommand{
    fn handle(&self, frame: Frame,_mtu:u16) -> std::io::Result<Option<Frame>> {
        #[allow(clippy::unwrap_used)]
        let response = Frame::new_from_slice(1,frame.data()).unwrap();
        Ok(Some(response))
    }

    fn application_id(&self) -> u8 {
        2
    }
}
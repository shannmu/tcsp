
use crate::protocol::{Application,Frame};

pub struct EchoCommand;


impl Application for EchoCommand{
    fn handle(&self, frame: &Frame,mtu:u16) -> std::io::Result<Option<Frame>> {
        let response = Frame::default();
        Ok(None)
    }

    fn application_id(&self) -> u8 {
        1
    }
}
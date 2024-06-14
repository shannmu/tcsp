
use crate::protocol::{Application,Frame};

struct EchoCommand;


impl Application for EchoCommand{
    fn handle(&self, frame: &Frame,mtu:u16) -> std::io::Result<Option<Frame>> {
        let response = Frame::default();
        Ok(None)
    }
}
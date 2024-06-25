use std::io;

use super::{Application, Frame};

pub struct EchoCommand;

impl Application for EchoCommand {
    fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        #[allow(clippy::unwrap_used)]
        let response = Frame::new_from_slice(Self::APPLICATION_ID, frame.data()).unwrap();
        Ok(Some(response))
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }
}

impl EchoCommand {
    pub(crate) const APPLICATION_ID: u8 = 2;
    pub(crate) fn request(&self, mtu: u16, content: &[u8]) -> std::io::Result<Frame> {
        if content.len() > mtu.into() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "too long content",
            ));
        }
        Frame::new_from_slice(2, content)
    }
}

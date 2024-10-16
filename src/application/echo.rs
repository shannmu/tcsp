use std::io;

use async_trait::async_trait;

use super::{Application, Frame};

pub struct EchoCommand;

#[async_trait]
impl Application for EchoCommand {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut response = Frame::new_from_slice(Self::APPLICATION_ID, frame.data())?;
        response.set_meta_from_request(frame.meta());
        
        Ok(Some(response))
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }

    fn application_name(&self) -> &'static str{
        "Echo"
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

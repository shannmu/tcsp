use super::{Application, Frame};

pub struct TeleMetry {}

#[repr(C)]
#[allow(unused)]
struct TeleMetryResponse {}

impl Application for TeleMetry {
    fn handle(&self, frame: Frame, mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut response = Frame::new(Self::APPLICATION_ID);
        response.set_meta(frame.meta());
        response.meta_mut().dest_id = 0;
        response.set_len(mtu)?;
        let buf = response.data_mut();
        #[allow(clippy::indexing_slicing)]
        for i in 0..mtu {
            buf[i as usize] = i as u8;
        }
        Ok(Some(response))
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }
}

impl TeleMetry {
    pub(crate) const APPLICATION_ID: u8 = 0;
    pub(crate) fn request() -> std::io::Result<Frame> {
        Ok(Frame::new(0))
    }
}

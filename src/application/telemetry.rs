use super::{Application, Frame};

pub struct TeleMetry {}

#[repr(C)]
#[allow(unused)]
struct TeleMetryResponse {}

impl Application for TeleMetry {
    fn handle(&self, frame: Frame, mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut response = Frame::new(0);
        response.set_meta(frame.meta());
        response.set_len(mtu)?;
        let buf = response.data_mut();
        for i in 0..mtu{
            buf[i as usize] = i as u8;
        }
        Ok(Some(response))
    }

    fn application_id(&self) -> u8 {
        0
    }
}

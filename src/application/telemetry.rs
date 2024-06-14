use crate::protocol::{Application,Frame};

struct TeleMetry{

}

#[repr(C)]
struct TeleMetryResponse{

}

impl Application for TeleMetry{
    fn handle(&self, frame: &Frame,mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut response = Frame::new(0);
        response.set_len(mtu - 2);
        for i in 0..mtu-2{
            response.data_mut()[i as usize] = i as u8;
        }
        Ok(Some(response))
    }
}
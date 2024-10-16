use std::time::Duration;

use async_trait::async_trait;
use tokio::time::timeout;

use super::{Application, Fallback, Frame};

const MAX_UDP_COMMAND_LENGTH: usize = 124;
// const UDP_CUSTOM_CODE: [u8; 4] = [0, 0, 0xea, 0x62];

pub struct UdpBackup<F> {
    fallback: F,
}

#[async_trait]
impl<F: Fallback> Application for UdpBackup<F> {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        if frame.data().len() > MAX_UDP_COMMAND_LENGTH {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Length should be less than: {:?}", MAX_UDP_COMMAND_LENGTH),
            ));
        }
        log::debug!("receive udp backup:{:?}",frame);
        let mut udp_commnad = vec![0; MAX_UDP_COMMAND_LENGTH];
        udp_commnad[0..frame.data().len()].copy_from_slice(frame.data());

        let send_future = self.fallback.fallback(udp_commnad);
        // the custom udp command does not return a result.
        let _reply = timeout(Duration::from_millis(100), send_future).await??;
        Ok(None)
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }

    fn application_name(&self) -> &'static str{
        "UDP command over tcsp"
    }
}

impl<F> UdpBackup<F> {
    pub(crate) const APPLICATION_ID: u8 = 6;

    pub(crate) fn generate_request(data: Vec<u8>, dest_id: u8) -> std::io::Result<Vec<Frame>> {
        let mut frame_vec = Vec::new();
        for chunk in data.chunks(MAX_UDP_COMMAND_LENGTH) {
            let mut frame = Frame::new(Self::APPLICATION_ID);
            frame.meta_mut().src_id = 0; // OBC
            frame.meta_mut().dest_id = dest_id;
            frame.set_len(chunk.len() as u16)?;
            frame.data_mut().clone_from_slice(chunk);
            frame_vec.push(frame);
        }
        Ok(frame_vec)
    }
}

impl<F: Fallback> UdpBackup<F> {
    pub fn new(fallback: F) -> Self {
        Self { fallback }
    }
}

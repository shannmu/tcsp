use std::time::Duration;

use async_trait::async_trait;
use tokio::time::timeout;

use super::{Application, Fallback, Frame};

const MAX_UDP_COMMAND_LENGTH: usize = 124;
const MAX_UDP_BUFFER_LENGTH: usize = 144;
const UDP_CUSTOM_CODE: [u8; 4] = [0, 0, 0xea, 0x62];

pub struct UdpBackup<F> {
    fallback: F,
}

#[async_trait]
impl<F: Fallback> Application for UdpBackup<F> {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut udp_commnad = Vec::with_capacity(MAX_UDP_BUFFER_LENGTH);
        let clone_command_length = frame.data().len().max(MAX_UDP_COMMAND_LENGTH);
        udp_commnad.extend(&UDP_CUSTOM_CODE);
        udp_commnad.extend(&frame.data()[..clone_command_length]);

        let send_future = self.fallback.fallback(udp_commnad);
        // the custom udp command does not return a result.
        let _reply = timeout(Duration::from_millis(100), send_future).await??;
        Ok(None)
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }
}

impl<F> UdpBackup<F> {
    pub(crate) const APPLICATION_ID: u8 = 6;
}

impl<F: Fallback> UdpBackup<F> {
    pub fn new(fallback: F) -> Self {
        Self { fallback }
    }
}

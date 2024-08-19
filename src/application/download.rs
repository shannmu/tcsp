use async_trait::async_trait;
use tokio::sync::Mutex;

use super::{Application, Fallback, Frame};

pub struct DownloadCommand<F> {
    state: Mutex<Box<DownloadState>>,
    fallback: F,
}

#[derive(Debug, Clone, Copy)]
enum DownloadState {
    Start,
    DataResponse(u8),
    Done,
}

#[async_trait]
impl<F: Fallback> Application for DownloadCommand<F> {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut guard = self.state.lock().await;
        let state = guard.as_mut();

        unimplemented!()
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }
}

impl<F: Fallback> DownloadCommand<F> {
    pub(crate) const APPLICATION_ID: u8 = 5;
    pub(crate) fn request(&self, mtu: u16, content: &[u8]) -> std::io::Result<Frame> {
        unimplemented!()
    }
}

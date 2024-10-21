use async_trait::async_trait;
use tokio::sync::Mutex;

use super::{Application, Fallback, Frame};

pub struct DownloadCommand<F> {
    state: Mutex<Box<DownloadState>>,
    fallback: F,
}

#[derive(Debug, Clone, Copy)]
enum DownloadState {
    DownloadStart,
    Downloading(u8),
    DownloadDone(u8),
}

#[async_trait]
impl<F: Fallback> Application for DownloadCommand<F> {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut guard = self.state.lock().await;
        let state = guard.as_mut();

        match state {
            DownloadState::DownloadStart => {
                let data_type = frame.data()[0];
                let response = Frame::new_from_slice(Self::APPLICATION_ID, &[data_type, 0xAA])?;
                *state = DownloadState::Downloading(data_type);
                Ok(Some(response))
            }
            DownloadState::Downloading(data_type) => {
                let data = frame.data();
                if *data_type != data[0] {
                    log::error!("data type mismatch in Downloading");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "data type mismatch",
                    ));
                }

                let data_frame_id = u16::from_be_bytes([data[1], data[2]]);
                let data_frame_sum = u16::from_be_bytes([data[3], data[4]]);
                if data_frame_id != 0 {
                    log::error!("data frame id not match in Downloading");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "data frame id not match",
                    ));
                }
                Ok(None)
            }
            DownloadState::DownloadDone(data_type) => {
                let response = Frame::new_from_slice(Self::APPLICATION_ID, &[*data_type, 0xAA])?;
                *state = DownloadState::DownloadStart;
                Ok(Some(response))
            }
        }
    }
    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }

    fn application_name(&self) -> &'static str {
        "Download files"
    }
}

impl<F: Fallback> DownloadCommand<F> {
    pub(crate) const APPLICATION_ID: u8 = 5;
    pub fn new(fallback: F) -> Self {
        Self {
            state: Mutex::new(Box::new(DownloadState::DownloadStart)),
            fallback,
        }
    }
    pub(crate) fn request(&self, mtu: u16, content: &[u8]) -> std::io::Result<Frame> {
        unimplemented!()
    }
}

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::{Application, Fallback, Frame};

pub struct UploadCommand<F> {
    fallback: F,
    state: Mutex<Box<UploadState>>,
}

#[derive(Debug, Clone, Copy)]
#[allow(variant_size_differences)]
enum UploadState {
    UploadStart,
    UploadResponse(u8),
    DataResponse(u8),
    UploadDone(u8),
    Done,
}

#[async_trait]
impl<F: Fallback> Application for UploadCommand<F> {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut guard = self.state.lock().await;
        let state = guard.as_mut();
        match state {
            UploadState::UploadStart => {
                let data_type = frame.data()[0];
                let response = Frame::new_from_slice(Self::APPLICATION_ID, &[data_type, 0xAA])?;
                *state = UploadState::UploadResponse(data_type);
                Ok(Some(response))
            }
            UploadState::UploadResponse(data_type) => {
                let data = frame.data();
                if *data_type != data[0] {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "data type mismatch",
                    ));
                }

                let data_frame_id = u16::from_be_bytes([data[1], data[2]]);
                let data_frame_sum = u16::from_be_bytes([data[3], data[4]]);
                if data_frame_id != 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "data frame id not match",
                    ));
                }

                // TODO: Handle data with zeromq

                let response = Frame::new_from_slice(
                    Self::APPLICATION_ID,
                    &[*data_type, data[1], data[2], 0xAA],
                )?;

                if data_frame_sum != data_frame_id {
                    *state = UploadState::DataResponse(*data_type);
                } else {
                    *state = UploadState::UploadDone(*data_type);
                }
                Ok(Some(response))
            }
            UploadState::DataResponse(data_type) => {
                let data = frame.data();
                if *data_type != data[0] {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "data type mismatch",
                    ));
                }

                let data_frame_id = u16::from_be_bytes([data[1], data[2]]);
                let data_frame_sum = u16::from_be_bytes([data[3], data[4]]);

                // TODO: Handle data with zeromq

                let response = Frame::new_from_slice(
                    Self::APPLICATION_ID,
                    &[*data_type, data[1], data[2], 0xAA],
                )?;

                if data_frame_sum != data_frame_id {
                    *state = UploadState::DataResponse(*data_type);
                } else {
                    *state = UploadState::UploadDone(*data_type);
                }
                Ok(Some(response))
            }
            UploadState::UploadDone(data_type) => {
                let response = Frame::new_from_slice(Self::APPLICATION_ID, &[*data_type, 0xAA])?;
                *state = UploadState::Done;
                Ok(Some(response))
            }
            UploadState::Done => Ok(None),
        }
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }
}

impl<F: Fallback> UploadCommand<F> {
    pub(crate) const APPLICATION_ID: u8 = 4;
    pub(crate) fn request(&self, mtu: u16, content: &[u8]) -> std::io::Result<Frame> {
        if content.len() > mtu.into() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "too long content",
            ));
        }
        Frame::new_from_slice(Self::APPLICATION_ID, content)
    }
}

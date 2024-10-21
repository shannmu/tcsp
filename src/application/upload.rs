use std::time::Duration;

use async_trait::async_trait;
use tokio::{sync::Mutex, time::timeout};

use super::{Application, Fallback, Frame};

/// TODO: Only support upload one file at a time
pub struct UploadCommand<F> {
    fallback: F,
    state: Mutex<Box<UploadState>>,
    buffer: Vec<u8>,
}

#[derive(Debug, Clone)]
#[allow(variant_size_differences)]
enum UploadState {
    UploadStart,

    UploadWaiting(u8),

    // Uploading(file_mode, file_name)
    Uploading((u8, String)),

    // UploadDone(file_descriptor)
    UploadDone((u8, String)),
}

#[async_trait]
impl<F: Fallback> Application for UploadCommand<F> {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut guard = self.state.lock().await;
        let state = guard.as_mut();
        match state {
            UploadState::UploadStart => {
                let file_mode = frame.data()[0]; // data_tpye means file mode here
                let response = Frame::new_from_slice(Self::APPLICATION_ID, &[file_mode, 0xAA])?;
                *state = UploadState::UploadWaiting(file_mode);
                Ok(Some(response))
            }

            UploadState::UploadWaiting(file_mode) => {
                let data = frame.data();
                if *file_mode != data[0] {
                    log::error!("data type mismatch in UploadWaiting");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "data type mismatch",
                    ));
                }

                let data_frame_id = u16::from_be_bytes([data[1], data[2]]);
                let data_frame_sum = u16::from_be_bytes([data[3], data[4]]);
                if data_frame_id != 0 {
                    log::error!("data frame id not match in UploadWaiting");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "data frame id not match",
                    ));
                }

                let file_name = String::from_utf8(data[5..].to_vec()).unwrap();
                let response = Frame::new_from_slice(Self::APPLICATION_ID, &[*file_mode, 0xAA])?;
                *state = UploadState::Uploading((*file_mode, file_name));
                Ok(Some(response))
            }
            UploadState::Uploading((file_mode, file_name)) => {
                let data = frame.data();
                if *file_mode != data[0] {
                    log::error!("data type mismatch in Uploading");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "data type mismatch",
                    ));
                }

                let data_frame_id = u16::from_be_bytes([data[1], data[2]]);
                let data_frame_sum = u16::from_be_bytes([data[3], data[4]]);
                if data_frame_id != 0 {
                    log::error!("data frame id not match in Uploading");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "data frame id not match",
                    ));
                }

                // TODO: Handle data with zeromq
                // - data[5..] is the data content
                // - handle response if error occurs
                let _relpy = self.fallback.fallback(data[5..].to_vec());
                let _reply = timeout(Duration::from_millis(100), _relpy).await??;

                let response = Frame::new_from_slice(
                    Self::APPLICATION_ID,
                    &[*file_mode, data[1], data[2], 0xAA],
                )?;

                if data_frame_sum != data_frame_id {
                    *state = UploadState::Uploading((*file_mode, file_name.to_owned()));
                } else {
                    *state = UploadState::UploadDone((*file_mode, file_name.to_owned()));
                }
                Ok(Some(response))
            }
            UploadState::UploadDone((file_mode, _file_name)) => {
                let response = Frame::new_from_slice(Self::APPLICATION_ID, &[*file_mode, 0xAA])?;
                *state = UploadState::UploadStart;
                Ok(Some(response))
            }
        }
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }

    fn application_name(&self) -> &'static str {
        "Upload files"
    }
}

impl<F: Fallback> UploadCommand<F> {
    pub(crate) const APPLICATION_ID: u8 = 4;

    pub fn new(fallback: F) -> Self {
        Self {
            fallback,
            state: Mutex::new(Box::new(UploadState::UploadStart)),
            buffer: Vec::new(),
        }
    }

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

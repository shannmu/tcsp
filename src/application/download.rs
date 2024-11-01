use std::collections::HashMap;

use async_trait::async_trait;
use serialport::DataBits;
use tokio::sync::Mutex;

use super::{Application, Fallback, Frame};

pub struct DownloadCommand<F> {
    state: Mutex<Box<DownloadState>>,
    fallback: F,
    buffer: Mutex<HashMap<u16, Vec<u8>>>,
}

#[derive(Debug, Clone)]
enum DownloadState {
    DownloadStart,

    // (file_mode, file_path, chunk_sum)
    Downloading((u8, String, u16)),
}

#[async_trait]
impl<F: Fallback> Application for DownloadCommand<F> {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut guard = self.state.lock().await;
        let state = guard.as_mut();

        match state {
            DownloadState::DownloadStart => {
                let file_mode = frame.meta().id;
                let data = frame.data();

                #[cfg(feature = "unstable_hard_upload_and_download")]
                let file_path = String::from_utf8("/home/user/uart_download".bytes().collect())
                    .expect("Invalid file path");

                #[cfg(not(feature = "unstable_hard_upload_and_download"))]
                let file_path = String::from_utf8(data.to_vec()).expect("Invalid file path");

                {
                    let file_path = std::path::PathBuf::from(file_path.clone());
                    log::debug!("Download file: {:?}", file_path);
                    if !file_path.exists() {
                        log::error!("File not found");
                        let response =
                            Frame::new_from_slice(Self::APPLICATION_ID, &[file_mode, 0xEE], true)?;
                        *state = DownloadState::DownloadStart;
                        return Ok(Some(response));
                    }
                }

                // Read the file content by file_path
                let file_content = std::fs::read(&file_path).expect("Failed to read file content");
                log::debug!("Read file success");

                // Save the file content to buffer, each item in buffer is 1024 bytes
                let mut buffer = self.buffer.lock().await;
                let mut index = 0;
                for chunk in file_content.chunks(1024) {
                    buffer.insert(index, chunk.to_vec());
                    index += 1;
                }

                log::debug!("Save to buffer success");

                // Send the first frame to the client
                let chunck_sum = buffer.len() as u16;

                let first_chunk = buffer.get(&0).expect("First chunk not found");

                log::debug!("Get the first chunk success");

                let mut response_data = vec![
                    0x00,
                    0x00,
                    u16::to_be_bytes(chunck_sum)[0],
                    u16::to_be_bytes(chunck_sum)[1],
                ];
                response_data.extend_from_slice(first_chunk);

                log::debug!("Construct the response success");

                let response = Frame::new_from_slice(Self::APPLICATION_ID, &response_data, true)?;
                *state = DownloadState::Downloading((file_mode, file_path, chunck_sum));
                Ok(Some(response))
            }

            DownloadState::Downloading((file_mode, file_path, chunk_sum)) => {
                let data = frame.data();
                let _file_mode = frame.meta().id;
                if *file_mode != _file_mode {
                    log::error!("data type mismatch in Downloading");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "data type mismatch",
                    ));
                }

                let data_frame_id = u16::from_be_bytes([data[1], data[2]]);

                let buffer = self.buffer.lock().await;
                let file_content = buffer.get(&data_frame_id).expect("Invalid frame id");

                let mut response_data = vec![
                    u16::to_be_bytes(data_frame_id)[0],
                    u16::to_be_bytes(data_frame_id)[1],
                    u16::to_be_bytes(*chunk_sum)[0],
                    u16::to_be_bytes(*chunk_sum)[1],
                ];

                response_data.extend_from_slice(file_content);
                let response = Frame::new_from_slice(Self::APPLICATION_ID, &response_data, true)?;
                if data_frame_id == *chunk_sum - 1 {
                    *state = DownloadState::DownloadStart;
                } else {
                    *state =
                        DownloadState::Downloading((*file_mode, file_path.to_owned(), *chunk_sum));
                }
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
    pub(crate) const APPLICATION_ID: u8 = 7;
    pub fn new(fallback: F) -> Self {
        Self {
            state: Mutex::new(Box::new(DownloadState::DownloadStart)),
            fallback,
            buffer: Mutex::new(HashMap::new()),
        }
    }
    pub(crate) fn request(&self, mtu: u16, content: &[u8]) -> std::io::Result<Frame> {
        unimplemented!()
    }
}

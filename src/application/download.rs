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
                let file_path = String::from_utf8(data.to_vec()).expect("Invalid file path");

                // Read the file content by file_path
                let file_content = std::fs::read(&file_path).expect("Failed to read file content");

                // Save the file content to buffer, each item in buffer is 1024 bytes
                let mut buffer = self.buffer.lock().await;
                let mut index = 0;
                for chunk in file_content.chunks(1024) {
                    buffer.insert(index, chunk.to_vec());
                    index += 1;
                }

                // Send the first frame to the client
                let chunck_sum = buffer.len() as u16;

                let first_chunk = buffer.get(&0).expect("First chunk not found");
                let mut response_data = vec![
                    file_mode,
                    0x00,
                    0x00,
                    u16::to_be_bytes(chunck_sum)[0],
                    u16::to_be_bytes(chunck_sum)[1],
                    0xAA,
                ];
                response_data.extend_from_slice(first_chunk);

                let response = Frame::new_from_slice(Self::APPLICATION_ID, &response_data)?;
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
                    *file_mode,
                    u16::to_be_bytes(data_frame_id)[0],
                    u16::to_be_bytes(data_frame_id)[1],
                    u16::to_be_bytes(*chunk_sum)[0],
                    u16::to_be_bytes(*chunk_sum)[1],
                    0xAA,
                ];

                response_data.extend_from_slice(file_content);
                let response = Frame::new_from_slice(Self::APPLICATION_ID, &response_data)?;
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

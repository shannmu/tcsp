use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DeviceAdaptorError {
    #[error("Frame construct error")]
    FrameError(String),

    #[error("Bus error:{:?}", 0)]
    BusError(Box<dyn std::error::Error>),

    #[error("No data available now")]
    Empty,
}

unsafe impl Send for DeviceAdaptorError {}

impl From<socketcan::Error> for DeviceAdaptorError {
    fn from(error: socketcan::Error) -> Self {
        Self::BusError(Box::new(error))
    }
}
impl From<io::Error> for DeviceAdaptorError {
    fn from(error: io::Error) -> Self {
        Self::BusError(Box::new(error))
    }
}
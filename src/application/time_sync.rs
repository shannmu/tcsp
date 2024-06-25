use chrono::{DateTime, Utc};
use futures_util::io;

use super::{Application, Frame};

pub struct TimeSync {}

impl Application for TimeSync {
    fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        log::info!("{:?}", frame.data());
        let time_slice: [u8; 4] = frame.data()[..4].try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Can not convert time slice to [u8;4]",
            )
        })?;
        let timestamp = u32::from_be_bytes(time_slice);
        let datetime = DateTime::from_timestamp(timestamp as i64, 0).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to convert {} into datetime", timestamp),
            )
        })?;
        log::debug!("datetime = {}", datetime);
        Ok(None)
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }
}

impl TimeSync {
    pub(crate) const APPLICATION_ID: u8 = 1;

    /// Create a new TimeSync request frame
    ///
    /// Provide a datetime to be used as the timestamp
    pub(crate) fn request(datetime: DateTime<Utc>) -> std::io::Result<Frame> {
        Frame::new_from_slice(1, &datetime.timestamp().to_be_bytes())
    }

    pub(crate) fn request_now() -> std::io::Result<Frame> {
        let datetime = Utc::now();
        Self::request(datetime)
    }
}

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::io;
use tokio::time::timeout;

use super::{Application, Fallback, Frame};

pub struct TimeSync<F> {
    fallback : F,
}

#[async_trait]
impl<F:Fallback> Application for TimeSync<F> {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        // log::info!("{:?}", frame.data());
        let time_slice: [u8; 4] = frame.data()[..4].try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Can not convert time slice to [u8;4]",
            )
        })?;
        let future_to_wait = self.fallback.fallback(vec![time_slice[0],time_slice[1],time_slice[2],time_slice[3],0,0]);
        // we ignore the reply
        let _reply = timeout(Duration::from_millis(100), future_to_wait).await??;
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

impl<F> TimeSync<F> {
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


impl<F: Fallback> TimeSync<F> {
    pub fn new(fallback: F) -> Self {
        Self { fallback }
    }
}

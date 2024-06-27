use std::io;

use async_trait::async_trait;
use bitflags::bitflags;
use thiserror::Error;

mod can;
mod uart;
mod channel;
pub use channel::Channel;
pub use uart::TyUartProtocol;
pub use uart::Uart;
pub use can::ty::TyCanProtocol;

#[async_trait]
pub trait DeviceAdaptor: Send + Sync{
    /// Send a bus frame to the bus
    async fn send(&self, frame: Frame) -> Result<(), DeviceAdaptorError>;

    /// Receive a bus frame from the bus.
    /// You might not receive the entier frame at one call, in this case, `DeviceAdaptorError::Empty` will be returned.
    async fn recv(&self) -> Result<Frame, DeviceAdaptorError>;

    /// The mtu of the bus frame. Typically, the data excced the mtu may discard by adaptor, or the adaptor can return an error.
    /// 
    /// Some devices like uart may have different mtu when giving different `FrameFlag`.
    fn mtu(&self, flag: FrameFlag) -> usize;
}

const FRAME_MAX_LENGTH: usize = 150;
const FRAME_PADDING: usize = 18;
const FRAME_DATA_LENGTH: usize = FRAME_MAX_LENGTH + FRAME_PADDING;
const FRAME_DEFAULT_START_OFFSET: u16 = 16;

#[derive(Debug, Default, Clone, Copy)]
pub struct FrameMeta {
    pub(crate)src_id: u8,
    pub(crate)dest_id: u8,
    pub(crate)id: u8,
    pub(crate)len: u16,
    pub(crate)data_type: u8,
    pub(crate)command_type: u8,
    pub(crate)flag: FrameFlag,
}

bitflags! {
    #[derive(Debug,Clone,Copy,Default, PartialEq, Eq)]
    pub struct FrameFlag: u8 {
        const CanTimeBroadcast = 1;
        const UartTelemetry = 1<<2;
    }
}

/// A `Frame` is a data structure that report meta and data payload of Can, Uart or other bus frame
/// 
/// The `Frame` use a fixed size(which is `FRAME_DATA_LENGTH`) of u8 buffer, and it is allocated on the heap.
/// The frame's buffer can expand or shrink at a certain range. For the heading side, you can expand at most `FRAME_DEFAULT_START_OFFSET` bytes.
/// And at the ending side, you can expand only FRAME_PADDING - FRAME_DEFAULT_START_OFFSET bytes, which is 2 bytes currently.
/// When you call methods like `expand_` or `shrink`, the field `length` in `meta` will change at the same. We will move length to a private field sooner.
/// So be careful when you use these methods.
#[derive(Debug, Clone)]
pub struct Frame {
    pub(crate)meta: FrameMeta,
    offset: u16,
    data: Box<[u8; FRAME_DATA_LENGTH]>,
}

impl Frame {
    pub(crate) fn new(meta: FrameMeta, data: &[u8]) -> io::Result<Self> {
        if data.len() > FRAME_MAX_LENGTH {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "buffer too large"));
        }
        let mut frame = Frame {
            meta,
            offset: FRAME_DEFAULT_START_OFFSET,
            data: Box::new([0u8; FRAME_DATA_LENGTH]),
        };
        #[allow(clippy::indexing_slicing)] // checked in `data.len() > FRAME_MAX_LENGTH`
        frame.data[FRAME_DEFAULT_START_OFFSET.into()..(FRAME_DEFAULT_START_OFFSET as usize + data.len())]
            .copy_from_slice(data);
        frame.meta.len = data.len() as u16;
        Ok(frame)
    }

    pub(crate) fn len(&self) -> usize {
        self.meta.len as usize
    }

    pub(crate) fn set_len(&mut self, len: u16) -> io::Result<()> {
        if len > FRAME_DATA_LENGTH as u16 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "set_len"));
        }
        self.meta.len = len;
        Ok(())
    }

    pub(crate) fn expand_head(&mut self, len: usize) -> io::Result<()> {
        let len = len as u16;
        let offset = self.offset as i32;
        if offset - (len as i32) < 0  {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "expand_head"));
        }
        self.offset -= len;
        self.meta.len += len;
        Ok(())
    }

    pub(crate) fn shrink_head(&mut self, len: usize) -> io::Result<()> {
        let len = len as u16;
        if self.offset + len >= FRAME_DATA_LENGTH as u16 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "expand_head"));
        }
        self.offset += len;
        if self.meta.len < len{
            self.meta.len = 0
        }else{
            self.meta.len -= len;
        }
        Ok(())
    }

    pub(crate) fn expand_tail(&mut self, len: usize) -> io::Result<()> {
        let len = len as u16;
        if self.offset + self.meta.len + len > FRAME_DATA_LENGTH as u16 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "expand_tail"));
        }
        self.meta.len += len;
        Ok(())
    }

    pub(crate) fn data(&self) -> &[u8] {
        let start = self.offset as usize;
        let end = start + self.meta.len as usize;
        &self.data[start..end]
    }

    pub(crate) fn data_mut(&mut self) -> &mut [u8] {
        let start = self.offset as usize;
        let end = start + self.meta.len as usize;
        &mut self.data[start..end]
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            meta: Default::default(),
            offset: FRAME_DEFAULT_START_OFFSET,
            data: Box::new([0u8; FRAME_DATA_LENGTH]),
        }
    }
}

#[derive(Error, Debug)]
pub enum DeviceAdaptorError {
    #[error("Frame construct error")]
    FrameError(String),

    #[error("Bus error")]
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


#[cfg(test)]
mod tests{
    use crate::adaptor::{FRAME_DATA_LENGTH, FRAME_DEFAULT_START_OFFSET, FRAME_MAX_LENGTH, FRAME_PADDING};

    use super::Frame;

    #[test]
    fn test_buffer_head_expand_and_shrink(){
        let mut buffer = Frame::default();
        assert!(buffer.expand_head(10).is_ok());
        assert!(buffer.expand_head((FRAME_DEFAULT_START_OFFSET - 10).into()).is_ok());
        assert!(buffer.expand_head(1).is_err());

        assert!(buffer.shrink_head(5).is_ok());
        assert_eq!(buffer.meta.len, FRAME_DEFAULT_START_OFFSET - 5);
        assert!(buffer.shrink_head(FRAME_DEFAULT_START_OFFSET.into()).is_ok());
        assert_eq!(buffer.meta.len, 0);

        assert!(buffer.shrink_head(FRAME_DATA_LENGTH).is_err());
    }

    #[test]
    fn test_buffer_shrink(){
        let mut buffer = Frame::default();
        assert!(buffer.expand_tail(1).is_ok());
        assert_eq!(buffer.meta.len, 1);
        assert!(buffer.expand_tail(FRAME_MAX_LENGTH).is_ok());
        assert_eq!(buffer.meta.len as usize, FRAME_MAX_LENGTH + 1);
        assert!(buffer.expand_tail(FRAME_PADDING - FRAME_DEFAULT_START_OFFSET as usize -1).is_ok());
        assert!(buffer.expand_tail(1).is_err());
    }
}
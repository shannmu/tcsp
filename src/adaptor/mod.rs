use std::io;

use async_trait::async_trait;
use bitflags::bitflags;
use thiserror::Error;

pub(crate) mod can;
pub(crate) mod uart;

pub(crate) use can::ty::TyCanProtocol;

#[async_trait]
pub(crate) trait DeviceAdaptor {
    async fn send(&self, frame: Frame) -> Result<(), DeviceAdaptorError>;
    async fn recv(&self) -> Result<Frame, DeviceAdaptorError>;
    fn mtu(&self, flag: FrameFlag) -> usize;
}

const FRAME_MAX_LENGTH: usize = 150;
const FRAME_PADDING: usize = 18;
const FRAME_DATA_LENGTH: usize = FRAME_MAX_LENGTH + FRAME_PADDING;
const FRAME_DEFAULT_START_OFFSET: u16 = 16;

#[derive(Debug, Default, Clone, Copy)]
pub(crate)struct FrameMeta {
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
    pub(crate) struct FrameFlag: u8 {
        const CanTimeBroadcast = 1;
        const UartTelemetry = 1<<2;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Frame {
    meta: FrameMeta,
    offset: u16,
    data: Box<[u8; FRAME_DATA_LENGTH]>,
}

impl Frame {
    fn new(meta: FrameMeta, data: &[u8]) -> Self {
        let mut frame = Frame {
            meta,
            offset: FRAME_DEFAULT_START_OFFSET,
            data: Box::new([0u8; FRAME_DATA_LENGTH]),
        };
        frame.data
            [FRAME_DEFAULT_START_OFFSET.into()..(FRAME_DEFAULT_START_OFFSET as usize + data.len())]
            .copy_from_slice(&data);
        frame.meta.len = data.len() as u16;
        frame
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
        if self.offset < len {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "expand_head"));
        }
        self.offset -= len;
        self.meta.len += len;
        Ok(())
    }

    pub(crate) fn expand_tail(&mut self, len: usize) -> io::Result<()> {
        let len = len as u16;
        if self.meta.len + len > FRAME_DATA_LENGTH as u16 {
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

    pub(crate) fn meta(&self) -> &FrameMeta{
        &self.meta
    }

    pub(crate) fn meta_mut(&mut self) -> &mut FrameMeta{
        &mut self.meta
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            meta: Default::default(),
            offset: Default::default(),
            data: Box::new([0u8; FRAME_DATA_LENGTH]),
        }
    }
}

#[derive(Error, Debug)]
pub(crate) enum DeviceAdaptorError {
    #[error("Frame construct error")]
    FrameError(String),

    #[error("Bus error")]
    BusError(Box<dyn std::error::Error>),

    #[error("No data available now")]
    Empty,
}

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

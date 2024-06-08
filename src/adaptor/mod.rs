use std::io;

use async_trait::async_trait;
use bitflags::bitflags;

pub(crate) mod can;
pub(crate) mod uart;

#[async_trait]
pub trait DeviceAdaptor {
    async fn send(&self, frame:Frame);
    async fn recv(&self) -> Option<Frame>;
}

const FRAME_MAX_LENGTH: usize = 150;
const FRAME_PADDING: usize = 18;
const FRAME_DATA_LENGTH: usize = FRAME_MAX_LENGTH + FRAME_PADDING;
const FRAME_DEFAULT_START_OFFSET: u8 = 8;

#[derive(Debug)]
struct FrameMeta {
    src_id: u8,
    dest_id: u8,
    len: u8,
    flag : FrameFlag
}

bitflags!{
    #[derive(Debug,Clone,Copy)]
    pub(crate) struct FrameFlag: u8 {
        const CanTimeBroadcast = 1;
        const UartTelemetry = 1<<2;
    }
}

#[derive(Debug)]
struct Frame {
    meta: FrameMeta,
    offset: u8,
    data: Box<[u8; FRAME_DATA_LENGTH]>,
}

impl Frame {
    fn new(meta: FrameMeta, data: &[u8]) -> Self {
        let mut frame = Frame {
            meta,
            offset: FRAME_DEFAULT_START_OFFSET,
            data: Box::new([0u8; FRAME_DATA_LENGTH]),
        };
        frame.data[FRAME_DEFAULT_START_OFFSET.into()..].copy_from_slice(&data);
        frame
    }

    fn len(&self) -> usize {
        self.meta.len as usize
    }

    fn expand_head(&mut self,len:usize) -> io::Result<()>{
        let len = len as u8;
        if self.offset < len {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "expand_head"));
        }
        self.offset -= len;
        self.meta.len += len;
        Ok(())
    }
    
    fn expand_tail(&mut self,len:usize) -> io::Result<()>{
        let len = len as u8;
        if self.meta.len + len > FRAME_DATA_LENGTH as u8 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "expand_tail"));
        }
        self.meta.len += len;
        Ok(())
    }

    fn data(&self) -> &[u8] {
        let start = self.offset as usize;
        let end = start + self.meta.len as usize;
        &self.data[start..end]
    }

    fn data_mut(&mut self) -> &mut [u8] {
        let start = self.offset as usize;
        let end = start + self.meta.len as usize;
        &mut self.data[start..end]
    }
}

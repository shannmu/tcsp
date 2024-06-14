use std::{io, mem::size_of};

use crate::adaptor::{Frame as BusFrame, FrameMeta};

use super::insert_header;

const VERSION_ID: u8 = 0x20;

#[repr(C)]
pub(crate) struct FrameHeader {
    pub(crate) version: u8,
    pub(crate) application: u8,
}

impl TryFrom<&[u8]> for FrameHeader {
    type Error = std::io::Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        // TODO:
        // if buf.len() < size_of::<FrameHeader>(){
        //     return Err(());
        // }
        let hdr: Self = unsafe { std::ptr::read(buf.as_ptr() as *const FrameHeader) };
        Ok(hdr)
    }
}

impl TryFrom<&mut [u8]> for &mut FrameHeader {
    type Error = std::io::Error;

    fn try_from(buf: &mut [u8]) -> Result<Self, Self::Error> {
        // TODO:
        // if buf.len() < size_of::<FrameHeader>(){
        //     return Err(());
        // }
        unsafe { Ok(&mut *(buf.as_mut_ptr() as *mut FrameHeader)) }
    }
}
#[derive(Default,Debug)]
pub(crate) struct Frame {
    bus_frame: BusFrame,
    application_id: u8,
    hdr_inserted: bool,
}

impl TryFrom<BusFrame> for Frame {
    type Error = std::io::Error;
    fn try_from(bus_frame: BusFrame) -> Result<Self, Self::Error> {
        let hdr = FrameHeader::try_from(bus_frame.data())?;
        if hdr.version != VERSION_ID {}
        Ok(Self {
            bus_frame,
            application_id: hdr.application,
            hdr_inserted: true,
        })
    }
}

impl From<Frame> for BusFrame {
    fn from(frame: Frame) -> Self {
        frame.bus_frame
    }
}

impl Frame {
    pub(crate) fn new(application_id:u8) -> Self {
        Self {
            bus_frame: BusFrame::default(),
            application_id,
            hdr_inserted: false,
        }
    }
    pub(crate) fn application(&self) -> u8 {
        self.application_id
    }

    pub(crate) fn data(&self) -> &[u8] {
        self.bus_frame.data()
    }

    pub(crate) fn data_mut(&mut self) -> &mut [u8] {
        self.bus_frame.data_mut()
    }

    pub(super) fn insert_header(&mut self) -> io::Result<()> {
        if !self.hdr_inserted {
            insert_header(&mut self.bus_frame, self.application_id)?;
        }
        Ok(())
    }

    pub(crate) fn set_len(&mut self, len: u16) -> io::Result<()>{
        self.bus_frame.set_len(len)
    }

    pub(crate) fn meta(&self) -> &FrameMeta{
        &self.bus_frame.meta
    }

    pub(crate) fn meta_mut(&mut self) -> &mut FrameMeta{
        &mut self.bus_frame.meta
    }

    pub(crate) fn set_meta(&mut self,meta:&FrameMeta){
        self.bus_frame.meta = *meta;
    }
}

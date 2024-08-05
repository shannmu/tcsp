use std::{io, mem::size_of};

use crate::adaptor::{Frame as BusFrame, FrameFlag, FrameMeta};


pub(crate)const VERSION_ID: u8 = 0x20;

#[repr(C)]
pub(crate) struct FrameHeader {
    pub(crate) version: u8,
    pub(crate) application: u8,
}

impl TryFrom<&[u8]> for FrameHeader {
    type Error = std::io::Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        if buf.len() < size_of::<FrameHeader>(){
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Buffer not larges enough",
            ));
        }
        let hdr: Self = unsafe { std::ptr::read(buf.as_ptr() as *const FrameHeader) };
        Ok(hdr)
    }
}

impl TryFrom<&mut [u8]> for &mut FrameHeader {
    type Error = std::io::Error;

    fn try_from(buf: &mut [u8]) -> Result<Self, Self::Error> {
        if buf.len() < size_of::<FrameHeader>(){
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Buffer not larges enough",
            ));
        }
        unsafe { Ok(&mut *(buf.as_mut_ptr() as *mut FrameHeader)) }
    }
}
#[derive(Default, Debug)]
pub struct Frame {
    bus_frame: BusFrame,
    application_id: u8,
    hdr_inserted: bool,
}

impl TryFrom<BusFrame> for Frame {
    type Error = std::io::Error;
    fn try_from(mut bus_frame: BusFrame) -> Result<Self, Self::Error> {
        install_header_if_needed(&mut bus_frame)?;
        let hdr = FrameHeader::try_from(bus_frame.data())?;
        if hdr.version != VERSION_ID {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Version ID not match",
            ));
        }
        bus_frame.shrink_head(size_of::<FrameHeader>())?;
        Ok(Self {
            bus_frame,
            application_id: hdr.application,
            hdr_inserted: true,
        })
    }
}

impl TryFrom<Frame> for BusFrame {
    type Error = std::io::Error;
    fn try_from(mut frame: Frame) -> Result<Self, Self::Error> {
        frame.insert_header()?;
        Ok(frame.bus_frame)
    }
}

impl Frame {
    pub(crate) fn new(application_id: u8) -> Self {
        Self {
            bus_frame: BusFrame::default(),
            application_id,
            hdr_inserted: false,
        }
    }

    pub(crate) fn new_from_slice(application_id: u8,data: &[u8]) -> io::Result<Self> {
        let bus_frame =  BusFrame::new(FrameMeta::default(),data)?;
        Ok(Self {
            bus_frame,
            application_id,
            hdr_inserted: false,
        })
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

    fn insert_header(&mut self) -> io::Result<()> {
        if !self.hdr_inserted {
            insert_header(&mut self.bus_frame, self.application_id)?;
        }
        Ok(())
    }

    pub(crate) fn set_len(&mut self, len: u16) -> io::Result<()> {
        self.bus_frame.set_len(len)
    }

    pub(crate) fn meta(&self) -> &FrameMeta {
        &self.bus_frame.meta
    }

    pub(crate) fn meta_mut(&mut self) -> &mut FrameMeta {
        &mut self.bus_frame.meta
    }
    
    pub(crate) fn set_meta(&mut self, meta: &FrameMeta) {
        self.bus_frame.meta = *meta;
    }
}

fn insert_header(bus_frame: &mut BusFrame, application_id: u8) -> io::Result<()> {
    bus_frame.expand_head(size_of::<FrameHeader>())?;
    let hdr: &mut FrameHeader = bus_frame.data_mut().try_into()?;
    hdr.version = VERSION_ID;
    hdr.application = application_id;
    Ok(())
}

fn install_header_if_needed(frame: &mut BusFrame) -> Result<(), io::Error> {
    let meta = &frame.meta;
    if meta.flag.contains(FrameFlag::UartTelemetry) {
        // The application id=1 refers to the telemetry service.
        insert_header(frame, 0)?;
    }else if meta.flag.contains(FrameFlag::CanTimeBroadcast) {
        // The CanTimeBroadcast's first two bytes should be 0x50 0x05
        // We can ignore them.
        frame.shrink_head(2)?;
        // The application id=1 refers to the time sync service.
        insert_header(frame, 1)?;
    }else{
        {}
    }
    Ok(())
}

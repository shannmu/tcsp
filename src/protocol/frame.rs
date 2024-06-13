use crate::adaptor::Frame as BusFrame;



#[repr(C)]
pub(super)struct FrameHeader{
    version : u8,
    application : u8,
    id : u16,
}

impl TryFrom<&[u8]> for FrameHeader{
    type Error = std::io::Error;

    fn try_from(buf : &[u8]) -> Result<Self, Self::Error> {
        // TODO:
        // if buf.len() < size_of::<FrameHeader>(){
        //     return Err(());
        // }
        let hdr : Self = unsafe{
            std::ptr::read(buf.as_ptr() as *const FrameHeader)
        };
        Ok(hdr)
    }
}


pub(super)struct Frame{
    bus_frame : BusFrame,
}

impl TryFrom<BusFrame> for Frame{
    type Error = std::io::Error;
    fn try_from(bus_frame: BusFrame) -> Result<Self, Self::Error> {
        let _hdr = FrameHeader::try_from(bus_frame.data())?;
        // TODO:check meta
        Ok(Self{
            bus_frame,
        })
    }
}

impl From<Frame> for BusFrame{
    fn from(frame: Frame) -> Self {
        frame.bus_frame
    }
}

impl Frame{
    pub(super) fn application(&self) -> u8{
        let hdr = FrameHeader::try_from(self.bus_frame.data()).unwrap();
        hdr.application
    }       

    pub(super) fn data(&self) -> &[u8]{
        self.bus_frame.data()
    }

    pub(super) fn data_mut(&mut self) -> &mut [u8]{
        self.bus_frame.data_mut()
    }


}
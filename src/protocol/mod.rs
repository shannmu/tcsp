use std::{io, mem::size_of, sync::Arc};

use crate::adaptor::{can::ty::TyCanProtocol, uart::TyUartProtocol, DeviceAdaptor, FrameFlag,Frame as BusFrame};

const MAX_APPLICATION_HANDLER: usize = 256;
const VERSION_ID: u8 = 0x20;
mod frame;
pub(crate) struct Tcsp<D>(Box<TcspInner<D>>);
pub(crate) use frame::Frame;

use frame::FrameHeader;

struct TcspInner<D> {
    adaptor: D,
    applications: [Option<Arc<dyn Application>>; MAX_APPLICATION_HANDLER],
}

pub(crate) trait Application {
    fn handle(&self, frame: &Frame, mtu: u16) -> std::io::Result<Option<Frame>>;

    fn application_id(&self) -> u8;
}

impl Tcsp<TyCanProtocol> {
    const ARRAY_REPEAT_VALUE: Option<Arc<(dyn Application + 'static)>> = None;
    pub(crate) fn new_can(adaptor: TyCanProtocol) -> Self {
        Tcsp(Box::new(TcspInner {
            adaptor,
            applications: [Self::ARRAY_REPEAT_VALUE; MAX_APPLICATION_HANDLER],
        }))
    }
}

impl Tcsp<TyUartProtocol> {
    const ARRAY_REPEAT_VALUE: Option<Arc<(dyn Application + 'static)>> = None;
    pub(crate) fn new_uart(adaptor: TyUartProtocol) -> Self {
        Tcsp(Box::new(TcspInner {
            adaptor,
            applications: [Self::ARRAY_REPEAT_VALUE; MAX_APPLICATION_HANDLER],
        }))
    }
}

fn install_header_if_needed(frame:&mut BusFrame) -> Result<(),io::Error>{
    let meta = &frame.meta;
    if meta.flag.contains(FrameFlag::UartTelemetry){
        insert_header(frame,0)?;
    }
    Ok(())
}

impl<D: DeviceAdaptor> Tcsp<D> {
    pub(crate) async fn listen(&self) {
        loop{
            if let Err(e) = self.handle().await{
                log::error!("Error occurs:{:?}",e);
            }
        }
    }

    async fn handle(&self) -> Result<(), io::Error> {
        if let Ok(mut bus_frame) = self.0.adaptor.recv().await {
            log::info!("receive frame from bus:{:?}",bus_frame);
            install_header_if_needed(&mut bus_frame)?;
            let frame = Frame::try_from(bus_frame)?;
            let mtu = self.0.adaptor.mtu(frame.meta().flag) as u16;
            let application_id = frame.application();
            if let Some(Some(application)) = self.0.applications.get(application_id as usize) {
                let response = application.handle(&frame, mtu)?;
                log::info!("response:{:?}",response);
                if let Some(mut response) = response {
                    response.insert_header()?;
                    self.0.adaptor.send(response.into()).await.unwrap();
                }
            }
        }
        Ok(())
    }

    pub(crate) fn register(
        &mut self,
        application: Arc<(dyn Application + 'static)>,
    ) {
        let id = application.application_id();
        self.0.applications[id as usize] = Some(application);
    }
}


pub(crate) fn insert_header(bus_frame : &mut BusFrame,application_id : u8) -> io::Result<()> {
        bus_frame.expand_head(size_of::<FrameHeader>())?;
        let hdr: &mut FrameHeader = bus_frame.data_mut().try_into()?;
        hdr.version = VERSION_ID;
        hdr.application = application_id;
    Ok(())
}
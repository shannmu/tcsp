use std::{io, sync::Arc};

use crate::adaptor::{can::ty::TyCanProtocol, uart::TyUartProtocol, DeviceAdaptor};

const MAX_APPLICATION_HANDLER: usize = 256;

mod frame;
pub(crate) struct Tcsp<D>(Box<TcspInner<D>>);
pub(crate) use frame::Frame;

struct TcspInner<D> {
    adaptor: D,
    applications: [Option<Arc<dyn Application>>; MAX_APPLICATION_HANDLER],
}

pub(crate) trait Application {
    fn handle(&self, frame: &Frame, mtu: u16) -> std::io::Result<Option<Frame>>;
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

impl<D: DeviceAdaptor> Tcsp<D> {
    pub(crate) async fn listen(&self) -> Result<(), io::Error> {
        if let Ok(bus_frame) = self.0.adaptor.recv().await {
            let frame = Frame::try_from(bus_frame)?;
            let mtu = self.0.adaptor.mtu(frame.upper_meta().flag) as u16;
            let application_id = frame.application();
            if let Some(Some(application)) = self.0.applications.get(application_id as usize) {
                let response = application.handle(&frame, mtu)?;
                if let Some(mut response) = response {
                    response.insert_header();
                    self.0.adaptor.send(response.into()).await.unwrap();
                }
            }
        }
        Ok(())
    }

    pub(crate) fn register(
        &mut self,
        application_id: u8,
        application: Arc<(dyn Application + 'static)>,
    ) {
        self.0.applications[application_id as usize] = Some(application);
    }
}

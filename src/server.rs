use std::mem::size_of;
use std::{io, sync::Arc};

use crate::adaptor::{Channel, DeviceAdaptor, TyCanProtocol, TyUartProtocol};

const MAX_APPLICATION_HANDLER: usize = 256;
pub(crate) struct TcspServer<D>(Box<TcspInner<D>>);

use crate::application::Application;
use crate::protocol::v1::frame::FrameHeader;
use crate::protocol::Frame;

struct TcspInner<D> {
    adaptor: D,
    applications: [Option<Arc<dyn Application>>; MAX_APPLICATION_HANDLER],
}

unsafe impl<D> Send for TcspInner<D> {}

impl TcspServer<TyCanProtocol> {
    pub(crate) fn new_can(adaptor: TyCanProtocol) -> Self {
        let applications = core::array::from_fn(|_| None);
        TcspServer(Box::new(TcspInner {
            adaptor,
            applications,
        }))
    }
}

impl TcspServer<TyUartProtocol> {
    pub(crate) fn new_uart(adaptor: TyUartProtocol) -> Self {
        let applications = core::array::from_fn(|_| None);
        TcspServer(Box::new(TcspInner {
            adaptor,
            applications,
        }))
    }
}

impl TcspServer<Channel> {
    pub(crate) fn new_channel(adaptor: Channel) -> Self {
        let applications = core::array::from_fn(|_| None);
        TcspServer(Box::new(TcspInner {
            adaptor,
            applications,
        }))
    }
}

impl<D: DeviceAdaptor> TcspServer<D> {
    pub(crate) async fn listen(&self) {
        loop {
            if let Err(e) = self.handle().await {
                log::error!("Error occurs:{:?}", e);
            }
        }
    }

    async fn handle(&self) -> Result<(), io::Error> {
        if let Ok(bus_frame) = self.0.adaptor.recv().await {
            let frame = Frame::try_from(bus_frame)?;
            log::info!("receive frame from bus:{:?}", frame);
            let mtu = (self.0.adaptor.mtu(frame.meta().flag) - size_of::<FrameHeader>()) as u16;
            let application_id = frame.application();
            if let Some(Some(application)) = self.0.applications.get(application_id as usize) {
                let response = application.handle(frame, mtu)?;
                log::info!("response:{:?}", response);
                if let Some(response) = response {
                    self.0.adaptor.send(response.try_into()?).await.unwrap();
                }
            }
        }
        Ok(())
    }

    pub(crate) fn register(&mut self, application: Arc<(dyn Application + 'static)>) {
        let id = application.application_id();
        self.0.applications[id as usize] = Some(application);
    }
}

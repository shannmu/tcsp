use std::mem::size_of;
use std::{io, sync::Arc};

use crate::adaptor::{Channel, DeviceAdaptor, TyCanProtocol, TyUartProtocol};

const MAX_APPLICATION_HANDLER: usize = 256;
pub(crate) struct TcspServer<D>(Arc<TcspInner<D>>);

use crate::application::Application;
use crate::protocol::v1::frame::FrameHeader;
use crate::protocol::Frame;

struct TcspInner<D> {
    adaptor: D,
    applications: [Option<Arc<dyn Application>>; MAX_APPLICATION_HANDLER],
}

macro_rules! create_server_and_application_table {
    ($adaptor:ident,$applications:ident) => {
        {
            let mut application_table = core::array::from_fn(|_| None);
            for application in $applications {
                let id : usize = application.application_id().into();
                application_table[id] = Some(application);
            }
            TcspServer(Arc::new(TcspInner {
                adaptor : $adaptor,
                applications: application_table,
            }))
        }
    };
}

impl TcspServer<TyCanProtocol> {
    pub(crate) fn new_can(
        adaptor: TyCanProtocol,
        applications: impl Iterator<Item = Arc<dyn Application>>,
    ) -> Self {
        create_server_and_application_table!(adaptor,applications)
    }
}

impl TcspServer<TyUartProtocol> {
    pub(crate) fn new_uart(
        adaptor: TyUartProtocol,
        applications: impl Iterator<Item = Arc<dyn Application>>,
    ) -> Self {
        create_server_and_application_table!(adaptor,applications)
    }
}

impl TcspServer<Channel> {
    pub(crate) fn new_channel(
        adaptor: Channel,
        applications: impl Iterator<Item = Arc<dyn Application>>,
    ) -> Self {
        create_server_and_application_table!(adaptor,applications)
    }
}

impl<D: DeviceAdaptor + 'static> TcspServer<D> {
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
            let server = self.0.clone();
            tokio::spawn(async move {
                let mtu = (server.adaptor.mtu(frame.meta().flag) - size_of::<FrameHeader>()) as u16;
                let application_id = frame.application();

                if let Some(Some(application)) = server.applications.get(application_id as usize) {
                    let application = application.clone();
                    let response = application.handle(frame, mtu).unwrap();
                    log::info!("response:{:?}", response);
                    if let Some(response) = response {
                        server
                            .adaptor
                            .send(response.try_into().unwrap())
                            .await
                            .unwrap();
                    }
                }
            });
        }
        Ok(())
    }
}

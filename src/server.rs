use std::mem::size_of;
use std::{io, sync::Arc};

use crate::adaptor::{Channel, DeviceAdaptor, TyCanProtocol, TyUartProtocol, Uart};

const MAX_APPLICATION_HANDLER: usize = 256;
pub struct TcspServer<D>(Arc<TcspInner<D>>);

use crate::application::Application;
use crate::protocol::v1::frame::FrameHeader;
use crate::protocol::Frame;

struct TcspInner<D> {
    adaptor: D,
    applications: [Option<Arc<dyn Application>>; MAX_APPLICATION_HANDLER],
}

macro_rules! create_server_and_application_table {
    ($adaptor:ident,$applications:ident) => {{
        let mut application_table = core::array::from_fn(|_| None);
        for application in $applications {
            let id: usize = application.application_id().into();
            application_table[id] = Some(application);
        }
        TcspServer(Arc::new(TcspInner {
            adaptor: $adaptor,
            applications: application_table,
        }))
    }};
}

impl TcspServer<TyCanProtocol> {
    pub(crate) fn new_can(
        adaptor: TyCanProtocol,
        applications: impl Iterator<Item = Arc<dyn Application>>,
    ) -> Self {
        create_server_and_application_table!(adaptor, applications)
    }
}

impl TcspServer<Uart> {
    pub(crate) fn new_uart(
        adaptor: Uart,
        applications: impl Iterator<Item = Arc<dyn Application>>,
    ) -> Self {
        create_server_and_application_table!(adaptor, applications)
    }
}

impl TcspServer<Channel> {
    pub(crate) fn new_channel(
        adaptor: Channel,
        applications: impl Iterator<Item = Arc<dyn Application>>,
    ) -> Self {
        create_server_and_application_table!(adaptor, applications)
    }
}

impl<D: DeviceAdaptor + 'static> TcspServer<D> {
    pub async fn listen(&self) {
        log::info!("server start");
        loop {
            if let Err(e) = self.handle().await {
                log::error!("Error occurs:{:?}", e);
            }
        }
    }

    async fn handle(&self) -> Result<(), io::Error> {
        if let Ok(bus_frame) = self.0.adaptor.recv().await {
            let frame = Frame::try_from(bus_frame)?;
            log::info!("receive application={}", frame.application());
            let server = Arc::<TcspInner<D>>::clone(&self.0);
            // TODO: we can spawn here. but we need to add flow control for sending, otherwise it may cause bus error
            // tokio::spawn(async move {
            let mtu = (server.adaptor.mtu(frame.meta().flag) - size_of::<FrameHeader>()) as u16;
            let application_id = frame.application();

            if let Some(Some(application)) = server.applications.get(application_id as usize) {
                let response_result = application.handle(frame, mtu);
                let response = match response_result {
                    Ok(response) => response,
                    Err(e) => {
                        log::error!("faild to handle application:{}", e);
                        return Ok(());
                    }
                };
                log::debug!("response:{:?}", response);
                if let Some(response) = response {
                    #[allow(clippy::unwrap_used)]
                    if let Err(e) = server.adaptor.send(response.try_into().unwrap()).await {
                        log::error!("faild to send application response:{}", e);
                    }
                }
            }
            // });
        }
        Ok(())
    }
}

pub struct TcspServerBuilder<A> {
    adaptor: A,
    applications: Vec<Arc<dyn Application>>,
}

impl TcspServerBuilder<TyCanProtocol> {
    pub fn new_can(adaptor: TyCanProtocol) -> Self {
        Self {
            adaptor,
            applications: Vec::new(),
        }
    }

    pub fn build(self) -> TcspServer<TyCanProtocol> {
        TcspServer::new_can(self.adaptor, self.applications.into_iter())
    }
}

impl TcspServerBuilder<Uart> {
    pub fn new_uart(adaptor: Uart) -> Self {
        Self {
            adaptor,
            applications: Vec::new(),
        }
    }

    pub fn build(self) -> TcspServer<Uart> {
        TcspServer::new_uart(self.adaptor, self.applications.into_iter())
    }
}

impl TcspServerBuilder<Channel> {
    pub fn new_channel(adaptor: Channel) -> Self {
        Self {
            adaptor,
            applications: Vec::new(),
        }
    }

    pub fn build(self) -> TcspServer<Channel> {
        TcspServer::new_channel(self.adaptor, self.applications.into_iter())
    }
}

impl<A: DeviceAdaptor> TcspServerBuilder<A> {
    pub fn with_application(mut self, application: Arc<dyn Application>) -> Self {
        self.applications.push(application);
        self
    }
}

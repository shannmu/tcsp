use std::{io, sync::Arc};

use frame::Frame;

use crate::adaptor::{DeviceAdaptor, Frame as BusFrame};

const MAX_APPLICATION_HANDLER: usize = 256;

mod frame;
struct Tcsp<D>(Box<TcspInner<D>>);
struct TcspInner<D> {
    adaptor: D,
    applications: [Option<Arc<dyn Application>>; MAX_APPLICATION_HANDLER],
}

trait Application {
    fn handle(&self, frame: &Frame) -> std::io::Result<Option<Vec<Frame>>>;
    // fn post_handle(&self,frame : &Frame) -> std::io::Result<()>;
}

impl<D: DeviceAdaptor> Tcsp<D> {
    pub(crate) async fn listen(&self) -> Result<(), io::Error> {
        if let Ok(bus_frame) = self.0.adaptor.recv().await {
            let frame = Frame::try_from(bus_frame)?;
            let application_id = frame.application();
            if let Some(Some(application)) = self.0.applications.get(application_id as usize) {
                let response = application.handle(&frame)?;
                if let Some(response) = response {
                    // for resp in response{
                    //     self.0.adaptor.send(resp).await.unwrap();
                    // }
                }
            }
        }
        Ok(())
    }
}

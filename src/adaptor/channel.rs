use std::sync::{mpsc::{Receiver, Sender}, Arc};

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::{DeviceAdaptor, DeviceAdaptorError, Frame, FrameFlag};


struct Channel(Arc<ChannelInner>);

struct ChannelInner{
    tx : Sender<Frame>,
    rx : Mutex<Receiver<Frame>>
}

impl Channel {
    pub fn new(tx: Sender<Frame>,rx:Receiver<Frame>) -> Self{
        Self(Arc::new(ChannelInner{
            tx,
            rx : Mutex::new(rx)
        }))
    }
}

#[async_trait]
impl DeviceAdaptor for Channel{
    async fn send(&self, frame: Frame) -> Result<(), DeviceAdaptorError>{
        self.0.tx.send(frame).map_err(|e|DeviceAdaptorError::BusError(Box::new(e)))
    }
    
    async fn recv(&self) -> Result<Frame, DeviceAdaptorError>{
        let lock = self.0.rx.lock().await;
        let frame = lock.recv().map_err(|e|DeviceAdaptorError::BusError(Box::new(e)))?;
        Ok(frame)
    }

    fn mtu(&self, _flag: FrameFlag) -> usize{
        150
    }


}
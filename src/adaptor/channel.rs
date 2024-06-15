use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};

use super::{DeviceAdaptor, DeviceAdaptorError, Frame, FrameFlag};

pub struct Channel(Arc<ChannelInner>);

struct ChannelInner {
    tx: Sender<Frame>,
    rx: Mutex<Receiver<Frame>>,
}

impl Channel {
    pub fn new(tx: Sender<Frame>, rx: Receiver<Frame>) -> Self {
        Self(Arc::new(ChannelInner {
            tx,
            rx: Mutex::new(rx),
        }))
    }
}

#[async_trait]
impl DeviceAdaptor for Channel {
    async fn send(&self, frame: Frame) -> Result<(), DeviceAdaptorError> {
        self.0
            .tx
            .send(frame)
            .await
            .map_err(|e| DeviceAdaptorError::BusError(Box::new(e)))
    }

    async fn recv(&self) -> Result<Frame, DeviceAdaptorError> {
        let mut lock = self.0.rx.lock().await;
        lock.recv().await.ok_or(DeviceAdaptorError::Empty)
    }

    fn mtu(&self, _flag: FrameFlag) -> usize {
        150
    }
}

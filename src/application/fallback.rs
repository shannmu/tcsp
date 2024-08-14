use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use zeromq::{Socket, SocketRecv, SocketSend, ZmqResult};

#[async_trait]
pub trait Fallback: Send + Sync {
    async fn fallback(&self, msg: Vec<u8>) -> std::io::Result<Vec<u8>>;
}

#[derive(Clone)]
pub struct ZeromqSocket {
    socket: Arc<Mutex<zeromq::ReqSocket>>,
}

#[async_trait]
impl Fallback for ZeromqSocket {
    #[allow(clippy::unwrap_used)]
    async fn fallback(&self, msg: Vec<u8>) -> std::io::Result<Vec<u8>> {
        let mut guard = self.socket.lock().await;
        guard.send(msg.into()).await.unwrap();

        let recv_msg = guard.recv().await.unwrap();
        if let Some(bytes) = recv_msg.get(0) {
            Ok(bytes.to_vec())
        } else {
            Ok(Vec::new())
        }
    }
}

impl ZeromqSocket {
    pub fn new() -> Self {
        let socket = Arc::new(Mutex::new(zeromq::ReqSocket::new()));
        Self { socket }
    }

    pub async fn connect(&self, endpoint: &str) -> ZmqResult<()> {
        self.socket.lock().await.connect(endpoint).await?;
        Ok(())
    }
}

impl Default for ZeromqSocket {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct DummyFallback;

#[async_trait]
impl Fallback for DummyFallback {
    async fn fallback(&self, msg: Vec<u8>) -> std::io::Result<Vec<u8>> {
        Ok(msg)
    }
}

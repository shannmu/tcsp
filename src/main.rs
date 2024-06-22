use std::{env, sync::Arc};

use adaptor::{Channel, DeviceAdaptorError, FrameMeta};
use application::{EchoCommand, TeleMetry};
use server::TcspServer;

use tokio::{self, sync::mpsc::channel};

pub(crate) mod adaptor;
use adaptor::{DeviceAdaptor, Frame, TyCanProtocol};
mod application;
mod protocol;
mod server;
#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
    env_logger::init();
    // let (tx_sender, tx_receiver) = channel(32);
    // let (rx_sender, rx_receiver) = channel(32);
    // let adaptor = Channel::new(tx_sender, rx_receiver);
    // let mut server = TcspServer::new_channel(adaptor);
    // let tel = TeleMetry {};
    // let echo = EchoCommand {};
    // server.register(Arc::new(tel));
    // server.register(Arc::new(echo));
    // server.listen().await;
    // Ok(())
    let socket = TyCanProtocol::new(0x42, "can0", "can0").unwrap();

    // test recv one packet

    // test recv multiple pakcet

    // test send one packet
    let mut meta = FrameMeta::default();
    meta.len = 6;
    let mut frame = Frame::new(meta, &[1, 2, 3, 4, 5, 6]);
    socket.send(frame).await.unwrap();

    // print
    // test send multiple packet
    let mut frame = Frame::default();
    frame.meta.dest_id = 0;
    frame.meta.len = 12;
    frame
        .data_mut()
        .copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    socket.send(frame).await.unwrap();
}

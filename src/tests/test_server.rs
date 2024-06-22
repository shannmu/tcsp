use std::{ mem::size_of, sync::Arc};

use tokio::{self, sync::mpsc::channel};

use crate::{
    adaptor::{Channel, DeviceAdaptor, Frame, FrameFlag, FrameMeta},
    application::{EchoCommand, TeleMetry, TimeSync},
    protocol::v1::frame::{FrameHeader, VERSION_ID},
    server::TcspServer,
};

#[tokio::test]
async fn test_server_channel() {
    let (tx_sender, mut tx_receiver) = channel(32);
    let (rx_sender, rx_receiver) = channel(32);
    let adaptor = Channel::new(tx_sender, rx_receiver);
    let mtu = adaptor.mtu(FrameFlag::empty());
    let mut server = TcspServer::new_channel(adaptor);
    let tel = TeleMetry {};
    let echo = EchoCommand {};
    let time = TimeSync {};

    server.register(Arc::new(tel));
    server.register(Arc::new(echo));
    server.register(Arc::new(time));

    tokio::spawn(async move {
        server.listen().await;
    });

    // suppose we receive a telemetry request
    let meta = FrameMeta::default();
    let frame = Frame::new(meta, &[VERSION_ID, 0x00]);
    rx_sender.send(frame).await.unwrap();

    // we expect to receive a response
    let resp = tx_receiver.recv().await.unwrap();
    assert_eq!(resp.meta.len as usize, mtu);

    // suppose we receive a echo request
    let packet = [VERSION_ID, 0x02]
        .into_iter()
        .chain(1..=42)
        .collect::<Vec<u8>>();
    let frame = Frame::new(FrameMeta::default(), &packet);
    rx_sender.send(frame).await.unwrap();
    // we expect to receive a response same as request
    let resp = tx_receiver.recv().await.unwrap();
    let buf = &resp.data()[size_of::<FrameHeader>()..size_of::<FrameHeader>()+42];
    assert_eq!(buf,(1..=42).collect::<Vec<u8>>());

    // suppose we recevie a time broadcast request
    let packet = [VERSION_ID, 0x01].into_iter()
        .chain(1719073956u32.to_be_bytes().into_iter())
        .collect::<Vec<u8>>();
    let frame = Frame::new(FrameMeta::default(), &packet);
    rx_sender.send(frame).await.unwrap();

}

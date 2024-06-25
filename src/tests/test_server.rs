use std::{mem::size_of, sync::Arc};

use tokio::{self, sync::mpsc::channel};

use crate::{
    adaptor::{Channel, DeviceAdaptor, Frame as BusFrame, FrameFlag, FrameMeta},
    application::{Application, EchoCommand, TeleMetry, TimeSync},
    protocol::v1::frame::{FrameHeader, VERSION_ID,Frame},
    server::TcspServer,
};

#[tokio::test]
async fn test_server_channel() {
    let (tx_sender, mut tx_receiver) = channel(32);
    let (rx_sender, rx_receiver) = channel(32);
    let adaptor = Channel::new(tx_sender, rx_receiver);
    let mtu = adaptor.mtu(FrameFlag::empty());
    let tel: Arc<dyn Application> = Arc::new(TeleMetry {});
    let echo: Arc<dyn Application> = Arc::new(EchoCommand {});
    let time: Arc<dyn Application> = Arc::new(TimeSync {});
    let applications = [tel, echo, time].into_iter();
    let server = TcspServer::new_channel(adaptor,applications);
    tokio::spawn(async move {
        server.listen().await;
    });

    // suppose we receive a telemetry request
    let telemetry_req = TeleMetry::request().unwrap();
    rx_sender.send(telemetry_req.try_into().unwrap()).await.unwrap();

    // we expect to receive a response
    let resp = tx_receiver.recv().await.unwrap();
    assert_eq!(resp.meta.len as usize, mtu);

    // suppose we receive a echo request
    let content = (1..=42)
        .collect::<Vec<u8>>();
    let echo = EchoCommand {};
    let echo_req = echo.request(150, &content).unwrap();
    rx_sender.send(echo_req.try_into().unwrap()).await.unwrap();
    // we expect to receive a response same as request
    let resp: Frame = tx_receiver.recv().await.unwrap().try_into().unwrap();
    assert_eq!(resp.application(), EchoCommand::APPLICATION_ID);
    let buf = &resp.data()[..42];
    assert_eq!(buf, (1..=42).collect::<Vec<u8>>());

    // suppose we recevie a time broadcast request
    let time_req = TimeSync::request_now().unwrap();
    rx_sender.send(time_req.try_into().unwrap()).await.unwrap();
}

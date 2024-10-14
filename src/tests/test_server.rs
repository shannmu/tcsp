use std::sync::Arc;

use tokio::{
    self,
    sync::{mpsc::channel, Mutex},
};

use crate::{
    adaptor::{send_using_ty_protocol, Channel},
    application::{Application, DummyFallback, EchoCommand, TeleMetry, TimeSync},
    protocol::v1::frame::Frame,
    server::TcspServer,
    UdpBackup,
};

#[tokio::test]
async fn test_server_channel() {
    let (tx_sender, mut tx_receiver) = channel(32);
    let (rx_sender, rx_receiver) = channel(32);
    let adaptor = Channel::new(tx_sender, rx_receiver);
    let socket = DummyFallback {};
    let tel: Arc<dyn Application> = Arc::new(TeleMetry::new(socket.clone()));
    let echo: Arc<dyn Application> = Arc::new(EchoCommand {});
    let time: Arc<dyn Application> = Arc::new(TimeSync::new(socket));
    let applications = [tel, echo, time].into_iter();
    let server = TcspServer::new_channel(adaptor, applications);
    tokio::spawn(async move {
        server.listen().await;
    });

    // suppose we receive a telemetry request
    let telemetry_req = TeleMetry::<()>::request(0, 0).unwrap();
    rx_sender
        .send(telemetry_req.try_into().unwrap())
        .await
        .unwrap();

    // we expect to receive a response
    let resp = tx_receiver.recv().await.unwrap();
    assert_eq!(resp.meta.len as usize, 102);

    // suppose we receive a echo request
    let content = (1..=42).collect::<Vec<u8>>();
    let echo = EchoCommand {};
    let echo_req = echo.request(150, &content).unwrap();
    rx_sender.send(echo_req.try_into().unwrap()).await.unwrap();
    // we expect to receive a response same as request
    let resp: Frame = tx_receiver.recv().await.unwrap().try_into().unwrap();
    assert_eq!(resp.application(), EchoCommand::APPLICATION_ID);
    let buf = &resp.data()[..42];
    assert_eq!(buf, (1..=42).collect::<Vec<u8>>());

    // suppose we recevie a time broadcast request
    let time_req = TimeSync::<()>::request_now().unwrap();
    rx_sender.send(time_req.try_into().unwrap()).await.unwrap();
}

#[tokio::test]
#[ignore]
#[allow(unused)]
/// A helper for testing can udp backup.
async fn generate_can_udp_request_example() {
    let can_frames = Mutex::new(Vec::new());
    let result = UdpBackup::<DummyFallback>::generate_request(vec![1, 2, 3, 4, 5], 0x43).unwrap();
    for frame in result.into_iter() {
        send_using_ty_protocol(&can_frames, 0, 0, frame.try_into().unwrap())
            .await
            .unwrap();
    }
    println!("{:?}",can_frames);
}

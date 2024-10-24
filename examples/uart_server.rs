use std::{sync::Arc, time::Duration};

use tcsp::{
    DownloadCommand, EchoCommand, Reboot, ResetNetwork, TcspServerBuilder, TeleMetry, TimeSync,
    Uart, UdpBackup, UploadCommand, ZeromqSocket,
};

mod common;
use common::init_logger;
use tokio::time::timeout;

#[tokio::main]
async fn main() {
    init_logger(log::Level::Debug).unwrap();

    let socket = ZeromqSocket::new();
    timeout(
        Duration::from_secs(2),
        socket.connect("tcp://127.0.0.1:5555"),
    )
    .await
    .expect("Connection timeout")
    .expect("Failed to connect");
    #[allow(clippy::unwrap_used)]
    let adaptor = Uart::new("/dev/ttyAMA1", 115200, 0x84).await;
    let server = TcspServerBuilder::new_uart(adaptor)
        .with_application(Arc::new(TeleMetry::new(socket.clone())))
        .with_application(Arc::new(EchoCommand {}))
        .with_application(Arc::new(TimeSync::new(socket.clone())))
        .with_application(Arc::new(Reboot {}))
        .with_application(Arc::new(UdpBackup::new(socket.clone())))
        .with_application(Arc::new(ResetNetwork {}))
        .with_application(Arc::new(UploadCommand::new(socket.clone())))
        .with_application(Arc::new(DownloadCommand::new(socket.clone())))
        .build();
    server.listen().await;
}

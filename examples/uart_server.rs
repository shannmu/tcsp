use std::sync::Arc;

use tcsp::{EchoCommand, Reboot, TcspServerBuilder, TeleMetry, TimeSync, Uart, ZeromqSocket};

mod common;
use common::init_logger;

#[tokio::main]
async fn main() {
    init_logger(log::Level::Debug).unwrap();

    let socket = ZeromqSocket::new();
    socket
        .connect("tcp://127.0.0.1:5555")
        .await
        .expect("Failed to connect");
    #[allow(clippy::unwrap_used)]
    let adaptor = Uart::new("/dev/ttyAMA1", 115200).await;
    let server = TcspServerBuilder::new_uart(adaptor)
        .with_application(Arc::new(TeleMetry::new(socket)))
        .with_application(Arc::new(EchoCommand {}))
        .with_application(Arc::new(TimeSync {}))
        .with_application(Arc::new(Reboot {}))
        .build();
    server.listen().await;
}

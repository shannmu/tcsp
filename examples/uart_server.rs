use std::{sync::Arc, time::Duration};

use tcsp::{
    DownloadCommand, EchoCommand, Reboot, ResetNetwork, TcspServerBuilder, TeleMetry, TimeSync,
    Uart, UdpBackup, UploadCommand, ZeromqSocket,
};

mod common;
use clap::Parser;
use common::init_logger;
use tokio::time::timeout;

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Args {
    #[arg(long, required = true, default_value = "/dev/ttyAMA1")]
    device_name: String,
    #[arg(long, required = true, default_value = "0x85")]
    device_id: u8,
}

#[tokio::main]
async fn main() {
    init_logger(log::Level::Debug).unwrap();

    let args = Args::parse();
    let device_name = args.device_name;
    let device_id = args.device_id;
    log::debug!(
        "device name = {}, device id = 0x{:x}",
        device_name,
        device_id
    );

    let socket = ZeromqSocket::new();
    timeout(
        Duration::from_secs(2),
        socket.connect("tcp://127.0.0.1:5555"),
    )
    .await
    .expect("Connection timeout")
    .expect("Failed to connect");
    #[allow(clippy::unwrap_used)]
    let adaptor = Uart::new(device_name.as_str(), 115200, device_id).await;
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

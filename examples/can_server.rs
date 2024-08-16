use std::{num::ParseIntError, sync::Arc, time::Duration};

use clap::Parser;
use tcsp::{
    EchoCommand, Reboot, TcspServerBuilder, TeleMetry, TimeSync, TyCanProtocol, UdpControl, ZeromqSocket
};

fn parse_number(s: &str) -> Result<u8, ParseIntError> {
    if let Some(stripped) = s.strip_prefix("0x") {
        u8::from_str_radix(stripped, 16)
    } else {
        s.parse::<u8>()
    }
}

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Args {
    #[arg(required = true,value_parser=parse_number)]
    can_id: u8,
}

mod common;
use common::init_logger;
use tokio::time::timeout;

#[tokio::main]
async fn main() {
    init_logger(log::Level::Debug).unwrap();
    let args = Args::parse();
    let canid = args.can_id;
    log::debug!("can id = 0x{:x}", canid);

    let socket = ZeromqSocket::new();
    timeout(
        Duration::from_secs(2),
        socket.connect("tcp://127.0.0.1:5555"),
    )
    .await
    .expect("Connection timeout")
    .expect("Failed to connect");

    #[allow(clippy::unwrap_used)]
    let adaptor = TyCanProtocol::new(canid, "can0", "can0").await.unwrap();
    let server = TcspServerBuilder::new_can(adaptor)
        .with_application(Arc::new(TeleMetry::new(socket.clone())))
        .with_application(Arc::new(EchoCommand {}))
        .with_application(Arc::new(TimeSync::new(socket.clone())))
        .with_application(Arc::new(Reboot {}))
        .with_application(Arc::new(UdpControl::new(socket)))
        .build();
    server.listen().await;
}

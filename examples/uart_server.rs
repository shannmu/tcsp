use std::sync::Arc;

use tcsp::{EchoCommand, Reboot, TcspServerBuilder, TeleMetry, TimeSync, Uart};

mod common;
use common::init_logger;

#[tokio::main]
async fn main() {
    init_logger(log::Level::Debug).unwrap();

    #[allow(clippy::unwrap_used)]
    let adaptor = Uart::new("/dev/ttyAMA1", 115200).await;
    let server = TcspServerBuilder::new_uart(adaptor)
        .with_application(Arc::new(TeleMetry {}))
        .with_application(Arc::new(EchoCommand {}))
        .with_application(Arc::new(TimeSync {}))
        .with_application(Arc::new(Reboot {}))
        .build();
    server.listen().await;
}

use std::sync::Arc;

use tcsp::{EchoCommand, Reboot, TcspServerBuilder, TeleMetry, TimeSync, TyCanProtocol};

#[tokio::main]
async fn main() {
    env_logger::init();
    #[allow(clippy::unwrap_used)]
    let adaptor = TyCanProtocol::new(0x43, "can0", "can0").unwrap();
    let server = TcspServerBuilder::new_can(adaptor)
        .with_application(Arc::new(TeleMetry {}))
        .with_application(Arc::new(EchoCommand {}))
        .with_application(Arc::new(TimeSync {}))
        .with_application(Arc::new(Reboot {}))
        .build();
    server.listen().await;
}

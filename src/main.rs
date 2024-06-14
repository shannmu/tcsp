use std::{env, sync::Arc};

use adaptor::{can::ty::TyCanProtocol};
use application::{EchoCommand, TeleMetry};
use protocol::Tcsp;
use socketcan::{Result};
use tokio;

pub(crate)mod adaptor;
mod protocol;
mod application;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let adaptor = TyCanProtocol::new(0x43, "can0", "can0").unwrap();
    let mut server = Tcsp::new_can(adaptor);
    let tel = TeleMetry{};
    let echo = EchoCommand{};
    server.register(Arc::new(tel));
    server.register(Arc::new(echo));
    server.listen().await;
    Ok(())
}
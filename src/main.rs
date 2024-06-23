use std::sync::Arc;

use application::{Application, EchoCommand, TeleMetry, TimeSync};
use server::TcspServer;

use tokio::{self};

pub(crate) mod adaptor;
use adaptor::TyCanProtocol;
mod application;
mod protocol;
mod server;
#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
    env_logger::init();
    // let (tx_sender, tx_receiver) = channel(32);
    // let (rx_sender, rx_receiver) = channel(32);
    // let adaptor = Channel::new(tx_sender, rx_receiver);
    // let mut server = TcspServer::new_channel(adaptor);
    // let tel = TeleMetry {};
    // let echo = EchoCommand {};
    // server.register(Arc::new(tel));
    // server.register(Arc::new(echo));
    // server.listen().await;
    // Ok(())
    let adaptor = TyCanProtocol::new(0x42, "can0", "can0").unwrap();
    let tel: Arc<dyn Application> = Arc::new(TeleMetry {});
    let echo: Arc<dyn Application> = Arc::new(EchoCommand {});
    let time: Arc<dyn Application> = Arc::new(TimeSync {});
    let applications = [tel, echo, time].into_iter();
    let server = TcspServer::new_can(adaptor,applications);
    server.listen().await;
}

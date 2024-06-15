use std::{env, sync::Arc};

use adaptor::Channel;
use application::{EchoCommand, TeleMetry};
use server::TcspServer;
use socketcan::Result;
use tokio::{
    self,
    sync::mpsc::channel,
};

pub(crate) mod adaptor;
mod application;
mod protocol;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let (tx_sender, tx_receiver) = channel(32);
    let (rx_sender, rx_receiver) = channel(32);
    let adaptor = Channel::new(tx_sender, rx_receiver);
    let mut server = TcspServer::new_channel(adaptor);
    let tel = TeleMetry {};
    let echo = EchoCommand {};
    server.register(Arc::new(tel));
    server.register(Arc::new(echo));
    server.listen().await;
    Ok(())
}

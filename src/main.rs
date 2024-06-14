use std::env;

use adaptor::{can::ty::TyCanProtocol};
use protocol::Tcsp;
use socketcan::{tokio::CanSocket, CanDataFrame, CanFrame, EmbeddedFrame, Id, Result, StandardId};
use tokio;

pub(crate)mod adaptor;
mod protocol;
mod application;

#[tokio::main]
async fn main() -> Result<()> {
    let adaptor = TyCanProtocol::new(0x43, "can0", "can0").unwrap();
    let server = Tcsp::new_can(adaptor);
    // server.register(application_id, application)
    Ok(())
}
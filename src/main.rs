use std::env;

use adaptor::{can::ty::TyCanProtocol};
use socketcan::{tokio::CanSocket, CanDataFrame, CanFrame, EmbeddedFrame, Id, Result, StandardId};
use tokio;

mod adaptor;

#[tokio::main]
async fn main() -> Result<()> {
    let mut can = Box::new(TyCanProtocol::new(0x43, "can0", "can0").unwrap());
    loop {
        if let Ok(frame) = can.recv().await {
            println!("{:?}", frame);
        }
    }
    Ok(())
}
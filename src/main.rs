use std::env;

use socketcan::{tokio::CanSocket, CanDataFrame, CanFrame, EmbeddedFrame, Id, Result, StandardId};
use tokio;

mod adaptor;

#[tokio::main]
async fn main() -> Result<()> {
    let name = env::args().nth(1).unwrap();
    println!("send to {:?}",name);
    let sock_tx = CanSocket::open(&name)?;
    let buf = [0,1,2,3,4,5];
    let frame = CanFrame::Data(CanDataFrame::new(Id::Standard(StandardId::new(0).unwrap()), &buf).unwrap());
    sock_tx.write_frame(frame)?.await?;

    Ok(())
}

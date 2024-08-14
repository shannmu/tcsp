use std::convert::TryInto;
use zeromq::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Start server");
    let mut socket = zeromq::RepSocket::new();
    socket.bind("tcp://127.0.0.1:5555").await?;

    loop {
        let repl: String = socket.recv().await?.try_into()?;
        dbg!(&repl);
        let data = (0..100).collect::<Vec<u8>>();
        socket.send(data.into()).await?;
    }
}
use std::{error::Error, sync::Arc, time::Duration};
use tokio::{spawn, sync::Mutex, time::sleep};
use zeromq::{Socket, SocketRecv, SocketSend};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let socket = Arc::new(Mutex::new(zeromq::ReqSocket::new()));
    socket.lock().await
        .connect("tcp://127.0.0.1:5555")
        .await
        .expect("Failed to connect");
    for _ in 0..5u64 {
        let c_socket = socket.clone();
        spawn(async move{
            let mut g = c_socket.lock().await;
            g.send("60000".into()).await.unwrap();
            let repl = g.recv().await.unwrap();
            let v = repl.into_vec();
            println!("{:?}",v);
        });
    }
    sleep(Duration::from_secs(1)).await;
    Ok(())
}
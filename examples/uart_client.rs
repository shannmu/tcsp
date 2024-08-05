use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::sleep;

mod common;
use common::init_logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger("uart_client.log", log::Level::Debug).unwrap();

    // Init a serial port
    let port = Mutex::new(
        serialport::new("/dev/ttyAMA1", 115200)
            .timeout(std::time::Duration::from_secs(5))
            .open()
            .unwrap(),
    );

    loop {
        // Send a frame to the server
        port.lock().await.write_all(&[
            0xeb, 0x90, 0x10, 0x00, 0x05, 0x35, 0x10, 0x01, 0x20, 0x00, 0x04,
        ])?;
        sleep(Duration::from_secs(1)).await;
        // recv data from the server
        let mut buf = [0u8; 150];
        let n = port.lock().await.read(&mut buf);
        match n {
            Ok(n) => {
                log::info!("recv data: {:?}", &buf[..n]);
            }
            Err(e) => {
                log::error!("read data error: {:?}", e);
            }
        }
    }
}

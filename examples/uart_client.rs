use serialport;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

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
            0xeb, 0x90, 0x01, 0x00, 0x06, 0x35, 0x00, 0x01, 0x01, 0x02, 0x03, 0x00,
        ])?;

        // recv data from the server
        let mut buf = [0u8; 150];
        let n = port.lock().await.read(&mut buf)?;
        log::info!("recv data: {:?}", &buf[..n]);
    }

    Ok(())
}

use std::time::Duration;

use clap::Parser;
use tokio::sync::Mutex;
use tokio::time::sleep;

mod common;
use common::init_logger;

#[derive(Debug, Parser)]
struct Cli {
    #[arg(required = true)]
    device_name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger(log::Level::Debug).unwrap();
    // Get the device name from argv
    let cli = Cli::parse();
    let device_name = cli.device_name;

    // Init a serial port
    let port = Mutex::new(
        serialport::new(device_name.as_str(), 115200)
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
        loop {
            let n = port.lock().await.read(&mut buf);
            let mut takes_frame = false;
            match n {
                Ok(n) => {
                    if buf[0] == 0xeb && buf[1] == 0x90 {
                        log::info!(
                            "recv frame: {:?} - time: {:?}",
                            &buf[..n],
                            chrono::Local::now()
                        );
                        takes_frame = true;
                    } else {
                        log::warn!(
                            "recv invalid frame: {:?} - time: {:?}",
                            &buf[..n],
                            chrono::Local::now()
                        );
                    }
                }
                Err(e) => {
                    log::error!(
                        "read data error: {:?} - time: {:?}",
                        e,
                        chrono::Local::now()
                    );
                }
            }

            sleep(Duration::from_secs(5)).await;
            if takes_frame {
                break;
            }
        }
    }
}

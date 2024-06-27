use std::{env, num::ParseIntError, sync::Arc};

use tcsp::{EchoCommand, Reboot, TcspServerBuilder, TeleMetry, TimeSync, TyCanProtocol};

fn parse_number(s: &str) -> Result<u8, ParseIntError> {
    if let Some(stripped) = s.strip_prefix("0x") {
        u8::from_str_radix(stripped, 16)
    } else {
        s.parse::<u8>()
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <Can ID>", args[0]);
        return;
    }
    let canid = parse_number(&args[1]).unwrap();
    log::debug!("can id = {}",canid);
    #[allow(clippy::unwrap_used)]
    let adaptor = TyCanProtocol::new(canid, "can0", "can0").await.unwrap();
    let server = TcspServerBuilder::new_can(adaptor)
        .with_application(Arc::new(TeleMetry {}))
        .with_application(Arc::new(EchoCommand {}))
        .with_application(Arc::new(TimeSync {}))
        .with_application(Arc::new(Reboot {}))
        .build();
    server.listen().await;
}

use std::{env, num::ParseIntError, sync::Arc, time::Duration};
use tcsp::{adaptor::DeviceAdaptor, TeleMetry};
use clap::Parser;
use tcsp::TyCanProtocol;
use tokio::time::sleep;


fn larger_than_zero(p: &str) -> Result<u32, String> {
    let port = p.parse::<u32>().map_err(|_| "Invalid number")?;
    if port == 0 {
        Err("Invalid number".to_owned())
    } else {
        Ok(port)
    }
}

fn parse_number(s: &str) -> Result<u8, ParseIntError> {
    if let Some(stripped) = s.strip_prefix("0x") {
        u8::from_str_radix(stripped, 16)
    } else {
        s.parse::<u8>()
    }
}

#[derive(Parser,Debug)]
#[command(about, long_about = None)]
struct Args {
    #[arg(required = true,value_parser=parse_number)]
    dest_id : u8,

    #[arg(short, long,default_value_t =1,value_parser=larger_than_zero)]
    number: u32,

    #[arg(short, long, default_value_t = false)]
    deamon: bool,
}

async fn send_once(dest_id:u8, adaptor:&mut TyCanProtocol){
    let telemetry_req = TeleMetry::request(0,dest_id).unwrap();
    if let Err(e) = adaptor.send(telemetry_req.try_into().unwrap()).await {
        log::error!("faild to send application response:{}", e);
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let args = Args::parse();
    let mut adaptor = TyCanProtocol::new(0, "can0", "can0").await.unwrap();
    if !args.deamon{
        for _ in 0..args.number{
            send_once(args.dest_id,&mut adaptor).await;
            sleep(Duration::from_secs(1)).await;
        }
    }else{
        loop{
            send_once(args.dest_id,&mut adaptor).await;
            sleep(Duration::from_secs(1)).await;
        }
    }
}

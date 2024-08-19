use std::{
    error::Error, mem::size_of, net::Ipv4Addr, process::Stdio, str::FromStr
};

use async_trait::async_trait;
use bitflags::bitflags;
use tokio::process::Command;

use super::{Application, Frame};

pub struct ResetNetwork;

#[repr(C)]
struct NetworkControlHeader{
    cmd : NetworkControlCommand,
    status : NetworkControlStatus,
}

#[repr(u8)]
#[derive(Debug)]
enum NetworkControlCommand{
    Unknown = 0,

    List = 1,
    ResetAll = 2,
}

#[repr(u8)]
#[derive(Debug)]
enum NetworkControlStatus{
    _Unknown = 0,

    Ok = 1,
    RunError = 2,
    UnknowCommand = 3,
}

impl From<u8> for NetworkControlCommand{
    fn from(value: u8) -> Self {
        match value {
            0 => NetworkControlCommand::List,
            1 => NetworkControlCommand::ResetAll,
            _ => NetworkControlCommand::Unknown,
        }
    }
}


#[async_trait]
impl Application for ResetNetwork {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let mut response = Frame::new_from_slice(Self::APPLICATION_ID, frame.data())?;
        response.set_meta_from_request(frame.meta());

        let cmd = NetworkControlCommand::from(frame.data()[0]);

        match cmd{
            NetworkControlCommand::List => {
                response.set_len((size_of::<NetworkControlHeader>() + size_of::<NetworkStatus>()) as u16)?;
                response.data_mut()[0] = frame.data()[0];
                if let Some(status) = NetworkStatus::new_from_buffer(&response.data_mut()[size_of::<NetworkControlHeader>()..size_of::<NetworkControlHeader>()+ size_of::<NetworkStatus>()]){
                    *status = list_status().await; 
                    response.data_mut()[1] = NetworkControlStatus::Ok as u8;
                }else{
                    response.data_mut()[1] = NetworkControlStatus::RunError as u8;
                }
                
            }
            NetworkControlCommand::ResetAll => {
                let is_ok = reset_all_network().await;
                response.set_len(size_of::<NetworkControlHeader>() as u16)?;
                response.data_mut()[0] = frame.data()[0];
                if is_ok{
                    response.data_mut()[0] = NetworkControlStatus::Ok as u8;
                }else{
                    response.data_mut()[1] = NetworkControlStatus::RunError as u8;
                }
            }
            NetworkControlCommand::Unknown => {
                response.set_len(size_of::<NetworkControlHeader>() as u16)?;
                response.data_mut()[0] = frame.data()[0];
                response.data_mut()[1] = NetworkControlStatus::UnknowCommand as u8;
            },
        }
            
        Ok(Some(response))
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }

    async fn init(&self) {}
}
bitflags! {
    #[derive(Debug, Clone,Copy,Default)]
    pub struct NetworkFlag: u32 {
        const UP = 1;
        const BROADCAST = 1<<2;
        const RUNNING = 1<<3;
        const MULTICAST = 1<<4;
        const LOOPBACK = 1<<5;
        const NOARP = 1<<6;
        const NOCHECKSUM = 1<<7;
        const QUORUMLOSS = 1<<8;

        const Unknown = 0;
    }
}

async fn reset_all_network() -> bool{
    let output = Command::new("netplan")
        .arg("apply")
        .stdout(Stdio::piped())
        .output()
        .await;
    output.is_ok()
}

/// Compile time check
const _MUST_NO_EXCEED_MTU : () = assert!(size_of::<NetworkControlHeader>() + size_of::<NetworkStatus>() < 100 );

#[repr(C)]
#[derive(Default, Debug)]
struct NetworkInterfaceStatus {
    ip: [u8; 4],
    state: NetworkFlag,
}

impl NetworkStatus {
    /// INVARIANT: `buf` should be a buffer to write `NetworkStatus`, and it must not less than size_of::<NetworkStatus>
    fn new_from_buffer(buf : &[u8]) -> Option<&'static mut Self> {
        if buf.len() < size_of::<Self>(){
            return None;
        }
        let ptr = buf.as_ptr() as *const NetworkStatus as *mut NetworkStatus;
        unsafe{
            Some(&mut *ptr)
        }
    }
}

#[repr(C)]
#[derive(Debug, Default)]
struct NetworkStatus {
    eth0: NetworkInterfaceStatus,
    eth1: NetworkInterfaceStatus,
}


impl From<&str> for NetworkFlag {
    fn from(value: &str) -> Self {
        match value {
            "UP" => NetworkFlag::UP,
            "BROADCAST" => NetworkFlag::BROADCAST,
            "RUNNING" => NetworkFlag::RUNNING,
            "MULTICAST" => NetworkFlag::MULTICAST,
            "LOOPBACK" => NetworkFlag::LOOPBACK,
            "NOARP" => NetworkFlag::NOARP,
            "NOCHECKSUM" => NetworkFlag::NOCHECKSUM,
            "QUORUMLOSS" => NetworkFlag::QUORUMLOSS,
            _ => NetworkFlag::Unknown,
        }
    }
}

async fn list_status() -> NetworkStatus {
    let eth0_status = parse_status("eth0")
        .await
        .unwrap_or(NetworkInterfaceStatus::default());
    let eth1_status = parse_status("eth1")
        .await
        .unwrap_or(NetworkInterfaceStatus::default());
    NetworkStatus {
        eth0: eth0_status,
        eth1: eth1_status,
    }
}

async fn parse_status(interface: &'static str) -> Result<NetworkInterfaceStatus, Box<dyn Error>> {
    let output = Command::new("ifconfig")
        .arg(interface)
        .stdout(Stdio::piped())
        .output()
        .await?;
    let mut status = NetworkInterfaceStatus::default();
    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        let flag = extract_network_flag(&stdout);
        let ipv4 = extract_ipv4(&stdout).unwrap_or(Ipv4Addr::new(0, 0, 0, 0));
        status.state = flag;
        status.ip = ipv4.octets();
        println!("{}: {:?}", interface, status);
    }
    Ok(status)
}


fn extract_network_flag(input: &str) -> NetworkFlag {
    if let Some(start) = input.find('<') {
        if let Some(end) = input.find('>') {
            let vec_of_flag = &input[start + 1..end];
            let mut flag = NetworkFlag::default();
            for str_flag in vec_of_flag.split(',') {
                flag |= NetworkFlag::from(str_flag);
            }
            return flag;
        }
    }
    NetworkFlag::default()
}

fn extract_ipv4(input: &str) -> Option<Ipv4Addr> {
    for line in input.lines() {
        if let Some(pos) = line.trim().find("inet ") {
            let parts: Vec<&str> = line.trim()[pos + 5..].split_whitespace().collect();
            if !parts.is_empty() {
                return Ipv4Addr::from_str(parts[0]).ok();
            }
        }
    }
    None
}

impl ResetNetwork {
    pub(crate) const APPLICATION_ID: u8 = 5;
}

#[tokio::test]
async fn test_tokio() {
    println!("{:?}", list_status().await);
}

use std::{
    error::Error,
    mem::size_of,
    net::Ipv4Addr,
    process::Stdio,
    slice::{from_raw_parts, from_raw_parts_mut},
    str::FromStr,
};

use async_trait::async_trait;
use bitflags::bitflags;
use tokio::process::Command;

use super::{Application, Frame};

pub struct ResetNetwork;

#[repr(C)]
struct NetworkControlHeader {
    cmd_and_status: u8,
}

impl From<NetworkControlHeader> for u8 {
    fn from(hdr: NetworkControlHeader) -> Self {
        hdr.cmd_and_status
    }
}

fn make_header(
    command: NetworkControlCommand,
    status: NetworkControlStatus,
) -> NetworkControlHeader {
    NetworkControlHeader {
        cmd_and_status: (command as u8) | ((status as u8) << 6),
    }
}

#[repr(u8)]
#[derive(Debug)]
enum NetworkControlCommand {
    Unknown = 0,

    List = 1,
    ResetAll = 2,
}

#[repr(u8)]
#[derive(Debug)]
enum NetworkControlStatus {
    _Unknown = 0,

    Ok = 1,
    RunError = 2,
    UnknowCommand = 3,
}

impl From<u8> for NetworkControlCommand {
    fn from(value: u8) -> Self {
        match value {
            1 => NetworkControlCommand::List,
            2 => NetworkControlCommand::ResetAll,
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
        match cmd {
            NetworkControlCommand::List => {
                response.set_len(
                    (size_of::<NetworkControlHeader>() + size_of::<NetworkStatus>()) as u16,
                )?;
                response.data_mut()[size_of::<NetworkControlHeader>()
                    ..size_of::<NetworkControlHeader>() + size_of::<NetworkStatus>()]
                    .copy_from_slice(list_status().await.into_network_endian_buffer());
                response.data_mut()[0] = make_header(cmd, NetworkControlStatus::Ok).into();
                log::debug!("receive net interface list. Response:{:?}", response);
            }
            NetworkControlCommand::ResetAll => {
                let is_ok = reset_all_network().await;
                response.set_len(size_of::<NetworkControlHeader>() as u16)?;
                if is_ok {
                    response.data_mut()[0] = make_header(cmd, NetworkControlStatus::Ok).into();
                    log::info!("Network interface reset success.");
                } else {
                    response.data_mut()[0] =
                        make_header(cmd, NetworkControlStatus::RunError).into();
                    log::error!("Network reset failed: Execute command failed.");
                }
            }
            NetworkControlCommand::Unknown => {
                response.set_len(size_of::<NetworkControlHeader>() as u16)?;
                response.data_mut()[0] =
                    make_header(cmd, NetworkControlStatus::UnknowCommand).into();
                log::error!("Unknow network control command.");
            }
        }

        Ok(Some(response))
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }

    fn application_name(&self) -> &'static str{
        "Reset Network"
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

impl NetworkFlag {
    fn as_network_nedian(&mut self) {
        #[cfg(target_endian = "little")]
        {
            *self = Self::from_bits_truncate(self.bits().to_be())
        }
        #[cfg(target_endian = "big")]
        {
            *self = Self::from_bits_truncate(self.bits().to_le())
        }
    }
}

async fn reset_all_network() -> bool {
    let output = Command::new("netplan")
        .arg("apply")
        .stdout(Stdio::piped())
        .output()
        .await;
    output.is_ok()
}

/// Compile time check
const _MUST_NO_EXCEED_MTU: () =
    assert!(size_of::<NetworkControlHeader>() + size_of::<NetworkStatus>() < 100);

#[repr(C)]
#[derive(Debug)]
struct NetworkInterfaceStatus {
    ip: Ipv4Addr,
    state: NetworkFlag,
    _reserve: [u8; 24],
}

impl Default for NetworkInterfaceStatus {
    fn default() -> Self {
        Self {
            ip: Ipv4Addr::new(0, 0, 0, 0),
            state: Default::default(),
            _reserve: Default::default(),
        }
    }
}
#[repr(C)]
#[derive(Debug, Default)]
struct NetworkStatus {
    eth0: NetworkInterfaceStatus,
    eth1: NetworkInterfaceStatus,
}

impl NetworkStatus {
    /// INVARIANT: `buf` should be a buffer to write `NetworkStatus`, and it must not less than size_of::<NetworkStatus>
    fn new_from_buffer(buf: &[u8]) -> Option<&'static mut Self> {
        if buf.len() < size_of::<Self>() {
            return None;
        }
        let ptr = buf.as_ptr() as *const NetworkStatus as *mut NetworkStatus;
        unsafe { Some(&mut *ptr) }
    }

    fn into_network_endian_buffer(mut self) -> &'static [u8] {
        self.eth0.state.as_network_nedian();
        self.eth1.state.as_network_nedian();
        let ptr = &self as *const NetworkStatus as *const u8;
        unsafe { from_raw_parts(ptr, size_of::<Self>()) }
    }
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
        status.ip = ipv4;
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

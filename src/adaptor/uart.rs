use std::os::fd::{AsFd, AsRawFd};
use std::convert::Into;

use async_trait::async_trait;

use nom::{
    bytes::complete::take, combinator::map_res, error::ErrorKind, sequence::tuple, IResult,
};


use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use termios;
use libc;
use nix::fcntl;
use nix::unistd;
use crc32fast::Hasher;

use super::{DeviceAdaptor, Frame, FrameFlag, FrameMeta};

struct Uart<'a> {
    device_name: &'a str,
    baud_rate: u32,
    req_id: u8,

}

#[async_trait]
impl<'a> DeviceAdaptor for Uart<'a> {
    async fn send(&self, buf: super::Frame) -> Result<(), super::DeviceAdaptorError> {
        let mut buf = buf.clone();
        // 1. open the uart device
        let mut opt = OpenOptions::new().read(true).write(true).custom_flags(libc::O_NOCTTY | libc::O_NDELAY).open(self.device_name).await.unwrap();
        let fd = opt.as_fd().as_raw_fd();

        // 2. get the mode of fd
        let mut old_termios = termios::Termios::from_fd(fd).unwrap();
        termios::tcgetattr(fd, &mut old_termios).unwrap();

        // 3. flush the input and output buf
        termios::tcflush(fd, termios::TCIFLUSH).unwrap();

        // 4. set the new mode of fd, including baud rate
        let mut new_termios = old_termios;
        new_termios.c_cflag = termios::os::linux::B230400 | termios::CS8 | termios::CLOCAL | termios::CREAD | termios::CSTOPB;
        termios::tcsetattr(fd, termios::TCSANOW, &mut new_termios);

        // 5. write the data to the uart device
        buf.expand_head(8);
        buf.expand_tail(1);
        let meta_len = buf.meta.len;
        let meta_data_type = buf.meta.data_type;
        let meta_command_type = buf.meta.command_type;

        let data = buf.data_mut();
        let mut hasher = Hasher::new();

        data[0] = 0xEB;
        data[1] = 0x90;
        data[2] = 0x01;
        data[3..5].copy_from_slice(&meta_len.to_be_bytes());
        data[5] = meta_data_type;
        data[6] = meta_command_type;
        data[7] = self.req_id;

        hasher.update(&data[3..data.len()-1]);
        data[data.len() - 1] = hasher.finalize() as u8;

        opt.write(&data);
        Ok(())
    }

    async fn recv(&self) -> Result<super::Frame, super::DeviceAdaptorError> {
        // Listen to the uart device, and return the data
        // 1. open the uart device
        let mut opt = OpenOptions::new().read(true).write(true).custom_flags(libc::O_NOCTTY | libc::O_NDELAY).open(self.device_name).await.unwrap();
        let fd = opt.as_fd().as_raw_fd();

        set_blocking(fd);

        unistd::isatty(fd);

        // 2. read the data from the uart device
        let mut buf = [0u8; 1024];
        let n = opt.read(&mut buf).await.unwrap();

        // 3. return the data
        let ty_uart = TyUartProtocol::from_slice_to_self(&buf[0..n]).unwrap().1;
        let mut framemeta: FrameMeta = FrameMeta::default();
        
        framemeta.len = ty_uart.data_len;
        framemeta.dest_id = ty_uart.platform_id;
        framemeta.id = ty_uart.req_id;
        framemeta.data_type = ty_uart.data_type as u8;
        framemeta.command_type = ty_uart.command_type.into();
        framemeta.flag = FrameFlag::default();
        let frame = Frame::new(framemeta, &ty_uart.data);
        
        Ok(frame)
    }

    fn mtu(&self, flag: FrameFlag) -> usize {
        if matches!(flag, FrameFlag::UartTelemetry){
            150
        } else {
            128
        }
    }
}

fn set_blocking(fd: std::os::fd::RawFd) -> nix::Result<()> {
    // get the file status flags
    let flags = fcntl::fcntl(fd, fcntl::FcntlArg::F_GETFL)?;
    
    // remove the O_NONBLOCK flag
    let mut new_flags: fcntl::OFlag = fcntl::OFlag::from_bits_truncate(flags);
    new_flags.remove(fcntl::OFlag::O_NONBLOCK);

    // set the new file status flags
    fcntl::fcntl(fd, fcntl::FcntlArg::F_SETFL(new_flags))?;
    Ok(())
}







#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum CommandType {
    TeleCommand = 0x35,
    TeleMetry = 0x05,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Header {
    Header = 0xEB90,
    Other,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum TeleCommand {
    BasicTeleCommand = 0x10,
    GeneralTeleCommand = 0x11,
    UDPTeleCommnadBackup = 0x12,
    UARTQuickTeleCommand = 0x20,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum TeleMetry {
    UDPTeleMetryBackup = 0x22,
    CANTeleMetryBackup = 0x23,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Command {
    TeleCommand(TeleCommand),
    TeleMetry(TeleMetry),
}

impl Into<u8> for Command {
    fn into(self) -> u8 {
        match self {
            Command::TeleCommand(TeleCommand::BasicTeleCommand) => 0x10,
            Command::TeleCommand(TeleCommand::GeneralTeleCommand) => 0x11,
            Command::TeleCommand(TeleCommand::UDPTeleCommnadBackup) => 0x12,
            Command::TeleCommand(TeleCommand::UARTQuickTeleCommand) => 0x20,
            Command::TeleMetry(TeleMetry::UDPTeleMetryBackup) => 0x22,
            Command::TeleMetry(TeleMetry::CANTeleMetryBackup) => 0x23,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TyUartProtocol {
    header: Header,
    platform_id: u8,
    data_len: u16,
    data_type: CommandType,
    command_type: Command,
    req_id: u8,
    data: Vec<u8>,
    checksum: u8,
}

impl TyUartProtocol {
    pub fn from_slice_to_self(input: &[u8]) -> IResult<&[u8], TyUartProtocol> {
        let original_input = input;
        let (input, (header, platform_id, data_len, data_type, command_type, req_id)) =
            tuple((
                Self::header_parser,
                Self::platform_id_parser,
                Self::data_len_parser,
                Self::data_type_parser,
                Self::command_type_parser,
                Self::req_id_parser,
            ))(input)?;
        let (input, data) = Self::data_parser(input, data_len)?;
        let (input, checksum) = Self::checksum_parser(input)?;

        assert_eq!(input.len(), 0, "input is not empty");
        // check data with crc32
        let mut hasher = Hasher::new();
        let crc_data = &original_input[2..original_input.len()-1];
        hasher.update(crc_data);
        assert_eq!(hasher.finalize(), checksum as u32, "checksum is not correct");

        Ok((
            input,
            TyUartProtocol {
                header,
                platform_id,
                data_len,
                data_type,
                command_type,
                req_id,
                data,
                checksum,
            },
        ))
    }

    fn header_parser(input: &[u8]) -> IResult<&[u8], Header> {
        map_res(take(2u64), |input: &[u8]| {
            let mut result = [0u8; 2];
            result.copy_from_slice(input);
            let res = u16::from_be_bytes(result);
            let res = match res {
                0xEB90 => Ok(Header::Header),
                _ => Err(ErrorKind::Tag),
            };
            res
        })(input)
    }
    
    fn platform_id_parser(input: &[u8]) -> IResult<&[u8], u8> {
        map_res(take(1u64), |input: &[u8]| {
            let mut result = [0u8; 1];
            result.copy_from_slice(input);
            let res: Result<u8, std::num::ParseIntError> = Ok(u8::from_be_bytes(result));
            res
        })(input)
    }
    
    fn data_len_parser(input: &[u8]) -> IResult<&[u8], u16> {
        map_res(take(2u64), |input: &[u8]| {
            let mut result = [0u8; 2];
            result.copy_from_slice(input);
            let res: Result<u16, std::num::ParseIntError> = Ok(u16::from_be_bytes(result));
            res
        })(input)
    }
    
    fn data_type_parser(input: &[u8]) -> IResult<&[u8], CommandType> {
        map_res(take(1u64), |input: &[u8]| {
            let mut result = [0u8; 1];
            result.copy_from_slice(input);
            let res = u8::from_be_bytes(result);
            let res = match res {
                0x35 => Ok(CommandType::TeleCommand),
                0x05 => Ok(CommandType::TeleMetry),
                // TODO: change the ErrorKind
                _ => Err(ErrorKind::Tag),
            };
            res
        })(input)
    }
    
    fn command_type_parser(input: &[u8]) -> IResult<&[u8], Command> {
        map_res(take(1u64), |input: &[u8]| {
            let mut result = [0u8; 1];
            result.copy_from_slice(input);
            let res = u8::from_be_bytes(result);
            let res = match res {
                0x10 => Ok(Command::TeleCommand(TeleCommand::BasicTeleCommand)),
                0x11 => Ok(Command::TeleCommand(TeleCommand::GeneralTeleCommand)),
                0x12 => Ok(Command::TeleCommand(TeleCommand::UDPTeleCommnadBackup)),
                0x20 => Ok(Command::TeleCommand(TeleCommand::UARTQuickTeleCommand)),
                0x22 => Ok(Command::TeleMetry(TeleMetry::UDPTeleMetryBackup)),
                0x23 => Ok(Command::TeleMetry(TeleMetry::CANTeleMetryBackup)),
                // TODO: change the ErrorKind
                _ => Err(ErrorKind::Tag),
            };
            res
        })(input)
    }
    
    fn req_id_parser(input: &[u8]) -> IResult<&[u8], u8> {
        map_res(take(1u64), |input: &[u8]| {
            let mut result = [0u8; 1];
            result.copy_from_slice(input);
            let res: Result<u8, std::num::ParseIntError> = Ok(u8::from_be_bytes(result));
            res
        })(input)
    }
    
    fn data_parser(input: &[u8], data_len: u16) -> IResult<&[u8], Vec<u8>> {
        let data_len = data_len - 3;
        map_res(take(data_len as u64), move |input: &[u8]| {
            let mut result = vec![0u8; data_len as usize];
            result.copy_from_slice(input);
            let res: Result<Vec<u8>, ErrorKind> = Ok(result);
            res
        })(input)
    }
    
    fn checksum_parser(input: &[u8]) -> IResult<&[u8], u8> {
        map_res(take(1u64), |input: &[u8]| {
            let mut result = [0u8; 1];
            result.copy_from_slice(input);
            let res: Result<u8, std::num::ParseFloatError> = Ok(u8::from_be_bytes(result));
            res
        })(input)
    }
}

impl TyUartProtocol {
    pub fn from_self_to_slice(&self) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend_from_slice(&(self.header as u16).to_be_bytes());
        result.extend_from_slice(&self.platform_id.to_be_bytes());
        result.extend_from_slice(&self.data_len.to_be_bytes());
        result.extend_from_slice(&(self.data_type as u8).to_be_bytes());
        let command_type: u8 = self.command_type.into();
        result.extend_from_slice(&command_type.to_be_bytes());
        result.extend_from_slice(&self.req_id.to_be_bytes());
        result.extend_from_slice(&self.data);
        result.extend_from_slice(&self.checksum.to_be_bytes());
        result
    }
}


#[test]
pub fn tyuart_from_slice_to_self_test() {
    let input = [
        0xEB, 0x90, 0x01, 0x00, 0x08, 0x35, 0x10, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
    ];
    let result = TyUartProtocol::from_slice_to_self(&input);
    assert_eq!(
        result,
        Ok((
            &[][..],
            TyUartProtocol {
                header: Header::Header,
                platform_id: 0x01,
                data_len: 0x0008,
                data_type: CommandType::TeleCommand,
                command_type: Command::TeleCommand(TeleCommand::BasicTeleCommand),
                req_id: 0x01,
                data: vec![0x02, 0x03, 0x04, 0x05, 0x06],
                checksum: 0x07
            }
        ))
    );
}

#[test]
pub fn tyuart_from_self_to_slice_test() {
    
}

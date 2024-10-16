#![allow(clippy::shadow_unrelated, clippy::unwrap_used)]
use std::convert::Into;
use std::time::Duration;

use async_trait::async_trait;

use nom::{bytes::complete::take, combinator::map_res, error::ErrorKind, sequence::tuple, IResult};

use serialport::SerialPort;
use tokio::sync::Mutex;

use super::{DeviceAdaptor, Frame, FrameFlag, FrameMeta};

#[cfg(feature = "unstable_add_frameheader")]
use crate::protocol::v1::frame::FrameHeader;

const CUSTOM_ALG: crc::Algorithm<u8> = crc::Algorithm {
    width: 8,
    poly: 0x80,
    init: 0xff,
    refin: false,
    refout: false,
    xorout: 0x0000,
    check: 0xae,
    residue: 0x0000,
};

#[derive(Debug)]
pub struct Uart {
    file: Mutex<Box<dyn SerialPort>>,
}

impl Uart {
    pub async fn new(device_name: &str, baud_rate: u32) -> Self {
        let port = serialport::new(device_name, baud_rate)
            .timeout(Duration::from_secs(5))
            .open()
            .unwrap();
        Self {
            file: Mutex::new(port),
        }
    }
}

#[async_trait]
impl DeviceAdaptor for Uart {
    async fn send(&self, buf: super::Frame) -> Result<(), super::DeviceAdaptorError> {
        let mut buf = buf.clone();

        buf.expand_head(8)?;
        buf.expand_tail(1)?;
        let meta_len = buf.meta.len;
        let meta_data_type = buf.meta.data_type;
        let meta_command_type = buf.meta.command_type;
        let meta_req_id = buf.meta.id;

        let data = &mut buf.data_mut()[0..(meta_len) as usize];
        let crc = crc::Crc::<u8>::new(&CUSTOM_ALG);
        let mut hasher = crc.digest();

        data[0] = 0xEB;
        data[1] = 0x90;
        data[2] = 0x01;
        data[3..5].copy_from_slice(&(meta_len - 6).to_be_bytes());
        data[5] = meta_data_type;
        data[6] = meta_command_type;
        data[7] = meta_req_id;

        hasher.update(&data[3..data.len() - 1]);
        data[data.len() - 1] = hasher.finalize();
        self.file.lock().await.write_all(data)?;

        Ok(())
    }

    async fn recv(&self) -> Result<super::Frame, super::DeviceAdaptorError> {
        // read the data from the uart device
        let mut buf = [0u8; 150];
        let n = self
            .file
            .lock()
            .await
            .read(&mut buf)
            .map_err(|_| super::DeviceAdaptorError::Empty)?;
        // return the data
        let ty_uart = TyUartProtocol::from_slice_to_self(&buf[0..n])
            .map_err(|_| super::DeviceAdaptorError::FrameError("recv data error".to_string()))?
            .1;

        let framemeta = FrameMeta {
            len: ty_uart.data_len,
            dest_id: ty_uart.platform_id,
            id: ty_uart.req_id,
            data_type: ty_uart.data_type as u8,
            command_type: ty_uart.command_type.into(),
            flag: FrameFlag::default(),
            ..Default::default()
        };
        let frame = Frame::new(framemeta, &ty_uart.data);

        frame.map_err(|_| super::DeviceAdaptorError::FrameError("recv data error".to_string()))
    }

    fn mtu(&self, flag: FrameFlag) -> usize {
        if matches!(flag, FrameFlag::UartTelemetry) {
            150
        } else {
            128
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum CommandType {
    TeleCommand = 0x35,
    TeleMetry = 0x05,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Header {
    Header = 0xEB90,
    _Other,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[allow(clippy::enum_variant_names)]
enum TeleCommand {
    BasicTeleCommand = 0x10,
    GeneralTeleCommand = 0x11,
    UDPTeleCommnadBackup = 0x12,
    UARTQuickTeleCommand = 0x20,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum TeleMetry {
    NormalTeleMetry1 = 0x00,
    NormalTeleMetry2 = 0x01,
    NormalTeleMetry3 = 0x02,
    NormalTeleMetry4 = 0x03,
    UDPTeleMetryBackup = 0x22,
    CANTeleMetryBackup = 0x23,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Command {
    TeleCommand(TeleCommand),
    TeleMetry(TeleMetry),
}

impl From<Command> for u8 {
    fn from(val: Command) -> Self {
        match val {
            Command::TeleMetry(TeleMetry::NormalTeleMetry1) => 0x00,
            Command::TeleMetry(TeleMetry::NormalTeleMetry2) => 0x01,
            Command::TeleMetry(TeleMetry::NormalTeleMetry3) => 0x02,
            Command::TeleMetry(TeleMetry::NormalTeleMetry4) => 0x03,
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
        log::debug!("Starting parsing recv data stage 1: input {:?}", input);
        let original_input = input;
        let (input, (header, platform_id, mut data_len, data_type, command_type, req_id)) =
            tuple((
                Self::header_parser,
                Self::platform_id_parser,
                Self::data_len_parser,
                Self::data_type_parser,
                Self::command_type_parser,
                Self::req_id_parser,
            ))(input)?;
        log::debug!("Starting parsing recv data stage 2");
        let (input, mut data) = Self::data_parser(input, data_len)?;
        log::debug!("Starting parsing recv data stage 3");
        let (input, checksum) = Self::checksum_parser(input)?;

        if !input.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Verify,
            )));
        }
        // check data with crc32
        let crc = crc::Crc::<u8>::new(&CUSTOM_ALG);
        let mut hasher = crc.digest();

        let crc_data = &original_input[3..original_input.len() - 1];
        hasher.update(crc_data);
        // if hasher.finalize() != checksum {
        //     return Err(nom::Err::Error(nom::error::Error::new(
        //         input,
        //         nom::error::ErrorKind::Verify,
        //     )));
        // }

        #[cfg(feature = "unstable_add_frameheader")]
        {
            let raw_header = vec![0x00u8, 0x00u8];
            let header = &mut FrameHeader::try_from(raw_header.as_slice()).unwrap();

            header.application = 0x00;
            header.version = 0x20;
            data.extend(&raw_header);
            data_len += size_of::<FrameHeader>() as u16;
        }
        log::debug!("recv data construct ok");
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
        log::debug!("Starting header_parser");
        map_res(take(2u64), |input: &[u8]| {
            let mut result = [0u8; 2];
            result.copy_from_slice(input);
            let res = u16::from_be_bytes(result);

            match res {
                0xEB90 => Ok(Header::Header),
                _ => Err(ErrorKind::Tag),
            }
        })(input)
    }

    fn platform_id_parser(input: &[u8]) -> IResult<&[u8], u8> {
        log::debug!("Starting platform_id_parser");
        map_res(take(1u64), |input: &[u8]| {
            let mut result = [0u8; 1];
            result.copy_from_slice(input);
            let res: Result<u8, std::num::ParseIntError> = Ok(u8::from_be_bytes(result));
            res
        })(input)
    }

    fn data_len_parser(input: &[u8]) -> IResult<&[u8], u16> {
        log::debug!("Starting data_len_parser");
        map_res(take(2u64), |input: &[u8]| {
            let mut result = [0u8; 2];
            result.copy_from_slice(input);
            let res: Result<u16, std::num::ParseIntError> = Ok(u16::from_be_bytes(result));
            res
        })(input)
    }

    fn data_type_parser(input: &[u8]) -> IResult<&[u8], CommandType> {
        log::debug!("Starting data_type_parser");
        map_res(take(1u64), |input: &[u8]| {
            let mut result = [0u8; 1];
            result.copy_from_slice(input);
            let res = u8::from_be_bytes(result);

            match res {
                0x35 => Ok(CommandType::TeleCommand),
                0x05 => Ok(CommandType::TeleMetry),
                // TODO: change the ErrorKind
                _ => Err(ErrorKind::Tag),
            }
        })(input)
    }

    fn command_type_parser(input: &[u8]) -> IResult<&[u8], Command> {
        log::debug!("Starting command_type_parser");
        map_res(take(1u64), |input: &[u8]| {
            let mut result = [0u8; 1];
            result.copy_from_slice(input);
            let res = u8::from_be_bytes(result);

            match res {
                0x00 => Ok(Command::TeleMetry(TeleMetry::NormalTeleMetry1)),
                0x01 => Ok(Command::TeleMetry(TeleMetry::NormalTeleMetry2)),
                0x02 => Ok(Command::TeleMetry(TeleMetry::NormalTeleMetry3)),
                0x03 => Ok(Command::TeleMetry(TeleMetry::NormalTeleMetry4)),
                0x10 => Ok(Command::TeleCommand(TeleCommand::BasicTeleCommand)),
                0x11 => Ok(Command::TeleCommand(TeleCommand::GeneralTeleCommand)),
                0x12 => Ok(Command::TeleCommand(TeleCommand::UDPTeleCommnadBackup)),
                0x20 => Ok(Command::TeleCommand(TeleCommand::UARTQuickTeleCommand)),
                0x22 => Ok(Command::TeleMetry(TeleMetry::UDPTeleMetryBackup)),
                0x23 => Ok(Command::TeleMetry(TeleMetry::CANTeleMetryBackup)),
                // TODO: change the ErrorKind
                _ => Err(ErrorKind::Tag),
            }
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
            result.copy_from_slice(&input[0..(data_len as usize)]);
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

#[ignore] // TODO: can not pass without hardware
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
#[ignore]
fn tyuart_from_self_to_slice_test() {}

#[tokio::test]
#[ignore]
async fn adaptor_uart_recv() {
    println!("into recv");
    let uart = Uart::new("/dev/ttyAMA3", 9600).await;
    let frame = uart.recv().await.unwrap();
    println!("{}", frame.meta.len);
    assert_eq!(frame.meta.len, 0x0005);
    assert_eq!(frame.meta.data_type, 0x35);
    assert_eq!(frame.meta.command_type, 0x10);
    assert!(frame.meta.flag.is_empty());
    assert_eq!(frame.meta.dest_id, 0x01);
    assert_eq!(frame.meta.id, 0x00);
    assert_eq!(frame.data(), vec![0x02, 0x03, 0x04, 0x05, 0x06]);

    tokio::time::sleep(Duration::from_millis(5000)).await;
}

#[tokio::test]
#[ignore]
async fn adaptor_uart_send() {
    let uart = Uart::new("/dev/ttyAMA2", 9600).await;
    let frame_meta = FrameMeta {
        data_type: 0x35,
        command_type: 0x10,
        ..Default::default()
    };
    let frame = Frame::new(frame_meta, &[0x02, 0x03, 0x04, 0x05, 0x06]).unwrap();
    println!("frame: {:?}", frame);
    uart.send(frame).await.unwrap();
}

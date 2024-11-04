#![allow(clippy::shadow_unrelated, clippy::unwrap_used)]
use std::convert::Into;
use std::time::Duration;

use async_trait::async_trait;

use nom::{
    bytes::complete::take, combinator::map_res, error::ErrorKind, sequence::tuple, AsBytes, IResult,
};

use serialport::SerialPort;
use std::num::Wrapping;
use tokio::sync::Mutex;

use super::{DeviceAdaptor, Frame, FrameFlag, FrameMeta};

const MAGIC_HEADER: u16 = 0xEB90;
const MAGIC_HEADER_BYTES: [u8; 2] = MAGIC_HEADER.to_be_bytes();

const DATA_TYPE_REQUEST: u8 = 0x05;
const DATA_TYPE_RESPONSE: u8 = 0x35;

fn checksum_8(data: &[u8]) -> u8 {
    let mut checksum = Wrapping(0u8);
    for byte in data {
        checksum += Wrapping(*byte);
    }
    checksum.0
}

const CRC_TAB: [u32; 256] = [
    0x00000000, 0xF26B8303, 0xE13B70F7, 0x1350F3F4, 0xC79A971F, 0x35F1141C, 0x26A1E7E8, 0xD4CA64EB,
    0x8AD958CF, 0x78B2DBCC, 0x6BE22838, 0x9989AB3B, 0x4D43CFD0, 0xBF284CD3, 0xAC78BF27, 0x5E133C24,
    0x105EC76F, 0xE235446C, 0xF165B798, 0x030E349B, 0xD7C45070, 0x25AFD373, 0x36FF2087, 0xC494A384,
    0x9A879FA0, 0x68EC1CA3, 0x7BBCEF57, 0x89D76C54, 0x5D1D08BF, 0xAF768BBC, 0xBC267848, 0x4E4DFB4B,
    0x20BD8EDE, 0xD2D60DDD, 0xC186FE29, 0x33ED7D2A, 0xE72719C1, 0x154C9AC2, 0x061C6936, 0xF477EA35,
    0xAA64D611, 0x580F5512, 0x4B5FA6E6, 0xB93425E5, 0x6DFE410E, 0x9F95C20D, 0x8CC531F9, 0x7EAEB2FA,
    0x30E349B1, 0xC288CAB2, 0xD1D83946, 0x23B3BA45, 0xF779DEAE, 0x05125DAD, 0x1642AE59, 0xE4292D5A,
    0xBA3A117E, 0x4851927D, 0x5B016189, 0xA96AE28A, 0x7DA08661, 0x8FCB0562, 0x9C9BF696, 0x6EF07595,
    0x417B1DBC, 0xB3109EBF, 0xA0406D4B, 0x522BEE48, 0x86E18AA3, 0x748A09A0, 0x67DAFA54, 0x95B17957,
    0xCBA24573, 0x39C9C670, 0x2A993584, 0xD8F2B687, 0x0C38D26C, 0xFE53516F, 0xED03A29B, 0x1F682198,
    0x5125DAD3, 0xA34E59D0, 0xB01EAA24, 0x42752927, 0x96BF4DCC, 0x64D4CECF, 0x77843D3B, 0x85EFBE38,
    0xDBFC821C, 0x2997011F, 0x3AC7F2EB, 0xC8AC71E8, 0x1C661503, 0xEE0D9600, 0xFD5D65F4, 0x0F36E6F7,
    0x61C69362, 0x93AD1061, 0x80FDE395, 0x72966096, 0xA65C047D, 0x5437877E, 0x4767748A, 0xB50CF789,
    0xEB1FCBAD, 0x197448AE, 0x0A24BB5A, 0xF84F3859, 0x2C855CB2, 0xDEEEDFB1, 0xCDBE2C45, 0x3FD5AF46,
    0x7198540D, 0x83F3D70E, 0x90A324FA, 0x62C8A7F9, 0xB602C312, 0x44694011, 0x5739B3E5, 0xA55230E6,
    0xFB410CC2, 0x092A8FC1, 0x1A7A7C35, 0xE811FF36, 0x3CDB9BDD, 0xCEB018DE, 0xDDE0EB2A, 0x2F8B6829,
    0x82F63B78, 0x709DB87B, 0x63CD4B8F, 0x91A6C88C, 0x456CAC67, 0xB7072F64, 0xA457DC90, 0x563C5F93,
    0x082F63B7, 0xFA44E0B4, 0xE9141340, 0x1B7F9043, 0xCFB5F4A8, 0x3DDE77AB, 0x2E8E845F, 0xDCE5075C,
    0x92A8FC17, 0x60C37F14, 0x73938CE0, 0x81F80FE3, 0x55326B08, 0xA759E80B, 0xB4091BFF, 0x466298FC,
    0x1871A4D8, 0xEA1A27DB, 0xF94AD42F, 0x0B21572C, 0xDFEB33C7, 0x2D80B0C4, 0x3ED04330, 0xCCBBC033,
    0xA24BB5A6, 0x502036A5, 0x4370C551, 0xB11B4652, 0x65D122B9, 0x97BAA1BA, 0x84EA524E, 0x7681D14D,
    0x2892ED69, 0xDAF96E6A, 0xC9A99D9E, 0x3BC21E9D, 0xEF087A76, 0x1D63F975, 0x0E330A81, 0xFC588982,
    0xB21572C9, 0x407EF1CA, 0x532E023E, 0xA145813D, 0x758FE5D6, 0x87E466D5, 0x94B49521, 0x66DF1622,
    0x38CC2A06, 0xCAA7A905, 0xD9F75AF1, 0x2B9CD9F2, 0xFF56BD19, 0x0D3D3E1A, 0x1E6DCDEE, 0xEC064EED,
    0xC38D26C4, 0x31E6A5C7, 0x22B65633, 0xD0DDD530, 0x0417B1DB, 0xF67C32D8, 0xE52CC12C, 0x1747422F,
    0x49547E0B, 0xBB3FFD08, 0xA86F0EFC, 0x5A048DFF, 0x8ECEE914, 0x7CA56A17, 0x6FF599E3, 0x9D9E1AE0,
    0xD3D3E1AB, 0x21B862A8, 0x32E8915C, 0xC083125F, 0x144976B4, 0xE622F5B7, 0xF5720643, 0x07198540,
    0x590AB964, 0xAB613A67, 0xB831C993, 0x4A5A4A90, 0x9E902E7B, 0x6CFBAD78, 0x7FAB5E8C, 0x8DC0DD8F,
    0xE330A81A, 0x115B2B19, 0x020BD8ED, 0xF0605BEE, 0x24AA3F05, 0xD6C1BC06, 0xC5914FF2, 0x37FACCF1,
    0x69E9F0D5, 0x9B8273D6, 0x88D28022, 0x7AB90321, 0xAE7367CA, 0x5C18E4C9, 0x4F48173D, 0xBD23943E,
    0xF36E6F75, 0x0105EC76, 0x12551F82, 0xE03E9C81, 0x34F4F86A, 0xC69F7B69, 0xD5CF889D, 0x27A40B9E,
    0x79B737BA, 0x8BDCB4B9, 0x988C474D, 0x6AE7C44E, 0xBE2DA0A5, 0x4C4623A6, 0x5F16D052, 0xAD7D5351,
];

// 计算 CRC32
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFF;
    for &byte in data {
        crc = CRC_TAB[(crc ^ byte as u32) as usize & 0xFF] ^ (crc >> 8);
    }
    crc ^ 0xFFFFFFFF
}

#[derive(Debug)]
pub struct Uart {
    file: Mutex<Box<dyn SerialPort>>,
    device_id: u8,
}

impl Uart {
    pub async fn new(device_name: &str, baud_rate: u32, device_id: u8) -> Self {
        let port = serialport::new(device_name, baud_rate)
            .timeout(Duration::from_secs(5))
            .open()
            .unwrap();
        Self {
            file: Mutex::new(port),
            device_id,
        }
    }
}

#[async_trait]
impl DeviceAdaptor for Uart {
    async fn send(&self, buf: super::Frame) -> Result<(), super::DeviceAdaptorError> {
        let mut buf = buf.clone();

        buf.expand_head(8)?;

        let meta_command_type = buf.meta.command_type;
        let meta_req_id = buf.meta.id;
        if meta_command_type == 0xC0 {
            buf.expand_tail(4)?;
        } else {
            buf.expand_tail(1)?;
        }
        let meta_len = buf.meta.len;

        let data = buf.data_mut();

        data[0] = MAGIC_HEADER_BYTES[0];
        data[1] = MAGIC_HEADER_BYTES[1];
        data[2] = self.device_id;
        if meta_command_type == 0xC0 {
            data[3..5].copy_from_slice(&(meta_len - 9).to_be_bytes());
        } else {
            data[3..5].copy_from_slice(&(meta_len - 6).to_be_bytes());
        }
        data[5] = DATA_TYPE_RESPONSE;
        data[6] = meta_command_type;
        data[7] = meta_req_id;

        if meta_command_type == 0xC0 {
            let crc = crc32(&data[3..data.len() - 4]);
            let len = data.len();
            data[len - 4..len].copy_from_slice(&crc.to_be_bytes());
        } else {
            data[data.len() - 1] = checksum_8(&data[3..data.len() - 1]);
        }

        self.file.lock().await.write_all(data)?;
        self.file.lock().await.flush()?;
        log::debug!("uart send data: {:?}", data);

        Ok(())
    }

    async fn recv(&self) -> Result<super::Frame, super::DeviceAdaptorError> {
        // NOTE: 根据 upload和download任务的metadata, 填充payload的前两个字节
        // read the data from the uart device
        let mut header_buf = [0u8; 5];
        self.file.lock().await.read_exact(&mut header_buf)?;

        let data_len = u16::from_be_bytes([header_buf[3], header_buf[4]]);
        let mut buf = vec![0u8; data_len as usize];
        self.file
            .lock()
            .await
            .read_exact(&mut buf)
            .map_err(|_| super::DeviceAdaptorError::Empty)?;

        let mut crc_buf: [u8; 4] = [0; 4];
        let crc_buf = {
            if buf[1] == 0xA1 {
                &mut crc_buf
            } else {
                &mut crc_buf[0..1]
            }
        };
        self.file.lock().await.read_exact(crc_buf)?;

        let mut data = vec![];
        data.extend(&header_buf);
        data.extend(&buf);
        data.extend(&*crc_buf);

        #[allow(unused_mut)]
        let mut ty_uart = TyUartProtocol::from_slice_to_self(&data)
            .map_err(|_| super::DeviceAdaptorError::FrameError("recv data error".to_string()))?
            .1;

        // TODO: Code here may be moved to impl of `TyUartProtocol`
        #[cfg(feature = "unstable_upload_and_download")]
        {
            if let Command::TeleCommand(TeleCommand::UploadRequestCommand) = ty_uart.command_type {
                let mut data = ty_uart.data.clone();
                // Preappend the data with `0x20, 0x04`
                data.insert(0, 0x20);
                data.insert(1, 0x04);
                ty_uart.data = data;
                ty_uart.data_len += 2;
            } else if let Command::TeleCommand(TeleCommand::UploadDataCommand) =
                ty_uart.command_type
            {
                let mut data = ty_uart.data.clone();
                // Preappend the data with `0x20, 0x04`
                data.insert(0, 0x20);
                data.insert(1, 0x04);
                ty_uart.data = data;
                ty_uart.data_len += 2;
            } else if let Command::TeleCommand(TeleCommand::DownloadCommand) = ty_uart.command_type
            {
                let mut data = ty_uart.data.clone();
                // Preappend the data with `0x20, 0x05`
                data.insert(0, 0x20);
                data.insert(1, 0x07);
                ty_uart.data = data;
                ty_uart.data_len += 2;
            }
        }

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
enum DataType {
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
    UploadRequestCommand = 0xA0,
    UploadDataCommand = 0xA1,
    DownloadCommand = 0xC0,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum TeleMetry {
    UARTQuickTeleMetry = 0x20,
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
            Command::TeleCommand(TeleCommand::BasicTeleCommand) => 0x10,
            Command::TeleCommand(TeleCommand::GeneralTeleCommand) => 0x11,
            Command::TeleCommand(TeleCommand::UDPTeleCommnadBackup) => 0x12,
            Command::TeleCommand(TeleCommand::UploadRequestCommand) => 0xA0,
            Command::TeleCommand(TeleCommand::UploadDataCommand) => 0xA1,
            Command::TeleCommand(TeleCommand::DownloadCommand) => 0xC0,
            Command::TeleMetry(TeleMetry::UARTQuickTeleMetry) => 0x20,
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
    data_type: DataType,
    command_type: Command,
    req_id: u8,
    data: Vec<u8>,
}

impl TyUartProtocol {
    pub fn from_slice_to_self(input: &[u8]) -> IResult<&[u8], TyUartProtocol> {
        log::debug!("Starting parsing recv data stage 1: input {:?}", input);
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

        let input = {
            if command_type == Command::TeleCommand(TeleCommand::UploadDataCommand) {
                let (input, _crc32) = Self::crc32_parser(input)?;
                let crc32 = crc32(&original_input[3..original_input.len() - 4]);
                if _crc32 != crc32 {
                    log::error!("recv data crc32 error");
                    return Err(nom::Err::Error(nom::error::Error::new(
                        input,
                        nom::error::ErrorKind::Verify,
                    )));
                }
                input
            } else {
                let (input, _checksum) = Self::checksum_8_parser(input)?;
                let checksum = checksum_8(&original_input[3..original_input.len() - 1]);
                if _checksum != checksum {
                    log::error!("recv data checksum error");
                    return Err(nom::Err::Error(nom::error::Error::new(
                        input,
                        nom::error::ErrorKind::Verify,
                    )));
                }
                input
            }
        };

        if !input.is_empty() {
            log::error!("recv data out of range");
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Verify,
            )));
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

    fn data_type_parser(input: &[u8]) -> IResult<&[u8], DataType> {
        log::debug!("Starting data_type_parser");
        map_res(take(1u64), |input: &[u8]| {
            let mut result = [0u8; 1];
            result.copy_from_slice(input);
            let res = u8::from_be_bytes(result);

            match res {
                0x35 => Ok(DataType::TeleCommand),
                0x05 => Ok(DataType::TeleMetry),
                _ => {
                    log::error!("data_type_parser error");
                    // TODO: change the ErrorKind
                    Err(ErrorKind::Tag)
                }
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
                0x10 => Ok(Command::TeleCommand(TeleCommand::BasicTeleCommand)),
                0x11 => Ok(Command::TeleCommand(TeleCommand::GeneralTeleCommand)),
                0x12 => Ok(Command::TeleCommand(TeleCommand::UDPTeleCommnadBackup)),
                0xA0 => Ok(Command::TeleCommand(TeleCommand::UploadRequestCommand)),
                0xA1 => Ok(Command::TeleCommand(TeleCommand::UploadDataCommand)),
                0xC0 => Ok(Command::TeleCommand(TeleCommand::DownloadCommand)),
                0x20 => Ok(Command::TeleMetry(TeleMetry::UARTQuickTeleMetry)),
                0x22 => Ok(Command::TeleMetry(TeleMetry::UDPTeleMetryBackup)),
                0x23 => Ok(Command::TeleMetry(TeleMetry::CANTeleMetryBackup)),
                _ => {
                    log::error!("command_type_parser error");
                    // TODO: change the ErrorKind
                    Err(ErrorKind::Tag)
                }
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

    fn checksum_8_parser(input: &[u8]) -> IResult<&[u8], u8> {
        map_res(take(1u64), |input: &[u8]| {
            let mut result = [0u8; 1];
            result.copy_from_slice(input);
            let res: Result<u8, std::num::ParseFloatError> = Ok(u8::from_be_bytes(result));
            res
        })(input)
    }

    fn crc32_parser(input: &[u8]) -> IResult<&[u8], u32> {
        map_res(take(4u64), |input: &[u8]| {
            let mut result = [0u8; 4];
            result.copy_from_slice(input);
            let res: Result<u32, std::num::ParseFloatError> = Ok(u32::from_be_bytes(result));
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
                data_type: DataType::TeleCommand,
                command_type: Command::TeleCommand(TeleCommand::BasicTeleCommand),
                req_id: 0x01,
                data: vec![0x02, 0x03, 0x04, 0x05, 0x06],
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
    let uart = Uart::new("/dev/ttyAMA3", 9600, 0x84).await;
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
    let uart = Uart::new("/dev/ttyAMA2", 9600, 0x84).await;
    let frame_meta = FrameMeta {
        data_type: 0x35,
        command_type: 0x10,
        ..Default::default()
    };
    let frame = Frame::new(frame_meta, &[0x02, 0x03, 0x04, 0x05, 0x06]).unwrap();
    println!("frame: {:?}", frame);
    uart.send(frame).await.unwrap();
}

use std::{
    io::{self, Error, ErrorKind},
    sync::atomic::AtomicU8,
};

use bitfield::bitfield;

use async_trait::async_trait;
use num_enum::TryFromPrimitive;
use socketcan::{
    tokio::AsyncCanSocket, CanDataFrame, CanFrame, CanSocket, EmbeddedFrame, ExtendedId, Frame,
};

use super::DeviceAdaptor;

const SLOT_SIZE: usize = 158;
const RECV_BUF_SLOT_NUM: usize = 128; // 8bits for id
const TOTAL_LEN_IN_DATA_OF_MULTI: usize = 0x2;
const TY_CAN_PROTOCOL_MTU: usize = 150;
const TY_CAN_PROTOCOL_SINGLE_FRAME_SIZE: usize = 6;

pub(crate) struct Can {}

impl Can {
    // fn new(name :&str) -> io::Result<Self>{
    //     let socket = AsyncCanSocket::open(&name)?;
    //     Ok(Self {
    //         socket,
    //     })
    // }
}

// #[async_trait]
// impl DeviceAdaptor for Can {
//     async fn send(&self, buf: Box<[u8]>) {}

//     async fn recv(&self) -> Box<[u8]> {
//         // TyCanProtocol有数据，则上报
//         Box::new([])
//     }
// }

bitfield! {
    struct TyCanId(u32);
    u8;
    pub get_pid, set_pid: 7, 0;
    pub get_flag, set_flag: 8;
    pub get_frame_type, set_frame_type: 12,9;
    pub get_dest_id, set_dest_id: 20,13;
    pub get_src_id, set_src_id: 28,21;
}

/// Tianyi can protocol
struct TyCanProtocol {
    src_id: u8,
    slot_map: [Slot; RECV_BUF_SLOT_NUM], // 20KB
    socket: AsyncCanSocket<CanSocket>,
    id_counter: AtomicU8,
}

#[derive(Clone, Copy)]
struct Slot {
    data: [u8; SLOT_SIZE],
    current_len: u8,
    total_len: u8,
}
impl Default for Slot {
    fn default() -> Self {
        Self {
            data: [0u8; SLOT_SIZE],
            current_len: 0,
            total_len: 0,
        }
    }
}
impl Slot {
    fn reset(&mut self) {
        self.current_len = 0;
        self.total_len = 0;
    }

    fn set_total_len(&mut self, len: u8) -> io::Result<()>  {
        if len > SLOT_SIZE as u8 {
            return Err(io::Error::new(ErrorKind::InvalidInput, "len invalid"));
        }
        Ok(())
    }

    fn copy_from_slice(&mut self, src: &[u8]) -> io::Result<()> {
        let current_len = self.current_len as usize;
        if current_len + src.len() > self.total_len as usize{
            return Err(io::Error::new(ErrorKind::InvalidInput, "overflow"));
        }
        self.data[current_len..src.len()].copy_from_slice(src);
        self.current_len += src.len() as u8;
        Ok(())
    }

    fn is_complete(&self) -> bool {
        self.total_len == 0 || self.current_len == self.total_len
    }
}

#[derive(TryFromPrimitive)]
#[repr(u8)]
enum TyCanProtocolFrameType {
    Recover = 0b0000,
    Single = 0b0001,
    MultiFirst = 0b0010,
    MultiMiddle = 0b0011,
    TimeBroadcast = 0b0100,

    Unknown = 0b1111,
}

impl TyCanProtocol {
    pub(crate) fn new(src_id: u8, socket_name: &str) -> Self {
        let socket = AsyncCanSocket::open(socket_name).unwrap();
        Self {
            src_id,
            slot_map: [Slot::default(); RECV_BUF_SLOT_NUM],
            socket,
            id_counter: AtomicU8::new(0),
        }
    }
    pub(crate)fn recv(&mut self, frame: &CanDataFrame) -> Option<super::Frame> {
        let ty_can_id = TyCanId(frame.raw_id());
        let is_csp = ty_can_id.get_flag();
        if is_csp {
            return None;
        }
        let idx = ty_can_id.get_pid();
        let frame_type = TyCanProtocolFrameType::try_from(ty_can_id.get_frame_type()).unwrap();
        let src_id = ty_can_id.get_src_id();
        let dest_id = ty_can_id.get_dest_id();
        let frame = match frame_type {
            TyCanProtocolFrameType::Single => {
                let len = frame.len();
                let meta = super::FrameMeta {
                    src_id,
                    dest_id,
                    len: len as u8,
                    flag: super::FrameFlag::empty(),
                };
                Some(super::Frame::new(meta, &frame.data()[..len]))
            }
            TyCanProtocolFrameType::MultiFirst => {
                let slot = &mut self.slot_map[idx as usize];
                let _ = slot.copy_from_slice(frame.data());

                let mut total_len_buf = [0u8; 2];
                total_len_buf.copy_from_slice(&frame.data()[0..TOTAL_LEN_IN_DATA_OF_MULTI]);
                let total_len = u16::from_be_bytes(total_len_buf);
                slot.set_total_len(total_len as u8);
                None
            }
            TyCanProtocolFrameType::MultiMiddle => {
                let slot = &mut self.slot_map[idx as usize];
                let _ = slot.copy_from_slice(frame.data());

                if slot.is_complete() {
                    let meta = super::FrameMeta {
                        src_id,
                        dest_id,
                        len: slot.total_len,
                        flag: super::FrameFlag::empty(),
                    };
                    Some(super::Frame::new(meta, &slot.data))
                } else {
                    None
                }
            }
            TyCanProtocolFrameType::TimeBroadcast => {
                let meta = super::FrameMeta {
                    src_id,
                    dest_id,
                    len: 8,
                    flag: super::FrameFlag::CanTimeBroadcast,
                };
                Some(super::Frame::new(meta, &frame.data()[..8]))
            }
            _ => None,
        };
        frame
    }

    async fn send(&mut self, frame: super::Frame) -> socketcan::Result<()> {
        let len = frame.meta.len;
        let mut new_id = TyCanId(0);
        new_id.set_src_id(self.src_id);
        new_id.set_dest_id(frame.meta.dest_id);
        new_id.set_flag(false);
        if len > TY_CAN_PROTOCOL_MTU as u8{
            return Err(socketcan::Error::Io(io::Error::new(ErrorKind::InvalidInput, "len invalid")));
        }
        if len < TY_CAN_PROTOCOL_SINGLE_FRAME_SIZE as u8 {
            let id = self
                .id_counter
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            new_id.set_pid(id);
            new_id.set_frame_type(TyCanProtocolFrameType::Single as u8);
            let can_frame =
                CanFrame::new(ExtendedId::new(new_id.0).unwrap(), frame.data()).unwrap();
            self.socket.write_frame(can_frame)?.await?;
        } else {
            let id = self
                .id_counter
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            new_id.set_pid(id);
            new_id.set_frame_type(TyCanProtocolFrameType::Single as u8);
            let can_frame =
                CanFrame::new(ExtendedId::new(new_id.0).unwrap(), frame.data()).unwrap();
            self.socket.write_frame(can_frame)?.await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_typeid() {

    }


    #[test]
    fn test_ty_protocol() {

    }

    #[test]
    fn test_slot() {
        let mut slot = super::Slot::default();
        let data = [0u8; 158];
        assert!(slot.set_total_len(159).is_err());
        assert!(slot.set_total_len(30).is_ok());
        assert!(slot.copy_from_slice(&data[..50]).is_err());
        assert!(slot.copy_from_slice(&data[..29]).is_ok());
        assert!(!slot.is_complete());
        assert!(slot.copy_from_slice(&data[29..30]).is_ok());
        assert!(slot.is_complete());

    }
}

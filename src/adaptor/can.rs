use std::io::{self, ErrorKind};

use bitfield::bitfield;

use async_trait::async_trait;
use num_enum::TryFromPrimitive;
use socketcan::{tokio::AsyncCanSocket, CanDataFrame, CanSocket, EmbeddedFrame, Frame};

use super::DeviceAdaptor;

const SLOT_SIZE: usize = 158;
const RECV_BUF_SLOT_NUM: usize = 128; // 8bits for id
const SEND_BUF_SLOT_NUM: usize = 128;
const SEND_BUF_SLOT_NUM_IN_BYTE: usize = SEND_BUF_SLOT_NUM / 8;

pub(crate) struct Can {}

impl Can {
    // fn new(name :&str) -> io::Result<Self>{
    //     let socket = AsyncCanSocket::open(&name)?;
    //     Ok(Self {
    //         socket,
    //     })
    // }
}

#[async_trait]
impl DeviceAdaptor for Can {
    async fn send(&self, buf: Box<[u8]>) {}

    async fn recv(&self) -> Box<[u8]> {
        // TyCanProtocol有数据，则上报
        Box::new([])
    }
}

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
    slot_map: [Slot; RECV_BUF_SLOT_NUM], // 20KB
    socket: AsyncCanSocket<CanSocket>,
}

struct Slot {
    data: [u8; SLOT_SIZE],
    current_len: u8,
    total_len: u8,
}

impl Slot {
    fn reset(&mut self) {
        self.current_len = 0;
        self.total_len = 0;
    }

    fn set_total_len(&mut self, len: u8) {
        self.total_len = len;
    }

    fn copy_from_slice(&mut self, src: &[u8]) -> io::Result<()> {
        let current_len = self.current_len as usize;
        if current_len + src.len() > SLOT_SIZE {
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

// struct SendBuf{
//     data : Box<[Slot;SEND_BUF_SLOT_NUM]>,
//     bitmap : [u8;SEND_BUF_SLOT_NUM_IN_BYTE],
// }

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

const TOTAL_LEN_IN_DATA_OF_MULTI: usize = 0x2;
const TY_CAN_PROTOCOL_MTU : usize = 150;
const TY_CAN_PROTOCOL_SINGLE_FRAME_SIZE : usize = 6;


impl TyCanProtocol {
    pub(crate) async fn async_recv(&mut self) {}

    pub(crate) async fn async_send(&mut self) {}

    fn recv(&mut self, frame: &CanDataFrame) -> Option<super::Frame> {
        let ty_can_id = TyCanId(frame.raw_id());
        let is_csp = ty_can_id.get_flag();
        if is_csp {
            return None;
        }
        let idx = ty_can_id.get_pid();
        let frame_type = TyCanProtocolFrameType::try_from(ty_can_id.get_frame_type()).unwrap();

        let frame = match frame_type {
            TyCanProtocolFrameType::Single => {
                let len = frame.len();
                let meta = super::FrameMeta{
                    id: idx,
                    len : len as u8,
                    flag : super::FrameFlag::empty()
                };
                Some(super::Frame::new(meta,&frame.data()[..len]))
            },
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
                    let meta = super::FrameMeta{
                        id: idx,
                        len: slot.total_len,
                        flag : super::FrameFlag::empty()
                    };
                    Some(super::Frame::new(meta,&slot.data))
                } else {
                    None
                }
            }
            TyCanProtocolFrameType::TimeBroadcast => {
                let meta = super::FrameMeta{
                    id: idx,
                    len : 8,
                    flag : super::FrameFlag::CanTimeBroadcast
                };
                Some(super::Frame::new(meta,&frame.data()[..8]))
            }
            _ => None,
        };
        frame
    }

    fn send(&mut self,frame: super::Frame){
        // frame.meta
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_ty_protocol() {}

    fn test_slot() {

    }
}

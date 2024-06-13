use crate::adaptor::DeviceAdaptorError;

use super::super::{Frame as TcspFrame, FrameFlag, FrameMeta};
use bitfield::bitfield;
use futures_util::StreamExt;
use num_enum::TryFromPrimitive;
use socketcan::{
    tokio::AsyncCanSocket, CanDataFrame, CanFilter, CanFrame, CanSocket, EmbeddedFrame, ExtendedId,
    Frame, SocketOptions,
};
use std::sync::atomic::AtomicU8;
use std::{
    io::{self, ErrorKind},
    mem::size_of,
};

use super::slot::Slot;

const RECV_BUF_SLOT_NUM: usize = 128; // 8bits for id
const TOTAL_LEN_IN_DATA_OF_MULTI: usize = 0x2;
const TY_CAN_PROTOCOL_MTU: usize = 150;
const TY_CAN_PROTOCOL_PAYLOAD_MAX_SIZE: usize = TY_CAN_PROTOCOL_MTU - 5;
const TY_CAN_PROTOCOL_SINGLE_FRAME_MAX: usize = 8 - size_of::<TySingleFrameHeader>();
const TY_CAN_PROTOCOL_CAN_FRAME_SIZE: usize = 8;
const TY_CAN_PROTOCOL_TYPE_RESPONSE: u8 = 0x35;
const TY_CAN_ID_FILTER_MASK: u32 = 0x1fe000;
const TY_CAN_ID_OFFSET: usize = 13;

bitfield! {
    struct TyCanId(u32);
    u8;
    pub get_pid, set_pid: 7, 0;
    pub get_is_csp, set_is_csp: 8;
    pub get_frame_type, set_frame_type: 12,9;
    pub get_dest_id, set_dest_id: 20,13;
    pub get_src_id, set_src_id: 28,21;
}

#[repr(C)]
struct TyMultiFrameHeader{
    total_len: u16, //be16
    hdr : TySingleFrameHeader,
}

#[repr(C)]
struct TySingleFrameHeader{
    type_: u8,
    id : u8
}
impl TySingleFrameHeader{
    fn read_mut(buf : &mut [u8]) -> Option<&'static mut Self>{
        if buf.len() < size_of::<Self>(){
            return None;
        }
        Some(unsafe { &mut *(buf.as_mut_ptr() as *mut Self) })
    }

    fn read(buf : & [u8]) -> Option<&'static  Self>{
        if buf.len() < size_of::<Self>(){
            return None;
        }
        Some(unsafe { & *(buf.as_ptr() as *const Self) })
    }

    fn type_(&self) -> u8{
        self.type_
    }

    fn id(&self) -> u8{
        self.id
    }

    fn set_id(&mut self,id:u8){
        self.id = id;
    }

    fn set_type(&mut self,type_:u8){
        self.type_ = type_;
    }
}
impl TyMultiFrameHeader{
    fn read_mut(buf : &mut [u8]) -> Option<&'static mut Self>{
        if buf.len() < size_of::<Self>(){
            return None;
        }
        Some(unsafe { &mut *(buf.as_mut_ptr() as *mut Self) })
    }

    fn read(buf : & [u8]) -> Option<&'static  Self>{
        if buf.len() < size_of::<Self>(){
            return None;
        }
        Some(unsafe { & *(buf.as_ptr() as *const Self) })
    }

    fn total_len(&self) -> u16{
        // the total len is a be16, we need to reverse it again
        self.total_len.to_be()
    }

    fn type_(&self) -> u8{
        self.hdr.type_
    }

    fn id(&self) -> u8{
        self.hdr.id
    }

    fn set_total_len(&mut self,len:u16){
        self.total_len = len.to_be();
    }

    fn set_id(&mut self,id:u8){
        self.hdr.id = id;
    }

    fn set_type(&mut self,type_:u8){
        self.hdr.type_ = type_;
    }
}

/// Tianyi can protocol
pub(crate) struct TyCanProtocol {
    src_id: u8,
    slot_map: [Slot; RECV_BUF_SLOT_NUM], // 20KB
    socket_rx: AsyncCanSocket<CanSocket>,
    socket_tx: AsyncCanSocket<CanSocket>,
    id_counter: AtomicU8,
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
    pub(crate) fn new(id: u8, socket_rx_name: &str, socket_tx_name: &str) -> io::Result<Self> {
        let socket_rx = AsyncCanSocket::open(socket_rx_name)?;
        let socket_tx = AsyncCanSocket::open(socket_tx_name)?;
        socket_rx.set_filters(&[CanFilter::new(
            (id as u32) << TY_CAN_ID_OFFSET,
            TY_CAN_ID_FILTER_MASK,
        )])?;
        Ok(Self {
            src_id: id,
            slot_map: [Slot::default(); RECV_BUF_SLOT_NUM],
            socket_rx,
            socket_tx,
            id_counter: AtomicU8::new(0),
        })
    }

    pub(crate) async fn recv(&mut self) -> Result<TcspFrame, DeviceAdaptorError> {
        if let Some(Ok(CanFrame::Data(frame))) = self.socket_rx.next().await {
            recv(&mut self.slot_map, &frame).ok_or(DeviceAdaptorError::Empty)
        } else {
            Err(DeviceAdaptorError::Empty)
        }
    }

    pub(crate) async fn send(&mut self, mut frame: TcspFrame) -> Result<(), DeviceAdaptorError> {
        let len = frame.meta.len;
        let mut new_id = TyCanId(0);
        new_id.set_src_id(self.src_id);
        new_id.set_dest_id(frame.meta.dest_id);
        new_id.set_is_csp(false);
        if len > TY_CAN_PROTOCOL_MTU as u16 {
            return Err(DeviceAdaptorError::FrameError("invalid length".to_owned()));
        }
        let id = self
            .id_counter
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        new_id.set_pid(id);
        if len <= TY_CAN_PROTOCOL_SINGLE_FRAME_MAX as u16 {
            let can_frame = CanFrame::new(
                ExtendedId::new(new_id.0).unwrap(),
                &frame.data()[0..len.into()],
            )
            .unwrap();
            attach_single_frame_hdr(&mut frame);
            self.socket_tx
                .write_frame(can_frame)?
                .await?;
        } else {
            // attach meta
            let mut remain: i32 = len.into();
            let mut offset: usize = 0;
            attach_multi_frame_hdr_and_checksum(&mut frame);

            // first packet
            new_id.set_frame_type(TyCanProtocolFrameType::MultiFirst as u8);
            let can_frame = CanFrame::new(
                ExtendedId::new(new_id.0).unwrap(),
                &frame.data()[0..TY_CAN_PROTOCOL_CAN_FRAME_SIZE],
            )
            .unwrap();
            self.socket_tx.write_frame(can_frame)?.await?;
            remain -= TY_CAN_PROTOCOL_CAN_FRAME_SIZE as i32;
            offset += TY_CAN_PROTOCOL_CAN_FRAME_SIZE;

            // middle packet
            new_id.set_frame_type(TyCanProtocolFrameType::MultiMiddle as u8);

            while remain > 0 {
                let this_len = if remain > TY_CAN_PROTOCOL_CAN_FRAME_SIZE as i32 {
                    TY_CAN_PROTOCOL_CAN_FRAME_SIZE as i32
                } else {
                    remain
                };
                let can_frame = CanFrame::new(
                    ExtendedId::new(new_id.0).unwrap(),
                    &frame.data()[offset..offset + this_len as usize],
                )
                .unwrap();

                self.socket_tx.write_frame(can_frame)?.await?;
                remain -= this_len;
                offset += this_len as usize;
            }
        }
        Ok(())
    }
}

fn attach_single_frame_hdr(frame:&mut TcspFrame) {
    frame.expand_head(size_of::<TySingleFrameHeader>()).unwrap();
    let hdr = TySingleFrameHeader::read_mut(frame.data_mut()).unwrap();
    hdr.set_id(frame.meta.id);
    hdr.set_type(TY_CAN_PROTOCOL_TYPE_RESPONSE);
}

fn attach_multi_frame_hdr_and_checksum(frame:&mut TcspFrame) {
    let len = frame.len() as u16;
    frame.expand_head(size_of::<TyMultiFrameHeader>()).unwrap();
    let hdr = TyMultiFrameHeader::read_mut(frame.data_mut()).unwrap();
    hdr.set_id(frame.meta.id);
    hdr.set_type(TY_CAN_PROTOCOL_TYPE_RESPONSE);
    hdr.set_total_len(len);
    let cs = checksum(frame.data());
    frame.expand_tail(1);
    frame.data_mut()[len as usize + size_of::<TyMultiFrameHeader>()] = cs;
}
fn checksum(buf : &[u8]) -> u8{
    let mut sum: u8 = 0;
    for b in buf.iter(){
        sum = sum.wrapping_add(*b);
    }
    sum
}

fn recv(slot_map: &mut [Slot; RECV_BUF_SLOT_NUM], frame: &CanDataFrame) -> Option<TcspFrame> {
    let ty_can_id = TyCanId(frame.raw_id());
    let is_csp = ty_can_id.get_is_csp();
    if is_csp {
        return None;
    }
    let idx = ty_can_id.get_pid();
    let frame_type = TyCanProtocolFrameType::try_from(ty_can_id.get_frame_type())
        .unwrap_or(TyCanProtocolFrameType::Unknown);
    let src_id = ty_can_id.get_src_id();
    let dest_id = ty_can_id.get_dest_id();
    let frame = match frame_type {
        TyCanProtocolFrameType::Single => {
            let len = frame.len();
            if let Some(hdr) = TySingleFrameHeader::read(&frame.data()){
                // hdr.
                let meta = FrameMeta {
                    src_id,
                    dest_id,
                    id : idx,
                    len: len as u16,
                    flag: FrameFlag::empty(),
                };
            }

            // Some(TcspFrame::new(meta, &frame.data()[..len]))
            None
        }
        TyCanProtocolFrameType::MultiFirst => {
            let slot = &mut slot_map[idx as usize];
            let _ = slot.copy_from_slice(frame.data());
            // let total_len = u16::from_be_bytes(&frame.data()[0..TOTAL_LEN_IN_DATA_OF_MULTI]);

            // slot.set_total_len(total_len as u8).unwrap();
            None
        }
        TyCanProtocolFrameType::MultiMiddle => {
            let slot = &mut slot_map[idx as usize];
            let _ = slot.copy_from_slice(frame.data());

            if slot.is_complete() {
                let meta = FrameMeta {
                    src_id,
                    dest_id,
                    id : idx,
                    len: slot.total_len(),
                    flag: FrameFlag::empty(),
                };
                Some(TcspFrame::new(meta, &slot.data()))
            } else {
                None
            }
        }
        TyCanProtocolFrameType::TimeBroadcast => {
            let meta = FrameMeta {
                src_id,
                dest_id,
                len: 8,
                id : idx,
                flag: FrameFlag::CanTimeBroadcast,
            };
            Some(TcspFrame::new(meta, &frame.data()[..8]))
        }
        _ => None,
    };
    frame
}

#[cfg(test)]
mod tests {
    use socketcan::{CanDataFrame, EmbeddedFrame, ExtendedId};

    use crate::adaptor::can::ty::{TY_CAN_ID_FILTER_MASK, TY_CAN_ID_OFFSET};

    use super::{TyCanId, TyCanProtocolFrameType};

    #[test]
    fn test_typeid() {
        let mut id = TyCanId(0);
        id.set_src_id(0x00);
        id.set_dest_id(0x2a);
        id.set_frame_type(TyCanProtocolFrameType::Single as u8);
        id.set_is_csp(false);
        id.set_pid(0x12);
        assert_eq!(id.0, 0x54212);

        let mut id = TyCanId(0);
        id.set_src_id(0x2a);
        id.set_dest_id(0);
        id.set_frame_type(TyCanProtocolFrameType::MultiFirst as u8);
        id.set_is_csp(false);
        id.set_pid(0x56);
        assert_eq!(id.0, 0x5400456);

        let mut id = TyCanId(0);
        id.set_src_id(0x2a);
        id.set_dest_id(0);
        id.set_frame_type(TyCanProtocolFrameType::MultiMiddle as u8);
        id.set_is_csp(false);
        id.set_pid(0x56);
        assert_eq!(id.0, 0x5400656);
        assert_eq!(TY_CAN_ID_FILTER_MASK & 0x54212, 0x2a << TY_CAN_ID_OFFSET)
    }

    #[test]
    fn test_ty_protocol_recv() {
        let mut id = TyCanId(0);
        id.set_src_id(0);
        id.set_dest_id(0x2a);
        id.set_frame_type(TyCanProtocolFrameType::Single as u8);
        id.set_is_csp(false);
        id.set_pid(0x56);
        let can_id = ExtendedId::new(id.0).unwrap();
        let data = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let frame : CanDataFrame = CanDataFrame::new(can_id, &data).unwrap();
        let mut slot_map = [super::super::slot::Slot::default(); super::RECV_BUF_SLOT_NUM];
        let frame = super::recv(&mut slot_map, &frame).unwrap();
        assert_eq!(frame.len(),8);
        assert_eq!(frame.meta.src_id,0);
        assert_eq!(frame.meta.dest_id,0x2a);
        assert_eq!(&frame.data()[0..frame.len()],&data);

        let mut id = TyCanId(0);
        id.set_src_id(0);
        id.set_dest_id(0x2a);
        id.set_frame_type(TyCanProtocolFrameType::MultiFirst as u8);
        id.set_is_csp(false);
        id.set_pid(0x56);
        let data = (1..140).collect::<Vec<u8>>();
        // let data_remain= [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x];
        let frame : CanDataFrame = CanDataFrame::new(can_id, &data).unwrap();
        let frame = super::recv(&mut slot_map, &frame).is_none();


    }

    #[test]
    fn test_ty_protocol_send() {
        
    }
}

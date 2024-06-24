use crate::adaptor::{DeviceAdaptor, DeviceAdaptorError};

use super::super::{Frame as BusFrame, FrameFlag, FrameMeta};
use async_trait::async_trait;
use bitfield::bitfield;
use num_enum::TryFromPrimitive;
use socketcan::{
    tokio::AsyncCanSocket, CanDataFrame, CanFilter, CanFrame, CanSocket, EmbeddedFrame, ExtendedId,
    Frame, SocketOptions,
};
use socketcan::{CanInterface, Socket};
use std::cell::UnsafeCell;
use std::sync::atomic::AtomicU8;
use std::thread::sleep;
use std::time::{Duration, Instant};
use std::{
    io::{self},
    mem::size_of,
};

use super::slot::Slot;

const RECV_BUF_SLOT_NUM: usize = 128; // 8bits for id
const TY_CAN_PROTOCOL_MTU: usize = 150;
const TY_CAN_PROTOCOL_PAYLOAD_MAX_SIZE: usize =
    TY_CAN_PROTOCOL_MTU - size_of::<TyMultiFrameHeader>() - TY_CAN_PROTOCOL_CHECKSUM_SIZE;
const TY_CAN_PROTOCOL_CHECKSUM_SIZE: usize = 1;
const TY_CAN_PROTOCOL_SINGLE_FRAME_MAX: usize = 8 - size_of::<TySingleFrameHeader>();
const TY_CAN_PROTOCOL_CAN_FRAME_SIZE: usize = 8;
const TY_CAN_PROTOCOL_TYPE_RESPONSE: u8 = 0x35;
const TY_CAN_PROTOCOL_TYPE_OBC_COMMAND_REQUEST: u8 = 0x05;
const TY_CAN_PROTOCOL_TYPE_OBC_BROADCAST_REQUEST: u8 = 0x0f;
const TY_CAN_PROTOCOL_UTILITES_SINGLE_REQUEST: u8 = 0x01;
const TY_CAN_PROTOCOL_UTILITES_SINGLE_RESPONSE: u8 = 0x02;
const TY_CAN_PROTOCOL_UTILITES_MULTI_REQUEST: u8 = 0x03;
const TY_CAN_PROTOCOL_UTILITES_MULTI_RESPONSE: u8 = 0x04;

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
struct TyMultiFrameHeader {
    total_len: u16, //be16
    hdr: TySingleFrameHeader,
}

#[repr(C)]
struct TySingleFrameHeader {
    type_: u8,
    utilites: u8,
}
impl TySingleFrameHeader {
    fn read_mut(buf: &mut [u8]) -> Option<&'static mut Self> {
        if buf.len() < size_of::<Self>() {
            return None;
        }
        Some(unsafe { &mut *(buf.as_mut_ptr() as *mut Self) })
    }

    fn read(buf: &[u8]) -> Option<&'static Self> {
        if buf.len() < size_of::<Self>() {
            return None;
        }
        Some(unsafe { &*(buf.as_ptr() as *const Self) })
    }

    fn type_(&self) -> u8 {
        self.type_
    }

    fn utilites(&self) -> u8 {
        self.utilites
    }

    fn set_utilites(&mut self, utilites: u8) {
        self.utilites = utilites;
    }

    fn set_type(&mut self, type_: u8) {
        self.type_ = type_;
    }

    fn check_valid(&self) -> bool {
        self.utilites() == TY_CAN_PROTOCOL_UTILITES_SINGLE_REQUEST
            && (self.type_() == TY_CAN_PROTOCOL_TYPE_OBC_COMMAND_REQUEST
                || self.type_() == TY_CAN_PROTOCOL_TYPE_OBC_BROADCAST_REQUEST)
    }
}
impl TyMultiFrameHeader {
    fn read_mut(buf: &mut [u8]) -> Option<&'static mut Self> {
        if buf.len() < size_of::<Self>() {
            return None;
        }
        Some(unsafe { &mut *(buf.as_mut_ptr() as *mut Self) })
    }

    fn read(buf: &[u8]) -> Option<&'static Self> {
        if buf.len() < size_of::<Self>() {
            return None;
        }
        Some(unsafe { &*(buf.as_ptr() as *const Self) })
    }

    fn total_len(&self) -> u16 {
        // the total len is a be16, we need to reverse it again
        self.total_len.to_be()
    }

    fn type_(&self) -> u8 {
        self.hdr.type_
    }

    fn utilites(&self) -> u8 {
        self.hdr.utilites
    }

    fn set_total_len(&mut self, len: u16) {
        self.total_len = len.to_be();
    }

    fn set_utilites(&mut self, utilites: u8) {
        self.hdr.utilites = utilites;
    }

    fn set_type(&mut self, type_: u8) {
        self.hdr.type_ = type_;
    }

    fn check_valid(&self) -> bool {
        self.hdr.utilites() == TY_CAN_PROTOCOL_UTILITES_MULTI_REQUEST
            && (self.hdr.type_() == TY_CAN_PROTOCOL_TYPE_OBC_COMMAND_REQUEST
                || self.hdr.type_() == TY_CAN_PROTOCOL_TYPE_OBC_BROADCAST_REQUEST)
    }
}

/// Tianyi can protocol
pub struct TyCanProtocol {
    src_id: u8,
    slot_map: RecvBuf, // 20KB
    socket_rx: CanSocket,
    socket_tx: AsyncCanSocket<CanSocket>,
    socket_rx_name: String,
    socket_tx_name: String,
    id_counter: AtomicU8,
}

/// Safety: Receive packets are all in order. Only one can frame is received simultaneously.
struct RecvBuf {
    buf: UnsafeCell<[Slot; RECV_BUF_SLOT_NUM]>,
}

unsafe impl Send for RecvBuf {}
unsafe impl Sync for RecvBuf {}

impl Default for RecvBuf {
    fn default() -> Self {
        Self {
            buf: UnsafeCell::new([Slot::default(); RECV_BUF_SLOT_NUM]),
        }
    }
}
impl RecvBuf {
    #[allow(clippy::mut_from_ref)]
    unsafe fn get_mut_unchecked(&self, idx: usize) -> &mut Slot {
        let buf = unsafe { &mut *self.buf.get() };
        &mut buf[idx]
    }
}
#[derive(TryFromPrimitive, Debug)]
#[repr(u8)]
enum TyCanProtocolFrameType {
    Reset = 0b0000,
    Single = 0b0001,
    MultiFirst = 0b0010,
    MultiMiddle = 0b0011,
    TimeBroadcast = 0b0100,

    Unknown = 0b1111,
}

#[async_trait]
impl DeviceAdaptor for TyCanProtocol {
    async fn recv(&self) -> Result<BusFrame, DeviceAdaptorError> {
        if let Ok(frame) = self.socket_rx.read_frame() {
            match frame {
                CanFrame::Data(data_frame) => {
                    let ty_can_id = TyCanId(data_frame.raw_id());
                    let frame_type = TyCanProtocolFrameType::try_from(ty_can_id.get_frame_type())
                        .unwrap_or(TyCanProtocolFrameType::Unknown);
                    if matches!(frame_type, TyCanProtocolFrameType::Reset) {
                        if let Err(e) = self.restart().await {
                            log::error!("restart failed:{:?}", e);
                        }
                    } else {
                        match recv(&self.slot_map, &data_frame, self.src_id) {
                            Ok(option_frame) => {
                                if let Some(bus_frame) = option_frame {
                                    return Ok(bus_frame);
                                }
                            }
                            Err(e) => {
                                log::error!("{}", e);
                            }
                        }
                    }
                }
                CanFrame::Error(error_frame) => {
                    log::info!("{:?}", error_frame);
                }
                _ => {}
            }
        }
        Err(DeviceAdaptorError::Empty)
    }

    async fn send(&self, mut frame: BusFrame) -> Result<(), DeviceAdaptorError> {
        let len = frame.meta.len;
        let mut new_id = TyCanId(0);
        new_id.set_src_id(self.src_id);
        new_id.set_dest_id(frame.meta.dest_id);
        new_id.set_is_csp(false);
        if len > TY_CAN_PROTOCOL_PAYLOAD_MAX_SIZE as u16 {
            return Err(DeviceAdaptorError::FrameError("invalid length".to_owned()));
        }
        let id = self
            .id_counter
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        new_id.set_pid(id);
        if len <= TY_CAN_PROTOCOL_SINGLE_FRAME_MAX as u16 {
            attach_single_frame_hdr(&mut frame)?;
            let new_len = frame.len();
            #[allow(clippy::unwrap_used)]
            let can_frame = CanFrame::new(
                ExtendedId::new(new_id.0).unwrap(),
                &frame.data()[0..new_len],
            )
            .unwrap();
            self.socket_tx.write_frame(can_frame)?.await?;
        } else {
            // attach meta
            attach_multi_frame_hdr_and_checksum(&mut frame)?;
            let mut remain: i32 = frame.meta.len.into();
            let mut offset: usize = 0;

            // first packet
            new_id.set_frame_type(TyCanProtocolFrameType::MultiFirst as u8);
            #[allow(clippy::unwrap_used)]
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
                #[allow(clippy::unwrap_used)]
                let next_can_frame = CanFrame::new(
                    ExtendedId::new(new_id.0).unwrap(),
                    &frame.data()[offset..offset + this_len as usize],
                )
                .unwrap();

                self.socket_tx.write_frame(next_can_frame)?.await?;
                remain -= this_len;
                offset += this_len as usize;
            }
        }
        Ok(())
    }

    fn mtu(&self, _flag: FrameFlag) -> usize {
        TY_CAN_PROTOCOL_PAYLOAD_MAX_SIZE
    }
}

impl TyCanProtocol {
    pub(crate) fn new(id: u8, socket_rx_name: &str, socket_tx_name: &str) -> io::Result<Self> {
        let socket_rx = CanSocket::open(socket_rx_name)?;
        let socket_tx = AsyncCanSocket::open(socket_tx_name)?;
        socket_rx.set_filters(&[CanFilter::new(
            (id as u32) << TY_CAN_ID_OFFSET,
            TY_CAN_ID_FILTER_MASK,
        )])?;
        log::debug!(
            "socket rx = {}, socket tx= {},filter = {}",
            socket_rx_name,
            socket_tx_name,
            (id as u32) << TY_CAN_ID_OFFSET
        );
        Ok(Self {
            src_id: id,
            slot_map: RecvBuf::default(),
            socket_rx,
            socket_tx,
            socket_rx_name: socket_rx_name.to_owned(),
            socket_tx_name: socket_tx_name.to_owned(),
            id_counter: AtomicU8::new(0),
        })
    }

    async fn restart(&self) -> io::Result<()> {
        log::info!("CAN socket restart");
        self.id_counter
            .store(0, std::sync::atomic::Ordering::Release);
        let rx_interface = CanInterface::open(&self.socket_rx_name)?;
        rx_interface
            .bring_down()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?;
        let tx_interface = CanInterface::open(&self.socket_tx_name)?;
        tx_interface
            .bring_down()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?;
        rx_interface
            .bring_up()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?;
        tx_interface
            .bring_up()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?;
        self.loopback_testing(Duration::from_secs(10)).await;
        log::info!("CAN socket reset done");
        Ok(())
    }

    async fn loopback_testing(&self, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            if (self.loopback_testing_inner().await).is_err() {
                sleep(Duration::from_secs(1)); // block all threads.
            } else {
                return true;
            }
            if start.elapsed() > timeout {
                return false;
            }
        }
    }
    async fn loopback_testing_inner(&self) -> Result<(), DeviceAdaptorError> {
        let mut frame = BusFrame::default();
        frame.meta.len = 8;
        frame.expand_head(8)?;
        let mut new_id = TyCanId(0);
        new_id.set_pid(0);
        new_id.set_frame_type(TyCanProtocolFrameType::Unknown as u8);
        new_id.set_src_id(self.src_id);
        new_id.set_dest_id(frame.meta.src_id);
        new_id.set_is_csp(false);
        #[allow(clippy::unwrap_used)]
        let can_frame =
            CanFrame::new(ExtendedId::new(new_id.0).unwrap(), &frame.data()[0..8]).unwrap();
        self.socket_tx.write_frame(can_frame)?.await?;
        Ok(())
    }
}

fn attach_single_frame_hdr(frame: &mut BusFrame) -> io::Result<usize> {
    frame.expand_head(size_of::<TySingleFrameHeader>())?;
    #[allow(clippy::unwrap_used)]
    let hdr = TySingleFrameHeader::read_mut(frame.data_mut()).unwrap();
    hdr.set_utilites(TY_CAN_PROTOCOL_UTILITES_SINGLE_RESPONSE);
    hdr.set_type(TY_CAN_PROTOCOL_TYPE_RESPONSE);
    Ok(size_of::<TySingleFrameHeader>())
}

fn attach_multi_frame_hdr_and_checksum(frame: &mut BusFrame) -> io::Result<usize> {
    let len = frame.len() as u16;
    frame.expand_head(size_of::<TyMultiFrameHeader>())?;
    #[allow(clippy::unwrap_used)]
    let hdr = TyMultiFrameHeader::read_mut(frame.data_mut()).unwrap();
    hdr.set_utilites(TY_CAN_PROTOCOL_UTILITES_MULTI_RESPONSE);
    hdr.set_type(TY_CAN_PROTOCOL_TYPE_RESPONSE);
    // 2 includes type_(1B) and utilites(1B)
    hdr.set_total_len(len + 2);
    let cs = get_checksum(frame.data());
    frame.expand_tail(1)?;
    frame.data_mut()[len as usize + size_of::<TyMultiFrameHeader>()] = cs;
    Ok(size_of::<TyMultiFrameHeader>() + size_of::<TyMultiFrameHeader>())
}
fn get_checksum(buf: &[u8]) -> u8 {
    let mut sum: u8 = 0;
    for b in buf.iter() {
        sum = sum.wrapping_add(*b);
    }
    sum
}

fn recv(slot_map: &RecvBuf, frame: &CanDataFrame, self_id: u8) -> io::Result<Option<BusFrame>> {
    let ty_can_id = TyCanId(frame.raw_id());
    let is_csp = ty_can_id.get_is_csp();
    let src_id = ty_can_id.get_src_id();
    if src_id == self_id {
        return Ok(None);
    }
    if is_csp {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "can not handle csp packet",
        ))?;
    }
    let idx = ty_can_id.get_pid();
    let frame_type = TyCanProtocolFrameType::try_from(ty_can_id.get_frame_type())
        .unwrap_or(TyCanProtocolFrameType::Unknown);
    let dest_id = ty_can_id.get_dest_id();
    let len = frame.len();
    log::info!("receive pkt,type={:?}", frame_type);
    match frame_type {
        TyCanProtocolFrameType::Single => {
            if let Some(hdr) = TySingleFrameHeader::read(frame.data()) {
                if hdr.check_valid() {
                    let meta = FrameMeta {
                        src_id,
                        dest_id,
                        id: idx,
                        len: (len - size_of::<TySingleFrameHeader>()) as u16,
                        flag: FrameFlag::empty(),
                        ..Default::default()
                    };
                    // the single frame data will not exceed length
                    return BusFrame::new(
                        meta,
                        &frame.data()[size_of::<TySingleFrameHeader>()..len],
                    )
                    .map(Some);
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "receive packet,but utilites={:2x},type={:2x}",
                            hdr.utilites(),
                            hdr.type_()
                        ),
                    ))?;
                }
            }
        }
        TyCanProtocolFrameType::MultiFirst => {
            if let Some(hdr) = TyMultiFrameHeader::read(frame.data()) {
                if hdr.check_valid() {
                    if hdr.total_len() < TY_CAN_PROTOCOL_CAN_FRAME_SIZE as u16 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "multi frame total len is too small",
                        ));
                    }
                    let slot = unsafe { slot_map.get_mut_unchecked(idx.into()) };
                    slot.reset();
                    // 3 include total_len(2B) and checksum(1B)
                    slot.set_total_len(hdr.total_len() + 3)?;
                    slot.copy_from_slice(frame.data())?;
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "receive packet,but utilites={:2x},type={:2x}",
                            hdr.utilites(),
                            hdr.type_()
                        ),
                    ));
                }
            }
        }
        TyCanProtocolFrameType::MultiMiddle => {
            let slot = unsafe { slot_map.get_mut_unchecked(idx.into()) };
            slot.copy_from_slice(frame.data())?;
            if slot.is_complete() {
                // check checksum
                let total_len = slot.total_len();
                let checksum = get_checksum(&slot.data()[..(total_len - 1) as usize]);
                let result = if checksum == slot.data()[total_len as usize - 1] {
                    let meta = FrameMeta {
                        src_id,
                        dest_id,
                        id: idx,
                        len: (total_len as usize - size_of::<TyMultiFrameHeader>() - 1) as u16,
                        flag: FrameFlag::empty(),
                        ..Default::default()
                    };
                    // we have checked previously in `copy_from_slice` that the buffer is not too large
                    BusFrame::new(
                        meta,
                        &slot.data()[size_of::<TyMultiFrameHeader>()..total_len as usize - 1],
                    )
                    .map(Some)
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("id={:?} checksum failed,expect {:?}", idx, checksum),
                    ))
                };
                slot.reset();
                return result;
            }
        }
        TyCanProtocolFrameType::TimeBroadcast => {
            if let Some(buf) = frame.data().get(..8) {
                // Expect data like 5005XXXXXXXXYY00
                // the single frame data will not exceed length
                if buf[0] == 0x50 && buf[1] == 0x05 && buf[7] == 0x00 {
                    let meta = FrameMeta {
                        src_id,
                        dest_id,
                        len: 4,
                        id: idx,
                        flag: FrameFlag::CanTimeBroadcast,
                        ..Default::default()
                    };
                    return BusFrame::new(meta, &frame.data()[..8]).map(Some);
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "receive time broadcast packet,but data is invalid:{:?}",
                            buf
                        ),
                    ))?
                }
            };
        }
        TyCanProtocolFrameType::Reset => {}
        _ => {
            log::error!(
                "receive packet,but frame type is invalid:{:?}",
                ty_can_id.get_frame_type()
            );
        }
    };
    Ok(None)
}

#[cfg(test)]
mod tests {
    use socketcan::{CanDataFrame, EmbeddedFrame, ExtendedId};

    use crate::adaptor::{
        can::ty::{
            attach_multi_frame_hdr_and_checksum, RecvBuf, TY_CAN_ID_FILTER_MASK, TY_CAN_ID_OFFSET,
            TY_CAN_PROTOCOL_TYPE_OBC_COMMAND_REQUEST, TY_CAN_PROTOCOL_TYPE_RESPONSE,
            TY_CAN_PROTOCOL_UTILITES_MULTI_REQUEST, TY_CAN_PROTOCOL_UTILITES_MULTI_RESPONSE,
            TY_CAN_PROTOCOL_UTILITES_SINGLE_REQUEST, TY_CAN_PROTOCOL_UTILITES_SINGLE_RESPONSE,
        },
        Frame, FrameFlag, FrameMeta,
    };

    use super::{attach_single_frame_hdr, TyCanId, TyCanProtocolFrameType};

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
        assert_eq!(TY_CAN_ID_FILTER_MASK & 0x54212, 0x2a << TY_CAN_ID_OFFSET);

        let mut id = TyCanId(0);
        id.set_src_id(0);
        id.set_dest_id(0x43);
        id.set_frame_type(TyCanProtocolFrameType::Reset as u8);
        id.set_is_csp(false);
        id.set_pid(0x0);
        println!("{:2x}", id.0);
    }

    #[test]
    fn test_ty_protocol_recv() {
        let mut id = TyCanId(0);
        id.set_src_id(0);
        id.set_dest_id(0x2a);
        id.set_frame_type(TyCanProtocolFrameType::Single as u8);
        id.set_is_csp(false);
        id.set_pid(0x12);
        let can_id = ExtendedId::new(id.0).unwrap();
        let data = [
            TY_CAN_PROTOCOL_TYPE_OBC_COMMAND_REQUEST,
            TY_CAN_PROTOCOL_UTILITES_SINGLE_REQUEST,
            0x03,
            0x04,
            0x05,
            0x06,
            0x07,
            0x08,
        ];
        let frame: CanDataFrame = CanDataFrame::new(can_id, &data).unwrap();
        let slot_map = RecvBuf::default();
        let frame = super::recv(&slot_map, &frame, 0x2a).unwrap().unwrap();
        assert_eq!(frame.len(), 6);
        assert_eq!(frame.meta.src_id, 0);
        assert_eq!(frame.meta.dest_id, 0x2a);
        assert_eq!(&frame.data()[..frame.len()], &data[2..]);

        // test recv multi frame
        let mut id = TyCanId(0);
        id.set_src_id(0);
        id.set_dest_id(0x2a);
        id.set_frame_type(TyCanProtocolFrameType::MultiFirst as u8);
        id.set_is_csp(false);
        id.set_pid(0x20);
        let first_can_id = ExtendedId::new(id.0).unwrap();
        id.set_frame_type(TyCanProtocolFrameType::MultiMiddle as u8);
        let rest_can_id = ExtendedId::new(id.0).unwrap();
        let data = [
            0,
            0x24_u8,
            TY_CAN_PROTOCOL_TYPE_OBC_COMMAND_REQUEST,
            TY_CAN_PROTOCOL_UTILITES_MULTI_REQUEST,
        ]
        .into_iter()
        .chain(1..=34)
        .chain(std::iter::once(127))
        .collect::<Vec<u8>>();
        let frame = CanDataFrame::new(first_can_id, &data[0..8]).unwrap();
        assert!(super::recv(&slot_map, &frame, 0x2a).unwrap().is_none());
        let frame = CanDataFrame::new(rest_can_id, &data[8..16]).unwrap();
        assert!(super::recv(&slot_map, &frame, 0x2a).unwrap().is_none());
        let frame = CanDataFrame::new(rest_can_id, &data[16..24]).unwrap();
        assert!(super::recv(&slot_map, &frame, 0x2a).unwrap().is_none());
        let frame = CanDataFrame::new(rest_can_id, &data[24..32]).unwrap();
        assert!(super::recv(&slot_map, &frame, 0x2a).unwrap().is_none());
        let frame: CanDataFrame = CanDataFrame::new(rest_can_id, &data[32..39]).unwrap();
        let frame = super::recv(&slot_map, &frame, 0x2a).unwrap().unwrap();
        assert_eq!(frame.meta.len, 39 - 4 - 1);
        assert_eq!(frame.meta.src_id, 0);
        assert_eq!(frame.meta.dest_id, 0x2a);
        assert_eq!(&frame.data()[..frame.len()], &data[4..38]);
    }

    #[test]
    fn test_ty_protocol_send() {
        let data = [1, 2, 3, 4, 5, 6];
        let mut tf = Frame::new(
            FrameMeta {
                src_id: 0,
                dest_id: 0x2a,
                id: 0x12,
                len: 6,
                flag: FrameFlag::empty(),
                ..Default::default()
            },
            &data,
        )
        .unwrap();
        attach_single_frame_hdr(&mut tf).unwrap();
        assert_eq!(tf.data()[0], TY_CAN_PROTOCOL_TYPE_RESPONSE);
        assert_eq!(tf.data()[1], TY_CAN_PROTOCOL_UTILITES_SINGLE_RESPONSE);
        assert_eq!(tf.meta.len, 8);

        let data: [u8; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let mut tf2 = Frame::new(FrameMeta::default(), &data).unwrap();
        attach_multi_frame_hdr_and_checksum(&mut tf2).unwrap();
        assert_eq!(tf2.data()[0], 0);
        assert_eq!(tf2.data()[1] as usize, data.len() + 2);
        assert_eq!(tf2.data()[2], TY_CAN_PROTOCOL_TYPE_RESPONSE);
        assert_eq!(tf2.data()[3], TY_CAN_PROTOCOL_UTILITES_MULTI_RESPONSE);
        assert_eq!(tf2.meta.len as usize, data.len() + 4 + 1);
    }
}

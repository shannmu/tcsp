use std::time::Duration;

use async_trait::async_trait;
use bitfield::bitfield;
use tokio::{sync::Mutex, time::timeout};

use super::{Application, Fallback, Frame};

const MAX_UDP_TRANSACATION_ON_FLIGHT: usize = 4;

#[derive(Clone, Copy)]
struct UdpControlCommandContext {
    current_length: u16,
    is_avaliable: bool,
    data: [u8; 150],
}

impl Default for UdpControlCommandContext {
    fn default() -> Self {
        Self {
            current_length: Default::default(),
            is_avaliable: Default::default(),
            data: [0u8; 150],
        }
    }
}

pub struct UdpControl<F> {
    fallback: F,
    buffer: Mutex<Box<[UdpControlCommandContext; MAX_UDP_TRANSACATION_ON_FLIGHT]>>,
}

bitfield! {
    #[derive(Clone, Copy)]
    struct Seq(u8);
    u8;
    pub get_seq, set_seq: 6, 0;
    pub is_last, set_last: 7;
}

// #[repr(C)]
// struct UdpControlCommandHeader {
//     seq: Seq,
// }

#[async_trait]
impl<F: Fallback> Application for UdpControl<F> {
    async fn handle(&self, frame: Frame, _mtu: u16) -> std::io::Result<Option<Frame>> {
        let seq_struct = Seq(frame.data()[0]);
        let seq = seq_struct.get_seq() % MAX_UDP_TRANSACATION_ON_FLIGHT as u8;
        let is_last = seq_struct.is_last();
        let payload = &frame.data()[1..];

        let mut guard = self.buffer.lock().await;
        #[allow(clippy::unwrap_used)]
        let buf = guard.get_mut(seq as usize).unwrap();
        if !buf.is_avaliable {
            // first slice
            buf.data[..payload.len()].copy_from_slice(payload);
            buf.current_length = payload.len() as u16;
            buf.is_avaliable = true;
        } else if is_last {
            // last slice
            let mut msg: Vec<u8> = vec![54, 48, 48, 48, 50];
            msg.extend_from_slice(&buf.data[..buf.current_length as usize]);
            msg.extend_from_slice(payload);
            buf.is_avaliable = false;
            let send_future = self.fallback.fallback(msg);
            let _reply = timeout(Duration::from_millis(100), send_future).await??;
        } else {
            // TODO: use `bytes` instead
            let mut msg: Vec<u8> = vec![54, 48, 48, 48, 50];
            msg.extend_from_slice(&buf.data[..buf.current_length as usize]);
            let payload_len = payload.len();
            let first_part = 150 - buf.current_length as usize;
            let second_part = payload_len - first_part;
            // let payload_length = payload.
            msg.extend_from_slice(&payload[..first_part]);

            buf.data[..second_part].copy_from_slice(&payload[first_part..]);
            // buf.current_length = buf.current_length;
            let send_future = self.fallback.fallback(msg);
            let _reply = timeout(Duration::from_millis(100), send_future).await??;
        }
        Ok(None)
    }

    fn application_id(&self) -> u8 {
        Self::APPLICATION_ID
    }
}

impl<F> UdpControl<F> {
    pub(crate) const APPLICATION_ID: u8 = 6;

    pub fn generate_request(command: &[u8], mtu: usize) -> Vec<Frame> {
        let last_slice = command.len().div_ceil(mtu - 1);
        let mut data = Vec::new();
        for (i, chunk) in command.chunks(mtu - 1).enumerate() {
            let mut frame = Frame::new(UdpControl::<()>::APPLICATION_ID);
            #[allow(clippy::unwrap_used)]
            frame.set_len((chunk.len() + 1) as u16).unwrap(); // actually 123 bytes of data
            if i == last_slice {
                frame.data_mut()[0] = 1 << 7 | i as u8;
            } else {
                frame.data_mut()[0] = i as u8;
            }
            frame.data_mut()[1..1+chunk.len()].copy_from_slice(chunk);
            data.push(frame)
        }
        data
    }
}

impl<F: Fallback> UdpControl<F> {
    pub fn new(fallback: F) -> Self {
        let buffer = Mutex::new(Box::new(
            [UdpControlCommandContext::default(); MAX_UDP_TRANSACATION_ON_FLIGHT],
        ));
        Self { fallback, buffer }
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use crate::{
        adaptor::send_using_ty_protocol,
        application::{Application, Fallback},
        protocol::Frame,
    };

    use super::UdpControl;

    #[derive(Clone)]
    pub(crate) struct DummyFallback {
        data: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl DummyFallback {
        fn new() -> Self {
            Self {
                data: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl Fallback for DummyFallback {
        async fn fallback(&self, msg: Vec<u8>) -> std::io::Result<Vec<u8>> {
            self.data.lock().await.push(msg);
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn test_separate_udp_control() {
        let dummy = DummyFallback::new();
        let udp = UdpControl::new(dummy.clone());

        // test 150 bytes separate as two parts:
        // first pkt(128bytes): 0xbe 0x01 0x20 0x06 0x00 (with 123bytes data)
        // second pkt(30bytes): 0xbe 0x01 0x20 0x06 0x80 (with 25bytes data)
        let mut frame = Frame::new(UdpControl::<()>::APPLICATION_ID);
        frame.set_len(124).unwrap(); // actually 123 bytes of data
        frame.data_mut()[0] = 0;
        udp.handle(frame, 124).await.unwrap();
        let mut frame2 = Frame::new(UdpControl::<()>::APPLICATION_ID);
        frame2.set_len(30).unwrap(); // actually 28 bytes of data
        frame2.data_mut()[0] = 0x80;
        udp.handle(frame2, 124).await.unwrap();

        assert_eq!(dummy.data.lock().await.len(), 1);

        // test 300 bytes separate as 3 parts:
        // first pkt(128bytes):  0xbe 0x01 0x20 0x06 0x00 (with 123bytes data)
        // second pkt(128bytes): 0xbe 0x01 0x20 0x06 0x00 (with 123bytes data)
        // last  pkt(59bytes):   0xbe 0x01 0x20 0x06 0x80 (with 54bytes data)
        let mut frame = Frame::new(UdpControl::<()>::APPLICATION_ID);
        frame.set_len(124).unwrap(); // actually 123 bytes of data
        frame.data_mut()[0] = 0;
        udp.handle(frame, 124).await.unwrap();
        let mut frame = Frame::new(UdpControl::<()>::APPLICATION_ID);
        frame.set_len(124).unwrap(); // actually 123 bytes of data
        frame.data_mut()[0] = 0;
        udp.handle(frame, 124).await.unwrap();
        assert_eq!(dummy.data.lock().await.len(), 2);

        let mut frame2 = Frame::new(UdpControl::<()>::APPLICATION_ID);
        frame2.set_len(59).unwrap(); // actually 54 bytes of data
        frame2.data_mut()[0] = 0x80;
        udp.handle(frame2, 124).await.unwrap();
        assert_eq!(dummy.data.lock().await.len(), 3);
    }

    #[tokio::test]
    async fn test_udp_control() {
        let data = Mutex::new(Vec::new());
        let frames = UdpControl::<()>::generate_request(&[1u8;150], 147 - 4);
        for (i,frame) in frames.into_iter().enumerate(){
            send_using_ty_protocol(&data, 0x43, i as u8, frame.try_into().unwrap())
            .await
            .unwrap();
        }
        for i in data.lock().await.iter() {
            if let socketcan::CanFrame::Data(d) = i {
                println!("{:?}", d)
            }
        }
    }
}

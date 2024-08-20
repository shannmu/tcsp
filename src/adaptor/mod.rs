use async_trait::async_trait;

mod can;
mod channel;
mod error;
mod frame;
mod uart;

pub use can::ty::TyCanProtocol;
pub(crate) use can::ty::send_using_ty_protocol;
pub use channel::Channel;
pub use error::DeviceAdaptorError;
pub use frame::{Frame, FrameFlag, FrameMeta};
pub use uart::TyUartProtocol;
pub use uart::Uart;

#[async_trait]
pub trait DeviceAdaptor: Send + Sync {
    /// Send a bus frame to the bus
    async fn send(&self, frame: Frame) -> Result<(), DeviceAdaptorError>;

    /// Receive a bus frame from the bus.
    /// You might not receive the entier frame at one call, in this case, `DeviceAdaptorError::Empty` will be returned.
    async fn recv(&self) -> Result<Frame, DeviceAdaptorError>;

    /// The mtu of the bus frame. Typically, the data excced the mtu may discard by adaptor, or the adaptor can return an error.
    ///
    /// Some devices like uart may have different mtu when giving different `FrameFlag`.
    fn mtu(&self, flag: FrameFlag) -> usize;
}

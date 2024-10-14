mod download;
mod echo;
mod reboot;
mod reset_network;
mod telemetry;
mod time_sync;
mod udp_backup;
mod upload;

/// The fallback is an adpter to the restrive adta or send data to the
mod fallback;

use async_trait::async_trait;
pub use echo::EchoCommand;
pub use fallback::{Fallback, ZeromqSocket};
pub use reboot::Reboot;
pub use telemetry::TeleMetry;
pub use time_sync::TimeSync;
pub use udp_backup::UdpBackup;
pub use reset_network::ResetNetwork;

#[cfg(test)]
pub(crate) use fallback::DummyFallback;

use crate::protocol::Frame;

#[async_trait]
pub trait Application: Send + Sync {
    /// TODO: what if the frame is very large? Start a new thread?
    /// Parse the bus frame into an application frame
    async fn handle(&self, frame: Frame, mtu: u16) -> std::io::Result<Option<Frame>>;

    fn application_id(&self) -> u8;

    async fn init(&self) {}
}

mod echo;
mod telemetry;
mod time_sync;

pub use telemetry::TeleMetry;
pub use echo::EchoCommand;
pub use time_sync::TimeSync;

use crate::protocol::Frame;


pub(crate) trait Application: Send + Sync{
    /// TODO: what if the frame is very large? Start a new thread?
    /// Parse the bus frame into an application frame
    fn handle(&self,frame:Frame,mtu:u16) -> std::io::Result<Option<Frame>>;

    fn application_id(&self) -> u8;
}
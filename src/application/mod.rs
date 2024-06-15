mod echo;
mod telemetry;

pub use telemetry::TeleMetry;
pub use echo::EchoCommand;

use crate::protocol::Frame;


pub(crate) trait Application {
    /// TODO: what if the frame is very large? Start a new thread?
    /// Parse the bus frame into an application frame
    fn handle(&self,frame:Frame,mtu:u16) -> std::io::Result<Option<Frame>>;

    fn application_id(&self) -> u8;
}
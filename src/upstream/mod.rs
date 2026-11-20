pub mod rotcl;
pub mod grbltrk;

use std::io::Result;
use crate::helpers::Pos;

pub trait UpstreamRotator {
    fn read_pos(&self) -> Result<Pos>;
    fn send_pos(&self, p: Pos) -> Result<()>;
}

pub use rotcl::Rotctled;
pub use grbltrk::GrblTrker;

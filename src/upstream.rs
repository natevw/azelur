pub mod rotcl;

use std::io::Result;
use crate::helpers::Pos;

pub trait UpstreamRotator {
    fn read_pos(&self) -> Result<Pos>;
    fn send_pos(&self, p: Pos) -> Result<()>;
}

use std::io::{BufRead, BufReader, Result, Write};
use std::net::TcpStream;

use crate::UpstreamRotator;
use crate::helpers::{Pos, parse_pair};

#[derive(Debug, Clone)]
pub struct Rotctled {
    // HACK: we re-connect whenever reading/changing position!
    // TODO: share a single/persistent upstream connection
    addr: String,
}

impl Rotctled {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: addr.into()
        }
    }
}

impl UpstreamRotator for Rotctled {
    fn read_pos(&self) -> Result<Pos> {
        let upstream =  TcpStream::connect(&self.addr)?;
        let incoming = BufReader::new(&upstream);
        let mut lines = incoming.lines();
        writeln!(&upstream, "p")?;
        let pair = vec![
            lines.next().unwrap()?,
            lines.next().unwrap()?,
        ];
        parse_pair(pair.iter().map(String::as_str)).ok_or_else(|| std::io::ErrorKind::InvalidData.into())
    }

    fn send_pos(&self, mut p: Pos) -> Result<()> {
        let upstream =  TcpStream::connect(&self.addr)?;
        p.az %= 360.0;
        if p.az < 0.0 {
            p.az += 360.0;
        }
        eprintln!("upstream az => {}, el => {}", p.az, p.el);
        writeln!(&upstream, "P {:.2} {:.2}", p.az, p.el)?;
        Ok(())
    }
}

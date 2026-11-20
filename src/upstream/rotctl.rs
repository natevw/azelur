use std::io::{BufRead, BufReader, Result, Read, Write};


use crate::UpstreamRotator;
use crate::helpers::{Pos, parse_pair, TryClone};

#[derive(Debug, Clone)]
pub struct Rotctled<T: Read + Write> {
    sock: T,
}

// new(addr: impl Into<String>)

impl<T: Read + Write> Rotctled<T> {
    pub fn new(sock: T) -> Self {
        Self { sock }
    }
}

impl<T: Read + Write + TryClone> UpstreamRotator for Rotctled<T> {
    fn read_pos(&self) -> Result<Pos> {
        let mut outgoing = self.sock.try_clone()?;
        let incoming = BufReader::new(self.sock.try_clone()?);
        let mut lines = incoming.lines();
        writeln!(outgoing, "p")?;
        let pair = vec![
            lines.next().unwrap()?,
            lines.next().unwrap()?,
        ];
        parse_pair(pair.iter().map(String::as_str)).ok_or_else(|| std::io::ErrorKind::InvalidData.into())
    }

    fn send_pos(&self, mut p: Pos) -> Result<()> {
        let mut outgoing = self.sock.try_clone()?;
        p.az %= 360.0;
        if p.az < 0.0 {
            p.az += 360.0;
        }
        eprintln!("upstream az => {}, el => {}", p.az, p.el);
        writeln!(outgoing, "P {:.2} {:.2}", p.az, p.el)?;
        Ok(())
    }
}

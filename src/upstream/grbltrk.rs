use std::io::{BufRead, BufReader, Result, Read, Write, Error};
use std::io::ErrorKind::InvalidData;

use std::fs::File;
use crate::helpers::{Pos, Degrees};
use crate::UpstreamRotator;

// xref https://github.com/Hamlib/Hamlib/blob/master/rotators/grbltrk/grbltrk.c

#[derive(Debug)]
pub struct GrblTrker<T: Read + Write> {
    dev: T,
}

impl<T: Read + Write> GrblTrker<T> {
    pub fn new(dev: T) -> Self {
        Self { dev }
    }
}

// HT: https://internals.rust-lang.org/t/why-is-there-no-tryclone-trait/19977
trait TryClone: Sized {
    fn try_clone(&self) -> Result<Self>;
}

impl TryClone for File {
    fn try_clone(&self) -> Result<Self> {
        // Resolves to the inherent `File::try_clone`, not this trait method,
        // because inherent methods take priority over trait methods.
        File::try_clone(self)
    }
}


type GrblCoord = f32;

fn to_grbl(angle: Degrees) -> GrblCoord {
    // xref https://github.com/Hamlib/Hamlib/blob/1b3fb4a6a30652c37a45b38cd766f5de515231b3/rotators/grbltrk/grbltrk.c#L47
    angle / 9.0
}

fn from_grbl(coord: GrblCoord) -> Degrees {
    coord * 9.0
}

fn parse_status(resp: &str) -> Option<(GrblCoord, GrblCoord)> {
    // e.g. <Idle|MPos:0.000,0.000,0.000|FS:0,0|Pn:P|WCO:5.000,0.000,0.000>
    let mut items = resp.split("|");
    let mut _mpos = items.find(|s| s.starts_with("MPos:"))?.split(":");
    _mpos.next()?;
    let mut mpos = _mpos.next()?.split(",");
    // xref https://github.com/Hamlib/Hamlib/blob/1b3fb4a6a30652c37a45b38cd766f5de515231b3/rotators/grbltrk/grbltrk.c#L368-L370
    Some((
        mpos.next()?.parse().ok()?,
        mpos.next()?.parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_parsing() {
        assert_eq!(
            parse_status("<Idle|MPos:0.000,0.000,0.000|FS:0,0|Pn:P|WCO:5.000,0.000,0.000>"),
            Some((0.0, 0.0)),
        );
        assert_eq!(
            parse_status("<Idle|MPos:1.5,-42,0.000|FS:0,0|Pn:P|WCO:5.000,0.000,0.000>"),
            Some((1.5, -42.0)),
        );
        assert_eq!(
            parse_status("<Idle|WPos:1.5,-42,0.000|FS:0,0|Pn:P|WCO:5.000,0.000,0.000>"),
            None,
        );
        assert_eq!(
            parse_status("<Idle|MPos:9|FS:0,0|Pn:P|WCO:5.000,0.000,0.000>"),
            None,
        );
        assert_eq!(
            parse_status("<||||>"),
            None,
        );
        assert_eq!(
            parse_status("<|>"),
            None,
        );
        assert_eq!(
            parse_status("<>"),
            None,
        );
        assert_eq!(
            parse_status(""),
            None,
        );
    }
}

impl<T: Read + Write + TryClone> UpstreamRotator for GrblTrker<T> {
    fn read_pos(&self) -> Result<Pos> {
        let incoming = BufReader::new(self.dev.try_clone()?);
        let mut outgoing = self.dev.try_clone()?;
        let mut lines = incoming.lines();
        write!(outgoing, "?")?;
        let resp = lines.next().unwrap()?;
        resp.starts_with("<").ok_or(Error::from(InvalidData))?;
        let (x, y) = parse_status(&resp)
            .ok_or(Error::from(InvalidData))?;
        Ok(Pos { az: from_grbl(x), el: from_grbl(y) })
    }

    fn send_pos(&self, mut p: Pos) -> Result<()> {
        let mut outgoing = self.dev.try_clone()?;
        p.az %= 360.0;
        if p.az < 0.0 {
            p.az += 360.0;
        }
        let (x, y) = (
            to_grbl(p.az),
            to_grbl(p.el),
        );
        eprintln!("upstream X => {}, Y => {}", x, y);
        // xref https://github.com/Hamlib/Hamlib/blob/1b3fb4a6a30652c37a45b38cd766f5de515231b3/rotators/grbltrk/grbltrk.c#L313
        writeln!(outgoing, "G0 X{:.4} Y{:.4}", x, y)?;
        Ok(())
    }
}

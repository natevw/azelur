use std::io::{BufRead, BufReader, Result, Write};
use std::fs::File;
use crate::helpers::Pos;
use crate::UpstreamRotator;

#[derive(Debug)]
pub struct GrblTrker {
    path: String,
    dev: File,
}

impl Clone for GrblTrker {
    fn clone(&self) -> Self {
        Self::new(&self.path)
    }
}


// xref https://github.com/Hamlib/Hamlib/blob/master/rotators/grbltrk/grbltrk.c

impl GrblTrker {
    pub fn new(path: impl Into<String>) -> Self {
        let path: String = path.into();
        Self {
            path: path.clone(),
            dev: File::open(&path).unwrap(),
        }
    }
}

impl UpstreamRotator for GrblTrker {
    fn read_pos(&self) -> Result<Pos> {
        let incoming = BufReader::new(&self.dev);
        let mut lines = incoming.lines();
        write!(&self.dev, "?")?;
        let resp = lines.next().unwrap()?;
        // TODO: parse out from `resp`!
        Ok(Pos { az: 0.0, el: 0.0 })
    }

    fn send_pos(&self, mut p: Pos) -> Result<()> {
        p.az %= 360.0;
        if p.az < 0.0 {
            p.az += 360.0;
        }
        eprintln!("upstream X => {}, Y => {}", p.az, p.el);
        writeln!(&self.dev, "G0 X{:.4} Y{:.4}", p.az, p.el)?;
        Ok(())
    }
}

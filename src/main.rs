use std::io::{BufRead, BufReader, Result, Write};
use std::net::{TcpListener, TcpStream};

type DegTenths<T> = T;
type DegHundredths<T> = T;

#[derive(Default)]
struct RotatorOfVirtuality {
    az: DegTenths<i32>,     // az° x 10
    el: DegHundredths<i16>, // el° x 100
}

fn handle_client(stream: TcpStream) -> Result<()> {
    let incoming = BufReader::new(&stream);
    for line in incoming.lines() {
        let line = line?;
        println!("{line}");
        let mut parts = line.split_ascii_whitespace();
        let cmd = parts.next();
        let Some(cmd) = cmd else {
            continue;
        };
        match cmd {
            "P" | "\\set_pos" => {
                let (Some(az_part), Some(el_part)) = (parts.next(), parts.next()) else {
                    continue;
                };
                let Some(az) = az_part.parse::<f32>().ok() else {
                    continue;
                };
                let Some(el) = el_part.parse::<f32>().ok() else {
                    continue;
                };                
                writeln!(&stream, "Az: {az} El: {el}")?;
            }
            _ => continue,
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut _x = RotatorOfVirtuality::default();
    _x.az = _x.el as i32;

    let listener = TcpListener::bind("0.0.0.0:4533")?;
    for stream in listener.incoming() {
        handle_client(stream?)?;
    }
    Ok(())
}

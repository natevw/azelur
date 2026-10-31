use std::io::{BufRead, BufReader, Result, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::{thread, time};

#[derive(Default)]
struct RotatorOfVirtuality {
    az: f32,
    el: f32,
    az_adj: f32,
    el_adj: f32,
}

// HT: https://stackoverflow.com/a/34969944/179583
fn parse_pair<'a, I>(mut parts: I) -> Option<(f32, f32)>
where
    I: Iterator<Item = &'a str>,
{
    let az_part = parts.next()?;
    let el_part = parts.next()?;

    let az: f32 = az_part.parse().ok()?;
    let el: f32 = el_part.parse().ok()?;
    return Some((az, el));
}

fn handle_client(
    stream: TcpStream,
    rotator: Arc<Mutex<RotatorOfVirtuality>>,
) -> Result<()> {
    let incoming = BufReader::new(&stream);
    for line in incoming.lines() {
        let line = line?;
        println!("{}: {}", stream.peer_addr().unwrap(), line);
        
        // WORKAROUND: SatDump complains if we respond too quickly…
        thread::sleep(time::Duration::from_millis(250));
        
        let mut rotator = rotator.lock().unwrap();
        let mut parts = line.split_ascii_whitespace();
        let cmd = parts.next();
        let Some(cmd) = cmd else {
            continue;
        };
        match cmd {
            "p" | "\\get_pos" => {
                // TODO: account for upstream status here…
                writeln!(&stream, "{:.2}", rotator.az)?;
                writeln!(&stream, "{:.2}", rotator.el)?;
            },
            "P" | "\\set_pos" => {
                let Some((az, el)) = parse_pair(parts) else {
                    writeln!(&stream, "RPRT -1")?;
                    continue;
                };
                rotator.az = az;
                rotator.el = el;
                // TODO: ideally confirm upstream response first…
                writeln!(&stream, "RPRT 0")?;
                
                let upstream_az = (az + rotator.az_adj) % 360.0;
                let upstream_el = el + rotator.el_adj;
                println!("Goto Az: {upstream_az} El: {upstream_el}");
                // HACK: re-connect whenever position needs changing!
                // TODO: share a signle/persistent upstream connection
                let upstream =  TcpStream::connect("192.168.42.209:4533")?;
                writeln!(&upstream, "P {:.2} {:.2}", upstream_az, upstream_el)?;
            },
            "x" | "\\get_adj" => {
                writeln!(&stream, "{:.2}", rotator.az_adj)?;
                writeln!(&stream, "{:.2}", rotator.el_adj)?;
            },
            "X" | "\\set_adj" => {
                let Some((az, el)) = parse_pair(parts) else {
                    writeln!(&stream, "RPRT -1")?;
                    continue;
                };
                println!("Adjust Az: {az} El: {el}");
                rotator.az_adj = az;
                rotator.el_adj = el;
                writeln!(&stream, "RPRT 0")?;
            },
            _ => continue,
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let listener = TcpListener::bind("0.0.0.0:4533")?;
    let rotator = Arc::new(Mutex::new(
        RotatorOfVirtuality::default()
    ));
    
    for stream in listener.incoming() {
        let stream = stream?;
        let rotator = Arc::clone(&rotator);
        thread::spawn(|| {
            handle_client(stream, rotator).unwrap();
        });
    }
    Ok(())
}

use std::io::{BufRead, BufReader, BufWriter, Result, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

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

fn send_pos(
    upstream_addr: &str,
    rotator: &RotatorOfVirtuality
) -> Result<()> {
    // HACK: we re-connect whenever position needs changing!
    // TODO: share a single/persistent upstream connection
    let upstream =  TcpStream::connect(upstream_addr)?;
    
    let _upstream_az = (rotator.az + rotator.az_adj) % 360.0;
    let upstream_az = if _upstream_az < 0.0 {
        _upstream_az + 360.0
    } else {
        _upstream_az
    };
    
    let upstream_el = rotator.el + rotator.el_adj;
    
    eprintln!("Goto Az: {upstream_az} El: {upstream_el}");
    writeln!(&upstream, "P {:.2} {:.2}", upstream_az, upstream_el)?;
    Ok(())
}

/*
fn get_pos(
    upstream_addr: &str,
) -> Result<(f32, f32)> {
    let upstream =  TcpStream::connect(upstream_addr)?;
    let incoming = BufReader::new(&upstream);
    writeln!(&upstream, "p")?;
    let x = incoming.lines().map(|l| l.unwrap());
    parse_pair(x)
}
*/

fn handle_client(
    stream: TcpStream,
    rotator: Arc<Mutex<RotatorOfVirtuality>>,
    upstream_addr: &str,
) -> Result<()> {
    let incoming = BufReader::new(&stream);
    
    // NOTE: SatDump ± expects each command response to come back in a single packet (≈recv call)
    // on our side, buffering the outgoing writes and flushing them seems to prevent it erroring out.
    // see https://github.com/SatDump/SatDump/blob/9fee0793c7d42bea2d401d330422e18c33c126fa/src-core/common/tracking/rotator/rotcl_handler.cpp#L28
    let mut outgoing = BufWriter::new(&stream);
    
    for line in incoming.lines() {
        let line = line?;
        eprintln!("{}: {}", stream.peer_addr().unwrap(), line);
        
        let mut rotator = rotator.lock().unwrap();
        let mut parts = line.split_ascii_whitespace();
        let cmd = parts.next();
        let Some(cmd) = cmd else {
            continue;
        };
        match cmd {
            "p" | "\\get_pos" => {
                writeln!(outgoing, "{:.2}", rotator.az)?;
                writeln!(outgoing, "{:.2}", rotator.el)?;
            },
            "P" | "\\set_pos" => {
                let Some((az, el)) = parse_pair(parts) else {
                    eprintln!("BAD DATA: replying w/error");
                    writeln!(outgoing, "RPRT -1")?;
                    continue;
                };
                rotator.az = az;
                rotator.el = el;
                writeln!(outgoing, "RPRT 0")?;
                send_pos(upstream_addr, &rotator)?;
                eprintln!("[sent after az: {az} el: {el}]");
            },
            "x" | "\\get_adj" => {
                writeln!(outgoing, "{:.2}", rotator.az_adj)?;
                writeln!(outgoing, "{:.2}", rotator.el_adj)?;
            },
            "X" | "\\set_adj" => {
                let Some((az, el)) = parse_pair(parts) else {
                    writeln!(outgoing, "RPRT -1")?;
                    continue;
                };
                rotator.az_adj = az;
                rotator.el_adj = el;
                writeln!(outgoing, "RPRT 0")?;
                send_pos(upstream_addr, &rotator)?;
                eprintln!("[sent after az_adj: {az} el_adj: {el}]");
            },
            _ => continue,
        }
        outgoing.flush()?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: run <listen_addr:port> <upstream_addr:port>");
        std::process::exit(1);
    }
    let our_addr = &args[1];
    let rot_addr = &args[2];
    eprintln!("Listening on {}", our_addr);
    eprintln!("Upstream is: {}", rot_addr);
    
    let listener = TcpListener::bind(our_addr)?;
    let rotator = Arc::new(Mutex::new(
        RotatorOfVirtuality::default()
    ));
    
    for stream in listener.incoming() {
        let stream = stream?;
        let upstream = rot_addr.clone();
        let rotator = Arc::clone(&rotator);
        let peer = stream.peer_addr().unwrap();
        thread::spawn(move || {
            eprintln!("Connection from {}", peer);
            handle_client(stream, rotator, &upstream).unwrap();
            eprintln!("Done with {}", peer);
        });
    }
    Ok(())
}

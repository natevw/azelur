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
    
    println!("Goto Az: {upstream_az} El: {upstream_el}");
    writeln!(&upstream, "P {:.2} {:.2}", upstream_az, upstream_el)?;
    Ok(())
}

fn handle_client(
    stream: TcpStream,
    rotator: Arc<Mutex<RotatorOfVirtuality>>,
    upstream_addr: &str,
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
                send_pos(upstream_addr, &rotator)?;
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
                send_pos(upstream_addr, &rotator)?;
            },
            _ => continue,
        }
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
    
    let listener = TcpListener::bind(our_addr)?;
    let rotator = Arc::new(Mutex::new(
        RotatorOfVirtuality::default()
    ));
    
    for stream in listener.incoming() {
        let stream = stream?;
        let upstream = rot_addr.clone();
        let rotator = Arc::clone(&rotator);
        thread::spawn(move || {
            handle_client(stream, rotator, &upstream).unwrap();
        });
    }
    Ok(())
}

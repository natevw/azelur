use std::io::{BufRead, BufReader, Result, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::{thread, time};

#[derive(Default)]
struct RotatorOfVirtuality {
    az: f32,
    el: f32,
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
                writeln!(&stream, "{:.2}", rotator.az)?;
                writeln!(&stream, "{:.2}", rotator.el)?;
            },
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
                println!("Az: {az} El: {el}");
                rotator.az = az;
                rotator.el = el;
                writeln!(&stream, "RPRT 0")?;
            }
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

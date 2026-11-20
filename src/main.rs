mod helpers;
mod upstream;

use std::io::{BufRead, BufReader, BufWriter, Result, Write};
use std::net::{TcpListener, TcpStream};
use std::fs::OpenOptions;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use crate::upstream::{UpstreamRotator, Rotctled, GrblTrker};
use crate::helpers::{
    AzAngle, Pos, parse_pair,
    predict_rotation, recommended_midpoint,
};

#[derive(Default, Debug)]
struct RotatorOfVirtuality {
    pos: Pos,
    adj: Pos,
}

// HT: https://stackoverflow.com/questions/19605132/is-it-possible-to-use-global-variables-in-rust/76394993#76394993
static MAX_WINDING: OnceLock<AzAngle> = OnceLock::new();

fn get_max_winding() -> AzAngle {
    *MAX_WINDING.get_or_init(|| {
        if let Ok(max) = std::env::var("AZEL_MAX_WINDING") {
            max.parse().unwrap()
        } else {
            360.0
        }
    })
}

const UNWIND_DELAY: Duration = Duration::from_millis(500);

fn handle_client(
    stream: TcpStream,
    virt: Arc<Mutex<RotatorOfVirtuality>>,
    rot: Arc<Mutex<Box<dyn UpstreamRotator + Send>>>,
) -> Result<()> {
    let incoming = BufReader::new(&stream);
    
    // NOTE: SatDump ± expects each command response to come back in a single packet (≈recv call)
    // on our side, buffering the outgoing writes and flushing them seems to prevent it erroring out.
    // see https://github.com/SatDump/SatDump/blob/9fee0793c7d42bea2d401d330422e18c33c126fa/src-core/common/tracking/rotator/rotcl_handler.cpp#L28
    let mut outgoing = BufWriter::new(&stream);
    
    for line in incoming.lines() {
        let line = line?;
        eprintln!("{}: {}", stream.peer_addr().unwrap(), line);
        
        let mut virt = virt.lock().unwrap();
        let rot = rot.lock().unwrap();
        let mut parts = line.split_ascii_whitespace();
        let cmd = parts.next();
        let Some(cmd) = cmd else {
            continue;
        };
        match cmd {
            "p" | "\\get_pos" => {
                writeln!(outgoing, "{:.2}", virt.pos.az)?;
                writeln!(outgoing, "{:.2}", virt.pos.el)?;
            },
            "P" | "\\set_pos" => {
                let Some(pos) = parse_pair(parts) else {
                    eprintln!("BAD DATA: replying w/error");
                    writeln!(outgoing, "RPRT -1")?;
                    continue;
                };
                writeln!(outgoing, "RPRT 0")?;
                
                let az_curr = virt.pos.az;
                let mut az_diff = predict_rotation(az_curr, pos.az);
                if let Some(mid_az) = recommended_midpoint(
                    az_curr,
                    &mut az_diff,
                    get_max_winding(),
                ) {
                    eprint!("⤺");
                    let mid_pos = Pos { az: mid_az, el: virt.pos.el };
                    // CAUTION: if upstream doesn't start this step by end of delay,
                    // the main `send_pos` below will supersede this unwind request!
                    rot.send_pos(mid_pos + virt.adj)?;
                    thread::sleep(UNWIND_DELAY);
                }
                // we accumulate predicted upstream rotation into `virt.pos`
                virt.pos = Pos { az: az_curr + az_diff, el: pos.el };
                rot.send_pos(virt.pos + virt.adj)?;
            },
            "x" | "\\get_adj" => {
                writeln!(outgoing, "{:.2}", virt.adj.az)?;
                writeln!(outgoing, "{:.2}", virt.adj.el)?;
            },
            "X" | "\\set_adj" => {
                let Some(adj) = parse_pair(parts) else {
                    writeln!(outgoing, "RPRT -1")?;
                    continue;
                };
                virt.adj = adj;
                writeln!(outgoing, "RPRT 0")?;
                rot.send_pos(virt.pos + virt.adj)?;
            },
            "W" | "\\want_pos" => {
                // change adjustment so rotator reads as desired
                let Some(wanted) = parse_pair(parts) else {
                    writeln!(outgoing, "RPRT -1")?;
                    continue;
                };
                //     sent = virt.pos + virt.adj
                // keep `sent` same while making `virt.pos = wanted`
                // i.e. `virt0.pos + virt0.adj = wanted + virt1.adj`
                let sent = virt.pos + virt.adj;
                virt.adj = sent - wanted;
                virt.pos = wanted;
                writeln!(outgoing, "RPRT 0")?;
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
        eprintln!("usage: run <listen_addr:port> <upstream_addr:port> ['<az_adj> <el_adj>'");
        std::process::exit(1);
    }
    let our_addr = &args[1];
    let rot_addr = &args[2];
    eprintln!("Listening on {}", our_addr);
    eprintln!("Upstream is: {}", rot_addr);
    
    let listener = TcpListener::bind(our_addr)?;
    let rotation_state = Arc::new(Mutex::new(
        RotatorOfVirtuality::default()
    ));
    
    if let Ok(adj) = std::env::var("AZEL_INITIAL_ADJ") {
        let adj_parts = adj.split(" ");
        let adj = parse_pair(adj_parts).unwrap();
        eprintln!("Setting initial adjustment of {:.2} {:.2}", adj.az, adj.el);
        
        let mut virt = rotation_state.lock().unwrap();
        virt.adj = adj;
    }
    
    // ~HACK: trigger any parse error on startup
    let maxw = get_max_winding();
    assert!(maxw >= 180.0);  // anything less is nonsensical
    
    
    let _upstream: Box<dyn UpstreamRotator + Send> = if rot_addr.starts_with("/") {
        let device = OpenOptions::new()
            .read(true)
            .write(true)
            .open(rot_addr)?;
        Box::new(GrblTrker::new(device))
    } else {
        let socket = TcpStream::connect(rot_addr)?;
        Box::new(Rotctled::new(socket))
    };
    
    let upstream = Arc::new(Mutex::new(_upstream));
    
    if let Ok(pos) = std::env::var("AZEL_INITIAL_POS") {
        let pos_parts = pos.split(" ");
        let pos = parse_pair(pos_parts).unwrap();
        eprintln!("Setting initial position to {:.2} {:.2}", pos.az, pos.el);
        let mut virt = rotation_state.lock().unwrap();
        virt.pos = pos;
    } else {
        let rot = upstream.lock().unwrap();
        let real = rot.read_pos()?;
        eprintln!("Current upstream position is {:.2} {:.2}", real.az, real.el);
        let mut virt = rotation_state.lock().unwrap();
        virt.pos = real - virt.adj;
        //eprintln!("{:#?}", virt);
    }
    
    for stream in listener.incoming() {
        let stream = stream?;
        let rotator = Arc::clone(&upstream);
        let state = Arc::clone(&rotation_state);
        let peer = stream.peer_addr().unwrap();
        thread::spawn(move || {
            eprintln!("Connection from {}", peer);
            handle_client(stream, state, rotator).unwrap();
            eprintln!("Done with {}", peer);
        });
    }
    Ok(())
}

use std::io::{BufRead, BufReader, BufWriter, Result, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

type Degrees = f32;
type AzAngle = Degrees;
type ElAngle = Degrees;

#[derive(Default, Debug, Copy, Clone)]
struct Pos {
    az: AzAngle,
    el: ElAngle,
}

impl std::ops::Sub for Pos {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Self {
            az: self.az - other.az,
            el: self.el - other.el,
        }
    }
}

impl std::ops::Add for Pos {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            az: self.az + other.az,
            el: self.el + other.el,
        }
    }
}


#[derive(Default, Debug)]
struct RotatorOfVirtuality {
    pos: Pos,
    adj: Pos,
}

// HT: https://stackoverflow.com/a/34969944/179583
fn parse_pair<'a, I>(mut parts: I) -> Option<Pos>
where
    I: Iterator<Item = &'a str>,
{
    let az_part = parts.next()?;
    let el_part = parts.next()?;

    let az: Degrees = az_part.parse().ok()?;
    let el: Degrees = el_part.parse().ok()?;
    return Some(Pos { az, el });
}

fn send_pos(
    upstream_addr: &str,
    virt: &RotatorOfVirtuality
) -> Result<()> {
    // HACK: we re-connect whenever position needs changing!
    // TODO: share a single/persistent upstream connection
    let upstream =  TcpStream::connect(upstream_addr)?;
    
    let _upstream_az = (virt.pos.az + virt.adj.az) % 360.0;
    let upstream_az = if _upstream_az < 0.0 {
        _upstream_az + 360.0
    } else {
        _upstream_az
    };
    
    let upstream_el = virt.pos.el + virt.adj.el;
    
    eprintln!("upstream az => {upstream_az}, el => {upstream_el}");
    writeln!(&upstream, "P {:.2} {:.2}", upstream_az, upstream_el)?;
    Ok(())
}

fn get_pos(
    upstream_addr: &str,
) -> Result<Pos> {
    let upstream =  TcpStream::connect(upstream_addr)?;
    let incoming = BufReader::new(&upstream);
    let mut lines = incoming.lines();
    writeln!(&upstream, "p")?;
    let pair = vec![
        lines.next().unwrap()?,
        lines.next().unwrap()?,
    ];
    parse_pair(pair.iter().map(String::as_str)).ok_or_else(|| std::io::ErrorKind::InvalidData.into())
}

fn predict_rotation(curr: AzAngle, dest: AzAngle) -> Degrees {
    let naïve = dest - curr;
    let rotation = naïve % 360.0;
    if rotation > 180.0 {
        rotation - 360.0
    } else if rotation < -180.0 {
        rotation + 360.0
    } else {
        rotation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_rotations() {
        assert_eq!(
            predict_rotation(5.0, 10.0), 5.0,
        );
        assert_eq!(
            predict_rotation(10.0, 5.0), -5.0,
        );
        assert_eq!(
            predict_rotation(365.0, 370.0), 5.0,
        );
        assert_eq!(
            predict_rotation(365.0, 5.0), 0.0,
        );
        assert_eq!(
            predict_rotation(365.0, -355.0), 0.0,
        );
        assert_eq!(
            predict_rotation(0.0, 185.0), -175.0,
        );
        assert_eq!(
            predict_rotation(0.0, -185.0), 175.0,
        );
        assert_eq!(
            predict_rotation(0.0, -175.0), -175.0,
        );
        assert_eq!(
            predict_rotation(360.0, 175.0), 175.0,
        );
        assert_eq!(
            predict_rotation(-123456.0, 42.0), 18.0,
        );
        assert_eq!(
            predict_rotation(-123456.0, -123467.0), -11.0,
        );
        assert_eq!(
            predict_rotation(-123456.0, 123186.0), 42.0,
        );
    }
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

fn recommended_midpoint(
    curr: AzAngle,
    diff: &mut Degrees,
    max_mag: AzAngle,
) -> Option<AzAngle> {
    let past_max = curr + *diff > max_mag;
    let past_min = curr + *diff < -max_mag;
    if past_max || past_min {
        // find an intermediate 180º from halfway to dest
        // e.g. we're at MAX_WINDING=360 and diff is +90:
        // halfway is 405°, whose antipode is (45 ± 180)°
        // if we go there, next diff should then unwind.
        let antimid = curr + *diff / 2.0 + 180.0;
        // adjust to newly intended (unwinding direction) difference
        *diff = if past_max { *diff - 360.0 } else { *diff + 360.0 };
        Some(antimid)
    } else {
        None
    }
}


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
                writeln!(outgoing, "{:.2}", rotator.pos.az)?;
                writeln!(outgoing, "{:.2}", rotator.pos.el)?;
            },
            "P" | "\\set_pos" => {
                let Some(pos) = parse_pair(parts) else {
                    eprintln!("BAD DATA: replying w/error");
                    writeln!(outgoing, "RPRT -1")?;
                    continue;
                };
                writeln!(outgoing, "RPRT 0")?;
                
                let az_curr = rotator.pos.az;
                let mut az_diff = predict_rotation(az_curr, pos.az);
                if let Some(mid_az) = recommended_midpoint(
                    az_curr,
                    &mut az_diff,
                    get_max_winding(),
                ) {
                    eprint!("⤺");
                    rotator.pos.az = mid_az;
                    // CAUTION: if upstream doesn't start this step by end of delay,
                    // the main `send_pos` below will supersede this unwind request!
                    send_pos(upstream_addr, &rotator)?;
                    thread::sleep(UNWIND_DELAY);
                }
                // we accumulate predicted upstream rotation into `virt.pos`
                rotator.pos = Pos { az: az_curr + az_diff, el: pos.el };
                send_pos(upstream_addr, &rotator)?;
            },
            "x" | "\\get_adj" => {
                writeln!(outgoing, "{:.2}", rotator.adj.az)?;
                writeln!(outgoing, "{:.2}", rotator.adj.el)?;
            },
            "X" | "\\set_adj" => {
                let Some(adj) = parse_pair(parts) else {
                    writeln!(outgoing, "RPRT -1")?;
                    continue;
                };
                rotator.adj = adj;
                writeln!(outgoing, "RPRT 0")?;
                send_pos(upstream_addr, &rotator)?;
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
                let sent = rotator.pos + rotator.adj;
                rotator.adj = sent - wanted;
                rotator.pos = wanted;
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
    let rotator = Arc::new(Mutex::new(
        RotatorOfVirtuality::default()
    ));
    
    // ~HACK: trigger any parse error on startup
    _ = get_max_winding();
    
    if let Ok(adj) = std::env::var("AZEL_INITIAL_ADJ") {
        let adj_parts = adj.split(" ");
        let adj = parse_pair(adj_parts).unwrap();
        eprintln!("Setting initial adjustment of {:.2} {:.2}", adj.az, adj.el);
        
        let mut rot = rotator.lock().unwrap();
        rot.adj = adj;
    }
    
    if let Ok(pos) = std::env::var("AZEL_INITIAL_POS") {
        let pos_parts = pos.split(" ");
        let pos = parse_pair(pos_parts).unwrap();
        eprintln!("Setting initial position to {:.2} {:.2}", pos.az, pos.el);
        let mut rot = rotator.lock().unwrap();
        rot.pos = pos;
    } else {
        let real = get_pos(rot_addr)?;
        eprintln!("Current upstream position is {:.2} {:.2}", real.az, real.el);
        let mut rot = rotator.lock().unwrap();
        rot.pos = real - rot.adj;
        //eprintln!("{:#?}", rot);
    }
    
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

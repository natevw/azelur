use std::net::{TcpListener, TcpStream};

type DegTenths<T> = T;
type DegHundredths<T> = T;

#[derive(Default)]
struct RotatorOfVirtuality {
    az: DegTenths<i32>,     // az° x 10
    el: DegHundredths<i16>, // el° x 100
}

fn handle_client(_stream: TcpStream) {
    // ...
}

fn main() -> std::io::Result<()> {
    let mut _x = RotatorOfVirtuality::default();
    _x.az = _x.el as i32;

    let listener = TcpListener::bind("0.0.0.0:4533")?;
    for stream in listener.incoming() {
        handle_client(stream?);
    }
    Ok(())
}

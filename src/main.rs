use std::net::{TcpListener, TcpStream};

fn handle_client(_stream: TcpStream) {
    // ...
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:4533")?;
    for stream in listener.incoming() {
        handle_client(stream?);
    }
    Ok(())
}

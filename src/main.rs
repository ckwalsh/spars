use std::fs::File;
use std::io::Write as _;
use std::net::TcpListener;

use spars::serve;
use spars::Handler;
use spars::Settings;

fn main() {
    let settings = Settings::from_env().expect("Invalid Settings");

    if let Some(p) = settings.server.pid_file {
        let pid = std::process::id();
        let mut f = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(p)
            .expect("Failed to open pid file");

        write!(f, "{pid}").expect("Failed to write pid");
    }

    let handler = Handler::try_from(settings.handler).expect("Could not build handler");
    let listener = TcpListener::bind(settings.server.addr).expect("Could not bind to port");

    if let Some(p) = settings.server.addr_file {
        let addr = listener.local_addr().expect("Failed to get local address");

        let mut f = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(p)
            .expect("Failed to open addr file");

        write!(f, "{addr}").expect("Failed to write addr");
    }

    serve(listener, handler).expect("Failed to run server");
}

// SPDX-FileCopyrightText: 2025 Cullen Walsh <ckwalsh@cullenwalsh.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

#[doc = include_str!("../../README.md")]
use std::fs::File;
use std::io::Write as _;
use std::net::TcpListener;
use std::sync::Arc;

use spars_httpd::serve;
use spars_httpd::Handler;
use spars_httpd::Settings;

fn main() {
    let settings = Settings::from_env().expect("Invalid Settings");

    if let Some(p) = settings.server.pid_file {
        let pid = std::process::id();
        let mut f = File::create(p).expect("Failed to open pid file");

        write!(f, "{pid}").expect("Failed to write pid");
    }

    let handler = Arc::new(Handler::try_from(settings.handler).expect("Could not build handler"));
    let listener = TcpListener::bind(settings.server.addr).expect("Could not bind to port");

    if let Some(p) = settings.server.addr_file {
        let addr = listener.local_addr().expect("Failed to get local address");

        let mut f = File::create(p).expect("Failed to open addr file");

        write!(f, "{addr}").expect("Failed to write addr");
    }

    serve(listener, handler).expect("Failed to run server");
}

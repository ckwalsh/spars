// SPDX-FileCopyrightText: 2025 Cullen Walsh <ckwalsh@cullenwalsh.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;

use async_io::Async;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::iterator::Signals;

const SIG_HANDLER_STACK_SIZE: usize = 8192;

pub fn spawn_signal_handler(
    stop_flag: Arc<AtomicBool>,
    listener: &Async<TcpListener>,
) -> std::io::Result<JoinHandle<std::io::Result<()>>> {
    let addr = listener.as_ref().local_addr()?;

    std::thread::Builder::new()
        .stack_size(SIG_HANDLER_STACK_SIZE)
        .spawn(move || {
            let mut signals = Signals::new(TERM_SIGNALS)?;

            signals.forever().next();

            stop_flag.store(true, Ordering::SeqCst);

            // Wake up the server
            let _ = TcpStream::connect(addr);

            Ok(())
        })
}

// SPDX-FileCopyrightText: 2025 Cullen Walsh <ckwalsh@cullenwalsh.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::any::Any;
use std::net::TcpListener;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use async_executor::LocalExecutor;
use async_io::Async;
use thiserror::Error;

mod conn;
mod handler;
mod response;
mod settings;

pub use handler::Handler;
pub use handler::HandlerBuildError;
pub use response::Response;
pub use response::StatusCode;
pub use settings::*;

#[cfg(feature = "signal-hook")]
mod signals;

#[derive(Debug, Error)]
pub enum SparsError {
    #[error("Failed to create async listener: {0}")]
    FailedToCreateAsyncListener(std::io::Error),

    #[error("Failed to create signal handler: {0}")]
    FailedToStartSignalHandler(std::io::Error),

    #[error("Failed to join signal handler")]
    FailedToJoinSignalHandler(Box<dyn Any + Send>),

    #[error("Signal handler failed: {0}")]
    SignalHandlerFailed(std::io::Error),
}

pub fn serve<L: TryInto<Async<TcpListener>, Error = std::io::Error>, H: Into<Arc<Handler>>>(
    listener: L,
    handler: H,
) -> Result<(), SparsError> {
    let listener = listener
        .try_into()
        .map_err(SparsError::FailedToCreateAsyncListener)?;
    let stop_flag = Arc::new(AtomicBool::new(false));

    #[cfg(feature = "signal-hook")]
    let signal_handler = signals::spawn_signal_handler(Arc::clone(&stop_flag), &listener)
        .map_err(SparsError::FailedToStartSignalHandler)?;

    serve_with_stop_flag(listener, handler, stop_flag)?;

    #[cfg(feature = "signal-hook")]
    signal_handler
        .join()
        .map_err(SparsError::FailedToJoinSignalHandler)?
        .map_err(SparsError::SignalHandlerFailed)?;

    Ok(())
}

pub fn serve_with_stop_flag<H: Into<Arc<Handler>>>(
    listener: Async<TcpListener>,
    handler: H,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), SparsError> {
    let handler = handler.into();

    let ex = LocalExecutor::new();

    async_io::block_on(ex.run(async {
        while !stop_flag.load(Ordering::SeqCst) {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    ex.spawn({
                        let handler = Arc::clone(&handler);
                        let stop_flag = Arc::clone(&stop_flag);

                        conn::handle_conn(handler, stop_flag, stream)
                    })
                    .detach();
                }
                Err(_e) => {
                    // eprintln!("Error while establishing connection: {}", _e);
                }
            }
        }
    }));

    Ok(())
}

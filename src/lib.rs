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

pub use handler::HandlerBuildError;
pub use settings::Settings;
pub use settings::SettingsFromEnvError;

#[cfg(feature = "signal-hook")]
mod signals;

#[derive(Debug, Error)]
pub enum SparsError {
    #[error("Could not build request handler {0}")]
    CouldNotBuildHandler(#[from] HandlerBuildError),

    #[error("Failed to create signal handler: {0}")]
    FailedToStartSignalHandler(std::io::Error),

    #[error("Could not bind to port: {0}")]
    CouldNotBind(std::io::Error),

    #[error("Failed to join signal handler")]
    FailedToJoinSignalHandler(Box<dyn Any + Send>),

    #[error("Signal handler failed: {0}")]
    SignalHandlerFailed(std::io::Error),
}

pub fn serve(settings: Settings) -> Result<(), SparsError> {
    let handler = Arc::new(handler::Handler::build_from_root(
        settings.root,
        &settings.index_file,
        settings.fallback_path.as_deref(),
        settings.allow_hidden,
    )?);

    let stop_flag = Arc::new(AtomicBool::new(false));

    #[cfg(feature = "signal-hook")]
    let signal_handler = signals::spawn_signal_handler(Arc::clone(&stop_flag), settings.addr)
        .map_err(SparsError::FailedToStartSignalHandler)?;

    let ex = LocalExecutor::new();

    let listener = Async::<TcpListener>::bind(settings.addr).map_err(SparsError::CouldNotBind)?;

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

    #[cfg(feature = "signal-hook")]
    signal_handler
        .join()
        .map_err(SparsError::FailedToJoinSignalHandler)?
        .map_err(SparsError::SignalHandlerFailed)?;

    Ok(())
}

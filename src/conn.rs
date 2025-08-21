use std::io::Write as _;
use std::mem::MaybeUninit;
use std::net::Shutdown;
use std::net::TcpStream;
use std::ops::DerefMut as _;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use async_io::Async;
use async_io::Timer;
use blocking::unblock;
use futures_lite::AsyncReadExt as _;
use futures_lite::FutureExt as _;

use crate::handler::Handler;
use crate::response::Response;
use crate::response::Responses;
use crate::response::StatusCode;

const REQUEST_BUF_CAP: usize = 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

struct ConnData {
    handler: Arc<Handler>,
    conn: Async<TcpStream>,
    request_buf: [u8; REQUEST_BUF_CAP],
    responses: Responses,
}

pub async fn handle_conn(
    handler: Arc<Handler>,
    stop_flag: Arc<AtomicBool>,
    conn: Async<TcpStream>,
) {
    let shared_arc = Arc::new(Mutex::new(ConnData {
        handler,
        conn,
        request_buf: [0; _],
        responses: Responses::new(),
    }));

    let mut request_buf_len: usize = 0;

    let mut timeout_at = Instant::now() + REQUEST_TIMEOUT;
    let mut bytes_consumed = 0;

    let mut guard = shared_arc.lock().unwrap();
    let mut shared = guard.deref_mut();

    'conn: while !stop_flag.load(Ordering::SeqCst) {
        if request_buf_len == shared.request_buf.len() {
            shared.responses.push(Response::StatusStr(
                StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            ));
            break;
        }

        let read_result = {
            shared
                .conn
                .read(&mut shared.request_buf[request_buf_len..])
                .or(async {
                    Timer::at(timeout_at).await;
                    Err(std::io::ErrorKind::TimedOut.into())
                })
                .await
        };

        request_buf_len += match read_result {
            Ok(0) => {
                break 'conn;
            }
            Ok(bytes_read) => bytes_read,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                shared
                    .responses
                    .push(Response::StatusStr(StatusCode::REQUEST_TIMEOUT));
                break 'conn;
            }
            Err(_e) => {
                match _e.kind() {
                    std::io::ErrorKind::ConnectionReset => (),
                    _ => {
                        // eprintln!("Read Error: {}", _e);
                        shared
                            .responses
                            .push(Response::StatusStr(StatusCode::INTERNAL_SERVER_ERROR));
                    }
                }
                break;
            }
        };

        'req: loop {
            let buf = &shared.request_buf[bytes_consumed..request_buf_len];

            let mut request = httparse::Request::new(&mut [httparse::EMPTY_HEADER; 0]);
            let mut headers = [MaybeUninit::uninit(); 50];
            let parse_result = request.parse_with_uninit_headers(buf, &mut headers);

            match parse_result {
                Ok(httparse::Status::Partial) => break 'req,
                Ok(httparse::Status::Complete(request_bytes)) => {
                    bytes_consumed += request_bytes;

                    let mut keep_alive = match request.version {
                        Some(0) => false,
                        Some(1) => true,
                        Some(_) | None => {
                            shared
                                .responses
                                .push(Response::StatusStr(StatusCode::HTTP_VERSION_NOT_SUPPORTED));
                            break 'conn;
                        }
                    };

                    for h in request.headers.iter() {
                        if h.name.eq_ignore_ascii_case("connection") {
                            keep_alive = h.value != b"close";
                        } else if h.name.eq_ignore_ascii_case("content-length") {
                            if h.value != b"0" {
                                shared
                                    .responses
                                    .push(Response::StatusStr(StatusCode::PAYLOAD_TOO_LARGE));
                                break 'conn;
                            }
                        } else if h.name.eq_ignore_ascii_case("transfer-encoding") {
                            shared
                                .responses
                                .push(Response::StatusStr(StatusCode::NOT_IMPLEMENTED));
                            break 'conn;
                        }
                    }

                    let response = match shared.handler.handle(request.method, request.path) {
                        Ok(response) => response,
                        Err(response) => {
                            keep_alive = false;
                            response
                        }
                    };

                    shared.responses.push(response);

                    if !keep_alive {
                        break 'conn;
                    }
                }
                Err(_) => {
                    shared
                        .responses
                        .push(Response::StatusStr(StatusCode::BAD_REQUEST));
                    bytes_consumed = request_buf_len;
                    break 'conn;
                }
            }
        }

        if bytes_consumed > 0 {
            if bytes_consumed < request_buf_len {
                shared
                    .request_buf
                    .copy_within(bytes_consumed..request_buf_len, 0);
            }
            request_buf_len -= bytes_consumed;
            bytes_consumed = 0;

            drop(guard);

            let write_result: std::io::Result<()> = unblock({
                let shared_arc = Arc::clone(&shared_arc);

                move || {
                    let mut guard = shared_arc.lock().unwrap();
                    let shared = guard.deref_mut();
                    // SAFETY: TcpStream is IoSafe, and I pinky promise to not destroy the connection
                    let sync_conn = unsafe { shared.conn.get_mut() };

                    for response in shared.responses.drain() {
                        response.write_to(sync_conn, true)?;
                        sync_conn.flush()?;
                    }

                    Ok(())
                }
            })
            .await;

            guard = shared_arc.lock().unwrap();
            shared = guard.deref_mut();

            if let Err(_e) = write_result {
                // eprintln!("Error while writing responses: {}", _e);
                break 'conn;
            }
            timeout_at = Instant::now() + REQUEST_TIMEOUT;
        }
    }

    if bytes_consumed != request_buf_len {
        shared
            .responses
            .push(Response::StatusStr(StatusCode::BAD_REQUEST));
    }

    if let Some(last_response) = shared.responses.pop() {
        drop(guard);

        let write_result: std::io::Result<()> = unblock({
            let shared_arc = Arc::clone(&shared_arc);

            move || {
                let mut guard = shared_arc.lock().unwrap();
                let shared = guard.deref_mut();
                // SAFETY: TcpStream is IoSafe, and I pinky promise to not destroy the connection
                let sync_conn = unsafe { shared.conn.get_mut() };

                sync_conn.shutdown(Shutdown::Read)?;

                for response in shared.responses.drain() {
                    response.write_to(sync_conn, true)?;
                    sync_conn.flush()?;
                }

                last_response.write_to(sync_conn, false)?;
                sync_conn.flush()?;

                sync_conn.shutdown(Shutdown::Write)
            }
        })
        .await;

        if let Err(_e) = write_result {
            match _e.kind() {
                std::io::ErrorKind::NotConnected => (),
                _ => {
                    // eprintln!("Error while writing completing connection: {}", e);
                }
            }
        }
    } else {
        drop(guard);

        let write_result: std::io::Result<()> = unblock({
            let shared_arc = Arc::clone(&shared_arc);

            move || {
                let mut guard = shared_arc.lock().unwrap();
                let shared = guard.deref_mut();
                // SAFETY: TcpStream is IoSafe, and I pinky promise to not destroy the connection
                let sync_conn = unsafe { shared.conn.get_mut() };

                sync_conn.shutdown(Shutdown::Both)
            }
        })
        .await;

        if let Err(_e) = write_result {
            match _e.kind() {
                std::io::ErrorKind::NotConnected => (),
                _ => {
                    // eprintln!("Error Closing TCP stream: {}", _e);
                }
            }
        }
    }
}

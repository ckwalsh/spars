// SPDX-FileCopyrightText: 2025 Cullen Walsh <ckwalsh@cullenwalsh.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt::Display;
use std::fs::File;
use std::io::Write as _;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;

use arrayvec::ArrayVec;
use compact_str::CompactString;

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum StatusCode {
    BAD_REQUEST,                     // 400
    METHOD_NOT_ALLOWED,              // 405
    REQUEST_TIMEOUT,                 // 408
    PAYLOAD_TOO_LARGE,               // 413
    URI_TOO_LONG,                    // 414
    REQUEST_HEADER_FIELDS_TOO_LARGE, // 431
    INTERNAL_SERVER_ERROR,           // 500
    NOT_IMPLEMENTED,                 // 501
    HTTP_VERSION_NOT_SUPPORTED,      // 505
}

const STATUS_STRS: [&str; 9] = [
    "400 Bad Request",
    "405 Method Not Allowed",
    "408 Request Timeout",
    "413 Content Too Large",
    "414 URI Too Long",
    "431 Request Header Fields Too Large",
    "500 Internal Server Error",
    "501 Not Implemented",
    "505 HTTP Version Not Supported",
];

impl Display for StatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            StatusCode::BAD_REQUEST => STATUS_STRS[0],
            StatusCode::METHOD_NOT_ALLOWED => STATUS_STRS[1],
            StatusCode::REQUEST_TIMEOUT => STATUS_STRS[2],
            StatusCode::PAYLOAD_TOO_LARGE => STATUS_STRS[3],
            StatusCode::URI_TOO_LONG => STATUS_STRS[4],
            StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE => STATUS_STRS[5],
            StatusCode::INTERNAL_SERVER_ERROR => STATUS_STRS[6],
            StatusCode::NOT_IMPLEMENTED => STATUS_STRS[7],
            StatusCode::HTTP_VERSION_NOT_SUPPORTED => STATUS_STRS[8],
        })
    }
}

#[derive(Debug)]
pub enum Response {
    Found {
        resolved_path: Option<Arc<Path>>,
        len: u64,
        mime_type: Option<&'static str>,
    },
    Redirect {
        path: Arc<str>,
        query: CompactString,
    },
    NotFound,
    StatusStr(StatusCode),
}

impl Response {
    pub fn write_to(self, conn: &mut TcpStream, keep_alive: bool) -> std::io::Result<()> {
        let mut buf = ArrayVec::<u8, 256>::new();
        let mut body_path = None;

        match self {
            Response::Found {
                resolved_path,
                len,
                mime_type,
            } => {
                write!(buf, "HTTP/1.1 200 OK\r\n")?; // 17
                write!(buf, "Content-Length: {len}\r\n")?; // 18 + len

                if let Some(mime_type) = mime_type {
                    write!(buf, "Content-Type: {mime_type}\r\n")?; // 16 + mime_type
                }

                if len > 0 {
                    body_path = resolved_path;
                }
            }
            Response::Redirect { path, query } => {
                write!(buf, "HTTP/1.1 302 Found\r\n")?; // 20
                write!(buf, "Content-Length: 0\r\n")?; // 19
                write!(buf, "Location: {path}{query}\r\n")?; // 12 + path + query
            }
            Response::NotFound => {
                write!(buf, "HTTP/1.1 404 Not Found\r\n")?; // 24
                write!(buf, "Content-Length: 0\r\n")?; // 19
            }
            Response::StatusStr(status) => {
                write!(buf, "HTTP/1.1 {status}\r\n")?; // 11 + status
                write!(buf, "Content-Length: 0\r\n")?; // 19
            }
        }

        if keep_alive {
            write!(buf, "Connection: keep-alive\r\n\r\n")?; // 26
        } else {
            write!(buf, "Connection: close\r\n\r\n")?;
        };

        conn.write_all(buf.as_mut_slice())?;

        if let Some(path) = body_path {
            let mut f = File::options().read(true).open(path)?;
            std::io::copy(&mut f, conn)?;
        }

        Ok(())
    }
}

pub struct Responses(ResponsesInner);

impl Default for Responses {
    fn default() -> Self {
        Self::new()
    }
}

impl Responses {
    pub fn new() -> Self {
        Self(ResponsesInner::Single(None))
    }

    pub fn push(&mut self, response: Response) {
        match &mut self.0 {
            ResponsesInner::Single(existing) => match existing.take() {
                None => {
                    *existing = Some(response);
                }
                Some(existing) => {
                    self.0 = ResponsesInner::Multiple(vec![existing, response]);
                }
            },
            ResponsesInner::Multiple(v) => {
                v.push(response);
            }
        }
    }

    pub fn pop(&mut self) -> Option<Response> {
        match &mut self.0 {
            ResponsesInner::Single(existing) => existing.take(),
            ResponsesInner::Multiple(v) => v.pop(),
        }
    }

    pub fn drain<'this>(&'this mut self) -> ResponsesDrain<'this> {
        match &mut self.0 {
            ResponsesInner::Single(response) => {
                ResponsesDrain(ResponsesDrainInner::Single(response.take()))
            }
            ResponsesInner::Multiple(v) => {
                ResponsesDrain(ResponsesDrainInner::Multiple(v.drain(..)))
            }
        }
    }
}

enum ResponsesInner {
    Single(Option<Response>),
    Multiple(Vec<Response>),
}

pub struct ResponsesDrain<'resp>(ResponsesDrainInner<'resp>);

enum ResponsesDrainInner<'resp> {
    Single(Option<Response>),
    Multiple(std::vec::Drain<'resp, Response>),
}

impl Iterator for ResponsesDrain<'_> {
    type Item = Response;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            ResponsesDrainInner::Single(response) => response.take(),
            ResponsesDrainInner::Multiple(iter) => iter.next(),
        }
    }
}

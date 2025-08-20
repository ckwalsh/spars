use std::fs::File;
use std::io::Write as _;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;

use compact_str::CompactString;

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
    StatusStr(&'static str),
}

impl Response {
    pub fn write_to(
        self,
        conn: &mut TcpStream,
        buf: &mut Vec<u8>,
        keep_alive: bool,
    ) -> std::io::Result<()> {
        buf.clear();

        let mut body_path = None;

        match self {
            Response::Found {
                resolved_path,
                len,
                mime_type,
            } => {
                // 77 + len + mime_type
                buf.reserve(128);

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
                // 77 + path + query
                buf.reserve(256);

                write!(buf, "HTTP/1.1 302 Found\r\n")?; // 20
                write!(buf, "Content-Length: 0\r\n")?; // 19
                write!(buf, "Location: {path}{query}\r\n")?; // 12 + path + query
            }
            Response::NotFound => {
                // 69
                buf.reserve(128);

                write!(buf, "HTTP/1.1 404 Not Found\r\n")?; // 24
                write!(buf, "Content-Length: 0\r\n")?; // 19
            }
            Response::StatusStr(status) => {
                // 56 + status
                buf.reserve(128);

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

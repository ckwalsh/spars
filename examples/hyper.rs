use std::convert::Infallible;
use std::io::Write as _;
use std::sync::Arc;

use futures_lite::FutureExt;
use futures_util::future::BoxFuture;
use futures_util::TryStreamExt as _;
use http_body_util::combinators::BoxBody;
use http_body_util::Either;
use http_body_util::Empty;
use http_body_util::StreamBody;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::Service;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper_util::rt::TokioIo;
use spars::Handler;
use spars::Settings;
use tokio::net::TcpListener;
use tokio_util::io::ReaderStream;

#[derive(Clone)]
struct Svc(Arc<Handler>);

impl Service<Request<hyper::body::Incoming>> for Svc {
    type Response = Response<Either<Empty<Bytes>, BoxBody<Bytes, std::io::Error>>>;

    type Error = Infallible;

    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn call(&self, req: Request<hyper::body::Incoming>) -> Self::Future {
        let method = Some(req.method().as_str());
        let path = req.uri().path_and_query().map(|p| p.as_str());
        let response = self.0.handle(method, path).unwrap_or_else(|r| r);

        async move {
            match response {
                spars::Response::Found {
                    resolved_path,
                    len,
                    mime_type,
                } => {
                    let mut builder = Response::builder()
                        .status(StatusCode::OK)
                        .header(hyper::header::CONTENT_LENGTH, len);

                    if let Some(mime_type) = mime_type {
                        builder = builder.header(hyper::header::CONTENT_TYPE, mime_type);
                    }

                    if let Some(path) = resolved_path {
                        let file = match tokio::fs::File::open(path).await {
                            Ok(file) => file,
                            Err(_) => {
                                return Ok(Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(Either::Left(Empty::new()))
                                    .unwrap());
                            }
                        };

                        let reader_stream = ReaderStream::new(file);
                        let frame_stream = reader_stream.map_ok(hyper::body::Frame::data);
                        let body = BoxBody::new(StreamBody::new(frame_stream));

                        Ok(builder.body(Either::Right(body)).unwrap())
                    } else {
                        Ok(builder.body(Either::Left(Empty::new())).unwrap())
                    }
                }
                spars::Response::Redirect { path, query } => Ok(Response::builder()
                    .status(StatusCode::FOUND)
                    .header(hyper::header::LOCATION, format!("{path}{query}"))
                    .body(Either::Left(Empty::new()))
                    .unwrap()),
                spars::Response::NotFound => Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Either::Left(Empty::new()))
                    .unwrap()),
                spars::Response::StatusStr(status_code) => {
                    let status = match status_code {
                        spars::StatusCode::BAD_REQUEST => StatusCode::BAD_REQUEST,
                        spars::StatusCode::METHOD_NOT_ALLOWED => StatusCode::METHOD_NOT_ALLOWED,
                        spars::StatusCode::REQUEST_TIMEOUT => StatusCode::REQUEST_TIMEOUT,
                        spars::StatusCode::PAYLOAD_TOO_LARGE => StatusCode::PAYLOAD_TOO_LARGE,
                        spars::StatusCode::URI_TOO_LONG => StatusCode::URI_TOO_LONG,
                        spars::StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE => {
                            StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE
                        }
                        spars::StatusCode::INTERNAL_SERVER_ERROR => {
                            StatusCode::INTERNAL_SERVER_ERROR
                        }
                        spars::StatusCode::NOT_IMPLEMENTED => StatusCode::NOT_IMPLEMENTED,
                        spars::StatusCode::HTTP_VERSION_NOT_SUPPORTED => {
                            StatusCode::HTTP_VERSION_NOT_SUPPORTED
                        }
                    };

                    Ok(Response::builder()
                        .status(status)
                        .body(Either::Left(Empty::new()))
                        .unwrap())
                }
            }
        }
        .boxed()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let settings = Settings::from_env()?;

    if let Some(p) = settings.server.pid_file {
        let pid = std::process::id();
        let mut f = std::fs::File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(p)
            .expect("Failed to open pid file");

        write!(f, "{pid}").expect("Failed to write pid");
    }

    let handler = Arc::new(Handler::try_from(settings.handler)?);

    let svc = Svc(handler);

    // We create a TcpListener and bind it to 127.0.0.1:3000
    let listener = TcpListener::bind(settings.server.addr).await?;

    if let Some(p) = settings.server.port_file {
        let addr = listener.local_addr().expect("Failed to get local address");
        let port = addr.port();

        let mut f = std::fs::File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(p)
            .expect("Failed to open port file");

        write!(f, "{port}").expect("Failed to write port");
    }

    // We start a loop to continuously accept incoming connections
    loop {
        let (stream, _) = listener.accept().await?;

        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn({
            let svc = svc.clone();

            async move {
                // Finally, we bind the incoming connection to our `hello` service
                if let Err(err) = http1::Builder::new()
                    // `service_fn` converts our function in a `Service`
                    .serve_connection(io, svc)
                    .await
                {
                    eprintln!("Error serving connection: {:?}", err);
                }
            }
        });
    }
}

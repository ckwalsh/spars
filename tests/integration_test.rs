// SPDX-FileCopyrightText: 2025 Cullen Walsh <ckwalsh@cullenwalsh.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::Write as _;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use spars_httpd::ExposeHiddenFiles;
use spars_httpd::Handler;
use spars_httpd::HandlerSettings;
use spars_httpd::serve_with_stop_flag;
use tempfile::TempDir;

struct StopGuard {
    flag: Arc<AtomicBool>,
    addr: SocketAddr,
}

impl StopGuard {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            addr,
        }
    }

    fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.flag)
    }
}

impl Drop for StopGuard {
    fn drop(&mut self) {
        self.flag.store(true, Ordering::SeqCst);

        // Wake up the server
        let _ = std::net::TcpStream::connect(self.addr);
    }
}

#[test]
fn test_integration_simple() -> std::io::Result<()> {
    let tmp_dir = TempDir::with_prefix("spars-").expect("Failed to create temp dir");

    let settings = HandlerSettings {
        root: tmp_dir.path().to_owned(),
        index_file: "index.html".to_owned(),
        fallback_path: None,
        expose_hidden: ExposeHiddenFiles::OnlyWellKnown,
    };

    let dirs = vec![
        ".well-known",
        ".git",
        ".well-known/.git",
        ".git/.well-known",
        "subdir_with_index",
        "subdir_without_index",
    ];

    for v in dirs {
        let mut path = settings.root.clone();
        path.push(v);
        std::fs::create_dir_all(path).expect("Failed to create dir");
    }

    let files = vec![
        "index.html",
        "config.json",
        "extensionless",
        ".well-known/foo.txt",
        ".git/foo.txt",
        ".well-known/.git/foo.txt",
        ".git/.well-known/foo.txt",
        "subdir_with_index/index.html",
    ];

    for v in files {
        let mut path = settings.root.clone();
        path.push(v);

        match std::fs::File::create(path) {
            Ok(mut f) => {
                write!(&mut f, "{v}").expect("Failed to write file contents");
            }
            Err(e) => {
                panic!("Failed to create file {v}: {e}");
            }
        }
    }

    let handler = Arc::new(Handler::try_from(settings).expect("Failed to create handler"));

    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?;
    let addr = listener.local_addr()?;

    let cases: Vec<(
        &'static str,
        http::StatusCode,
        &'static str,
        Vec<(http::HeaderName, &'static str)>,
    )> = vec![
        (
            "/index.html",
            http::StatusCode::OK,
            "index.html",
            vec![(http::header::CONTENT_TYPE, "text/html")],
        ),
        (
            "/config.json",
            http::StatusCode::OK,
            "config.json",
            vec![(http::header::CONTENT_TYPE, "application/json")],
        ),
        (
            "/extensionless",
            http::StatusCode::OK,
            "extensionless",
            vec![],
        ),
        (
            "/",
            http::StatusCode::OK,
            "index.html",
            vec![(http::header::CONTENT_TYPE, "text/html")],
        ),
        (
            "/index.html?foo=bar",
            http::StatusCode::OK,
            "index.html",
            vec![(http::header::CONTENT_TYPE, "text/html")],
        ),
        ("/missing.txt", http::StatusCode::NOT_FOUND, "", vec![]),
        (
            "/.well-known/foo.txt",
            http::StatusCode::OK,
            ".well-known/foo.txt",
            vec![(http::header::CONTENT_TYPE, "text/plain")],
        ),
        ("/.git/foo.txt", http::StatusCode::NOT_FOUND, "", vec![]),
        (
            "/.well-known/.git/foo.txt",
            http::StatusCode::NOT_FOUND,
            "",
            vec![],
        ),
        (
            "/.git/.well-known/foo.txt",
            http::StatusCode::NOT_FOUND,
            "",
            vec![],
        ),
        (
            "/subdir_with_index/index.html",
            http::StatusCode::OK,
            "subdir_with_index/index.html",
            vec![(http::header::CONTENT_TYPE, "text/html")],
        ),
        (
            "/subdir_with_index/",
            http::StatusCode::OK,
            "subdir_with_index/index.html",
            vec![(http::header::CONTENT_TYPE, "text/html")],
        ),
        (
            "/subdir_with_index",
            http::StatusCode::FOUND,
            "",
            vec![(http::header::LOCATION, "/subdir_with_index/")],
        ),
        (
            "/subdir_with_index?foo=bar",
            http::StatusCode::FOUND,
            "",
            vec![(http::header::LOCATION, "/subdir_with_index/?foo=bar")],
        ),
    ];

    std::thread::scope(|s| {
        let guard = StopGuard::new(addr);

        s.spawn({
            let stop_flag = guard.flag();
            let listener = listener
                .try_into()
                .expect("failed to create async listener");

            move || {
                serve_with_stop_flag(listener, handler, stop_flag).expect("Server Failed");
            }
        });

        for (uri, expected_status, expected_body, expected_headers) in cases.into_iter() {
            let client = reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("Could not build http client");

            let resp = match client.get(format!("http://{addr}{uri}")).send() {
                Ok(r) => r,
                Err(e) => panic!("{uri}: Request failed: {e}"),
            };

            assert_eq!(resp.status(), expected_status, "{uri}: Status mismatch");

            let headers = resp.headers();
            let len = format!("{}", expected_body.len());
            let mut expected_headers: http::HeaderMap = http::HeaderMap::from_iter(
                expected_headers
                    .into_iter()
                    .map(|(k, v)| (k, v.parse().unwrap())),
            );
            expected_headers.insert(http::header::CONTENT_LENGTH, len.parse().unwrap());
            expected_headers.insert(http::header::CONNECTION, "keep-alive".parse().unwrap());

            for key in headers.keys() {
                assert_eq!(
                    headers.get(key),
                    expected_headers.remove(key).as_ref(),
                    "{uri}: Mismatch for header `{key}`"
                );
            }
            assert_eq!(
                expected_headers,
                http::HeaderMap::new(),
                "{uri}: Header count mismatch"
            );

            assert_eq!(
                resp.text().expect("Invalid Text"),
                expected_body,
                "{uri}: text mismatch"
            );
        }
    });

    Ok(())
}

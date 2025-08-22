use std::ffi::OsStr;
use std::path::Path;

fn mime_from_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "htm" | "html" => Some("text/html"),
        "js" => Some("text/javascript"),
        "css" => Some("text/css"),
        "json" => Some("application/json"),
        "txt" => Some("text/plain"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

pub fn mime_from_path<P: AsRef<Path>>(path: P) -> Option<&'static str> {
    path.as_ref()
        .extension()
        .and_then(OsStr::to_str)
        .and_then(mime_from_ext)
}

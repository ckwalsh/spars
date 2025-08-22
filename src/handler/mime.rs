// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(not(feature = "mime_guess"))]
mod fallback;
#[cfg(feature = "mime_guess")]
mod mime_guess;

#[cfg(not(feature = "mime_guess"))]
pub use fallback::*;
#[cfg(feature = "mime_guess")]
pub use mime_guess::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assumed_exts_on_paths() {
        assert_eq!(mime_from_path("/index.htm"), Some("text/html"));
        assert_eq!(mime_from_path("/index.html"), Some("text/html"));
        assert_eq!(mime_from_path("/index.js"), Some("text/javascript"));
        assert_eq!(mime_from_path("/index.css"), Some("text/css"));
        assert_eq!(mime_from_path("/index.json"), Some("application/json"));
        assert_eq!(mime_from_path("/index.png"), Some("image/png"));
        assert_eq!(mime_from_path("/index.jpg"), Some("image/jpeg"));
        assert_eq!(mime_from_path("/index.jpeg"), Some("image/jpeg"));
        assert_eq!(mime_from_path("/index.gif"), Some("image/gif"));
        assert_eq!(mime_from_path("/index.svg"), Some("image/svg+xml"));

        assert_eq!(mime_from_path("/noextension"), None);
        assert_eq!(mime_from_path("/html"), None);
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;

use mime_guess::MimeGuess;

pub fn mime_from_path<P: AsRef<Path>>(path: P) -> Option<&'static str> {
    MimeGuess::from_path(path).first_raw()
}

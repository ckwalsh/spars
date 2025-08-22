// SPDX-FileCopyrightText: 2025 Cullen Walsh <ckwalsh@cullenwalsh.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::str::FromStr;

pub struct HandlerSettings {
    pub root: PathBuf,
    pub index_file: String,
    pub fallback_path: Option<String>,
    pub expose_hidden: ExposeHiddenFiles,
}

#[derive(Default)]
pub enum ExposeHiddenFiles {
    #[default]
    OnlyWellKnown,
    Hide,
    Expose,
}

impl From<&str> for ExposeHiddenFiles {
    fn from(s: &str) -> Self {
        if s.eq_ignore_ascii_case("hide") {
            Self::Hide
        } else if s.eq_ignore_ascii_case("expose") {
            Self::Expose
        } else if s.eq_ignore_ascii_case("wellknown") {
            Self::OnlyWellKnown
        } else if let Ok(expose) = bool::from_str(s) {
            if expose {
                Self::Expose
            } else {
                Self::Hide
            }
        } else {
            Self::OnlyWellKnown
        }
    }
}

impl Default for HandlerSettings {
    fn default() -> Self {
        Self {
            root: PathBuf::from("./public"),
            index_file: "index.html".to_owned(),
            fallback_path: Some("404.html".to_owned()),
            expose_hidden: Default::default(),
        }
    }
}

type Result<T, E = super::SettingsFromEnvError> = std::result::Result<T, E>;

impl HandlerSettings {
    pub fn from_env() -> Result<Self> {
        let mut settings = Self::default();

        if let Some(s) = super::env("ROOT")? {
            settings.root = PathBuf::from(s)
                .canonicalize()
                .map_err(super::SettingsFromEnvError::BadFilePath)?;

            let metadata = settings
                .root
                .metadata()
                .map_err(super::SettingsFromEnvError::BadFilePath)?;

            if !metadata.is_dir() {
                return Err(super::SettingsFromEnvError::RootNotADirectory);
            }
        }

        if let Some(s) = super::env("INDEX_FILE")? {
            settings.index_file = s;
        }

        if let Some(s) = super::env("FALLBACK_PATH")? {
            settings.fallback_path = if s.is_empty() { None } else { Some(s) };
        }

        if let Some(s) = super::env("EXPOSE_HIDDEN")? {
            settings.expose_hidden = s.as_str().into();
        }

        Ok(settings)
    }
}

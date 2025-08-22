// SPDX-FileCopyrightText: 2025 Cullen Walsh <ckwalsh@cullenwalsh.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::env::VarError;
use std::net::AddrParseError;
use std::num::ParseIntError;
use std::str::ParseBoolError;

use thiserror::Error;

mod handler;
mod server;

pub use handler::*;
pub use server::*;

#[derive(Default)]
pub struct Settings {
    pub server: ServerSettings,
    pub handler: HandlerSettings,
}

#[derive(Debug, Error)]
pub enum SettingsFromEnvError {
    #[error("VarError while reading `{0}`: {1}")]
    VarError(&'static str, VarError),

    #[error("Bad file path: {0}")]
    BadFilePath(std::io::Error),

    #[error("Bad bind address: {0}")]
    BadAddr(AddrParseError),

    #[error("Bad port: {0}")]
    BadPort(ParseIntError),

    #[error("Root is not a directory")]
    RootNotADirectory,

    #[error("Bad allow hidden value: {0}")]
    BadAllowHidden(ParseBoolError),
}

type Result<T, E = SettingsFromEnvError> = std::result::Result<T, E>;

impl Settings {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            server: ServerSettings::from_env()?,
            handler: HandlerSettings::from_env()?,
        })
    }
}

fn env(key: &'static str) -> Result<Option<String>> {
    match std::env::var(key) {
        Ok(s) => Ok(Some(s)),
        Err(VarError::NotPresent) => Ok(None),
        Err(e) => Err(super::SettingsFromEnvError::VarError(key, e)),
    }
}

use std::env::VarError;
use std::net::AddrParseError;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::num::ParseIntError;
use std::path::PathBuf;
use std::str::FromStr;
use std::str::ParseBoolError;

use thiserror::Error;

pub struct Settings {
    pub addr: SocketAddr,
    pub root: PathBuf,
    pub index_file: String,
    pub fallback_path: Option<String>,
    pub allow_hidden: bool,
}

const DEFAULT_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const DEFAULT_PORT: u16 = 3000;
const DEFAULT_ROOT: &str = "./public";
const DEFAULT_INDEX_FILE: &str = "index.html";
const DEFAULT_FALLBACK_PATH: Option<&str> = None;
const DEFAULT_ALLOW_HIDDEN: bool = false;

impl Default for Settings {
    fn default() -> Self {
        Self {
            addr: SocketAddr::new(DEFAULT_ADDR, DEFAULT_PORT),
            root: PathBuf::from(DEFAULT_ROOT),
            index_file: DEFAULT_INDEX_FILE.to_owned(),
            fallback_path: DEFAULT_FALLBACK_PATH.map(str::to_owned),
            allow_hidden: DEFAULT_ALLOW_HIDDEN,
        }
    }
}

const VAR_ADDR: &str = "ADDR";
const VAR_PORT: &str = "PORT";
const VAR_ROOT: &str = "ROOT";
const VAR_INDEX_FILE: &str = "INDEX_FILE";
const VAR_FALLBACK_PATH: &str = "FALLBACK_PATH";
const VAR_ALLOW_HIDDEN: &str = "ALLOW_HIDDEN";

#[derive(Debug, Error)]
pub enum SettingsFromEnvError {
    #[error("VarError while reading `{0}`: {1}")]
    VarError(&'static str, VarError),

    #[error("Bad bind address: {0}")]
    BadAddr(AddrParseError),

    #[error("Bad port: {0}")]
    BadPort(ParseIntError),

    #[error("Bad root: {0}")]
    BadRoot(std::io::Error),

    #[error("Bad allow hidden value: {0}")]
    BadAllowHidden(ParseBoolError),
}

type Result<T, E = SettingsFromEnvError> = std::result::Result<T, E>;

impl Settings {
    pub fn from_env() -> Result<Self> {
        let addr = SocketAddr::new(Self::addr_from_env()?, Self::port_from_env()?);
        let root = Self::root_from_env()?;
        let index_file = Self::index_file_from_env()?;
        let fallback_path = Self::fallback_path_from_env()?;
        let allow_hidden = Self::allow_hidden_from_env()?;

        Ok(Self {
            addr,
            root,
            index_file,
            fallback_path,
            allow_hidden,
        })
    }

    fn addr_from_env() -> Result<IpAddr> {
        Self::var(VAR_ADDR)?
            .map(|s| IpAddr::from_str(&s))
            .unwrap_or(Ok(DEFAULT_ADDR))
            .map_err(SettingsFromEnvError::BadAddr)
    }

    fn port_from_env() -> Result<u16> {
        Self::var(VAR_PORT)?
            .map(|s| u16::from_str(&s))
            .unwrap_or(Ok(DEFAULT_PORT))
            .map_err(SettingsFromEnvError::BadPort)
    }

    fn root_from_env() -> Result<PathBuf> {
        let root = PathBuf::from(Self::var(VAR_ROOT)?.unwrap_or(DEFAULT_ROOT.to_owned()))
            .canonicalize()
            .map_err(SettingsFromEnvError::BadRoot)?;

        let metadata = root.metadata().map_err(SettingsFromEnvError::BadRoot)?;

        if metadata.is_dir() {
            Ok(root)
        } else {
            Err(SettingsFromEnvError::BadRoot(
                std::io::ErrorKind::NotADirectory.into(),
            ))
        }
    }

    fn index_file_from_env() -> Result<String> {
        Ok(Self::var(VAR_INDEX_FILE)?.unwrap_or_else(|| DEFAULT_INDEX_FILE.to_owned()))
    }

    fn fallback_path_from_env() -> Result<Option<String>> {
        Ok(Self::var(VAR_FALLBACK_PATH)?.or_else(|| DEFAULT_FALLBACK_PATH.map(str::to_owned)))
    }

    fn allow_hidden_from_env() -> Result<bool> {
        Self::var(VAR_ALLOW_HIDDEN)?
            .map(|s| bool::from_str(&s))
            .unwrap_or(Ok(DEFAULT_ALLOW_HIDDEN))
            .map_err(SettingsFromEnvError::BadAllowHidden)
    }

    fn var(key: &'static str) -> Result<Option<String>> {
        match std::env::var(key) {
            Ok(s) => Ok(Some(s)),
            Err(VarError::NotPresent) => Ok(None),
            Err(e) => Err(SettingsFromEnvError::VarError(key, e)),
        }
    }
}

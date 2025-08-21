use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

pub struct ServerSettings {
    pub addr: SocketAddr,
    pub pid_file: Option<PathBuf>,
    pub addr_file: Option<PathBuf>,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000),
            pid_file: None,
            addr_file: None,
        }
    }
}

type Result<T, E = super::SettingsFromEnvError> = std::result::Result<T, E>;

impl ServerSettings {
    pub fn from_env() -> Result<Self> {
        let mut settings = Self::default();

        if let Some(s) = super::env("ADDR")? {
            settings
                .addr
                .set_ip(IpAddr::from_str(&s).map_err(super::SettingsFromEnvError::BadAddr)?);
        }

        if let Some(s) = super::env("PORT")? {
            settings
                .addr
                .set_port(u16::from_str(&s).map_err(super::SettingsFromEnvError::BadPort)?);
        }

        if let Some(s) = super::env("PID_FILE")? {
            settings.pid_file = Some(PathBuf::from(s));
        }

        if let Some(s) = super::env("ADDR_FILE")? {
            settings.addr_file = Some(PathBuf::from(s));
        }

        Ok(settings)
    }
}

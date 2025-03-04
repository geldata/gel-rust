use gel_stream::{ResolvedTarget, TargetName};
use std::net::{IpAddr, Ipv6Addr};

#[cfg(feature = "serde")]
use serde::Serialize;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostTarget {
    PostgreSQL,
    Gel,
    GelAdmin,
}

impl HostTarget {
    fn target_name(&self) -> Result<&'static str, std::io::Error> {
        match self {
            HostTarget::PostgreSQL => Ok("PGSQL"),
            HostTarget::Gel => Ok("EDGEDB"),
            HostTarget::GelAdmin => Ok("EDGEDB.admin"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct Host(pub HostType, pub u16);

impl Host {
    pub fn target_name(&self, target: HostTarget) -> Result<TargetName, std::io::Error> {
        match &self.0 {
            HostType::Hostname(hostname) => Ok(TargetName::new_tcp((hostname, self.1))),
            HostType::IP(ip, Some(interface)) => Ok(TargetName::new_tcp((
                format!("{}%{}", ip, interface),
                self.1,
            ))),
            HostType::IP(ip, None) => Ok(TargetName::new_tcp((format!("{}", ip), self.1))),
            HostType::Path(path) => TargetName::new_unix_path(format!(
                "{}/.s.{}.{}",
                path,
                target.target_name()?,
                self.1
            )),
            #[allow(unused)]
            HostType::Abstract(name) => {
                #[cfg(any(target_os = "linux", target_os = "android"))]
                {
                    TargetName::new_unix_domain(format!(
                        "{}/.s.{}.{}",
                        name,
                        target.target_name()?,
                        self.1
                    ))
                }
                #[cfg(not(any(target_os = "linux", target_os = "android")))]
                {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "Abstract sockets unsupported on this platform",
                    ))
                }
            }
        }
    }

    pub fn is_unix(&self) -> bool {
        matches!(self.0, HostType::Path(_) | HostType::Abstract(_))
    }
}

pub trait ToAddrsSyncVec {
    fn to_addrs_sync(
        &self,
        target: HostTarget,
    ) -> Vec<(Host, Result<Vec<ResolvedTarget>, std::io::Error>)>;
}

impl ToAddrsSyncVec for Vec<Host> {
    fn to_addrs_sync(
        &self,
        target: HostTarget,
    ) -> Vec<(Host, Result<Vec<ResolvedTarget>, std::io::Error>)> {
        let mut result = Vec::with_capacity(self.len());
        for host in self {
            match host.target_name(target) {
                Ok(target_name) => match target_name.to_addrs_sync() {
                    Ok(addrs) => result.push((host.clone(), Ok(addrs))),
                    Err(err) => result.push((host.clone(), Err(err))),
                },
                Err(err) => {
                    result.push((host.clone(), Err(err)));
                }
            }
        }
        result
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub enum HostType {
    Hostname(String),
    IP(IpAddr, Option<String>),
    Path(String),
    Abstract(String),
}

impl std::fmt::Display for HostType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostType::Hostname(hostname) => write!(f, "{}", hostname),
            HostType::IP(ip, Some(interface)) => write!(f, "{}%{}", ip, interface),
            HostType::IP(ip, None) => {
                write!(f, "{}", ip)
            }
            HostType::Path(path) => write!(f, "{}", path),
            HostType::Abstract(name) => write!(f, "@{}", name),
        }
    }
}

impl HostType {
    pub fn try_from_str(s: &str) -> Result<Self, &str> {
        if s.is_empty() {
            return Err(s);
        }
        if s.contains('[') || s.contains(']') {
            return Err(s);
        }
        if s.starts_with('/') {
            return Ok(HostType::Path(s.to_string()));
        }
        if let Some(s) = s.strip_prefix('@') {
            return Ok(HostType::Abstract(s.to_string()));
        }
        if s.contains('%') {
            let (ip_str, interface) = s.split_once('%').unwrap();
            if interface.is_empty() {
                return Err(s);
            }
            let ip = ip_str.parse::<Ipv6Addr>().map_err(|_| s)?;
            return Ok(HostType::IP(IpAddr::V6(ip), Some(interface.to_string())));
        }
        if let Ok(ip) = s.parse::<IpAddr>() {
            Ok(HostType::IP(ip, None))
        } else {
            if s.contains(':') || s.contains(',') {
                return Err(s);
            }
            Ok(HostType::Hostname(s.to_string()))
        }
    }
}

pub struct HostParseError;

impl std::str::FromStr for HostType {
    type Err = HostParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        HostType::try_from_str(s).map_err(|_| HostParseError)
    }
}

impl<S: AsRef<str>> From<&url::Host<S>> for HostType {
    fn from(host: &url::Host<S>) -> Self {
        match host {
            url::Host::Domain(domain) => HostType::Hostname(domain.as_ref().to_string()),
            url::Host::Ipv4(ip) => HostType::IP(IpAddr::V4(*ip), None),
            url::Host::Ipv6(ip) => HostType::IP(IpAddr::V6(*ip), None),
        }
    }
}

//! Parses DSNs for Gel database connections.

mod branding;
mod config;
mod credentials;
mod duration;
mod env;
pub mod error;
mod instance_name;
mod param;
mod params;
mod project;
mod stored;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::{
    env::SystemEnvVars, file::SystemFileAccess, user::SystemUserProfile, EnvVar, FileAccess,
    UserProfile,
};
pub use config::*;
pub use credentials::*;
use error::Warning;
pub use instance_name::*;
pub use param::*;
pub use params::*;

#[cfg(feature = "unstable")]
pub use env::define_env;

#[cfg(feature = "unstable")]
pub use project::{Project, ProjectDir, ProjectSearchResult};

#[cfg(feature = "unstable")]
pub use stored::{StoredCredentials, StoredInformation};

/// Internal helper to parse a duration string into a `std::time::Duration`.
#[doc(hidden)]
pub fn parse_duration(s: &str) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
    use std::str::FromStr;
    Ok(std::time::Duration::from_micros(
        duration::Duration::from_str(s)?.micros as u64,
    ))
}

/// Internal helper to format a `std::time::Duration` into a duration string.
#[doc(hidden)]
pub fn format_duration(d: &std::time::Duration) -> String {
    duration::Duration::from_micros(d.as_micros() as i64).to_string()
}

fn config_dirs<U: UserProfile>(user: &U) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if cfg!(unix) {
        if let Some(dir) = user.config_dir() {
            dirs.push(dir.join("edgedb"));
            dirs.push(dir.join("gel"));
        }
    }
    if cfg!(windows) {
        if let Some(dir) = user.data_local_dir() {
            dirs.push(dir.join("EdgeDB").join("config"));
            dirs.push(dir.join("Gel").join("config"));
        }
    }
    dirs
}

type LoggingFn = Box<dyn Fn(&str) + 'static>;
type WarningFn = Box<dyn Fn(Warning) + 'static>;

#[derive(Default)]
pub(crate) struct Logging {
    tracing: Option<LoggingFn>,
    warning: Option<WarningFn>,
    #[cfg(feature = "log")]
    log_trace: bool,
    #[cfg(feature = "log")]
    log_warning: bool,
}

impl Logging {
    fn trace(&self, message: impl Fn(&dyn Fn(&str))) {
        let mut needs_trace = false;
        #[cfg(feature = "log")]
        let auto_trace = cfg!(feature = "auto-log-trace");

        #[cfg(feature = "log")]
        {
            if self.log_trace || auto_trace {
                needs_trace = log::log_enabled!(log::Level::Trace);
            }
        }

        if self.tracing.is_some() {
            needs_trace = true;
        }

        if needs_trace {
            message(&|message| {
                #[cfg(feature = "log")]
                {
                    if self.log_trace || auto_trace {
                        log::trace!("{}", message);
                    }
                }
                {
                    if let Some(tracing) = &self.tracing {
                        tracing(message);
                    }
                }
            });
        }
    }

    fn warn(&self, warning: Warning) {
        #[cfg(feature = "log")]
        {
            let auto_warning = cfg!(feature = "auto-log-warning");
            if self.log_warning || auto_warning {
                log::warn!("{}", warning);
            }
        }
        if let Some(warning_fn) = &self.warning {
            warning_fn(warning);
        }
    }
}

/// A collection of warnings.
///
/// To collect warnings from a [`Builder`], pass a [`Warnings`] instance to the
/// [`Builder::with_warnings`] method:
///
/// ```
/// # use gel_dsn::gel::*;
/// let warnings = Warnings::default();
/// let builder = Builder::new().without_system().with_warning(warnings.clone().warn_fn());
/// ```
#[derive(Default, Clone)]
pub struct Warnings {
    warnings: Arc<Mutex<Vec<Warning>>>,
}

impl Warnings {
    pub fn into_vec(self) -> Vec<Warning> {
        match Arc::try_unwrap(self.warnings) {
            Ok(mutex) => mutex.into_inner().unwrap(),
            Err(arc) => arc.lock().unwrap().clone(),
        }
    }

    pub fn warn(&self, warning: Warning) {
        let mut warnings = self.warnings.lock().unwrap();
        warnings.push(warning);
    }

    pub fn warn_fn(self) -> WarningFn {
        Box::new(move |warning| self.warn(warning))
    }
}

/// A collection of trace messages.
///
/// To collect traces from a [`Builder`], pass a [`Traces`] instance to the
/// `Builder`'s [`with_tracing`] method:
///
/// ```
/// # use gel_dsn::gel::*;
/// let traces = Traces::default();
/// let builder = Builder::new().without_system().with_tracing(traces.clone().trace_fn());
/// ```
#[derive(Default, Clone)]
pub struct Traces {
    traces: Arc<Mutex<Vec<String>>>,
}

impl Traces {
    pub fn into_vec(self) -> Vec<String> {
        match Arc::try_unwrap(self.traces) {
            Ok(mutex) => mutex.into_inner().unwrap(),
            Err(arc) => arc.lock().unwrap().clone(),
        }
    }

    pub fn trace(&self, message: &str) {
        let mut traces = self.traces.lock().unwrap();
        traces.push(message.to_string());
    }

    pub fn trace_fn(self) -> LoggingFn {
        Box::new(move |message| self.trace(message))
    }
}

pub(crate) struct BuildContextImpl<E: EnvVar = SystemEnvVars, F: FileAccess = SystemFileAccess> {
    env: E,
    files: F,
    pub config_dir: Option<Vec<PathBuf>>,
    pub(crate) logging: Logging,
}

impl Default for BuildContextImpl<SystemEnvVars, SystemFileAccess> {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildContextImpl<SystemEnvVars, SystemFileAccess> {
    /// Create a new build context with default values.
    pub fn new() -> Self {
        Self {
            env: SystemEnvVars,
            files: SystemFileAccess,
            config_dir: Some(config_dirs(&SystemUserProfile)),
            logging: Logging::default(),
        }
    }
}

impl<E: EnvVar, F: FileAccess> BuildContextImpl<E, F> {
    /// Create a new build context with default values.
    pub fn new_with_user_profile<U: UserProfile>(env: E, files: F, user: U) -> Self {
        let config_dir = config_dirs(&user);
        Self {
            env,
            files,
            config_dir: Some(config_dir),
            logging: Logging::default(),
        }
    }

    #[cfg(test)]
    /// Create a new build context with default values.
    pub fn new_with(env: E, files: F) -> Self {
        Self {
            env,
            files,
            config_dir: None,
            logging: Logging::default(),
        }
    }
}

macro_rules! context_trace {
    ($context:expr, $message:expr $(, $arg:expr)*) => {
        $context.trace(|f: &dyn Fn(&str)| f(&format!($message, $($arg),*)));
    };
}

pub(crate) use context_trace;

pub(crate) trait BuildContext {
    fn cwd(&self) -> Option<PathBuf>;
    fn files(&self) -> &impl FileAccess;
    fn warn(&self, warning: error::Warning);
    fn read_config_file<T: FromParamStr>(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<Option<T>, T::Err>;
    fn write_config_file(
        &self,
        path: impl AsRef<Path>,
        content: &str,
    ) -> Result<(), std::io::Error>;
    fn delete_config_file(&self, path: impl AsRef<Path>) -> Result<(), std::io::Error>;
    fn find_config_path(&self, path: impl AsRef<Path>) -> std::io::Result<PathBuf>;
    fn list_config_files(&self, path: impl AsRef<Path>) -> Result<Vec<PathBuf>, std::io::Error>;
    fn read_env(&self, name: &str) -> Result<std::borrow::Cow<str>, std::env::VarError>;
    fn trace(&self, message: impl Fn(&dyn Fn(&str)));
}

impl<E: EnvVar, F: FileAccess> BuildContext for BuildContextImpl<E, F> {
    fn cwd(&self) -> Option<PathBuf> {
        self.files.cwd()
    }

    fn files(&self) -> &impl FileAccess {
        &self.files
    }

    fn warn(&self, warning: error::Warning) {
        self.logging.warn(warning);
    }

    fn read_config_file<T: FromParamStr>(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<Option<T>, T::Err> {
        for config_dir in self.config_dir.iter().flatten() {
            let path = config_dir.join(path.as_ref());
            context_trace!(self, "Reading config file: {}", path.display());
            if let Ok(file) = self.files.read(&path) {
                // TODO?
                let res = T::from_param_str(&file, self);
                context_trace!(
                    self,
                    "File content: {:?}",
                    res.as_ref().map(|_| ()).map_err(|_| ())
                );
                return match res {
                    Ok(value) => Ok(Some(value)),
                    Err(e) => Err(e),
                };
            }
        }

        Ok(None)
    }

    fn write_config_file(
        &self,
        path: impl AsRef<Path>,
        content: &str,
    ) -> Result<(), std::io::Error> {
        let path = path.as_ref();
        // TODO: We need to be able to handle multiple config dirs. For now, just
        // use the first one.
        #[allow(clippy::never_loop)]
        for config_dir in self.config_dir.iter().flatten() {
            let path = config_dir.join(path);
            context_trace!(self, "Writing config file: {}", path.display());
            self.files.write(&path, content)?;
            return Ok(());
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Config path not found",
        ))
    }

    /// Delete a configuration file. If the file does not exist, this is a no-op.
    fn delete_config_file(&self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let path = path.as_ref();
        let mut res = Ok(());

        // Attempt to delete from all configuration directories, ignoring
        // non-existent files.
        for config_dir in self.config_dir.iter().flatten() {
            let path = config_dir.join(path);
            context_trace!(self, "Deleting config file: {}", path.display());
            if let Err(e) = self.files.delete(&path) {
                if e.kind() == std::io::ErrorKind::NotFound {
                    continue;
                }
                context_trace!(self, "Failed to delete config file: {}", e);
                res = Err(e);
            }
        }

        res
    }

    fn list_config_files(&self, path: impl AsRef<Path>) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut files = Vec::new();
        for config_dir in self.config_dir.iter().flatten() {
            let path = config_dir.join(path.as_ref());
            context_trace!(self, "Checking config path: {}", path.display());
            for file in self.files.list_dir(&path)? {
                context_trace!(self, "Found config file: {}", file.display());
                files.push(file);
            }
        }

        Ok(files)
    }

    fn find_config_path(&self, path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
        for config_dir in self.config_dir.iter().flatten() {
            context_trace!(self, "Checking config path: {}", config_dir.display());
            if matches!(self.files.exists_dir(config_dir), Ok(true)) {
                return Ok(config_dir.join(path));
            }
        }

        // If we couldn't find an existing one, use the first config dir
        if let Some(config_dir) = self.config_dir.iter().flatten().next() {
            return Ok(config_dir.join(path));
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Config file not found",
        ))
    }

    fn read_env(&self, name: &str) -> Result<std::borrow::Cow<str>, std::env::VarError> {
        self.env.read(name)
    }

    fn trace(&self, message: impl Fn(&dyn Fn(&str))) {
        self.logging.trace(message);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::host::{Host, HostType};
    use std::{collections::HashMap, time::Duration};

    #[test]
    fn test_parse() {
        let cfg = Builder::default()
            .dsn("edgedb://hostname:1234")
            .without_system()
            .build();

        assert_eq!(
            cfg.unwrap(),
            Config {
                host: Host::new(HostType::try_from_str("hostname").unwrap(), 1234,),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_credentials_file() {
        let credentials = json!({
            "port": 10702,
            "user": "test3n",
            "password": "lZTBy1RVCfOpBAOwSCwIyBIR",
            "database": "test3n"
        });

        let credentials_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(credentials_file.path(), credentials.to_string()).unwrap();

        let credentials = Builder::new()
            .credentials_file(credentials_file.path())
            .with_fs()
            .build()
            .expect("Failed to build credentials");

        assert_eq!(credentials.host, Host::new(DEFAULT_HOST.clone(), 10702));
        assert_eq!(&credentials.user, "test3n");
        assert_eq!(
            credentials.db,
            DatabaseBranch::Database("test3n".to_string())
        );
        assert_eq!(
            credentials.authentication,
            Authentication::Password("lZTBy1RVCfOpBAOwSCwIyBIR".into())
        );
    }

    #[test]
    fn test_schemes() {
        let dsn_schemes = ["edgedb", "gel"];
        for dsn_scheme in dsn_schemes {
            let cfg = Builder::new()
                .dsn(format!("{dsn_scheme}://localhost:1756"))
                .build()
                .unwrap();

            let host = cfg.host.target_name().unwrap();
            assert_eq!(host.host(), Some("localhost".into()));
            assert_eq!(host.port(), Some(1756));
        }
    }

    #[test]
    fn test_unix_path() {
        // Test unix path without a port
        let cfg = Builder::new()
            .unix_path("/test/.s.EDGEDB.8888")
            .build()
            .unwrap();

        let host = cfg.host.target_name().unwrap();
        assert_eq!(host.path(), Some(Path::new("/test/.s.EDGEDB.8888")));

        // Test unix path with a port
        let cfg = Builder::new()
            .port(8888)
            .unix_path("/test")
            .build()
            .unwrap();
        let host = cfg.host.target_name().unwrap();
        assert_eq!(host.path(), Some(Path::new("/test")));

        // Test unix path with a port
        let cfg = Builder::new()
            .port(8888)
            .unix_path(UnixPath::with_port_suffix(PathBuf::from("/prefix.")))
            .build()
            .unwrap();
        let host = cfg.host.target_name().unwrap();
        assert_eq!(host.path(), Some(Path::new("/prefix.8888")));
    }

    /// Test that the hidden CloudCerts env var is parsed correctly.
    #[test]
    fn test_cloud_certs() {
        let cloud_cert =
            HashMap::from_iter([("_GEL_CLOUD_CERTS".to_string(), "local".to_string())]);
        let cfg = Builder::new()
            .port(5656)
            .without_system()
            .with_env_impl(cloud_cert)
            .build()
            .unwrap();
        assert_eq!(cfg.cloud_certs, Some(CloudCerts::Local));
    }

    #[test]
    fn test_tcp_keepalive() {
        let cfg = Builder::new()
            .port(5656)
            .tcp_keepalive(TcpKeepalive::Explicit(Duration::from_secs(10)))
            .without_system()
            .build()
            .unwrap();
        assert_eq!(
            cfg.tcp_keepalive,
            TcpKeepalive::Explicit(Duration::from_secs(10))
        );
    }
}

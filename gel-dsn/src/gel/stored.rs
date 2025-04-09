use std::{path::PathBuf, str::FromStr, sync::Arc};
use crate::gel::error::Warning;

use super::{
    context_trace, error::ParseError, BuildContext, CredentialsFile, InstanceName,
    DEFAULT_BRANCH_NAME_CONNECT, DEFAULT_DATABASE_NAME,
};

/// Read and write stored information such as [`CredentialsFile`] and [`Project`].
#[allow(private_bounds)]
pub struct StoredInformation<C: BuildContext> {
    context: Arc<C>,
}

#[cfg(feature = "unstable")]
#[allow(private_bounds)]
impl<C: BuildContext> StoredInformation<C> {
    pub(crate) fn new(context: C) -> Self {
        let context = Arc::new(context);

        Self { context }
    }

    pub fn paths(&self) -> Paths<C, Arc<C>> {
        Paths::new(self.context.clone())
    }

    pub fn credentials(&self) -> StoredCredentials<C, Arc<C>> {
        StoredCredentials::new(self.context.clone())
    }
}

pub struct Paths<CT: BuildContext, C: std::ops::Deref<Target = CT>> {
    context: C,
    _marker: std::marker::PhantomData<CT>,
}

impl<CT: BuildContext, C: std::ops::Deref<Target = CT>> Paths<CT, C> {
    pub fn new(context: C) -> Self {
        Self {
            context,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn for_system(&self) -> SystemPaths {
        let paths = self.context.paths();
        SystemPaths {
            home_dir: paths.homedir.clone(),
            config_dir: paths.config_dirs.first().cloned(),
            data_local_dir: paths.data_local_dir.clone(),
            data_dir: paths.data_dir.clone(),
            cache_dir: paths.cache_dir.clone(),
        }
    }

    pub fn for_instance(&self, local_name: &str) -> Option<InstancePaths> {
        if let (Some(data_dir), Some(config_dir)) = (
            &self.context.paths().data_dir,
            &self.context.paths().config_dir,
        ) {
            Some(InstancePaths {
                data_dir: data_dir.join("data").join(local_name),
                credentials_path: config_dir
                    .join("credentials")
                    .join(format!("{}.json", local_name)),
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct SystemPaths {
    /// The user's home directory.
    pub home_dir: Option<PathBuf>,

    /// The resolved system configuration directory, including the branding name.
    ///
    /// Configuration data should be stored in this directory.
    pub config_dir: Option<PathBuf>,

    /// The resolved system data directory, including the branding name.
    ///
    /// Data files expected to be shared between machines should be stored in
    /// this directory.
    pub data_dir: Option<PathBuf>,

    /// The resolved system local data directory, including the branding name.
    ///
    /// Data files expected to be local to a single machine should be stored in
    /// this directory. Not all platforms may support this.
    pub data_local_dir: Option<PathBuf>,

    /// The resolved system cache directory, including the branding name.
    ///
    /// Cache files should be stored in this directory, and will not be
    /// guaranteed to survive between invocations of the program.
    pub cache_dir: Option<PathBuf>,
}

impl SystemPaths {}

#[derive(Debug)]
pub struct InstancePaths {
    /// The base data path for an instance.
    pub data_dir: PathBuf,
    /// The path to the credentials file for an instance.
    pub credentials_path: PathBuf,
}

impl InstancePaths {}

/// The persistent collection of stored credentials.
#[allow(private_bounds)]
pub struct StoredCredentials<CT: BuildContext, C: std::ops::Deref<Target = CT>> {
    context: C,
    _marker: std::marker::PhantomData<CT>,
}

#[allow(private_bounds)]
impl<'a, CT: BuildContext> StoredCredentials<CT, &'a CT> {
    pub(crate) fn new_ref(context: &'a CT) -> Self {
        Self {
            context,
            _marker: std::marker::PhantomData,
        }
    }
}

#[allow(private_bounds)]
impl<CT: BuildContext, C: std::ops::Deref<Target = CT>> StoredCredentials<CT, C> {
    pub(crate) fn new(context: C) -> Self {
        Self {
            context,
            _marker: std::marker::PhantomData,
        }
    }

    /// List all stored credentials.
    pub fn list(&self) -> Result<Vec<InstanceName>, std::io::Error> {
        let files = self.context.list_config_files("credentials/")?;
        let mut instances = Vec::new();
        for mut file in files {
            if file.extension() != Some(std::ffi::OsStr::new("json")) {
                continue;
            }
            if !file.set_extension("") {
                continue;
            }
            let Some(instance) = file.file_name() else {
                context_trace!(
                    self.context,
                    "Skipping file without a name: {}",
                    file.display()
                );
                continue;
            };
            let Some(s) = instance.to_str() else {
                context_trace!(
                    self.context,
                    "Skipping file with non-UTF-8 name: {}",
                    file.display()
                );
                continue;
            };
            let Ok(instance) = InstanceName::from_str(s) else {
                context_trace!(
                    self.context,
                    "Skipping file with invalid instance name: {}",
                    file.display()
                );
                continue;
            };
            instances.push(instance);
        }
        Ok(instances)
    }

    /// Read the credentials for the given instance.
    pub fn read(&self, instance: InstanceName) -> Result<Option<CredentialsFile>, ParseError> {
        let path = format!("credentials/{instance}.json");
        let content = self.context.read_config_file::<CredentialsFile>(&path)?;
        if let Some(content) = &content {
            if !content.warnings().is_empty() {
                if let Ok(s) = serde_json::to_string(content) {
                    self.context.warn(Warning::UpdatedOutdatedCredentials);
                    if self.context.write_config_file(&path, &s).is_ok() {
                        context_trace!(
                            self.context,
                            "Updated out-of-date credentials file: {path}"
                        );
                    } else {
                        context_trace!(
                            self.context,
                            "Failed to update credentials file: {path}"
                        );
                    }
                } else {
                    context_trace!(self.context, "Failed to serialize credentials");
                }
            }
        }
        Ok(content)
    }

    /// Write the credentials for the given instance.
    pub fn write(
        &self,
        instance: InstanceName,
        content: &CredentialsFile,
    ) -> Result<(), std::io::Error> {
        let mut content = content.clone();
        // Special case: treat database=__default__ and branch=edgedb as not set
        if content.database.as_deref() == Some(DEFAULT_DATABASE_NAME)
            && content.branch.as_deref() == Some(DEFAULT_BRANCH_NAME_CONNECT)
        {
            content.database = None;
            content.branch = None;
        }
        let path = format!("credentials/{instance}.json");
        self.context
            .write_config_file(path, &serde_json::to_string(&content)?)
    }

    /// Delete the credentials for the given instance. If the credentials
    /// do not exist, this is a no-op.
    pub fn delete(&self, instance: InstanceName) -> Result<(), std::io::Error> {
        let path = format!("credentials/{instance}.json");
        self.context.delete_config_file(&path)
    }
}

#[cfg(test)]
mod tests {
    use crate::gel::{Builder, CredentialsFile, InstanceName};
    use crate::FileAccess;
    use std::path::{Path, PathBuf};
    use std::{collections::HashMap, sync::Mutex};

    #[test]
    fn test_list() {
        let files = Mutex::new(HashMap::<PathBuf, String>::new());
        let stored = Builder::default()
            .without_system()
            .with_env_impl(())
            .with_fs_impl(files)
            .with_user_impl("edgedb")
            .with_warning(|w| println!("warning: {}", w))
            .with_tracing(|s| println!("{}", s))
            .stored_info();

        let credentials = stored.credentials();
        let instances = credentials.list().expect("failed to list credentials");
        assert!(instances.is_empty());

        credentials
            .write(
                InstanceName::Local("local".to_string()),
                &CredentialsFile::default(),
            )
            .unwrap();
        credentials
            .write(
                InstanceName::Local("local2".to_string()),
                &CredentialsFile::default(),
            )
            .unwrap();

        let instances = credentials.list().expect("failed to list credentials");
        assert_eq!(instances.len(), 2, "expected 2 instances: {:?}", instances);
        assert!(instances.contains(&InstanceName::Local("local".to_string())));
        assert!(instances.contains(&InstanceName::Local("local2".to_string())));
    }

    #[test]
    fn test_read_outdated() {
        let files = Mutex::new(HashMap::<PathBuf, String>::new());
        files
            .write(
                Path::new("/home/edgedb/.config/edgedb/credentials/local.json"),
                "{\"tls_verify_hostname\": true}",
            )
            .unwrap();
        let stored = Builder::default()
            .without_system()
            .with_env_impl(())
            .with_fs_impl(files)
            .with_user_impl("edgedb")
            .with_warning(|w| println!("warning: {}", w))
            .with_tracing(|s| println!("{}", s))
            .stored_info();

        let credentials = stored.credentials();
        let content = credentials
            .read(InstanceName::Local("local".to_string()))
            .unwrap();
        assert!(content.is_some());
    }

    /// Ensure that read/write works with the real filesystem, starting with
    /// empty config dirs.
    #[test]
    fn test_real_fs() {
        let tempdir = tempfile::tempdir().unwrap();
        let userdir = tempdir.path().join("home").join("someuser");

        let stored = Builder::default()
            .without_system()
            .with_fs()
            .with_user_impl(userdir)
            .with_warning(|w| println!("warning: {}", w))
            .with_tracing(|s| println!("{}", s))
            .stored_info();
        let credentials = stored.credentials();

        let instances = credentials.list().unwrap();
        assert_eq!(instances.len(), 0);

        let creds = credentials
            .read(InstanceName::Local("doesnotexist".to_string()))
            .unwrap();
        assert!(creds.is_none());

        credentials
            .write(
                InstanceName::Local("local".to_string()),
                &CredentialsFile::default(),
            )
            .unwrap();
        let instances = credentials.list().unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0], InstanceName::Local("local".to_string()));

        let creds = credentials
            .read(InstanceName::Local("local".to_string()))
            .unwrap();
        assert!(creds.is_some());
    }

    #[test]
    fn test_system_paths() {
        let stored = Builder::default().with_system().stored_info();
        let paths = stored.paths();

        let system = paths.for_system();
        eprintln!("system: {:?}", system);
        assert!(system.home_dir.is_some());
        assert!(system.config_dir.is_some());
        assert!(system.data_dir.is_some());
        assert!(system.data_local_dir.is_some());
        assert!(system.cache_dir.is_some());

        let instance = paths.for_instance("local").unwrap();
        eprintln!("instance: {:?}", instance);

        if cfg!(unix) {
            assert_eq!(
                instance.data_dir,
                dirs::data_dir().unwrap().join("edgedb/data/local")
            );
            assert_eq!(
                instance.credentials_path,
                dirs::config_dir()
                    .unwrap()
                    .join("edgedb/credentials/local.json")
            );
        } else if cfg!(windows) {
            assert_eq!(
                instance.data_dir,
                dirs::data_local_dir().unwrap().join("EdgeDB/data/local")
            );
            assert_eq!(
                instance.credentials_path,
                dirs::config_dir()
                    .unwrap()
                    .join("EdgeDB/credentials/local.json")
            );
        } else {
            unreachable!("Unsupported platform");
        }
    }
}

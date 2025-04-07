use std::{str::FromStr, sync::Arc};

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

    pub fn credentials(&self) -> StoredCredentials<C, Arc<C>> {
        StoredCredentials::new(self.context.clone())
    }
}

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
                    if self.context.write_config_file(path, &s).is_ok() {
                        context_trace!(self.context, "Updated out-of-date credentials");
                    } else {
                        context_trace!(self.context, "Failed to update credentials");
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
}

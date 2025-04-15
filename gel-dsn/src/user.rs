use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

pub struct SystemUserProfile;

/// A trait for abstracting user profiles.
pub trait UserProfile {
    fn username(&self) -> Option<Cow<str>>;
    fn homedir(&self) -> Option<Cow<Path>>;
    fn config_dir(&self) -> Option<Cow<Path>>;
    fn data_dir(&self) -> Option<Cow<Path>>;
    fn data_local_dir(&self) -> Option<Cow<Path>>;
    fn config_dirs(&self) -> Vec<Cow<Path>>;
    fn cache_dir(&self) -> Option<Cow<Path>>;
    /// The directory for storing runstate files. The instance name is
    /// substituted for `{}`.
    fn runstate_dir(&self) -> Option<Cow<Path>>;
}

impl UserProfile for () {
    fn username(&self) -> Option<Cow<str>> {
        None
    }

    fn homedir(&self) -> Option<Cow<Path>> {
        None
    }

    fn config_dir(&self) -> Option<Cow<Path>> {
        None
    }

    fn data_dir(&self) -> Option<Cow<Path>> {
        None
    }

    fn data_local_dir(&self) -> Option<Cow<Path>> {
        None
    }

    fn cache_dir(&self) -> Option<Cow<Path>> {
        None
    }

    fn config_dirs(&self) -> Vec<Cow<Path>> {
        vec![]
    }

    fn runstate_dir(&self) -> Option<Cow<Path>> {
        None
    }
}

impl UserProfile for &'static str {
    fn username(&self) -> Option<Cow<str>> {
        Some(Cow::Borrowed(self))
    }

    fn homedir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(PathBuf::from(format!("/home/{self}"))))
    }

    fn config_dir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(PathBuf::from(format!("/home/{self}/.config"))))
    }

    fn data_dir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(PathBuf::from(format!("/home/{self}/.local"))))
    }

    fn data_local_dir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(PathBuf::from(format!("/home/{self}/.local"))))
    }

    fn cache_dir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(PathBuf::from(format!(
            "/home/{self}/.cache/edgedb"
        ))))
    }

    fn config_dirs(&self) -> Vec<Cow<Path>> {
        vec![
            Cow::Owned(PathBuf::from(format!("/home/{self}/.config/gel"))),
            Cow::Owned(PathBuf::from(format!("/home/{self}/.config/edgedb"))),
        ]
    }

    fn runstate_dir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(PathBuf::from(format!(
            "/home/{self}/.cache/edgedb/run/{{}}"
        ))))
    }
}

impl UserProfile for PathBuf {
    fn username(&self) -> Option<Cow<str>> {
        Some(Cow::Borrowed(
            self.file_name()
                .expect("no file name to infer username")
                .to_str()
                .unwrap(),
        ))
    }

    fn homedir(&self) -> Option<Cow<Path>> {
        Some(Cow::Borrowed(self))
    }

    fn config_dir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(self.join(".config")))
    }

    fn data_local_dir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(self.join(".local")))
    }

    fn data_dir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(self.join(".local")))
    }

    fn cache_dir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(self.join(".cache").join("edgedb")))
    }

    fn config_dirs(&self) -> Vec<Cow<Path>> {
        vec![
            Cow::Owned(self.join(".config").join("gel")),
            Cow::Owned(self.join(".config").join("edgedb")),
        ]
    }

    fn runstate_dir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(
            self.join(".cache").join("edgedb").join("run").join("{}"),
        ))
    }
}

impl UserProfile for SystemUserProfile {
    fn username(&self) -> Option<Cow<str>> {
        whoami::fallible::username().ok().map(Cow::Owned)
    }

    fn homedir(&self) -> Option<Cow<Path>> {
        dirs::home_dir().map(Cow::Owned)
    }

    fn config_dir(&self) -> Option<Cow<Path>> {
        dirs::config_dir().map(Cow::Owned)
    }

    fn data_local_dir(&self) -> Option<Cow<Path>> {
        if cfg!(windows) {
            dirs::data_local_dir().map(|p| Cow::Owned(p.join("EdgeDB")))
        } else if cfg!(unix) {
            dirs::data_local_dir().map(|p| Cow::Owned(p.join("edgedb")))
        } else {
            None
        }
    }

    fn data_dir(&self) -> Option<Cow<Path>> {
        if cfg!(windows) {
            dirs::data_dir().map(|p| Cow::Owned(p.join("EdgeDB")))
        } else if cfg!(unix) {
            dirs::data_dir().map(|p| Cow::Owned(p.join("edgedb")))
        } else {
            None
        }
    }

    fn cache_dir(&self) -> Option<Cow<Path>> {
        if cfg!(windows) {
            dirs::data_local_dir().map(|p| Cow::Owned(p.join("EdgeDB").join("cache")))
        } else if cfg!(unix) {
            dirs::cache_dir().map(|p| Cow::Owned(p.join("edgedb")))
        } else {
            None
        }
    }

    fn config_dirs(&self) -> Vec<Cow<Path>> {
        let mut dirs = Vec::new();
        if cfg!(unix) {
            if let Some(dir) = self.config_dir() {
                dirs.push(Cow::Owned(dir.join("edgedb")));
                dirs.push(Cow::Owned(dir.join("gel")));
            }
        }
        if cfg!(windows) {
            // Windows config files are stored locally, not in the roaming
            // profile directory.
            if let Some(dir) = dirs::data_local_dir() {
                dirs.push(Cow::Owned(dir.join("EdgeDB").join("config")));
                dirs.push(Cow::Owned(dir.join("Gel").join("config")));
            }
        }
        dirs
    }

    fn runstate_dir(&self) -> Option<Cow<Path>> {
        if cfg!(windows) {
            dirs::cache_dir().map(|p| Cow::Owned(p.join("EdgeDB").join("run").join("{}")))
        } else if cfg!(unix) {
            if let Some(runtime_dir) = dirs::runtime_dir() {
                // On Linux, use /run/user/$uid/edgedb-XXX
                Some(Cow::Owned(runtime_dir.join("edgedb-{}")))
            } else {
                // On other platforms, use ~/.cache/edgedb/run/XXX
                dirs::cache_dir().map(|p| Cow::Owned(p.join("edgedb").join("run").join("{}")))
            }
        } else {
            None
        }
    }
}

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
    fn data_local_dir(&self) -> Option<Cow<Path>>;
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

    fn data_local_dir(&self) -> Option<Cow<Path>> {
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

    fn data_local_dir(&self) -> Option<Cow<Path>> {
        Some(Cow::Owned(PathBuf::from(format!("/home/{self}/.local"))))
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
        dirs::data_local_dir().map(Cow::Owned)
    }
}

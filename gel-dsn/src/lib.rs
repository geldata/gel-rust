use std::borrow::Cow;

mod env;
mod file;
#[cfg(feature = "gel")]
pub mod gel;
mod host;
#[cfg(feature = "postgres")]
pub mod postgres;

pub use env::EnvVar;
pub use file::FileAccess;

/// A trait for abstracting user profiles.
pub trait UserProfile {
    fn username(&self) -> Option<Cow<str>>;
    fn homedir(&self) -> Option<Cow<str>>;
}

#[derive(Debug, Default)]
pub struct Warnings {
    warnings: Vec<String>,
}

impl Warnings {
    pub fn warn(&mut self, message: &str) {
        self.warnings.push(message.to_string());
    }

    pub fn into_vec(self) -> Vec<String> {
        self.warnings
    }
}

#[derive(Debug, Default)]
pub struct Traces {
    traces: Vec<String>,
}

impl Traces {
    pub fn trace(&mut self, message: &str) {
        self.traces.push(message.to_string());
    }

    pub fn into_vec(self) -> Vec<String> {
        self.traces
    }
}

use std::{borrow::Cow, collections::HashMap, marker::PhantomData, path::Path};

mod env;
mod file;
pub mod gel;
mod host;
pub mod postgres;

pub use env::EnvVar;
pub use file::FileAccess;

pub trait UserProfile {
    fn username(&self) -> Option<Cow<str>>;
    fn homedir(&self) -> Option<Cow<str>>;
}

#[derive(Debug, Default)]
pub struct Warnings {
    warnings: Vec<String>,
}

use std::{collections::HashMap, path::Path};

pub struct SystemFileAccess;

/// A trait for abstracting the reading of files.
pub trait FileAccess {
    fn default() -> impl FileAccess {
        SystemFileAccess
    }
    fn read(&self, path: &Path) -> Result<String, std::io::Error>;
}

impl FileAccess for &[(&Path, &str)] {
    fn read(&self, path: &Path) -> Result<String, std::io::Error> {
        self.iter()
            .find(|(key, _)| *key == path)
            .map(|(_, value)| value.to_string())
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File not found",
            ))
    }
}

impl<K, V> FileAccess for HashMap<K, V>
where
    K: std::hash::Hash + Eq + std::borrow::Borrow<Path>,
    V: std::borrow::Borrow<str>,
{
    fn read(&self, name: &Path) -> Result<String, std::io::Error> {
        self.get(name)
            .map(|value| value.borrow().into())
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File not found",
            ))
    }
}

impl FileAccess for () {
    fn read(&self, _: &Path) -> Result<String, std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ))
    }
}

impl FileAccess for SystemFileAccess {
    fn read(&self, path: &Path) -> Result<String, std::io::Error> {
        use std::io::Read;
        let mut file = std::fs::File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }
}

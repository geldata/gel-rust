use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct SystemFileAccess;

/// A trait for abstracting the reading of files.
pub trait FileAccess {
    fn default() -> impl FileAccess {
        SystemFileAccess
    }

    fn read(&self, path: &Path) -> Result<String, std::io::Error>;

    fn cwd(&self) -> Option<PathBuf> {
        None
    }

    fn exists(&self, path: &Path) -> Result<bool, std::io::Error> {
        match self.read(path) {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, std::io::Error> {
        Ok(path.to_path_buf())
    }

    /// Returns all files known by this file accessor. This is inefficient, but
    /// only used for mock implementations.
    fn all_files(&self) -> Option<Vec<PathBuf>> {
        None
    }

    fn exists_dir(&self, path: &Path) -> Result<bool, std::io::Error> {
        let Some(all_files) = self.all_files() else {
            unreachable!("FileAccess::exists_dir incorrectly implemented");
        };

        Ok(all_files.iter().any(|file| {
            if let Ok(file) = file.strip_prefix(path) {
                file.components().count() >= 1
            } else {
                false
            }
        }))
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
        let Some(all_files) = self.all_files() else {
            unreachable!("FileAccess::list_dir incorrectly implemented");
        };

        Ok(all_files
            .iter()
            .filter(|file| {
                if let Ok(file) = file.strip_prefix(path) {
                    file.components().count() == 1
                } else {
                    false
                }
            })
            .cloned()
            .collect())
    }

    fn write(&self, _: &Path, _: &str) -> Result<(), std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "FileAccess::write_file is not implemented",
        ))
    }
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

    fn all_files(&self) -> Option<Vec<PathBuf>> {
        Some(self.iter().map(|(key, _)| key.into()).collect())
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

    fn all_files(&self) -> Option<Vec<PathBuf>> {
        Some(self.keys().map(|key| key.borrow().into()).collect())
    }
}

impl<K, V> FileAccess for Mutex<HashMap<K, V>>
where
    K: std::hash::Hash + Eq + std::borrow::Borrow<Path> + for<'a> From<&'a Path>,
    V: std::borrow::Borrow<str> + for<'a> From<&'a str>,
{
    fn read(&self, name: &Path) -> Result<String, std::io::Error> {
        self.lock()
            .unwrap()
            .get(name)
            .map(|value| value.borrow().into())
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File not found",
            ))
    }

    fn write(&self, path: &Path, content: &str) -> Result<(), std::io::Error> {
        self.lock().unwrap().insert(path.into(), content.into());
        Ok(())
    }

    fn all_files(&self) -> Option<Vec<PathBuf>> {
        Some(
            self.lock()
                .unwrap()
                .keys()
                .map(|key| key.borrow().into())
                .collect(),
        )
    }
}

impl FileAccess for () {
    fn read(&self, _: &Path) -> Result<String, std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ))
    }

    fn exists_dir(&self, _: &Path) -> Result<bool, std::io::Error> {
        Ok(false)
    }

    fn all_files(&self) -> Option<Vec<PathBuf>> {
        Some(Vec::new())
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

    fn cwd(&self) -> Option<PathBuf> {
        std::env::current_dir().ok()
    }

    fn exists(&self, path: &Path) -> Result<bool, std::io::Error> {
        std::fs::exists(path)
    }

    fn exists_dir(&self, path: &Path) -> Result<bool, std::io::Error> {
        std::fs::metadata(path).map(|metadata| metadata.is_dir())
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, std::io::Error> {
        std::fs::canonicalize(path)
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut files = vec![];
        for file in std::fs::read_dir(path)? {
            if let Ok(file) = file {
                let path = file.path();
                if path.is_file() {
                    files.push(path);
                }
            }
        }
        Ok(files)
    }

    fn all_files(&self) -> Option<Vec<PathBuf>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_dir() {
        let mut files = HashMap::new();
        files.insert(
            PathBuf::from("/home/edgedb/.config/edgedb/credentials/local.json"),
            "{}",
        );
        files.insert(
            PathBuf::from("/home/edgedb/.config/edgedb/credentials/local2.json"),
            "{}",
        );
        let found = files
            .list_dir(&PathBuf::from("/home/edgedb/.config/edgedb/credentials/"))
            .unwrap();
        assert_eq!(found.len(), 2);
        let found = files
            .list_dir(&PathBuf::from("/home/edgedb/.config/edgedb/"))
            .unwrap();
        assert_eq!(found.len(), 0);
    }

    #[test]
    fn test_exists_dir() {
        let files = Mutex::new(HashMap::<PathBuf, String>::new());
        assert!(!files
            .exists_dir(&PathBuf::from("/home/edgedb/.config/edgedb/credentials/"))
            .unwrap());
        files
            .write(
                &PathBuf::from("/home/edgedb/.config/edgedb/credentials/local.json"),
                "{}",
            )
            .unwrap();
        let found = files
            .exists_dir(&PathBuf::from("/home/edgedb/.config/edgedb/credentials/"))
            .unwrap();
        assert_eq!(found, true);
        let found = files
            .exists_dir(&PathBuf::from("/home/edgedb/.config/edgedb/"))
            .unwrap();
        assert_eq!(found, true);
        let found = files
            .exists_dir(&PathBuf::from("/home/edgedb/.config/"))
            .unwrap();
        assert_eq!(found, true);
    }
}

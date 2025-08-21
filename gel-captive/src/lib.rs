mod server;
mod utils;

use std::path;

pub use server::{ServerInfo, ServerProcess};

pub struct ServerBuilder {
    log_file_path: Option<path::PathBuf>
}

impl ServerBuilder {
    pub fn new() -> Self {
        Self {
            log_file_path: None,
        }
    }

    pub fn start(self) -> ServerProcess {
        ServerProcess::start(self)
    }

    /// Sets filename of the server output log.
    /// Defaults to a random temp file.
    pub fn log_file_path(&mut self, path: Option<path::PathBuf>) {
        self.log_file_path = path;
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

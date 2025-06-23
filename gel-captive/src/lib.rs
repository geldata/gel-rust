mod server;
mod utils;

pub use server::{ServerInfo, ServerProcess};

pub struct ServerBuilder {
    // TODO
}

impl ServerBuilder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn start(self) -> ServerProcess {
        ServerProcess::start()
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

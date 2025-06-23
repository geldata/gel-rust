pub use gel_db_protocol::errors::EdbError;

mod server_state_machine;

pub mod server {
    pub use super::server_state_machine::*;
}

#[derive(Debug, derive_more::Error, derive_more::Display, derive_more::From)]
pub enum ConnectionError {
    /// Invalid state error, suggesting a logic error in code rather than a server or client failure.
    /// Use the `invalid_state!` macro instead which will print a backtrace.
    #[display("Invalid state")]
    #[deprecated = "Use invalid_state!"]
    __InvalidState,

    /// Error returned by the server.
    #[display("Server error: {_0}")]
    ServerError(#[from] EdbError),

    /// The server sent something we didn't expect
    #[display("Unexpected server response: {_0}")]
    UnexpectedResponse(#[error(not(source))] String),

    /// Error related to SCRAM authentication.
    #[display("SCRAM: {_0}")]
    Scram(#[from] crate::scram::SCRAMError),

    /// I/O error encountered during connection operations.
    #[display("I/O error: {_0}")]
    Io(#[from] std::io::Error),

    /// UTF-8 decoding error.
    #[display("UTF8 error: {_0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    #[display("Protocol error: {_0}")]
    ParseError(#[from] gel_pg_protocol::prelude::ParseError),
}

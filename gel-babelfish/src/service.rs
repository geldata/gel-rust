use gel_auth::CredentialData;

use crate::{hyper::HyperStreamBody, stream::ListenerStream};
use std::{
    future::Future,
    sync::{Arc, Mutex},
};

/// A stream language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamLanguage {
    /// A Postgres stream using the Postgres wire protocol.
    Postgres,
    /// A Gel-language stream, using the Gel/EdgeDB wire protocol.
    Gel(GelVersion, GelVariant),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GelVersion {
    V0,
    V1,
    V2,
    V3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GelVariant {
    // Raw wire protocol.
    Wire,
    // TODO: the future "tutorial" handler
    Notebook,
}

#[derive(Debug)]

pub enum AuthTarget {
    /// A stream of the given language.
    Stream(StreamLanguage),
    /// A HTTP request to the given path.
    HTTP(String),
}

#[derive(Clone, Debug)]
pub enum BranchDB {
    /// Branch only.
    Branch(String),
    /// Database name (legacy).
    DB(String),
    /// Postgres database name.
    PGDB(String),
}

#[derive(derive_more::Display, derive_more::Error, Debug)]
pub enum IdentityError {
    #[display("No user specified")]
    NoUser,
    #[display("No database specified")]
    NoDb,
}

#[derive(Clone, Debug)]
pub struct ConnectionIdentityBuilder {
    tenant: Arc<Mutex<Option<String>>>,
    db: Arc<Mutex<Option<BranchDB>>>,
    user: Arc<Mutex<Option<String>>>,
}

impl Default for ConnectionIdentityBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionIdentityBuilder {
    pub fn new() -> Self {
        Self {
            tenant: Arc::new(Mutex::new(None)),
            db: Arc::new(Mutex::new(None)),
            user: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_tenant(&self, tenant: String) -> &Self {
        *self.tenant.lock().unwrap() = Some(tenant);
        self
    }

    pub fn set_database(&self, database: String) -> &Self {
        if !database.is_empty() {
            // Only set if currently non-empty
            let mut db = self.db.lock().unwrap();
            if db.is_none() {
                *db = Some(BranchDB::DB(database));
            }
        }
        self
    }

    pub fn set_branch(&self, branch: String) -> &Self {
        if !branch.is_empty() {
            *self.db.lock().unwrap() = Some(BranchDB::Branch(branch));
        }
        self
    }

    pub fn set_pg_database(&self, database: String) -> &Self {
        if !database.is_empty() {
            *self.db.lock().unwrap() = Some(BranchDB::PGDB(database));
        }
        self
    }

    pub fn set_user(&self, user: String) -> &Self {
        *self.user.lock().unwrap() = Some(user);
        self
    }

    /// Create a new, disconnected builder.
    pub fn new_builder(&self) -> Self {
        Self {
            tenant: Arc::new(Mutex::new(self.tenant.lock().unwrap().clone())),
            db: Arc::new(Mutex::new(self.db.lock().unwrap().clone())),
            user: Arc::new(Mutex::new(self.user.lock().unwrap().clone())),
        }
    }

    fn unwrap_or_clone<T: Clone>(arc: Arc<Mutex<T>>) -> T {
        match Arc::try_unwrap(arc) {
            Ok(mutex) => mutex.into_inner().unwrap(),
            Err(arc) => arc.lock().unwrap().clone(),
        }
    }

    pub fn build(self) -> Result<ConnectionIdentity, IdentityError> {
        let tenant = Self::unwrap_or_clone(self.tenant);
        let user = Self::unwrap_or_clone(self.user).ok_or(IdentityError::NoUser)?;
        let db = Self::unwrap_or_clone(self.db).ok_or(IdentityError::NoDb)?;

        Ok(ConnectionIdentity { tenant, db, user })
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionIdentity {
    pub tenant: Option<String>,
    pub db: BranchDB,
    pub user: String,
}

/// Handles incoming connections from the listener which might be streams or
/// HTTP. This is implemented by the embedding server.
pub trait BabelfishService: std::fmt::Debug + Send + Sync + 'static {
    /// Given the provided connection identity in [`ConnectionIdentity`], and the target
    /// in [`AuthTarget`], return the credential data.
    fn lookup_auth(
        &self,
        identity: ConnectionIdentity,
        target: AuthTarget,
    ) -> impl Future<Output = Result<CredentialData, std::io::Error>> + Send + Sync;

    /// Accept a fully-authenticated stream with the given identity and language.
    fn accept_stream(
        &self,
        identity: ConnectionIdentity,
        language: StreamLanguage,
        stream: ListenerStream,
    ) -> impl Future<Output = Result<(), std::io::Error>> + Send;

    /// Accept a fully-authenticated HTTP request with the given identity.
    fn accept_http(
        &self,
        identity: ConnectionIdentity,
        req: hyper::http::Request<hyper::body::Incoming>,
    ) -> impl Future<Output = Result<hyper::http::Response<HyperStreamBody>, std::io::Error>> + Send + Sync;

    /// Accept an unauthenticated HTTP request.
    fn accept_http_unauthenticated(
        &self,
        req: hyper::http::Request<hyper::body::Incoming>,
    ) -> impl Future<Output = Result<hyper::http::Response<HyperStreamBody>, std::io::Error>> + Send + Sync;
}

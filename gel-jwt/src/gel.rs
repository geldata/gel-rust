//! Implementation of the Gel JWT token format.
//!
//! Gel JWT tokens begin with `edbt_`, `edbt1_` (or their nbwt/nbwt1 variants), and are followed by a
//! base64-encoded JWT token.
use crate::{
    registry::IsKey, Any, Key, KeyRegistry, SignatureError, SigningContext, ValidationContext,
    ValidationError,
};
use base64ct::{Base64Unpadded, Encoding};
use std::collections::{HashMap, HashSet};
use tracing::warn;
use uuid::Uuid;

#[derive(derive_more::Error, derive_more::Display, derive_more::From, Debug, PartialEq, Eq)]
pub enum TokenValidationError {
    #[display("Verification failed")]
    ValidationError(#[from] ValidationError),
    #[display("malformed JWT")]
    InvalidToken,
    #[display("secret key does not authorize access to this instance")]
    #[from(ignore)]
    InvalidInstance(#[error(not(source))] String),
    #[display("secret key does not authorize access in role {_0}")]
    #[from(ignore)]
    InvalidRole(#[error(not(source))] String),
    #[display("secret key does not authorize access to database {_0}")]
    #[from(ignore)]
    InvalidDatabase(#[error(not(source))] String),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct TokenClaims {
    pub instances: TokenMatch,
    pub roles: TokenMatch,
    pub databases: TokenMatch,
    pub issuer: Option<String>,
}

impl TokenClaims {
    /// Validate the token against the given instance name, user, and database name.
    pub fn validate(
        &self,
        instance_name: &str,
        user: &str,
        dbname: &str,
    ) -> Result<(), TokenValidationError> {
        if !self.instances.matches(instance_name) {
            warn!("Instance not in token: {instance_name}");
            return Err(TokenValidationError::InvalidInstance(
                instance_name.to_string(),
            ));
        }
        if !self.roles.matches(user) {
            warn!("Role not in token: {user}");
            return Err(TokenValidationError::InvalidRole(user.to_string()));
        }
        if !self.databases.matches(dbname) {
            warn!("Database not in token: {dbname}");
            return Err(TokenValidationError::InvalidDatabase(dbname.to_string()));
        }
        Ok(())
    }

    fn from_claims(
        token_version: u8,
        decoded: &HashMap<String, crate::Any>,
    ) -> Result<Self, TokenValidationError> {
        let issuer = match decoded.get("iss") {
            Some(Any::String(s)) => Some(s.to_string()),
            None => None,
            _ => {
                warn!("Invalid token: issuer is not a string");
                return Err(TokenValidationError::InvalidToken);
            }
        };

        let claims = if token_version == 0 {
            // Legacy v0 token: "edgedb.server.any_role" is a boolean, "edgedb.server.roles" is an array of strings
            let roles =
                TokenMatch::from_claims(decoded, "edgedb.server.any_role", "edgedb.server.roles")?;
            Self {
                roles,
                instances: TokenMatch::All,
                databases: TokenMatch::All,
                issuer,
            }
        } else {
            // New v1 token: "edb.{i,r,d}.all" are booleans, "edb.{i,r,d}" are arrays of strings
            let instances = TokenMatch::from_claims(decoded, "edb.i.all", "edb.i")?;
            let roles = TokenMatch::from_claims(decoded, "edb.r.all", "edb.r")?;
            let databases = TokenMatch::from_claims(decoded, "edb.d.all", "edb.d")?;
            Self {
                instances,
                roles,
                databases,
                issuer,
            }
        };
        Ok(claims)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum TokenMatch {
    #[default]
    None,
    All,
    Some(HashSet<String>),
}

impl TokenMatch {
    fn from_claims(
        claims: &HashMap<String, crate::Any>,
        all_key: &str,
        list_key: &str,
    ) -> Result<Self, crate::ValidationError> {
        if let Some(crate::Any::Bool(true)) = claims.get(all_key) {
            return Ok(TokenMatch::All);
        }

        if let Some(crate::Any::Array(values)) = claims.get(list_key) {
            let mut list = HashSet::new();
            for value in values {
                if let crate::Any::String(s) = value {
                    list.insert(s.to_string());
                }
            }
            return Ok(TokenMatch::Some(list));
        }

        Ok(TokenMatch::None)
    }

    fn matches(&self, value: &str) -> bool {
        match self {
            TokenMatch::None => false,
            TokenMatch::All => true,
            TokenMatch::Some(set) => set.contains(value),
        }
    }
}

/// Augments a `KeyRegistry` with methods for validating and decoding Gel
/// tokens.
pub trait GelPublicKeyRegistry {
    /// Decode a Gel token without validating it.
    fn unsafely_decode_gel_token(&self, token: &str) -> Result<TokenClaims, TokenValidationError>;

    /// Validate a Gel token and decode it.
    fn validate_gel_token(
        &self,
        token: &str,
        validation_ctx: &ValidationContext,
    ) -> Result<TokenClaims, TokenValidationError>;
}

/// Augments a `KeyRegistry` with private keys with methods for generating Gel
/// tokens.
pub trait GelPrivateKeyRegistry {
    fn generate_gel_token(
        &self,
        instances: Option<Vec<String>>,
        roles: Option<Vec<String>>,
        databases: Option<Vec<String>>,
        additional_claims: Option<HashMap<String, Any>>,
        signing_ctx: &SigningContext,
    ) -> Result<String, SignatureError>;

    fn generate_legacy_token(
        &self,
        roles: Option<Vec<String>>,
        signing_ctx: &SigningContext,
    ) -> Result<String, SignatureError>;
}

impl<K: IsKey> GelPublicKeyRegistry for KeyRegistry<K> {
    fn unsafely_decode_gel_token(&self, token: &str) -> Result<TokenClaims, TokenValidationError> {
        let mut token_version = 0;
        let encoded_token = if let Some(stripped) = token.strip_prefix("nbwt1_") {
            token_version = 1;
            stripped
        } else if let Some(stripped) = token.strip_prefix("nbwt_") {
            stripped
        } else if let Some(stripped) = token.strip_prefix("edbt1_") {
            token_version = 1;
            stripped
        } else if let Some(stripped) = token.strip_prefix("edbt_") {
            stripped
        } else {
            warn!(
                "Invalid token prefix: [{}...]",
                &token[0..token.len().min(7)]
            );
            return Err(TokenValidationError::InvalidToken);
        };

        let decoded = self.unsafely_decode_without_validation(encoded_token)?;
        TokenClaims::from_claims(token_version, &decoded)
    }

    fn validate_gel_token(
        &self,
        token: &str,
        validation_ctx: &ValidationContext,
    ) -> Result<TokenClaims, TokenValidationError> {
        let mut token_version = 0;
        let encoded_token = if let Some(stripped) = token.strip_prefix("nbwt1_") {
            token_version = 1;
            stripped
        } else if let Some(stripped) = token.strip_prefix("nbwt_") {
            stripped
        } else if let Some(stripped) = token.strip_prefix("edbt1_") {
            token_version = 1;
            stripped
        } else if let Some(stripped) = token.strip_prefix("edbt_") {
            stripped
        } else {
            warn!(
                "Invalid token prefix: [{}...]",
                &token[0..token.len().min(7)]
            );
            return Err(TokenValidationError::InvalidToken);
        };

        // Validate and decode the JWT
        let decoded = match self.validate(encoded_token, validation_ctx) {
            Ok(claims) => claims,
            Err(e) => {
                warn!("Invalid token: {}", e.error_string_not_for_user());
                return Err(TokenValidationError::ValidationError(e));
            }
        };

        TokenClaims::from_claims(token_version, &decoded)
    }
}

impl GelPrivateKeyRegistry for KeyRegistry<Key> {
    fn generate_gel_token(
        &self,
        instances: Option<Vec<String>>,
        roles: Option<Vec<String>>,
        databases: Option<Vec<String>>,
        additional_claims: Option<HashMap<String, Any>>,
        signing_ctx: &SigningContext,
    ) -> Result<String, SignatureError> {
        let mut claims_map = HashMap::new();

        // Handle instances
        if instances.is_none() {
            claims_map.insert("edb.i.all".to_string(), Any::from(true));
        } else if let Some(instances) = instances {
            claims_map.insert("edb.i".to_string(), Any::from(instances));
        }

        // Handle roles
        if roles.is_none() {
            claims_map.insert("edb.r.all".to_string(), Any::from(true));
        } else if let Some(roles) = roles {
            claims_map.insert("edb.r".to_string(), Any::from(roles));
        }

        // Handle databases
        if databases.is_none() {
            claims_map.insert("edb.d.all".to_string(), Any::from(true));
        } else if let Some(databases) = databases {
            claims_map.insert("edb.d".to_string(), Any::from(databases));
        }

        // Add additional claims
        if let Some(additional) = additional_claims {
            for (key, value) in additional {
                claims_map.insert(key, value);
            }
        }

        // Add a JTI if and only if it's not already present
        if !claims_map.contains_key("jti") {
            let jti = Uuid::new_v4();
            // Encode UUID as base64 to make the token shorter
            let jti_base64 = Base64Unpadded::encode_string(jti.as_bytes());
            claims_map.insert("jti".to_string(), Any::from(jti_base64));
        }

        let token = self.sign(claims_map, signing_ctx)?;

        Ok(format!("edbt1_{}", token))
    }

    fn generate_legacy_token(
        &self,
        roles: Option<Vec<String>>,
        signing_ctx: &SigningContext,
    ) -> Result<String, SignatureError> {
        let mut claims_map = HashMap::new();
        let jti = Uuid::new_v4();
        let jti_base64 = Base64Unpadded::encode_string(jti.as_bytes());
        claims_map.insert("jti".to_string(), Any::from(jti_base64));

        if let Some(roles) = roles {
            claims_map.insert("edgedb.server.roles".to_string(), Any::from(roles));
        } else {
            claims_map.insert("edgedb.server.any_role".to_string(), Any::from(true));
        }

        let token = self.sign(claims_map, signing_ctx)?;
        Ok(format!("edbt_{}", token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KeyType;

    #[test]
    fn test_gel_token_any() {
        let mut registry = KeyRegistry::new();
        registry.generate_key(None, KeyType::HS256).unwrap();

        let token = registry
            .generate_gel_token(None, None, None, None, &SigningContext::default())
            .unwrap();
        let claims = registry
            .validate_gel_token(&token, &ValidationContext::default())
            .unwrap();
        assert_eq!(
            TokenClaims {
                instances: TokenMatch::All,
                roles: TokenMatch::All,
                databases: TokenMatch::All,
                issuer: None,
            },
            claims
        );
        assert!(claims.validate("test", "test", "test").is_ok());

        let Some(token) = token.strip_prefix("edbt1_") else {
            panic!("token does not start with edbt1_");
        };
        eprintln!("token: {}", token);
        let decoded = registry.unsafely_decode_without_validation(token).unwrap();
        assert_eq!(decoded.get("edb.i.all").unwrap(), &Any::from(true));
        assert_eq!(decoded.get("edb.r.all").unwrap(), &Any::from(true));
        assert_eq!(decoded.get("edb.d.all").unwrap(), &Any::from(true));
    }

    #[test]
    fn test_gel_token_specified() {
        let mut registry = KeyRegistry::new();
        registry.generate_key(None, KeyType::HS256).unwrap();

        let token = registry
            .generate_gel_token(
                Some(vec!["instance".to_string()]),
                Some(vec!["role".to_string()]),
                Some(vec!["database".to_string()]),
                None,
                &SigningContext::default(),
            )
            .unwrap();
        let claims = registry
            .validate_gel_token(&token, &ValidationContext::default())
            .unwrap();
        assert_eq!(
            TokenClaims {
                instances: TokenMatch::Some(HashSet::from(["instance".to_string()])),
                roles: TokenMatch::Some(HashSet::from(["role".to_string()])),
                databases: TokenMatch::Some(HashSet::from(["database".to_string()])),
                issuer: None,
            },
            claims
        );
        assert!(claims.validate("instance", "role", "database").is_ok());
        assert_eq!(
            claims.validate("other", "role", "database"),
            Err(TokenValidationError::InvalidInstance("other".to_string()))
        );
        assert_eq!(
            claims.validate("instance", "other", "database"),
            Err(TokenValidationError::InvalidRole("other".to_string()))
        );
        assert_eq!(
            claims.validate("instance", "role", "other"),
            Err(TokenValidationError::InvalidDatabase("other".to_string()))
        );
    }
}

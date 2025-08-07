use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub mod raw;
pub mod structure;

#[derive(
    Clone, Serialize, Deserialize, PartialEq, Eq, Hash, derive_more::Display, derive_more::Debug,
)]
pub enum ConfigSchemaPrimitiveType {
    #[display("std::str")]
    #[debug("std::str")]
    Str,
    #[display("std::bool")]
    #[debug("std::bool")]
    Bool,
    #[display("std::int16")]
    #[debug("std::int16")]
    Int16,
    #[display("std::int32")]
    #[debug("std::int32")]
    Int32,
    #[display("std::int64")]
    #[debug("std::int64")]
    Int64,
    #[display("std::float32")]
    #[debug("std::float32")]
    Float32,
    #[display("std::float64")]
    #[debug("std::float64")]
    Float64,
    #[display("std::bigint")]
    #[debug("std::bigint")]
    BigInt,
    #[display("std::decimal")]
    #[debug("std::decimal")]
    Decimal,
    #[display("std::uuid")]
    #[debug("std::uuid")]
    Uuid,
    #[display("std::duration")]
    #[debug("std::duration")]
    Duration,
    #[display("std::bytes")]
    #[debug("std::bytes")]
    Bytes,
    #[display("std::sequence")]
    #[debug("std::sequence")]
    Sequence,
    #[display("cfg::memory")]
    #[debug("cfg::memory")]
    Memory,
}

impl FromStr for ConfigSchemaPrimitiveType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "std::str" => Ok(Self::Str),
            "std::bool" => Ok(Self::Bool),
            "std::int16" => Ok(Self::Int16),
            "std::int32" => Ok(Self::Int32),
            "std::int64" => Ok(Self::Int64),
            "std::float32" => Ok(Self::Float32),
            "std::float64" => Ok(Self::Float64),
            "std::bigint" => Ok(Self::BigInt),
            "std::decimal" => Ok(Self::Decimal),
            "std::uuid" => Ok(Self::Uuid),
            "std::duration" => Ok(Self::Duration),
            "std::bytes" => Ok(Self::Bytes),
            "std::sequence" => Ok(Self::Sequence),
            "cfg::memory" => Ok(Self::Memory),
            _ => Err(()),
        }
    }
}

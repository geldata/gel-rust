/*!
([Website reference](https://www.edgedb.com/docs/reference/protocol/messages#parse)) Capabilities, CompilationFlags etc. from the message protocol.
*/

use crate::model::Uuid;
use bytes::Bytes;

use crate::descriptors::Typedesc;
use crate::encoding::Input;
use crate::errors::DecodeError;
use crate::features::ProtocolVersion;

pub use crate::client_message::{Cardinality, InputLanguage, IoFormat};

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Capabilities: u64 {
        const MODIFICATIONS =       0b00000001;
        const SESSION_CONFIG =      0b00000010;
        const TRANSACTION =         0b00000100;
        const DDL =                 0b00001000;
        const PERSISTENT_CONFIG =   0b00010000;
        const ALL =                 0b00011111;
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct CompilationFlags: u64 {
        const INJECT_OUTPUT_TYPE_IDS =       0b00000001;
        const INJECT_OUTPUT_TYPE_NAMES =     0b00000010;
        const INJECT_OUTPUT_OBJECT_IDS =     0b00000100;
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct DumpFlags: u64 {
        const DUMP_SECRETS =                 0b00000001;
    }
}

#[derive(Debug, Clone)]
pub struct CompilationOptions {
    pub implicit_limit: Option<u64>,
    pub implicit_typenames: bool,
    pub implicit_typeids: bool,
    pub allow_capabilities: Capabilities,
    pub explicit_objectids: bool,
    pub io_format: IoFormat,
    pub expected_cardinality: Cardinality,
    pub input_language: InputLanguage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub typedesc_id: Uuid,
    pub data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTypedesc {
    pub proto: ProtocolVersion,
    pub id: Uuid,
    pub data: Bytes,
}

impl RawTypedesc {
    pub fn uninitialized() -> RawTypedesc {
        RawTypedesc {
            proto: ProtocolVersion::current(),
            id: Uuid::from_u128(0),
            data: Bytes::new(),
        }
    }
    pub fn decode(&self) -> Result<Typedesc, DecodeError> {
        let cur = &mut Input::new(self.proto.clone(), self.data.clone());
        Typedesc::decode_with_id(self.id, cur)
    }
}

impl State {
    pub fn empty() -> State {
        State {
            typedesc_id: Uuid::from_u128(0),
            data: Bytes::new(),
        }
    }
    pub fn descriptor_id(&self) -> Uuid {
        self.typedesc_id
    }
}

impl CompilationOptions {
    pub fn flags(&self) -> CompilationFlags {
        let mut cflags = CompilationFlags::empty();
        if self.implicit_typenames {
            cflags |= CompilationFlags::INJECT_OUTPUT_TYPE_NAMES;
        }
        if self.implicit_typeids {
            cflags |= CompilationFlags::INJECT_OUTPUT_TYPE_IDS;
        }
        // TODO(tailhook) object ids
        cflags
    }
}

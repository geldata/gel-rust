/*!
([Website reference](https://www.edgedb.com/docs/reference/protocol/messages)) The [ClientMessage] enum and related types.

```rust,ignore
pub enum ClientMessage {
    ClientHandshake(ClientHandshake),
    ExecuteScript(ExecuteScript),
    Prepare(Prepare),
    Parse(Parse),
    DescribeStatement(DescribeStatement),
    Execute0(Execute0),
    Execute1(Execute1),
    OptimisticExecute(OptimisticExecute),
    UnknownMessage(u8, Bytes),
    AuthenticationSaslInitialResponse(SaslInitialResponse),
    AuthenticationSaslResponse(SaslResponse),
    Dump(Dump),
    Restore(Restore),
    RestoreBlock(RestoreBlock),
    RestoreEof,
    Sync,
    Flush,
    Terminate,
}
```
*/

use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;

use bytes::{Buf, BufMut, Bytes};
use snafu::{ensure, OptionExt};
use uuid::Uuid;

pub use crate::common::CompilationOptions;
pub use crate::common::DumpFlags;
pub use crate::common::{Capabilities, Cardinality, CompilationFlags};
pub use crate::common::{RawTypedesc, State};
use crate::encoding::{encode, Decode, Encode, Input, Output};
use crate::encoding::{Annotations, KeyValues};
use crate::errors::{self, DecodeError, EncodeError};
use crate::new_protocol;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClientMessage {
    AuthenticationSaslInitialResponse(SaslInitialResponse),
    AuthenticationSaslResponse(SaslResponse),
    ClientHandshake(ClientHandshake),
    Dump2(Dump2),
    Dump3(Dump3),
    Parse(Parse), // protocol > 1.0
    ExecuteScript(ExecuteScript),
    Execute0(Execute0),
    Execute1(Execute1),
    Restore(Restore),
    RestoreBlock(RestoreBlock),
    RestoreEof,
    Sync,
    Terminate,
    Prepare(Prepare), // protocol < 1.0
    DescribeStatement(DescribeStatement),
    OptimisticExecute(OptimisticExecute),
    UnknownMessage(u8, Bytes),
    Flush,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaslInitialResponse {
    pub method: String,
    pub data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaslResponse {
    pub data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientHandshake {
    pub major_ver: u16,
    pub minor_ver: u16,
    pub params: HashMap<String, String>,
    pub extensions: HashMap<String, Annotations>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteScript {
    pub headers: KeyValues,
    pub script_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepare {
    pub headers: KeyValues,
    pub io_format: IoFormat,
    pub expected_cardinality: Cardinality,
    pub statement_name: Bytes,
    pub command_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    pub annotations: Option<Arc<Annotations>>,
    pub allowed_capabilities: Capabilities,
    pub compilation_flags: CompilationFlags,
    pub implicit_limit: Option<u64>,
    pub output_format: IoFormat,
    pub expected_cardinality: Cardinality,
    pub command_text: String,
    pub state: State,
    pub input_language: InputLanguage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribeStatement {
    pub headers: KeyValues,
    pub aspect: DescribeAspect,
    pub statement_name: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execute0 {
    pub headers: KeyValues,
    pub statement_name: Bytes,
    pub arguments: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execute1 {
    pub annotations: Option<Arc<Annotations>>,
    pub allowed_capabilities: Capabilities,
    pub compilation_flags: CompilationFlags,
    pub implicit_limit: Option<u64>,
    pub output_format: IoFormat,
    pub expected_cardinality: Cardinality,
    pub command_text: String,
    pub state: State,
    pub input_typedesc_id: Uuid,
    pub output_typedesc_id: Uuid,
    pub arguments: Bytes,
    pub input_language: InputLanguage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimisticExecute {
    pub headers: KeyValues,
    pub io_format: IoFormat,
    pub expected_cardinality: Cardinality,
    pub command_text: String,
    pub input_typedesc_id: Uuid,
    pub output_typedesc_id: Uuid,
    pub arguments: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dump2 {
    pub headers: KeyValues,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dump3 {
    pub annotations: Option<Arc<Annotations>>,
    pub flags: DumpFlags,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restore {
    pub headers: KeyValues,
    pub jobs: u16,
    pub data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreBlock {
    pub data: Bytes,
}

pub use crate::new_protocol::{DescribeAspect, InputLanguage, IoFormat};

struct Empty;
impl ClientMessage {
    pub fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        use ClientMessage::*;
        match self {
            ClientHandshake(h) => encode(buf, 0x56, h),
            AuthenticationSaslInitialResponse(h) => encode(buf, 0x70, h),
            AuthenticationSaslResponse(h) => encode(buf, 0x72, h),
            ExecuteScript(h) => encode(buf, 0x51, h),
            Prepare(h) => encode(buf, 0x50, h),
            Parse(h) => encode(buf, 0x50, h),
            DescribeStatement(h) => encode(buf, 0x44, h),
            Execute0(h) => encode(buf, 0x45, h),
            OptimisticExecute(h) => encode(buf, 0x4f, h),
            Execute1(h) => encode(buf, 0x4f, h),
            Dump2(h) => encode(buf, 0x3e, h),
            Dump3(h) => encode(buf, 0x3e, h),
            Restore(h) => encode(buf, 0x3c, h),
            RestoreBlock(h) => encode(buf, 0x3d, h),
            RestoreEof => encode(buf, 0x2e, &Empty),
            Sync => encode(buf, 0x53, &Empty),
            Flush => encode(buf, 0x48, &Empty),
            Terminate => encode(buf, 0x58, &Empty),

            UnknownMessage(_, _) => errors::UnknownMessageCantBeEncoded.fail()?,
        }
    }
    /// Decode exactly one frame from the buffer.
    ///
    /// This expects a full frame to already be in the buffer. It can return
    /// an arbitrary error or be silent if a message is only partially present
    /// in the buffer or if extra data is present.
    pub fn decode(buf: &mut Input) -> Result<ClientMessage, DecodeError> {
        use self::ClientMessage as M;
        let result = match buf[0] {
            0x56 => ClientHandshake::decode(buf).map(M::ClientHandshake)?,
            0x70 => SaslInitialResponse::decode(buf).map(M::AuthenticationSaslInitialResponse)?,
            0x72 => SaslResponse::decode(buf).map(M::AuthenticationSaslResponse)?,
            0x51 => ExecuteScript::decode(buf).map(M::ExecuteScript)?,
            0x50 => {
                if buf.proto().is_1() {
                    Parse::decode(buf).map(M::Parse)?
                } else {
                    Prepare::decode(buf).map(M::Prepare)?
                }
            }
            0x45 => Execute0::decode(buf).map(M::Execute0)?,
            0x4f => {
                if buf.proto().is_1() {
                    Execute1::decode(buf).map(M::Execute1)?
                } else {
                    OptimisticExecute::decode(buf).map(M::OptimisticExecute)?
                }
            }
            0x3e => {
                if buf.proto().is_3() {
                    Dump3::decode(buf).map(M::Dump3)?
                } else {
                    Dump2::decode(buf).map(M::Dump2)?
                }
            }
            0x3c => Restore::decode(buf).map(M::Restore)?,
            0x3d => RestoreBlock::decode(buf).map(M::RestoreBlock)?,
            0x2e => {
                buf.advance(5);
                M::RestoreEof
            }
            0x53 => {
                buf.advance(5);
                M::Sync
            }
            0x48 => {
                buf.advance(5);
                M::Flush
            }
            0x58 => {
                buf.advance(5);
                M::Terminate
            }
            0x44 => DescribeStatement::decode(buf).map(M::DescribeStatement)?,
            code => M::UnknownMessage(code, buf.copy_to_bytes(buf.remaining())),
        };
        // ensure!(buf.remaining() == 0, errors::ExtraData);
        Ok(result)
    }
}

impl Encode for Empty {
    fn encode(&self, _buf: &mut Output) -> Result<(), EncodeError> {
        Ok(())
    }
}

impl Encode for ClientHandshake {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        buf.reserve(8);
        buf.put_u16(self.major_ver);
        buf.put_u16(self.minor_ver);
        buf.put_u16(
            u16::try_from(self.params.len())
                .ok()
                .context(errors::TooManyParams)?,
        );
        for (k, v) in &self.params {
            k.encode(buf)?;
            v.encode(buf)?;
        }
        buf.reserve(2);
        buf.put_u16(
            u16::try_from(self.extensions.len())
                .ok()
                .context(errors::TooManyExtensions)?,
        );
        for (name, headers) in &self.extensions {
            // This is wrong!
            // name.encode(buf)?;
            // buf.reserve(2);
            // buf.put_u16(
            //     u16::try_from(headers.len())
            //         .ok()
            //         .context(errors::TooManyHeaders)?,
            // );
            // for (&name, value) in headers {
            //     buf.reserve(2);
            //     buf.put_u16(name);
            //     value.encode(buf)?;
            // }
        }
        Ok(())
    }
}

impl Decode for ClientHandshake {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::ClientHandshake::new(buf)?;
        let mut params = HashMap::new();
        for param in message.params() {
            params.insert(
                param.name().to_string_lossy().to_string(),
                param.value().to_string_lossy().to_string(),
            );
        }

        let mut extensions = HashMap::new();
        for ext in message.extensions() {
            let mut headers = HashMap::new();
            for ann in ext.annotations() {
                headers.insert(
                    ann.name().to_string_lossy().to_string(),
                    ann.value().to_string_lossy().to_string(),
                );
            }
            extensions.insert(ext.name().to_string_lossy().to_string(), headers);
        }

        let decoded = ClientHandshake {
            major_ver: message.major_ver(),
            minor_ver: message.minor_ver(),
            params,
            extensions,
        };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl Encode for SaslInitialResponse {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        self.method.encode(buf)?;
        self.data.encode(buf)?;
        Ok(())
    }
}

impl Decode for SaslInitialResponse {
    fn decode(buf: &mut Input) -> Result<SaslInitialResponse, DecodeError> {
        let message = new_protocol::AuthenticationSASLInitialResponse::new(buf)?;
        let decoded = SaslInitialResponse {
            method: message.method().to_string_lossy().to_string(),
            data: message.sasl_data().into_slice().to_owned().into(),
        };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl Encode for SaslResponse {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        self.data.encode(buf)?;
        Ok(())
    }
}

impl Decode for SaslResponse {
    fn decode(buf: &mut Input) -> Result<SaslResponse, DecodeError> {
        let message = new_protocol::AuthenticationSASLResponse::new(buf)?;
        let decoded = SaslResponse {
            data: message.sasl_data().into_slice().to_owned().into(),
        };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl Encode for ExecuteScript {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        buf.reserve(6);
        buf.put_u16(
            u16::try_from(self.headers.len())
                .ok()
                .context(errors::TooManyHeaders)?,
        );
        for (&name, value) in &self.headers {
            buf.reserve(2);
            buf.put_u16(name);
            value.encode(buf)?;
        }
        self.script_text.encode(buf)?;
        Ok(())
    }
}

impl Decode for ExecuteScript {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::ExecuteScript0::new(buf)?;
        let mut headers = HashMap::new();
        for header in message.headers() {
            headers.insert(header.code(), header.value().into_slice().to_owned().into());
        }

        let decoded = ExecuteScript {
            headers,
            script_text: message.script_text().to_string_lossy().to_string(),
        };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl Encode for Prepare {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        debug_assert!(!buf.proto().is_1());
        buf.reserve(12);
        buf.put_u16(
            u16::try_from(self.headers.len())
                .ok()
                .context(errors::TooManyHeaders)?,
        );
        for (&name, value) in &self.headers {
            buf.reserve(2);
            buf.put_u16(name);
            value.encode(buf)?;
        }
        buf.reserve(10);
        buf.put_u8(self.io_format as u8);
        buf.put_u8(self.expected_cardinality as u8);
        self.statement_name.encode(buf)?;
        self.command_text.encode(buf)?;
        Ok(())
    }
}

impl Decode for Prepare {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::Prepare0::new(buf)?;
        let mut headers = HashMap::new();
        for header in message.headers() {
            headers.insert(header.code(), header.value().into_slice().to_owned().into());
        }

        let decoded = Prepare {
            headers,
            io_format: message.io_format(),
            expected_cardinality: TryFrom::try_from(message.expected_cardinality())?,
            statement_name: message.statement_name().into_slice().to_owned().into(),
            command_text: message.command_text().to_string_lossy().to_string(),
        };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl Encode for DescribeStatement {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        buf.reserve(7);
        buf.put_u16(
            u16::try_from(self.headers.len())
                .ok()
                .context(errors::TooManyHeaders)?,
        );
        buf.reserve(5);
        buf.put_u8(self.aspect as u8);
        self.statement_name.encode(buf)?;
        Ok(())
    }
}

impl Decode for DescribeStatement {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::DescribeStatement0::new(buf)?;
        let mut headers = HashMap::new();
        for header in message.headers() {
            headers.insert(header.code(), header.value().into_slice().to_owned().into());
        }

        let decoded = DescribeStatement {
            headers,
            aspect: message.aspect(),
            statement_name: message.statement_name().into_slice().to_owned().into(),
        };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl Encode for Execute0 {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        debug_assert!(!buf.proto().is_1());
        buf.reserve(10);
        buf.put_u16(
            u16::try_from(self.headers.len())
                .ok()
                .context(errors::TooManyHeaders)?,
        );
        for (&name, value) in &self.headers {
            buf.reserve(2);
            buf.put_u16(name);
            value.encode(buf)?;
        }
        self.statement_name.encode(buf)?;
        self.arguments.encode(buf)?;
        Ok(())
    }
}

impl Decode for Execute0 {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::Execute0::new(buf)?;
        let mut headers = HashMap::new();
        for header in message.headers() {
            headers.insert(header.code(), header.value().into_slice().to_owned().into());
        }

        let decoded = Execute0 {
            headers,
            statement_name: message.statement_name().into_slice().to_owned().into(),
            arguments: message.arguments().into_slice().to_owned().into(),
        };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl OptimisticExecute {
    pub fn new(
        flags: &CompilationOptions,
        query: &str,
        arguments: impl Into<Bytes>,
        input_typedesc_id: Uuid,
        output_typedesc_id: Uuid,
    ) -> OptimisticExecute {
        let mut headers = KeyValues::new();
        if let Some(limit) = flags.implicit_limit {
            headers.insert(0xFF01, Bytes::from(limit.to_string()));
        }
        if flags.implicit_typenames {
            headers.insert(0xFF02, "true".into());
        }
        if flags.implicit_typeids {
            headers.insert(0xFF03, "true".into());
        }
        let caps = flags.allow_capabilities.bits().to_be_bytes();
        headers.insert(0xFF04, caps[..].to_vec().into());
        if flags.explicit_objectids {
            headers.insert(0xFF03, "true".into());
        }
        OptimisticExecute {
            headers,
            io_format: flags.io_format,
            expected_cardinality: flags.expected_cardinality,
            command_text: query.into(),
            input_typedesc_id,
            output_typedesc_id,
            arguments: arguments.into(),
        }
    }
}

impl Encode for OptimisticExecute {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        buf.reserve(2 + 1 + 1 + 4 + 16 + 16 + 4);
        buf.put_u16(
            u16::try_from(self.headers.len())
                .ok()
                .context(errors::TooManyHeaders)?,
        );
        for (&name, value) in &self.headers {
            buf.reserve(2);
            buf.put_u16(name);
            value.encode(buf)?;
        }
        buf.reserve(1 + 1 + 4 + 16 + 16 + 4);
        buf.put_u8(self.io_format as u8);
        buf.put_u8(self.expected_cardinality as u8);
        self.command_text.encode(buf)?;
        self.input_typedesc_id.encode(buf)?;
        self.output_typedesc_id.encode(buf)?;
        self.arguments.encode(buf)?;
        Ok(())
    }
}

impl Decode for OptimisticExecute {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::OptimisticExecute0::new(buf)?;
        let mut headers = HashMap::new();
        for header in message.headers() {
            headers.insert(header.code(), header.value().into_slice().to_owned().into());
        }

        let decoded = OptimisticExecute {
            headers,
            io_format: message.io_format(),
            expected_cardinality: TryFrom::try_from(message.expected_cardinality())?,
            command_text: message.command_text().to_string_lossy().to_string(),
            input_typedesc_id: message.input_typedesc_id(),
            output_typedesc_id: message.output_typedesc_id(),
            arguments: message.arguments().into_slice().to_owned().into(),
        };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl Encode for Execute1 {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        buf.reserve(2 + 3 * 8 + 1 + 1 + 4 + 16 + 4 + 16 + 16 + 4);
        if let Some(annotations) = self.annotations.as_deref() {
            buf.put_u16(
                u16::try_from(annotations.len())
                    .ok()
                    .context(errors::TooManyHeaders)?,
            );
            for (name, value) in annotations {
                buf.reserve(4);
                name.encode(buf)?;
                value.encode(buf)?;
            }
        } else {
            buf.put_u16(0);
        }
        buf.reserve(3 * 8 + 1 + 1 + 4 + 16 + 4 + 16 + 16 + 4);
        buf.put_u64(self.allowed_capabilities.bits());
        buf.put_u64(self.compilation_flags.bits());
        buf.put_u64(self.implicit_limit.unwrap_or(0));
        if buf.proto().is_multilingual() {
            buf.put_u8(self.input_language as u8);
        }
        buf.put_u8(self.output_format as u8);
        buf.put_u8(self.expected_cardinality as u8);
        self.command_text.encode(buf)?;
        self.state.typedesc_id.encode(buf)?;
        self.state.data.encode(buf)?;
        self.input_typedesc_id.encode(buf)?;
        self.output_typedesc_id.encode(buf)?;
        self.arguments.encode(buf)?;
        Ok(())
    }
}

impl Decode for Execute1 {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        if buf.proto().is_multilingual() {
            let message = new_protocol::Execute::new(buf)?;

            // Convert annotations
            let annotations = if message.annotations().len() > 0 {
                let mut ann_map = HashMap::new();
                for ann in message.annotations() {
                    ann_map.insert(
                        ann.name().to_string_lossy().to_string(),
                        ann.value().to_string_lossy().to_string(),
                    );
                }
                Some(Arc::new(ann_map))
            } else {
                None
            };

            // Convert state
            let state = State {
                typedesc_id: message.state_typedesc_id(),
                data: message.state_data().into_slice().to_owned().into(),
            };

            let decoded = Execute1 {
                annotations,
                allowed_capabilities: decode_capabilities(message.allowed_capabilities())?,
                compilation_flags: decode_compilation_flags(message.compilation_flags())?,
                implicit_limit: match message.implicit_limit() {
                    0 => None,
                    val => Some(val),
                },
                output_format: message.output_format(),
                expected_cardinality: TryFrom::try_from(message.expected_cardinality())?,
                command_text: message.command_text().to_string_lossy().to_string(),
                state,
                input_typedesc_id: message.input_typedesc_id(),
                output_typedesc_id: message.output_typedesc_id(),
                arguments: message.arguments().into_slice().to_owned().into(),
                input_language: message.input_language(),
            };
            buf.advance(message.buf.len());
            Ok(decoded)
        } else {
            let message = new_protocol::Execute2::new(buf)?;

            // Convert annotations
            let annotations = if message.annotations().len() > 0 {
                let mut ann_map = HashMap::new();
                for ann in message.annotations() {
                    ann_map.insert(
                        ann.name().to_string_lossy().to_string(),
                        ann.value().to_string_lossy().to_string(),
                    );
                }
                Some(Arc::new(ann_map))
            } else {
                None
            };

            // Convert state
            let state = State {
                typedesc_id: message.state_typedesc_id(),
                data: message.state_data().into_slice().to_owned().into(),
            };

            let decoded = Execute1 {
                annotations,
                allowed_capabilities: decode_capabilities(message.allowed_capabilities())?,
                compilation_flags: decode_compilation_flags(message.compilation_flags())?,
                implicit_limit: match message.implicit_limit() {
                    0 => None,
                    val => Some(val),
                },
                output_format: message.output_format(),
                expected_cardinality: TryFrom::try_from(message.expected_cardinality())?,
                command_text: message.command_text().to_string_lossy().to_string(),
                state,
                input_typedesc_id: message.input_typedesc_id(),
                output_typedesc_id: message.output_typedesc_id(),
                arguments: message.arguments().into_slice().to_owned().into(),
                input_language: InputLanguage::EdgeQL,
            };
            buf.advance(message.buf.len());
            Ok(decoded)
        }
    }
}

impl Encode for Dump2 {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        buf.reserve(10);
        buf.put_u16(
            u16::try_from(self.headers.len())
                .ok()
                .context(errors::TooManyHeaders)?,
        );
        for (&name, value) in &self.headers {
            buf.reserve(2);
            buf.put_u16(name);
            value.encode(buf)?;
        }
        Ok(())
    }
}

impl Decode for Dump2 {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::Dump2::new(buf)?;
        let mut headers = HashMap::new();
        for header in message.headers() {
            headers.insert(header.code(), header.value().into_slice().to_owned().into());
        }

        let decoded = Dump2 { headers };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl Encode for Dump3 {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        buf.reserve(2 + 8);
        if let Some(annotations) = self.annotations.as_deref() {
            buf.put_u16(
                u16::try_from(annotations.len())
                    .ok()
                    .context(errors::TooManyHeaders)?,
            );
            for (name, value) in annotations {
                buf.reserve(4);
                name.encode(buf)?;
                value.encode(buf)?;
            }
        } else {
            buf.put_u16(0);
        }
        buf.put_u64(self.flags.bits());
        Ok(())
    }
}

impl Decode for Dump3 {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::Dump3::new(buf)?;
        let mut annotations = HashMap::new();
        for ann in message.annotations() {
            annotations.insert(
                ann.name().to_string_lossy().to_string(),
                ann.value().to_string_lossy().to_string(),
            );
        }

        let decoded = Dump3 {
            annotations: Some(Arc::new(annotations)),
            flags: decode_dump_flags(message.flags())?,
        };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl Encode for Restore {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        buf.reserve(4 + self.data.len());
        buf.put_u16(
            u16::try_from(self.headers.len())
                .ok()
                .context(errors::TooManyHeaders)?,
        );
        for (&name, value) in &self.headers {
            buf.reserve(2);
            buf.put_u16(name);
            value.encode(buf)?;
        }
        buf.put_u16(self.jobs);
        buf.extend(&self.data);
        Ok(())
    }
}

impl Decode for Restore {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::Restore::new(buf)?;
        let mut headers = HashMap::new();
        for header in message.headers() {
            headers.insert(header.code(), header.value().into_slice().to_owned().into());
        }

        let decoded = Restore {
            headers,
            jobs: message.jobs(),
            data: message.data().as_ref().to_owned().into(),
        };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl Encode for RestoreBlock {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        buf.extend(&self.data);
        Ok(())
    }
}

impl Decode for RestoreBlock {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::RestoreBlock::new(buf)?;
        let decoded = RestoreBlock {
            data: message.block_data().into_slice().to_owned().into(),
        };
        buf.advance(message.buf.len());
        Ok(decoded)
    }
}

impl Parse {
    pub fn new(
        opts: &CompilationOptions,
        query: &str,
        state: State,
        annotations: Option<Arc<Annotations>>,
    ) -> Parse {
        Parse {
            annotations,
            allowed_capabilities: opts.allow_capabilities,
            compilation_flags: opts.flags(),
            implicit_limit: opts.implicit_limit,
            output_format: opts.io_format,
            expected_cardinality: opts.expected_cardinality,
            command_text: query.into(),
            state,
            input_language: opts.input_language,
        }
    }
}

impl Prepare {
    pub fn new(flags: &CompilationOptions, query: &str) -> Prepare {
        let mut headers = KeyValues::new();
        if let Some(limit) = flags.implicit_limit {
            headers.insert(0xFF01, Bytes::from(limit.to_string()));
        }
        if flags.implicit_typenames {
            headers.insert(0xFF02, "true".into());
        }
        if flags.implicit_typeids {
            headers.insert(0xFF03, "true".into());
        }
        let caps = flags.allow_capabilities.bits().to_be_bytes();
        headers.insert(0xFF04, caps[..].to_vec().into());
        if flags.explicit_objectids {
            headers.insert(0xFF03, "true".into());
        }
        Prepare {
            headers,
            io_format: flags.io_format,
            expected_cardinality: flags.expected_cardinality,
            statement_name: Bytes::from(""),
            command_text: query.into(),
        }
    }
}

fn decode_capabilities(val: u64) -> Result<Capabilities, DecodeError> {
    Capabilities::from_bits(val)
        .ok_or_else(|| errors::InvalidCapabilities { capabilities: val }.build())
}

fn decode_compilation_flags(val: u64) -> Result<CompilationFlags, DecodeError> {
    CompilationFlags::from_bits(val).ok_or_else(|| {
        errors::InvalidCompilationFlags {
            compilation_flags: val,
        }
        .build()
    })
}

fn decode_dump_flags(val: u64) -> Result<DumpFlags, DecodeError> {
    DumpFlags::from_bits(val).ok_or_else(|| errors::InvalidDumpFlags { dump_flags: val }.build())
}

impl Decode for Parse {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        if buf.proto().is_multilingual() {
            let message = new_protocol::Parse::new(buf)?;

            // Convert annotations
            let annotations = if message.annotations().len() > 0 {
                let mut ann_map = HashMap::new();
                for ann in message.annotations() {
                    ann_map.insert(
                        ann.name().to_string_lossy().to_string(),
                        ann.value().to_string_lossy().to_string(),
                    );
                }
                Some(Arc::new(ann_map))
            } else {
                None
            };

            // Convert state
            let state = State {
                typedesc_id: message.state_typedesc_id(),
                data: message.state_data().into_slice().to_owned().into(),
            };

            let decoded = Parse {
                annotations,
                allowed_capabilities: decode_capabilities(message.allowed_capabilities())?,
                compilation_flags: decode_compilation_flags(message.compilation_flags())?,
                implicit_limit: match message.implicit_limit() {
                    0 => None,
                    val => Some(val),
                },
                output_format: message.output_format(),
                expected_cardinality: TryFrom::try_from(message.expected_cardinality())?,
                command_text: message.command_text().to_string_lossy().to_string(),
                state,
                input_language: message.input_language(),
            };
            buf.advance(message.buf.len());
            Ok(decoded)
        } else {
            let message = new_protocol::Parse2::new(buf)?;

            // Convert annotations
            let annotations = if message.annotations().len() > 0 {
                let mut ann_map = HashMap::new();
                for ann in message.annotations() {
                    ann_map.insert(
                        ann.name().to_string_lossy().to_string(),
                        ann.value().to_string_lossy().to_string(),
                    );
                }
                Some(Arc::new(ann_map))
            } else {
                None
            };

            // Convert state
            let state = State {
                typedesc_id: message.state_typedesc_id(),
                data: message.state_data().into_slice().to_owned().into(),
            };

            let decoded = Parse {
                annotations,
                allowed_capabilities: decode_capabilities(message.allowed_capabilities())?,
                compilation_flags: decode_compilation_flags(message.compilation_flags())?,
                implicit_limit: match message.implicit_limit() {
                    0 => None,
                    val => Some(val),
                },
                output_format: message.output_format(),
                expected_cardinality: TryFrom::try_from(message.expected_cardinality())?,
                command_text: message.command_text().to_string_lossy().to_string(),
                state,
                input_language: InputLanguage::EdgeQL, // Default for non-multilingual
            };
            buf.advance(message.buf.len());
            Ok(decoded)
        }
    }
}

impl Encode for Parse {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        debug_assert!(buf.proto().is_1());
        buf.reserve(52);
        if let Some(annotations) = self.annotations.as_deref() {
            buf.put_u16(
                u16::try_from(annotations.len())
                    .ok()
                    .context(errors::TooManyHeaders)?,
            );
            for (name, value) in annotations {
                buf.reserve(8);
                name.encode(buf)?;
                value.encode(buf)?;
            }
        } else {
            buf.put_u16(0);
        }
        buf.reserve(50);
        buf.put_u64(self.allowed_capabilities.bits());
        buf.put_u64(self.compilation_flags.bits());
        buf.put_u64(self.implicit_limit.unwrap_or(0));
        if buf.proto().is_multilingual() {
            buf.put_u8(self.input_language as u8);
        }
        buf.put_u8(self.output_format as u8);
        buf.put_u8(self.expected_cardinality as u8);
        self.command_text.encode(buf)?;
        self.state.typedesc_id.encode(buf)?;
        self.state.data.encode(buf)?;
        Ok(())
    }
}

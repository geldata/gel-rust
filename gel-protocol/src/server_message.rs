/*!
The [ServerMessage] enum and related types. Gel website documentation on messages [here](https://www.edgedb.com/docs/reference/protocol/messages).

```rust,ignore
pub enum ServerMessage {
    ServerHandshake(ServerHandshake),
    UnknownMessage(u8, Bytes),
    LogMessage(LogMessage),
    ErrorResponse(ErrorResponse),
    Authentication(Authentication),
    ReadyForCommand(ReadyForCommand),
    ServerKeyData(ServerKeyData),
    ParameterStatus(ParameterStatus),
    CommandComplete0(CommandComplete0),
    CommandComplete1(CommandComplete1),
    PrepareComplete(PrepareComplete),
    CommandDataDescription0(CommandDataDescription0), // protocol < 1.0
    CommandDataDescription1(CommandDataDescription1), // protocol >= 1.0
    StateDataDescription(StateDataDescription),
    Data(Data),
    RestoreReady(RestoreReady),
    DumpHeader(RawPacket),
    DumpBlock(RawPacket),
}
```
*/

use std::collections::HashMap;
use std::convert::TryFrom;

use bytes::{Buf, BufMut, Bytes};
use uuid::Uuid;

use crate::common::Capabilities;
pub use crate::common::{Cardinality, RawTypedesc, State};
use crate::descriptors::Typedesc;
use crate::encoding::{Annotations, Decode, Encode, Input, KeyValues, Output};
use crate::errors::{self, DecodeError, EncodeError, MessageTooLong};
use crate::features::ProtocolVersion;
use crate::new_protocol::{
    self, prelude::EncoderForExt, AnnotationBuilder, ProtocolExtensionBuilder,
    ServerHandshakeBuilder,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ServerMessage {
    Authentication(Authentication),
    CommandComplete0(CommandComplete0),
    CommandComplete1(CommandComplete1),
    CommandDataDescription0(CommandDataDescription0), // protocol < 1.0
    CommandDataDescription1(CommandDataDescription1), // protocol >= 1.0
    StateDataDescription(StateDataDescription),
    Data(Data),
    // Don't decode Dump packets here as we only need to process them as
    // whole
    DumpHeader(RawPacket),
    DumpBlock(RawPacket),
    ErrorResponse(ErrorResponse),
    LogMessage(LogMessage),
    ParameterStatus(ParameterStatus),
    ReadyForCommand(ReadyForCommand),
    RestoreReady(RestoreReady),
    ServerHandshake(ServerHandshake),
    ServerKeyData(ServerKeyData),
    UnknownMessage(u8, Bytes),
    PrepareComplete(PrepareComplete),
}

pub use crate::new_protocol::TransactionState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyForCommand {
    pub annotations: Annotations,
    pub transaction_state: TransactionState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authentication {
    Ok,
    Sasl { methods: Vec<String> },
    SaslContinue { data: Bytes },
    SaslFinal { data: Bytes },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Error,
    Fatal,
    Panic,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageSeverity {
    Debug,
    Info,
    Notice,
    Warning,
    Unknown(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorResponse {
    pub severity: ErrorSeverity,
    pub code: u32,
    pub message: String,
    pub attributes: KeyValues,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogMessage {
    pub severity: MessageSeverity,
    pub code: u32,
    pub text: String,
    pub annotations: Annotations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerHandshake {
    pub major_ver: u16,
    pub minor_ver: u16,
    pub extensions: HashMap<String, Annotations>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerKeyData {
    pub data: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterStatus {
    pub proto: ProtocolVersion,
    pub name: Bytes,
    pub value: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandComplete0 {
    pub headers: KeyValues,
    pub status_data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandComplete1 {
    pub annotations: Annotations,
    pub capabilities: Capabilities,
    pub status: String,
    pub state: Option<State>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareComplete {
    pub headers: KeyValues,
    pub cardinality: Cardinality,
    pub input_typedesc_id: Uuid,
    pub output_typedesc_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseComplete {
    pub headers: KeyValues,
    pub cardinality: Cardinality,
    pub input_typedesc_id: Uuid,
    pub output_typedesc_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDataDescription0 {
    pub headers: KeyValues,
    pub result_cardinality: Cardinality,
    pub input: RawTypedesc,
    pub output: RawTypedesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDataDescription1 {
    pub annotations: Annotations,
    pub capabilities: Capabilities,
    pub result_cardinality: Cardinality,
    pub input: RawTypedesc,
    pub output: RawTypedesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateDataDescription {
    pub typedesc: RawTypedesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Data {
    pub data: Vec<Bytes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReady {
    pub headers: KeyValues,
    pub jobs: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPacket {
    pub data: Bytes,
}

fn encode<T: Encode>(buf: &mut Output, msg: &T) -> Result<(), EncodeError> {
    msg.encode(buf)?;
    Ok(())
}

impl CommandDataDescription0 {
    pub fn output(&self) -> Result<Typedesc, DecodeError> {
        self.output.decode()
    }
    pub fn input(&self) -> Result<Typedesc, DecodeError> {
        self.input.decode()
    }
}

impl CommandDataDescription1 {
    pub fn output(&self) -> Result<Typedesc, DecodeError> {
        self.output.decode()
    }
    pub fn input(&self) -> Result<Typedesc, DecodeError> {
        self.input.decode()
    }
}

/// Bridges the old and new world.
fn encode_output<T: 'static>(
    buf: &mut Output,
    builder: impl new_protocol::prelude::EncoderFor<T>,
) -> Result<(), EncodeError> {
    let len = builder.measure();
    buf.reserve(len);
    let len = builder
        .encode_buffer_uninit(buf.uninit())
        .map_err(|_| MessageTooLong.build())?
        .len();
    unsafe { buf.advance_mut(len) };
    Ok(())
}

impl From<CommandDataDescription0> for CommandDataDescription1 {
    fn from(value: CommandDataDescription0) -> Self {
        Self {
            annotations: HashMap::new(),
            capabilities: decode_capabilities0(&value.headers).unwrap_or(Capabilities::ALL),
            result_cardinality: value.result_cardinality,
            input: value.input,
            output: value.output,
        }
    }
}

impl StateDataDescription {
    pub fn parse(self) -> Result<Typedesc, DecodeError> {
        self.typedesc.decode()
    }
}

impl ParameterStatus {
    pub fn parse_system_config(self) -> Result<(Typedesc, Bytes), DecodeError> {
        let cur = &mut Input::new(self.proto.clone(), self.value);
        let typedesc_data = Bytes::decode(cur)?;
        let data = Bytes::decode(cur)?;

        let typedesc_buf = &mut Input::new(self.proto, typedesc_data);
        let typedesc_id = Uuid::decode(typedesc_buf)?;
        let typedesc = Typedesc::decode_with_id(typedesc_id, typedesc_buf)?;
        Ok((typedesc, data))
    }
}

impl ServerMessage {
    pub fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        use ServerMessage::*;
        match self {
            ServerHandshake(h) => encode(buf, h),
            ErrorResponse(h) => encode(buf, h),
            LogMessage(h) => encode(buf, h),
            Authentication(h) => encode(buf, h),
            ReadyForCommand(h) => encode(buf, h),
            ServerKeyData(h) => encode(buf, h),
            ParameterStatus(h) => encode(buf, h),
            CommandComplete0(h) => encode(buf, h),
            CommandComplete1(h) => encode(buf, h),
            PrepareComplete(h) => encode(buf, h),
            CommandDataDescription0(h) => encode(buf, h),
            CommandDataDescription1(h) => encode(buf, h),
            StateDataDescription(h) => encode(buf, h),
            Data(h) => encode(buf, h),
            RestoreReady(h) => encode(buf, h),
            DumpHeader(h) => encode(buf, h),
            DumpBlock(h) => encode(buf, h),

            UnknownMessage(_, _) => errors::UnknownMessageCantBeEncoded.fail()?,
        }
    }
    /// Decode exactly one frame from the buffer.
    ///
    /// This expects a full frame to already be in the buffer. It can return
    /// an arbitrary error or be silent if a message is only partially present
    /// in the buffer or if extra data is present.
    pub fn decode(buf: &mut Input) -> Result<ServerMessage, DecodeError> {
        use self::ServerMessage as M;
        let message = new_protocol::Message::new(buf)?;
        let mut next = buf.slice(..message.mlen() + 1);
        buf.advance(message.mlen() + 1);
        let buf = &mut next;

        let result = match buf[0] {
            0x76 => ServerHandshake::decode(buf).map(M::ServerHandshake)?,
            0x45 => ErrorResponse::decode(buf).map(M::ErrorResponse)?,
            0x4c => LogMessage::decode(buf).map(M::LogMessage)?,
            0x52 => Authentication::decode(buf).map(M::Authentication)?,
            0x5a => ReadyForCommand::decode(buf).map(M::ReadyForCommand)?,
            0x4b => ServerKeyData::decode(buf).map(M::ServerKeyData)?,
            0x53 => ParameterStatus::decode(buf).map(M::ParameterStatus)?,
            0x43 => {
                if buf.proto().is_1() {
                    CommandComplete1::decode(buf).map(M::CommandComplete1)?
                } else {
                    CommandComplete0::decode(buf).map(M::CommandComplete0)?
                }
            }
            0x31 => PrepareComplete::decode(buf).map(M::PrepareComplete)?,
            0x44 => Data::decode(buf).map(M::Data)?,
            0x2b => RestoreReady::decode(buf).map(M::RestoreReady)?,
            0x40 => RawPacket::decode(buf).map(M::DumpHeader)?,
            0x3d => RawPacket::decode(buf).map(M::DumpBlock)?,
            0x54 => {
                if buf.proto().is_1() {
                    CommandDataDescription1::decode(buf).map(M::CommandDataDescription1)?
                } else {
                    CommandDataDescription0::decode(buf).map(M::CommandDataDescription0)?
                }
            }
            0x73 => StateDataDescription::decode(buf).map(M::StateDataDescription)?,
            code => M::UnknownMessage(code, buf.copy_to_bytes(buf.remaining())),
        };
        Ok(result)
    }
}

impl Encode for ServerHandshake {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        let extensions = || {
            self.extensions.iter().map(|(name, headers)| {
                let annotations = move || {
                    headers
                        .iter()
                        .map(|(name, value)| AnnotationBuilder { name, value })
                };
                ProtocolExtensionBuilder { name, annotations }
            })
        };
        let builder = ServerHandshakeBuilder {
            major_ver: self.major_ver,
            minor_ver: self.minor_ver,
            extensions,
        };

        encode_output(buf, builder)?;
        Ok(())
    }
}

impl Decode for ServerHandshake {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::ServerHandshake::new(buf)?;
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

        let decoded = ServerHandshake {
            major_ver: message.major_ver(),
            minor_ver: message.minor_ver(),
            extensions,
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for ErrorResponse {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        let builder = new_protocol::ErrorResponseBuilder {
            severity: self.severity.to_u8(),
            error_code: self.code,
            message: &self.message,
            attributes: || {
                self.attributes
                    .iter()
                    .map(|(name, value)| new_protocol::KeyValueBuilder {
                        code: *name,
                        value: value.as_ref(),
                    })
            },
        };
        encode_output(buf, builder)?;

        Ok(())
    }
}

impl Decode for ErrorResponse {
    fn decode(buf: &mut Input) -> Result<ErrorResponse, DecodeError> {
        let message = new_protocol::ErrorResponse::new(buf)?;
        let mut attributes = HashMap::new();
        for attr in message.attributes() {
            attributes.insert(attr.code(), attr.value().into_slice().to_vec().into());
        }

        let decoded = ErrorResponse {
            severity: ErrorSeverity::from_u8(message.severity()),
            code: message.error_code() as u32,
            message: message.message().to_string_lossy().to_string(),
            attributes,
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for LogMessage {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        let builder = new_protocol::LogMessageBuilder {
            severity: self.severity.to_u8(),
            code: self.code as i32,
            text: &self.text,
            annotations: || {
                self.annotations
                    .iter()
                    .map(|(name, value)| new_protocol::AnnotationBuilder { name, value })
            },
        };
        encode_output(buf, builder)?;

        Ok(())
    }
}

impl Decode for LogMessage {
    fn decode(buf: &mut Input) -> Result<LogMessage, DecodeError> {
        let message = new_protocol::LogMessage::new(buf)?;
        let mut annotations = HashMap::new();
        for ann in message.annotations() {
            annotations.insert(
                ann.name().to_string_lossy().to_string(),
                ann.value().to_string_lossy().to_string(),
            );
        }

        let decoded = LogMessage {
            severity: MessageSeverity::from_u8(message.severity()),
            code: message.code() as u32,
            text: message.text().to_string_lossy().to_string(),
            annotations,
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for Authentication {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        use Authentication as A;
        match self {
            A::Ok => encode_output(buf, new_protocol::AuthenticationOkBuilder {})?,
            A::Sasl { methods } => {
                let builder = new_protocol::AuthenticationRequiredSASLMessageBuilder {
                    methods: methods.as_slice(),
                };
                encode_output(buf, builder)?;
            }
            A::SaslContinue { data } => {
                let builder = new_protocol::AuthenticationSASLContinueBuilder {
                    sasl_data: data.as_ref(),
                };
                encode_output(buf, builder)?;
            }
            A::SaslFinal { data } => {
                let builder = new_protocol::AuthenticationSASLFinalBuilder {
                    sasl_data: data.as_ref(),
                };
                encode_output(buf, builder)?;
            }
        }
        Ok(())
    }
}

impl Decode for Authentication {
    fn decode(buf: &mut Input) -> Result<Authentication, DecodeError> {
        let auth = new_protocol::Authentication::new(buf)?;
        match auth.auth_status() {
            0x0 => Ok(Authentication::Ok),
            0x0A => {
                let auth = new_protocol::AuthenticationRequiredSASLMessage::new(buf)?;
                let mut methods = Vec::new();
                for method in auth.methods() {
                    methods.push(method.to_string_lossy().to_string());
                }
                Ok(Authentication::Sasl { methods })
            }
            0x0B => {
                let auth = new_protocol::AuthenticationSASLContinue::new(buf)?;
                Ok(Authentication::SaslContinue {
                    data: auth.sasl_data().into_slice().to_owned().into(),
                })
            }
            0x0C => {
                let auth = new_protocol::AuthenticationSASLFinal::new(buf)?;
                Ok(Authentication::SaslFinal {
                    data: auth.sasl_data().into_slice().to_owned().into(),
                })
            }
            _ => errors::AuthStatusInvalid {
                auth_status: buf[0],
            }
            .fail()?,
        }
    }
}

impl Encode for ReadyForCommand {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        let builder = new_protocol::ReadyForCommandBuilder {
            transaction_state: self.transaction_state,
            annotations: || {
                self.annotations
                    .iter()
                    .map(|(name, value)| new_protocol::AnnotationBuilder { name, value })
            },
        };
        buf.reserve(builder.measure());
        let len = builder
            .encode_buffer_uninit(buf.uninit())
            .map_err(|_| MessageTooLong.build())?
            .len();
        unsafe { buf.advance_mut(len) };

        Ok(())
    }
}
impl Decode for ReadyForCommand {
    fn decode(buf: &mut Input) -> Result<ReadyForCommand, DecodeError> {
        let message = new_protocol::ReadyForCommand::new(buf)?;
        let mut annotations = HashMap::new();
        for ann in message.annotations() {
            annotations.insert(
                ann.name().to_string_lossy().to_string(),
                ann.value().to_string_lossy().to_string(),
            );
        }

        let decoded = ReadyForCommand {
            annotations,
            transaction_state: message.transaction_state(),
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl ErrorSeverity {
    pub fn from_u8(code: u8) -> ErrorSeverity {
        use ErrorSeverity::*;
        match code {
            120 => Error,
            200 => Fatal,
            255 => Panic,
            _ => Unknown(code),
        }
    }
    pub fn to_u8(&self) -> u8 {
        use ErrorSeverity::*;
        match *self {
            Error => 120,
            Fatal => 200,
            Panic => 255,
            Unknown(code) => code,
        }
    }
}

impl MessageSeverity {
    fn from_u8(code: u8) -> MessageSeverity {
        use MessageSeverity::*;
        match code {
            20 => Debug,
            40 => Info,
            60 => Notice,
            80 => Warning,
            _ => Unknown(code),
        }
    }
    fn to_u8(self) -> u8 {
        use MessageSeverity::*;
        match self {
            Debug => 20,
            Info => 40,
            Notice => 60,
            Warning => 80,
            Unknown(code) => code,
        }
    }
}

impl Encode for ServerKeyData {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        let builder = new_protocol::ServerKeyDataBuilder { data: self.data };
        encode_output(buf, builder)?;
        Ok(())
    }
}
impl Decode for ServerKeyData {
    fn decode(buf: &mut Input) -> Result<ServerKeyData, DecodeError> {
        let message = new_protocol::ServerKeyData::new(buf)?;
        let decoded = ServerKeyData {
            data: message.data(),
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for ParameterStatus {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        let builder = new_protocol::ParameterStatusBuilder {
            name: self.name.as_ref(),
            value: self.value.as_ref(),
        };
        encode_output(buf, builder)?;
        Ok(())
    }
}
impl Decode for ParameterStatus {
    fn decode(buf: &mut Input) -> Result<ParameterStatus, DecodeError> {
        let message = new_protocol::ParameterStatus::new(buf)?;
        let decoded = ParameterStatus {
            proto: buf.proto().clone(),
            name: message.name().into_slice().to_owned().into(),
            value: message.value().into_slice().to_owned().into(),
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for CommandComplete0 {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        let builder = new_protocol::CommandComplete0Builder {
            headers: || {
                self.headers
                    .iter()
                    .map(|(name, value)| new_protocol::KeyValueBuilder {
                        code: *name,
                        value: value.as_ref(),
                    })
            },
            status_data: self.status_data.as_ref(),
        };
        encode_output(buf, builder)?;

        Ok(())
    }
}

impl Decode for CommandComplete0 {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::CommandComplete0::new(buf)?;
        let mut headers = HashMap::new();
        for header in message.headers() {
            headers.insert(header.code(), header.value().into_slice().to_owned().into());
        }

        let decoded = CommandComplete0 {
            headers,
            status_data: message.status_data().into_slice().to_owned().into(),
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for CommandComplete1 {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        let builder = new_protocol::CommandCompleteBuilder {
            annotations: || {
                self.annotations
                    .iter()
                    .map(|(name, value)| new_protocol::AnnotationBuilder { name, value })
            },
            capabilities: self.capabilities.bits(),
            status: &self.status,
            state_data: self
                .state
                .as_ref()
                .map(|state| state.data.as_ref())
                .unwrap_or_default(),
            state_typedesc_id: self
                .state
                .as_ref()
                .map(|state| state.typedesc_id)
                .unwrap_or_default(),
        };
        encode_output(buf, builder)?;

        Ok(())
    }
}

impl Decode for CommandComplete1 {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::CommandComplete::new(buf)?;
        let mut annotations = HashMap::new();
        for ann in message.annotations() {
            annotations.insert(
                ann.name().to_string_lossy().to_string(),
                ann.value().to_string_lossy().to_string(),
            );
        }

        let decoded = CommandComplete1 {
            annotations,
            capabilities: Capabilities::from_bits_retain(message.capabilities()),
            status: message.status().to_string_lossy().to_string(),
            state: if message.state_typedesc_id() == Uuid::from_u128(0) {
                None
            } else {
                Some(State {
                    typedesc_id: message.state_typedesc_id(),
                    data: message.state_data().into_slice().to_owned().into(),
                })
            },
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for PrepareComplete {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        let builder = new_protocol::PrepareComplete0Builder {
            headers: || {
                self.headers
                    .iter()
                    .map(|(name, value)| new_protocol::KeyValueBuilder {
                        code: *name,
                        value: value.as_ref(),
                    })
            },
            cardinality: self.cardinality as u8,
            input_typedesc_id: self.input_typedesc_id,
            output_typedesc_id: self.output_typedesc_id,
        };
        encode_output(buf, builder)?;
        Ok(())
    }
}

impl Decode for PrepareComplete {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::PrepareComplete0::new(buf)?;
        let mut headers = HashMap::new();
        for header in message.headers() {
            headers.insert(header.code(), header.value().into_slice().to_owned().into());
        }

        let decoded = PrepareComplete {
            headers,
            cardinality: TryFrom::try_from(message.cardinality())?,
            input_typedesc_id: message.input_typedesc_id(),
            output_typedesc_id: message.output_typedesc_id(),
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for CommandDataDescription0 {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        debug_assert!(!buf.proto().is_1());
        let builder = new_protocol::CommandDataDescription0Builder {
            headers: || {
                self.headers
                    .iter()
                    .map(|(name, value)| new_protocol::KeyValueBuilder {
                        code: *name,
                        value: value.as_ref(),
                    })
            },
            result_cardinality: self.result_cardinality as u8,
            input_typedesc_id: self.input.id,
            input_typedesc: self.input.data.as_ref(),
            output_typedesc_id: self.output.id,
            output_typedesc: self.output.data.as_ref(),
        };
        encode_output(buf, builder)?;
        Ok(())
    }
}

impl Decode for CommandDataDescription0 {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::CommandDataDescription0::new(buf)?;
        let mut headers = HashMap::new();
        for header in message.headers() {
            headers.insert(header.code(), header.value().into_slice().to_owned().into());
        }

        let decoded = CommandDataDescription0 {
            headers,
            result_cardinality: TryFrom::try_from(message.result_cardinality())?,
            input: RawTypedesc {
                proto: buf.proto().clone(),
                id: message.input_typedesc_id(),
                data: message.input_typedesc().into_slice().to_owned().into(),
            },
            output: RawTypedesc {
                proto: buf.proto().clone(),
                id: message.output_typedesc_id(),
                data: message.output_typedesc().into_slice().to_owned().into(),
            },
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for CommandDataDescription1 {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        debug_assert!(buf.proto().is_1());
        let builder = new_protocol::CommandDataDescriptionBuilder {
            annotations: || {
                self.annotations
                    .iter()
                    .map(|(name, value)| new_protocol::AnnotationBuilder { name, value })
            },
            capabilities: self.capabilities.bits(),
            result_cardinality: self.result_cardinality as u8,
            input_typedesc_id: self.input.id,
            input_typedesc: self.input.data.as_ref(),
            output_typedesc_id: self.output.id,
            output_typedesc: self.output.data.as_ref(),
        };
        encode_output(buf, builder)?;
        Ok(())
    }
}

impl Decode for CommandDataDescription1 {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::CommandDataDescription::new(buf)?;
        let mut annotations = HashMap::new();
        for ann in message.annotations() {
            annotations.insert(
                ann.name().to_string_lossy().to_string(),
                ann.value().to_string_lossy().to_string(),
            );
        }

        let decoded = CommandDataDescription1 {
            annotations,
            capabilities: Capabilities::from_bits_retain(message.capabilities()),
            result_cardinality: TryFrom::try_from(message.result_cardinality())?,
            input: RawTypedesc {
                proto: buf.proto().clone(),
                id: message.input_typedesc_id(),
                data: message.input_typedesc().into_slice().to_owned().into(),
            },
            output: RawTypedesc {
                proto: buf.proto().clone(),
                id: message.output_typedesc_id(),
                data: message.output_typedesc().into_slice().to_owned().into(),
            },
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for StateDataDescription {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        debug_assert!(buf.proto().is_1());
        let builder = new_protocol::StateDataDescriptionBuilder {
            typedesc_id: self.typedesc.id,
            typedesc: self.typedesc.data.as_ref(),
        };
        encode_output(buf, builder)?;
        Ok(())
    }
}

impl Decode for StateDataDescription {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::StateDataDescription::new(buf)?;
        let decoded = StateDataDescription {
            typedesc: RawTypedesc {
                proto: buf.proto().clone(),
                id: message.typedesc_id(),
                data: message.typedesc().into_slice().to_owned().into(),
            },
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for Data {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        let builder = new_protocol::DataBuilder {
            data: || {
                self.data
                    .iter()
                    .map(|chunk| new_protocol::DataElementBuilder {
                        data: chunk.as_ref(),
                    })
            },
        };
        encode_output(buf, builder)?;
        Ok(())
    }
}

impl Decode for Data {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::Data::new(buf)?;
        let mut data = Vec::new();
        for element in message.data() {
            data.push(element.data().into_slice().to_owned().into());
        }

        let decoded = Data { data };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for RestoreReady {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        let builder = new_protocol::RestoreReadyBuilder {
            headers: || {
                self.headers
                    .iter()
                    .map(|(name, value)| new_protocol::KeyValueBuilder {
                        code: *name,
                        value: value.as_ref(),
                    })
            },
            jobs: self.jobs,
        };
        encode_output(buf, builder)?;
        Ok(())
    }
}

impl Decode for RestoreReady {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        let message = new_protocol::RestoreReady::new(buf)?;
        let mut headers = HashMap::new();
        for header in message.headers() {
            headers.insert(header.code(), header.value().into_slice().to_vec().into());
        }

        let decoded = RestoreReady {
            headers,
            jobs: message.jobs() as u16,
        };
        buf.advance(message.as_ref().len());
        Ok(decoded)
    }
}

impl Encode for RawPacket {
    fn encode(&self, buf: &mut Output) -> Result<(), EncodeError> {
        buf.extend(&self.data);
        Ok(())
    }
}

impl Decode for RawPacket {
    fn decode(buf: &mut Input) -> Result<Self, DecodeError> {
        Ok(RawPacket {
            data: buf.copy_to_bytes(buf.remaining()),
        })
    }
}

impl PrepareComplete {
    pub fn get_capabilities(&self) -> Option<Capabilities> {
        decode_capabilities0(&self.headers)
    }
}

fn decode_capabilities0(headers: &KeyValues) -> Option<Capabilities> {
    headers.get(&0x1001).and_then(|bytes| {
        if bytes.len() == 8 {
            let mut array = [0u8; 8];
            array.copy_from_slice(bytes);
            Some(Capabilities::from_bits_retain(u64::from_be_bytes(array)))
        } else {
            None
        }
    })
}

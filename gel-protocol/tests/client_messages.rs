use std::collections::HashMap;
use std::error::Error;

use bytes::{Bytes, BytesMut};
use gel_protocol::common::InputLanguage;
use uuid::Uuid;

use gel_protocol::client_message::Execute1;
use gel_protocol::client_message::Restore;
use gel_protocol::client_message::SaslInitialResponse;
use gel_protocol::client_message::SaslResponse;
use gel_protocol::client_message::{Cardinality, IoFormat, Parse};
use gel_protocol::client_message::{ClientHandshake, ClientMessage};
use gel_protocol::common::{Capabilities, CompilationFlags, State};
use gel_protocol::encoding::{Input, Output};
use gel_protocol::features::ProtocolVersion;

mod base;

macro_rules! encoding_eq_ver {
    ($major: expr, $minor: expr, $message: expr, $bytes: expr) => {
        let proto = ProtocolVersion::new($major, $minor);
        let data: &[u8] = $bytes;
        let mut bytes = BytesMut::new();
        $message.encode(&mut Output::new(&proto, &mut bytes))?;
        println!("Serialized bytes {:?}", bytes);
        let bytes = bytes.freeze();
        assert_eq!(&bytes[..], data);
        assert_eq!(
            ClientMessage::decode(&mut Input::new(proto, Bytes::copy_from_slice(data)))?,
            $message,
        );
    };
}

macro_rules! encoding_eq {
    ($message: expr, $bytes: expr) => {
        let (major, minor) = ProtocolVersion::current().version_tuple();
        encoding_eq_ver!(major, minor, $message, $bytes);
    };
}

#[test]
fn client_handshake() -> Result<(), Box<dyn Error>> {
    encoding_eq!(
        ClientMessage::ClientHandshake(ClientHandshake {
            major_ver: 1,
            minor_ver: 2,
            params: HashMap::new(),
            extensions: HashMap::new(),
        }),
        b"\x56\x00\x00\x00\x0C\x00\x01\x00\x02\x00\x00\x00\x00"
    );
    Ok(())
}

#[test]
fn parse2() -> Result<(), Box<dyn Error>> {
    encoding_eq_ver!(
        2,
        0,
        ClientMessage::Parse(Parse {
            annotations: None,
            allowed_capabilities: Capabilities::MODIFICATIONS,
            compilation_flags: CompilationFlags::INJECT_OUTPUT_TYPE_NAMES,
            implicit_limit: Some(77),
            input_language: InputLanguage::EdgeQL,
            output_format: IoFormat::Binary,
            expected_cardinality: Cardinality::AtMostOne,
            command_text: String::from("SELECT 1;"),
            state: State {
                typedesc_id: Uuid::from_u128(0),
                data: Bytes::from(""),
            },
        }),
        b"P\0\0\0A\0\0\0\0\0\0\0\0\0\x01\0\0\0\0\0\0\0\x02\0\0\0\0\0\0\0Mbo\
          \0\0\0\tSELECT 1;\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"
    );
    Ok(())
}

#[test]
fn parse3() -> Result<(), Box<dyn Error>> {
    encoding_eq_ver!(
        3,
        0,
        ClientMessage::Parse(Parse {
            annotations: None,
            allowed_capabilities: Capabilities::MODIFICATIONS,
            compilation_flags: CompilationFlags::INJECT_OUTPUT_TYPE_NAMES,
            implicit_limit: Some(77),
            input_language: InputLanguage::EdgeQL,
            output_format: IoFormat::Binary,
            expected_cardinality: Cardinality::AtMostOne,
            command_text: String::from("SELECT 1;"),
            state: State {
                typedesc_id: Uuid::from_u128(0),
                data: Bytes::from(""),
            },
        }),
        b"P\0\0\0B\0\0\0\0\0\0\0\0\0\x01\0\0\0\0\0\0\0\x02\0\0\0\0\0\0\0MEbo\
          \0\0\0\tSELECT 1;\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"
    );
    Ok(())
}

#[test]
fn execute2() -> Result<(), Box<dyn Error>> {
    encoding_eq_ver!(
        2,
        0,
        ClientMessage::Execute1(Execute1 {
            annotations: None,
            allowed_capabilities: Capabilities::MODIFICATIONS,
            compilation_flags: CompilationFlags::INJECT_OUTPUT_TYPE_NAMES,
            implicit_limit: Some(77),
            input_language: InputLanguage::EdgeQL,
            output_format: IoFormat::Binary,
            expected_cardinality: Cardinality::AtMostOne,
            command_text: String::from("SELECT 1;"),
            state: State {
                typedesc_id: Uuid::from_u128(0),
                data: Bytes::from(""),
            },
            input_typedesc_id: Uuid::from_u128(123),
            output_typedesc_id: Uuid::from_u128(456),
            arguments: Bytes::new(),
        }),
        b"O\0\0\0e\0\0\0\0\0\0\0\0\0\x01\0\0\0\0\0\0\0\x02\0\0\0\0\0\0\0Mbo\
          \0\0\0\tSELECT 1;\
          \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
          \0\0\0{\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\xc8\0\0\0\0"
    );
    Ok(())
}

#[test]
fn execute3() -> Result<(), Box<dyn Error>> {
    encoding_eq_ver!(
        3,
        0,
        ClientMessage::Execute1(Execute1 {
            annotations: None,
            allowed_capabilities: Capabilities::MODIFICATIONS,
            compilation_flags: CompilationFlags::INJECT_OUTPUT_TYPE_NAMES,
            implicit_limit: Some(77),
            input_language: InputLanguage::EdgeQL,
            output_format: IoFormat::Binary,
            expected_cardinality: Cardinality::AtMostOne,
            command_text: String::from("SELECT 1;"),
            state: State {
                typedesc_id: Uuid::from_u128(0),
                data: Bytes::from(""),
            },
            input_typedesc_id: Uuid::from_u128(123),
            output_typedesc_id: Uuid::from_u128(456),
            arguments: Bytes::new(),
        }),
        b"O\0\0\0f\0\0\0\0\0\0\0\0\0\x01\0\0\0\0\0\0\0\x02\0\0\0\0\0\0\0MEbo\
          \0\0\0\tSELECT 1;\
          \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
          \0\0\0{\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\xc8\0\0\0\0"
    );
    Ok(())
}

#[test]
fn sync() -> Result<(), Box<dyn Error>> {
    encoding_eq!(ClientMessage::Sync, b"S\0\0\0\x04");
    Ok(())
}

#[test]
fn terminate() -> Result<(), Box<dyn Error>> {
    encoding_eq!(ClientMessage::Terminate, b"X\0\0\0\x04");
    Ok(())
}

#[test]
fn authentication() -> Result<(), Box<dyn Error>> {
    encoding_eq!(
        ClientMessage::AuthenticationSaslInitialResponse(SaslInitialResponse {
            method: "SCRAM-SHA-256".into(),
            data: "n,,n=tutorial,r=%NR65>7bQ2S3jzl^k$G&b1^A".into(),
        }),
        bconcat!(b"p\0\0\0A\0\0\0\rSCRAM-SHA-256"
                 b"\0\0\0(n,,n=tutorial,"
                 b"r=%NR65>7bQ2S3jzl^k$G&b1^A")
    );
    encoding_eq!(
        ClientMessage::AuthenticationSaslResponse(SaslResponse {
            data: bconcat!(b"c=biws,"
                           b"r=%NR65>7bQ2S3jzl^k$G&b1^A"
                           b"YsykYKRbp/Gli53UEElsGb4I,"
                           b"p=UNQQkuQ0m5RRy24Ovzj/"
                           b"sCevUB36WTDbGXIWbCIsJmo=")
            .clone()
            .freeze(),
        }),
        bconcat!(b"r\0\0\0p"
                 b"\0\0\0hc=biws,"
                 b"r=%NR65>7bQ2S3jzl^k$G&b1^A"
                 b"YsykYKRbp/Gli53UEElsGb4I,"
                 b"p=UNQQkuQ0m5RRy24Ovzj/"
                 b"sCevUB36WTDbGXIWbCIsJmo=")
    );
    Ok(())
}

#[test]
fn restore() -> Result<(), Box<dyn Error>> {
    encoding_eq!(
        ClientMessage::Restore(Restore {
            headers: HashMap::new(),
            jobs: 1,
            data: Bytes::from_static(b"TEST"),
        }),
        b"<\x00\x00\x00\x0C\x00\x00\x00\x01TEST"
    );
    Ok(())
}

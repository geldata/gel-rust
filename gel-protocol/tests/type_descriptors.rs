use bytes::{Buf, Bytes};
use std::error::Error;

use gel_protocol::descriptors::BaseScalarTypeDescriptor;
use gel_protocol::descriptors::ObjectTypeDescriptor;
use gel_protocol::descriptors::ScalarTypeDescriptor;
use gel_protocol::descriptors::TupleTypeDescriptor;
use gel_protocol::descriptors::{Descriptor, TypePos};
use gel_protocol::descriptors::{ObjectShapeDescriptor, ShapeElement};
use gel_protocol::encoding::Input;
use gel_protocol::errors::DecodeError;
use gel_protocol::features::ProtocolVersion;
use uuid::Uuid;

mod base;

fn decode(pv: ProtocolVersion, bytes: &[u8]) -> Result<Vec<Descriptor>, DecodeError> {
    let bytes = Bytes::copy_from_slice(bytes);
    let mut input = Input::new(pv, bytes);
    let mut result = Vec::new();
    while input.remaining() > 0 {
        result.push(Descriptor::decode(&mut input)?);
    }
    assert!(input.remaining() == 0);
    Ok(result)
}

fn decode_2_0(bytes: &[u8]) -> Result<Vec<Descriptor>, DecodeError> {
    decode(ProtocolVersion::new(2, 0), bytes)
}

#[test]
fn single_int_2_0() -> Result<(), Box<dyn Error>> {
    assert_eq!(
        decode_2_0(b"\0\0\0\"\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\x05\0\0\0\nstd::int64\x01\0\0")?,
        vec![Descriptor::Scalar(ScalarTypeDescriptor {
            id: "00000000-0000-0000-0000-000000000105"
                .parse::<Uuid>()?
                .into(),
            name: Some(String::from("std::int64")),
            schema_defined: Some(true),
            ancestors: vec![],
            base_type_pos: None,
        })]
    );
    Ok(())
}

#[test]
fn single_derived_int_2_0() -> Result<(), Box<dyn Error>> {
    assert_eq!(
        decode_2_0(bconcat!(
            b"\0\0\0\"\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\x05\0\0\0\n"
            b"std::int64\x01\0\0\0\0\0)\x03\x91v\xff\x8c\x95\xb6\x11\xef\x9c"
            b" [\x0e\x8c=\xaa\xc8\0\0\0\x0fdefault::my_int\x01\0\x01\0\0\0\0\0"
            b"-\x03J\xa0\x08{\x95\xb7\x11\xef\xbd\xe2?\xfa\xe3\r\x13\xe9\0\0\0"
            b"\x11default::my_int_2\x01\0\x02\0\x01\0\0"
        ))?,
        vec![
            Descriptor::Scalar(ScalarTypeDescriptor {
                id: "00000000-0000-0000-0000-000000000105"
                    .parse::<Uuid>()?
                    .into(),
                name: Some(String::from("std::int64")),
                schema_defined: Some(true),
                ancestors: vec![],
                base_type_pos: None,
            }),
            Descriptor::Scalar(ScalarTypeDescriptor {
                id: "9176ff8c-95b6-11ef-9c20-5b0e8c3daac8"
                    .parse::<Uuid>()?
                    .into(),
                name: Some(String::from("default::my_int")),
                schema_defined: Some(true),
                ancestors: vec![TypePos(0)],
                base_type_pos: Some(TypePos(0)),
            }),
            Descriptor::Scalar(ScalarTypeDescriptor {
                id: "4aa0087b-95b7-11ef-bde2-3ffae30d13e9"
                    .parse::<Uuid>()?
                    .into(),
                name: Some(String::from("default::my_int_2")),
                schema_defined: Some(true),
                ancestors: vec![TypePos(1), TypePos(0)],
                base_type_pos: Some(TypePos(0)),
            }),
        ]
    );
    Ok(())
}

#[test]
fn object_2_0() -> Result<(), Box<dyn Error>> {
    use gel_protocol::common::Cardinality::*;
    // SELECT Foo {
    //   id,
    //   title,
    //   [IS Bar].body,
    // }
    assert_eq!(
        decode_2_0(bconcat!(
        b"\0\0\0 \x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\x01\0\0\0\x08"
        b"std::str\x01\0\0\0\0\0!\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01"
        b"\0\0\0\0\tstd::uuid\x01\0\0\0\0\0\"\n\xc3\xcc\xa7R\x95\xb7"
        b"\x11\xef\xb4\x87\x1d\x1b\x9f\xa20\x03\0\0\0\x0cdefault::Foo"
        b"\x01\0\0\0\"\n\r\xdc\xd7\x1e\x95\xb8\x11\xef\x82M!7\x80\\^4"
        b"\0\0\0\x0cdefault::Bar\x01\0\0\0^\x01\x1dMg\xe7{\xdd]9\x90\x97"
        b"O\x82\xfa\xd8\xaf7\0\0\x02\0\x04\0\0\0\x01A\0\0\0\t__tname__"
        b"\0\0\0\x02\0\0\0\0A\0\0\0\x02id\0\x01\0\x02\0\0\0\0o\0\0\0\x05"
        b"title\0\0\0\x02\0\0\0\0o\0\0\0\x04body\0\0\0\x03"
        ))?,
        vec![
            Descriptor::Scalar(ScalarTypeDescriptor {
                id: "00000000-0000-0000-0000-000000000101"
                    .parse::<Uuid>()?
                    .into(),
                name: Some(String::from("std::str")),
                schema_defined: Some(true),
                ancestors: vec![],
                base_type_pos: None,
            }),
            Descriptor::Scalar(ScalarTypeDescriptor {
                id: "00000000-0000-0000-0000-000000000100"
                    .parse::<Uuid>()?
                    .into(),
                name: Some(String::from("std::uuid")),
                schema_defined: Some(true),
                ancestors: vec![],
                base_type_pos: None,
            }),
            Descriptor::Object(ObjectTypeDescriptor {
                id: "c3cca752-95b7-11ef-b487-1d1b9fa23003"
                    .parse::<Uuid>()?
                    .into(),
                name: Some(String::from("default::Foo")),
                schema_defined: Some(true),
            }),
            Descriptor::Object(ObjectTypeDescriptor {
                id: "0ddcd71e-95b8-11ef-824d-2137805c5e34"
                    .parse::<Uuid>()?
                    .into(),
                name: Some(String::from("default::Bar")),
                schema_defined: Some(true),
            }),
            Descriptor::ObjectShape(ObjectShapeDescriptor {
                id: "1d4d67e7-7bdd-5d39-9097-4f82fad8af37"
                    .parse::<Uuid>()?
                    .into(),
                ephemeral_free_shape: false,
                type_pos: Some(TypePos(2)),
                elements: vec![
                    ShapeElement {
                        flag_implicit: true,
                        flag_link_property: false,
                        flag_link: false,
                        cardinality: Some(One),
                        name: String::from("__tname__"),
                        type_pos: TypePos(0),
                        source_type_pos: Some(TypePos(2)),
                    },
                    ShapeElement {
                        flag_implicit: false,
                        flag_link_property: false,
                        flag_link: false,
                        cardinality: Some(One),
                        name: String::from("id"),
                        type_pos: TypePos(1),
                        source_type_pos: Some(TypePos(2)),
                    },
                    ShapeElement {
                        flag_implicit: false,
                        flag_link_property: false,
                        flag_link: false,
                        cardinality: Some(AtMostOne),
                        name: String::from("title"),
                        type_pos: TypePos(0),
                        source_type_pos: Some(TypePos(2)),
                    },
                    ShapeElement {
                        flag_implicit: false,
                        flag_link_property: false,
                        flag_link: false,
                        cardinality: Some(AtMostOne),
                        name: String::from("body"),
                        type_pos: TypePos(0),
                        source_type_pos: Some(TypePos(3)),
                    },
                ]
            })
        ]
    );
    Ok(())
}

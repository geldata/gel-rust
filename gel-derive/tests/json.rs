use gel_derive::Queryable;
use gel_protocol::queryable::{Decoder, Queryable};
use serde::Deserialize;

#[derive(Debug, PartialEq, Deserialize)]
struct Data {
    field1: u32,
}

#[derive(Queryable, Debug, PartialEq)]
struct ShapeWithJson {
    name: String,
    #[gel(json)]
    data: Data,
}

#[derive(Queryable, Deserialize, Debug, PartialEq)]
#[gel(json)]
struct JsonRow {
    field2: u32,
}

#[test]
fn json_field() {
    let data = b"\0\0\0\x02\0\0\0\x19\0\0\0\x02id\0\0\x0e\xda\0\0\0\x10\x01{\"field1\": 123}";
    let order = (vec![0_usize, 1], ((), ()));
    let res = ShapeWithJson::decode(&Decoder::default(), &order, data);
    assert_eq!(
        res.unwrap(),
        ShapeWithJson {
            name: "id".into(),
            data: Data { field1: 123 },
        }
    );
}

#[test]
fn json_row() {
    let data = b"\x01{\"field2\": 234}";
    let res = JsonRow::decode(&Decoder::default(), &(), data);
    assert_eq!(res.unwrap(), JsonRow { field2: 234 });
}

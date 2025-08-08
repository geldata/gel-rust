use gel_config::schema2::{current_schema, parser::parse_toml};
use pretty_assertions::assert_eq;
use serde::Deserialize;

#[test]
fn test_complex() {
    let schema = current_schema();
    let toml = std::fs::read_to_string("tests/client/complex.toml").unwrap();
    let toml = toml::de::Deserializer::parse(&toml).unwrap();
    let toml = toml::Table::deserialize(toml).unwrap();

    let ops = parse_toml(&schema, &toml).unwrap();
    eprintln!("{}", ops.to_ddl());
    assert_eq!(
        std::fs::read_to_string("tests/client/complex.ddl").unwrap(),
        ops.to_ddl(),
    );
}

#[test]
fn test_full() {
    let schema = current_schema();
    let toml = std::fs::read_to_string("tests/client/full.toml").unwrap();
    let toml = toml::de::Deserializer::parse(&toml).unwrap();
    let toml = toml::Table::deserialize(toml).unwrap();
    let ops = parse_toml(&schema, &toml).unwrap();
    eprintln!("{}", ops.to_ddl());
    assert_eq!(
        std::fs::read_to_string("tests/client/full.ddl").unwrap(),
        ops.to_ddl(),
    );
}

#[test]
fn test_object() {
    let schema = current_schema();
    let toml = std::fs::read_to_string("tests/client/object.toml").unwrap();
    let toml = toml::de::Deserializer::parse(&toml).unwrap();
    let toml = toml::Table::deserialize(toml).unwrap();
    let ops = parse_toml(&schema, &toml).unwrap();
    eprintln!("{}", ops.to_ddl());
    assert_eq!(
        std::fs::read_to_string("tests/client/object.ddl").unwrap(),
        ops.to_ddl(),
    );
}

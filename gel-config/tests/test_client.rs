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
    if std::env::var("UPDATE_EXPECTED").is_ok() {
        std::fs::write("tests/client/complex.ddl", ops.to_ddl()).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string("tests/client/complex.ddl").unwrap(),
        ops.to_ddl(),
    );
}

#[test]
fn test_escaping() {
    let schema = current_schema();
    let toml = std::fs::read_to_string("tests/client/escaping.toml").unwrap();
    let toml = toml::de::Deserializer::parse(&toml).unwrap();
    let toml = toml::Table::deserialize(toml).unwrap();
    let ops = parse_toml(&schema, &toml).unwrap();
    eprintln!("{}", ops.to_ddl());
    if std::env::var("UPDATE_EXPECTED").is_ok() {
        std::fs::write("tests/client/escaping.ddl", ops.to_ddl()).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string("tests/client/escaping.ddl").unwrap(),
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
    if std::env::var("UPDATE_EXPECTED").is_ok() {
        std::fs::write("tests/client/full.ddl", ops.to_ddl()).unwrap();
    }
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
    if std::env::var("UPDATE_EXPECTED").is_ok() {
        std::fs::write("tests/client/object.ddl", ops.to_ddl()).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string("tests/client/object.ddl").unwrap(),
        ops.to_ddl(),
    );
}

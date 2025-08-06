use gel_config::{current, validation::validate};
use serde::Deserialize;

#[test]
fn test_complex() {
    let schema = current::default_schema();
    let toml = std::fs::read_to_string("tests/client/complex.toml").unwrap();
    let toml = toml::de::Deserializer::parse(&toml).unwrap();
    let toml = toml::Table::deserialize(toml).unwrap();
    let branch = toml.get("branch").unwrap().get("config").unwrap();
    let commands = validate(branch.clone(), &schema).unwrap();
    println!("branch:\n{:?}", commands);
    let instance = toml.get("instance").unwrap().get("config").unwrap();
    let commands = validate(instance.clone(), &schema).unwrap();
    println!("instance:\n{:?}", commands);
}

#[test]
fn test_full() {
    let schema = current::default_schema();
    let toml = std::fs::read_to_string("tests/client/full.toml").unwrap();
    let toml = toml::de::Deserializer::parse(&toml).unwrap();
    let toml = toml::Table::deserialize(toml).unwrap();
    let branch = toml.get("branch").unwrap().get("config").unwrap();
    let commands = validate(branch.clone(), &schema).unwrap();
    println!("branch:\n{:?}", commands);
    let instance = toml.get("instance").unwrap().get("config").unwrap();
    let commands = validate(instance.clone(), &schema).unwrap();
    println!("instance:\n{:?}", commands);
}

#[test]
fn test_object() {
    let schema = current::default_schema();
    let toml = std::fs::read_to_string("tests/client/object.toml").unwrap();
    let toml = toml::de::Deserializer::parse(&toml).unwrap();
    let toml = toml::Table::deserialize(toml).unwrap();
    let branch = toml.get("branch").unwrap().get("config").unwrap();
    let commands = validate(branch.clone(), &schema).unwrap();
    println!("{:?}", commands);
}

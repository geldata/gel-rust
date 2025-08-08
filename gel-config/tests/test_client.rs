use gel_config::{
    current_config,
    parser::{ParserError, parse_toml},
};
use pretty_assertions::assert_eq;
use serde::Deserialize;

fn run_test_case(test_name: &str) {
    let domains = current_config();
    let toml = std::fs::read_to_string(format!("tests/client/{test_name}.toml")).unwrap();
    let toml = toml::de::Deserializer::parse(&toml).unwrap();
    let toml = toml::Table::deserialize(toml).unwrap();

    let (ops, warnings) = parse_toml(&domains, &toml).unwrap();
    let mut ddl = String::new();
    for warning in warnings {
        ddl.push_str(&format!("# {warning}\n"));
    }
    ddl.push_str(&ops.to_ddl());
    eprintln!("{ddl}");
    if std::env::var("UPDATE_EXPECTED").is_ok() {
        std::fs::write(format!("tests/client/{test_name}.ddl"), ops.to_ddl()).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(format!("tests/client/{test_name}.ddl")).unwrap(),
        ddl,
    );
}

fn run_error_test_case(test_name: &str, expected_error: ParserError) {
    let domains = current_config();
    let toml = std::fs::read_to_string(format!("tests/client/{test_name}.toml")).unwrap();
    let toml = toml::de::Deserializer::parse(&toml).unwrap();
    let toml = toml::Table::deserialize(toml).unwrap();

    let err = parse_toml(&domains, &toml).unwrap_err();
    eprintln!("{err}");
    assert_eq!(err, expected_error);
}

#[test]
fn test_complex() {
    run_test_case("complex");
}

#[test]
fn test_escaping() {
    run_test_case("escaping");
}

#[test]
fn test_full() {
    run_test_case("full");
}

#[test]
fn test_object() {
    run_test_case("object");
}

#[test]
fn test_warning() {
    run_test_case("warning");
}

#[test]
fn test_error_missing_tname() {
    run_error_test_case(
        "error-missing-tname",
        ParserError::InvalidTname("branch.config.email_providers".to_string(), "".to_string()),
    );
}

#[test]
fn test_error_bad_type() {
    run_error_test_case(
        "error-bad-type",
        ParserError::InvalidValueType(
            "branch.config.session_idle_transaction_timeout".to_string(),
            "std::duration".to_string(),
        ),
    );
}

#[test]
fn test_error_bad_type_2() {
    run_error_test_case(
        "error-bad-type-2",
        ParserError::InvalidValueType(
            "branch.config.allow_bare_ddl".to_string(),
            "cfg::AllowBareDDL".to_string(),
        ),
    );
}

#[test]
fn test_error_bad_enum() {
    run_error_test_case(
        "error-bad-enum",
        ParserError::InvalidEnumValue(
            "branch.config.allow_bare_ddl".to_string(),
            "nope".to_string(),
            "cfg::AllowBareDDL".to_string(),
        ),
    );
}

#[test]
fn test_error_protected_property() {
    run_error_test_case(
        "error-protected-property",
        ParserError::ProtectedProperty(
            "branch.config.ext::auth::AppleOAuthProvider.name".to_string(),
        ),
    );
}

use std::{path::PathBuf, str::FromStr};

#[test]
#[cfg(not(target_family = "windows"))]
fn test_01() {
    let builder = gel_captive::ServerBuilder::new();
    let process = builder.start();
    process.apply_schema(&PathBuf::from_str("./tests/dbschema").unwrap());

    assert!(process.version_major > 0);
    assert!(process.info.port > 1000);

    process
        .cli()
        .arg("query")
        .arg("--output-format=tab-separated")
        .arg("SELECT sys::get_current_database()")
        .spawn()
        .unwrap()
        .wait_with_output()
        .unwrap();

    drop(process);
}

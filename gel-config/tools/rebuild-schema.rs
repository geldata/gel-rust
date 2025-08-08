use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

use gel_config::raw::ConfigSchema;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let schema_query_file = root.join("src").join("schema2").join("schema.edgeql");
    let schema_json_file = root.join("src").join("schema2").join("schema.json.gz");

    println!("Extending schema with extensions...");
    let process = std::process::Command::new("gel")
        .arg("query")
        .arg("-I")
        .arg("schema_extract")
        .arg(
            r#"
    create extension pgvector;
    create extension pgcrypto;
    create extension pg_trgm;
    create extension ai;
    create extension auth;
    "#,
        )
        .output()
        .unwrap();
    let stdout = String::from_utf8(process.stdout).unwrap();
    let stderr = String::from_utf8(process.stderr).unwrap();
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }
    eprintln!("{stdout}");

    println!("Extracting schema...");
    let process = std::process::Command::new("gel")
        .arg("query")
        .arg("-I")
        .arg("schema_extract")
        .arg("--file")
        .arg(schema_query_file)
        .output()
        .unwrap();
    let stdout = String::from_utf8(process.stdout).unwrap();
    let stderr = String::from_utf8(process.stderr).unwrap();
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }

    println!();
    println!("Results:");
    println!("--------");
    println!();
    println!("Input JSON size: {}", stdout.len());
    let schema: ConfigSchema = serde_json::from_str(&stdout).unwrap();
    println!("Got {} types:", schema.types.len());
    for typ in &schema.types {
        println!("  {}", typ.name);
    }

    let data = Vec::new();
    let mut encoder = BufWriter::new(flate2::write::GzEncoder::new(
        data,
        flate2::Compression::best(),
    ));
    serde_json::to_writer(&mut encoder, &schema).unwrap();
    encoder.flush().unwrap();
    let data = encoder.into_inner().unwrap().finish().unwrap();
    println!("Output JSON gzipped size: {}", data.len());
    std::fs::write(schema_json_file, data).unwrap();
}

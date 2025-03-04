use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use gel_dsn::{
    gel::{error::ParseError, ConnectionOptions},
    EnvVar, FileAccess,
};
use serde::{Deserialize, Serialize};

const JSON: &str = include_str!("shared-client-testcases/connection_testcases.json");

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionTestcase {
    name: String,
    #[serde(default)]
    opts: Option<ConnectionOptions>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    fs: Option<Fs>,
    #[serde(flatten)]
    outcome: TestOutcome,
    platform: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, derive_more::Display)]
#[serde(untagged)]
enum StringOrNumber {
    #[display("{}", _0)]
    String(String),
    #[display("{}", _0)]
    Number(f64),
}

#[derive(Debug, Serialize, Deserialize)]
struct Fs {
    files: Option<HashMap<String, serde_json::Value>>,
    homedir: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
struct TestResult {
    address: (String, usize),
    branch: String,
    database: String,
    password: Option<String>,
    #[serde(rename = "secretKey")]
    secret_key: Option<String>,
    #[serde(rename = "serverSettings")]
    server_settings: serde_json::Value,
    #[serde(rename = "tlsCAData")]
    tls_ca_data: Option<String>,
    #[serde(rename = "tlsSecurity")]
    tls_security: String,
    #[serde(rename = "tlsServerName")]
    tls_server_name: Option<String>,
    user: String,
    #[serde(
        rename = "waitUntilAvailable",
        deserialize_with = "deserialize_duration",
        serialize_with = "serialize_duration"
    )]
    wait_until_available: Duration,
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use std::str::FromStr;
    let duration_str: &str = serde::Deserialize::deserialize(deserializer)?;
    let duration =
        gel_protocol::model::Duration::from_str(duration_str).map_err(serde::de::Error::custom)?;
    Ok(std::time::Duration::from_micros(duration.to_micros() as u64))
}

fn serialize_duration<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let duration =
        gel_protocol::model::RelativeDuration::try_from_micros(duration.as_micros() as i64)
            .unwrap();
    serializer.serialize_str(&duration.to_string())
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
struct TestError {
    r#type: String,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
enum TestOutcome {
    #[serde(rename = "result")]
    Result(TestResult),
    #[serde(rename = "error")]
    Error(TestError),
}

impl FileAccess for &ConnectionTestcase {
    fn read(&self, path: &Path) -> Result<String, std::io::Error> {
        if let Some(fs) = &self.fs {
            if let Some(files) = &fs.files {
                if let Some(content) = files.get(path.to_str().unwrap()) {
                    if content.is_string() {
                        return Ok(content.as_str().unwrap().to_string());
                    }
                }
                if let Some(parent) = files.get(path.parent().unwrap().to_str().unwrap()) {
                    let parent = parent.as_object().unwrap();
                    if let Some(content) = parent.get(path.file_name().unwrap().to_str().unwrap()) {
                        if content.is_string() {
                            return Ok(content.as_str().unwrap().to_string());
                        }
                    }
                }
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ))
    }
}

impl EnvVar for &ConnectionTestcase {
    fn read(&self, name: &str) -> Result<Cow<'static, str>, std::env::VarError> {
        if let Some(env) = &self.env {
            if let Some(value) = env.get(name) {
                return Ok(Cow::Owned(value.to_string()));
            }
        }
        Err(std::env::VarError::NotPresent)
    }
}

fn main() {
    let testcases: Vec<ConnectionTestcase> = serde_json::from_str(JSON).unwrap();
    let mut failed = 0;
    let mut passed = 0;
    let mut skipped = 0;
    let filter = std::env::args().nth(1).unwrap_or_default();

    for mut testcase in testcases {
        if !testcase.name.contains(&filter) {
            skipped += 1;
            continue;
        }

        #[cfg(not(windows))]
        if testcase.platform.as_deref() == Some("windows") {
            println!("Skipping Windows-only testcase: {}", testcase.name);
            continue;
        }

        #[cfg(not(unix))]
        if testcase.platform.as_deref() == Some("macos") || testcast.platform.is_none() {
            println!("Skipping Unix-only testcase: {}", testcase.name);
            continue;
        }

        if let TestOutcome::Result(a) = &mut testcase.outcome {
            if a.address.0.contains("%") {
                if let Some(opts) = &testcase.opts {
                    if opts.dsn.as_ref().unwrap_or(&"".to_string()).contains("%") {
                        println!("Fuzzy match: {} omitting ipv6 scope", testcase.name);
                        a.address.0 = a.address.0.split_once('%').unwrap().0.to_string();
                        continue;
                    }
                }
            }
        }

        let expected = match &testcase.outcome {
            TestOutcome::Result(a) => serde_json::to_string_pretty(a).unwrap(),
            TestOutcome::Error(a) => serde_json::to_string_pretty(a).unwrap(),
        };

        let project = match testcase.platform {
            Some(ref platform) => match platform.as_str() {
                "windows" => PathBuf::from(
                    r#"C:\Users\edgedb\AppData\Local\EdgeDB\config\projects\test-${HASH}"#,
                ),
                "macos" => PathBuf::from(
                    "/Users/edgedb/Library/Application Support/edgedb/projects/test-${HASH}",
                ),
                _ => panic!("Unknown platform: {}", platform),
            },
            None => PathBuf::from("/home/edgedb/.config/edgedb/projects/test-${HASH}"),
        };

        let project = if let Some(fs) = &testcase.fs {
            if let Some(files) = &fs.files {
                if files
                    .keys()
                    .any(|k| k.ends_with("edgedb.toml") || k.ends_with("gel.toml"))
                {
                    Some(project)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let config_dir = if let Some(fs) = &testcase.fs {
            if let Some(homedir) = &fs.homedir {
                match testcase.platform {
                    Some(ref platform) => match platform.as_str() {
                        "windows" => {
                            Some(PathBuf::from(homedir).join("AppData\\Local\\EdgeDB\\config"))
                        }
                        "macos" => {
                            Some(PathBuf::from(homedir).join("Library/Application Support/edgedb"))
                        }
                        _ => panic!("Unknown platform: {}", platform),
                    },
                    None => Some(PathBuf::from(homedir).join(".config/edgedb")),
                }
            } else {
                None
            }
        } else {
            None
        };

        let (result, warnings, mut traces) = gel_dsn::gel::parse_from(
            testcase.opts.clone().unwrap_or_default(),
            project.as_deref(),
            config_dir.as_deref(),
            &testcase,
            &testcase,
        );

        let actual = match &result {
            Ok(a) => serde_json::to_string_pretty(&a.to_json()).unwrap(),
            Err(e) => {
                serde_json::to_string_pretty(&serde_json::json!({"type": e.error_type()})).unwrap()
            }
        };

        let mut fuzzy_match = false;
        if testcase.outcome
            == TestOutcome::Error(TestError {
                r#type: "invalid_dsn_or_instance_name".to_string(),
            })
            && matches!(
                result,
                Err(ParseError::InvalidDsn) | Err(ParseError::InvalidInstanceName(_))
            )
        {
            println!("Fuzzy match: {}", testcase.name);
            fuzzy_match = true;
        }

        if actual == expected || fuzzy_match {
            passed += 1;
            traces.trace(&format!("Passed: {}", testcase.name));
        } else {
            failed += 1;
            traces.trace(&format!("Failed: {}", testcase.name));

            println!("---------------------------------------------");
            for trace in traces.into_vec() {
                println!("{}", trace);
            }
            for warning in warnings.into_vec() {
                println!("{}", warning);
            }
            println!(
                "Failed: {}",
                pretty_assertions::StrComparison::new(&expected, &actual)
            );
        }
    }

    println!("Passed: {}", passed);
    println!("Failed: {}", failed);
    println!("Skipped: {}", skipped);

    if failed > 0 {
        std::process::exit(1);
    }
}

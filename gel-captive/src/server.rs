use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process;
use std::sync::Mutex;

use crate::utils::execute_and_print_errors;

pub struct ServerProcess {
    pub info: ServerInfo,
    pub version_major: u8,
    process: Mutex<Option<process::Child>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ServerInfo {
    pub port: u16,
    pub socket_dir: String,
    pub main_pid: i32,
    pub tls_cert_file: String,
    pub tls_cert_newly_generated: bool,
    pub jws_keys_newly_generated: bool,
}

impl ServerProcess {
    pub(crate) fn start() -> ServerProcess {
        let bin_name = if let Ok(ver) = env::var("GEL_MAJOR_VERSION") {
            format!("gel-server-{ver}")
        } else {
            "gel-server".to_string()
        };

        let version_major = get_server_version(&bin_name);

        let mut cmd = process::Command::new(&bin_name);
        cmd.env("GEL_SERVER_SECURITY", "insecure_dev_mode");
        cmd.arg("--temp-dir");
        cmd.arg("--testmode");
        cmd.arg("--port=auto");
        cmd.arg("--tls-cert-mode=generate_self_signed");

        // pipe server status on into a reader
        #[cfg(unix)]
        let get_status_file = {
            use command_fds::CommandFdExt;

            let (status_read, status_write) = nix::unistd::pipe().unwrap();
            cmd.arg("--emit-server-status=fd://3");
            cmd.fd_mappings(vec![command_fds::FdMapping {
                parent_fd: status_write,
                child_fd: 3,
            }])
            .unwrap();
            move || File::from(status_read)
        };
        #[cfg(not(unix))]
        let get_status_file = {
            let mut status_filepath = std::env::temp_dir();
            status_filepath.push(format!(
                "gel-server-status-{}.txt",
                unique_test_run_identifier()
            ));
            cmd.arg(format!(
                "--emit-server-status=file://{}",
                status_filepath.as_os_str().to_string_lossy()
            ));

            move || loop {
                match File::open(&status_filepath) {
                    Ok(f) => break f,
                    Err(_) => std::thread::sleep(std::time::Duration::from_secs(1)),
                }
            }
        };

        #[cfg(unix)]
        if nix::unistd::Uid::effective().as_raw() == 0 {
            use std::os::unix::process::CommandExt;
            // This is moslty true in vagga containers, so run gel/postgres
            // by any non-root user
            cmd.uid(1);
        }

        // pipe stderr into a buffer that's printed only when there is an error
        cmd.stderr(process::Stdio::piped());

        eprintln!("Starting {bin_name}...");

        let mut process = cmd
            .spawn()
            .unwrap_or_else(|_| panic!("Cannot run {bin_name}"));

        // write log file
        let stdout = process.stderr.take().unwrap();
        std::thread::spawn(move || write_log_into_file(stdout));

        // wait for server to start
        let info = wait_for_server_status(get_status_file).unwrap();

        ServerProcess {
            info,
            version_major,
            process: Mutex::new(Some(process)),
        }
    }

    /// Initializes a
    pub fn cli(&self) -> process::Command {
        let mut cmd = process::Command::new("gel");
        cmd.arg("--no-cli-update-check");
        cmd.arg("--admin");
        cmd.arg("--unix-path").arg(&self.info.socket_dir);
        cmd.arg("--port").arg(self.info.port.to_string());
        cmd.env("CLICOLOR", "0");
        cmd
    }

    pub fn apply_schema(&self, schema_dir: &Path) {
        let schema_dir = schema_dir.canonicalize().unwrap();

        eprintln!("Applying schema in {schema_dir:?}");

        // copy schema dir to tmp so we don't pollute the committed dir
        let mut tmp_schema_dir = std::env::temp_dir();
        tmp_schema_dir.push(format!("gel-dbschema-{}", unique_test_run_identifier()));
        std::fs::create_dir(&tmp_schema_dir).unwrap();
        fs_extra::dir::copy(
            schema_dir,
            &tmp_schema_dir,
            &fs_extra::dir::CopyOptions::new()
                .overwrite(true)
                .content_only(true),
        )
        .expect("cannot copy schema to a tmp dir");

        // migration create
        execute_and_print_errors(
            self.cli()
                .arg("migration")
                .arg("create")
                .arg("--schema-dir")
                .arg(&tmp_schema_dir)
                .arg("--non-interactive"),
            "gel-cli",
            "create a migration",
        );

        // migration apply
        execute_and_print_errors(
            self.cli()
                .arg("migration")
                .arg("apply")
                .arg("--schema-dir")
                .arg(&tmp_schema_dir),
            "gel-cli",
            "apply a migration",
        );
    }
}

#[cfg(unix)]
impl Drop for ServerProcess {
    fn drop(&mut self) {
        let Some(mut process) = self.process.lock().unwrap().take() else {
            return;
        };

        eprintln!("Stopping...");

        #[cfg(not(windows))]
        {
            use nix::sys::signal::{self, Signal};
            use nix::unistd::Pid;

            let pid = Pid::from_raw(process.id() as i32);
            if let Err(e) = signal::kill(pid, Signal::SIGTERM) {
                eprintln!("could not send SIGTERM to gel-server: {e:?}");
            };
        }

        #[cfg(windows)]
        {
            // This is suboptimal -- ideally we need to close the process
            // gracefully on Windows too.
            if let Err(e) = process.kill() {
                eprintln!("could not kill gel-server: {:?}", e);
            }
        }

        process.wait().ok();

        eprintln!("Stopped.");
    }
}

fn get_server_version(bin_name: &str) -> u8 {
    let mut cmd = process::Command::new(bin_name);
    cmd.arg("--version");
    cmd.stdout(process::Stdio::piped());

    let mut process = cmd.spawn().unwrap();
    let server_stdout = process.stdout.take().expect("stdout is pipe");
    let buf = BufReader::new(server_stdout);

    let mut version_str = None;
    for line in buf.lines() {
        match line {
            Ok(line) => {
                if let Some(line) = line.strip_prefix("gel-server, version ") {
                    version_str = Some(line.split('+').next().unwrap().to_string());
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error reading from server: {e}");
                break;
            }
        }
    }

    let version_str = version_str.unwrap();
    let major = version_str.split('.').next().unwrap();
    let major = major.parse::<u8>().unwrap();

    process.wait().unwrap();
    major
}

/// Reads the stream of file `status_file` until gel-server notifies that it is ready
fn wait_for_server_status(get_status_file: impl FnOnce() -> File) -> Result<ServerInfo, String> {
    eprintln!("Reading status...");

    // try reading until a success
    let pipe = BufReader::new(get_status_file());
    for line in pipe.lines() {
        match line {
            Ok(line) => {
                if let Some(data) = line.strip_prefix("READY=") {
                    eprintln!("READY={data}");
                    return Ok(serde_json::from_str(data).expect("valid server data"));
                }
            }
            Err(e) => {
                eprintln!("Error reading from server: {e}");
                return Err(e.to_string());
            }
        }
    }
    Err("no server info emitted".into())
}

/// Writes a stream to a log file in a temporary directory.
fn write_log_into_file(stream: impl std::io::Read) {
    let log_dir = env::temp_dir();

    let id = unique_test_run_identifier();

    let mut log_file = log_dir.clone();
    let file_name = format!("gel-server-{id}.log").to_string();
    log_file.push(file_name);

    eprintln!("Writing server logs into {:?}", &log_file);

    std::fs::create_dir_all(&log_dir).unwrap();
    let mut log_file = File::create(log_file).unwrap();

    let mut reader = BufReader::new(stream);
    std::io::copy(&mut reader, &mut log_file).unwrap();
}

fn unique_test_run_identifier() -> String {
    let millis_since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();
    millis_since_epoch.to_string()
}

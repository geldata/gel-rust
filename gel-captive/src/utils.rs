use std::process::Command;

pub(crate) fn execute_and_print_errors(cmd: &mut Command, program: &str, action: &str) {
    let Ok(process) = cmd.spawn() else {
        panic!("ERROR: {program} failed to execute.\n  Hint: make sure it exists in your path")
    };
    let output = process.wait_with_output().unwrap();

    if !output.status.success() {
        eprintln!("ERROR: {program} failed when trying to {action}");
        eprintln!("------ {program} (STDOUT) -----");
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("------ {program} (STDERR) -----");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        panic!("{} failed", program);
    }
}

use std::process::Command;

#[test]
fn cli_help_succeeds() {
    let output = Command::new(env!("CARGO_BIN_EXE_quicssh-rs"))
        .arg("--help")
        .output()
        .expect("failed to spawn quicssh-rs");

    assert!(
        output.status.success(),
        "cli --help exited with status {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: quicssh-rs"),
        "unexpected help output: {}",
        stdout
    );
}

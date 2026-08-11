//! Black-box tests for the public `rem` executable contract.

use std::process::Command;

#[test]
fn version_flag_prints_the_version_without_starting_the_tui() {
    // GIVEN
    let executable = env!("CARGO_BIN_EXE_rem");
    let expected = format!("rem {}\n", env!("CARGO_PKG_VERSION"));

    // WHEN
    let actual = Command::new(executable)
        .arg("--version")
        .output()
        .expect("rem --version should run");

    // THEN
    assert!(actual.status.success());
    assert_eq!(
        String::from_utf8(actual.stdout).expect("stdout should be UTF-8"),
        expected
    );
    assert!(actual.stderr.is_empty());
}

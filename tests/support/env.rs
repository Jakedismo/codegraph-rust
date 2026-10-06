// ABOUTME: Runs environment-dependent tests in isolated subprocesses.
// ABOUTME: Sets child environments without mutating the multithreaded test runner.

/// Returns true only in the isolated child; the parent runs and verifies that child.
pub fn run(test_name: &str, variables: &[(&str, Option<&str>)]) -> bool {
    const MARKER: &str = "CODEGRAPH_ENV_TEST_CHILD";
    // module_path! includes the crate name, while libtest names omit it.
    let name = test_name
        .split_once("::")
        .map_or(test_name, |(_, name)| name);
    if std::env::var(MARKER).as_deref() == Ok(name) {
        return true;
    }
    let mut child = std::process::Command::new(std::env::current_exe().expect("test executable"));
    child.args(["--exact", name, "--nocapture", "--test-threads=1"]);
    child.env(MARKER, name);
    for (key, value) in variables {
        match value {
            Some(value) => {
                child.env(key, value);
            }
            None => {
                child.env_remove(key);
            }
        }
    }
    let output = child.output().expect("run isolated environment test");
    assert!(
        output.status.success(),
        "isolated test {name} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "isolated test {name} was not executed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    false
}

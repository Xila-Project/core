#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[test]
fn information_requests_exit_successfully_without_startup() {
    for flag in ["--help", "-h", "--version", "-V"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_native_example"))
            .arg(flag)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("native_example {flag} did not exit after printing information");
            }
            thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "native_example {flag} failed");
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout).unwrap();
        if flag == "--help" || flag == "-h" {
            assert!(stdout.contains("Usage: native_example [OPTIONS]"));
        } else {
            assert_eq!(stdout.trim(), "native_example 0.1.0");
        }
    }
}

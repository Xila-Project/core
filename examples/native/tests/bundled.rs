#[path = "../src/bundled.rs"]
mod bundled;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "xila-bundle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn no_bundle_preserves_development_build_fallback() {
    let directory = Directory::new();
    assert_eq!(bundled::locate(&directory.0).unwrap(), None);
}

#[test]
fn complete_bundle_is_found_independently_of_working_directory() {
    let directory = Directory::new();
    let modules = directory.0.join("executables");
    std::fs::create_dir(&modules).unwrap();
    let calculator = modules.join("calculator.wasm");
    let weather = modules.join("weather.wasm");
    std::fs::write(&calculator, b"calculator").unwrap();
    std::fs::write(&weather, b"weather").unwrap();
    assert_eq!(
        bundled::locate(&directory.0).unwrap(),
        Some([calculator, weather])
    );
}

#[test]
fn partial_bundle_never_falls_back_to_cargo() {
    let directory = Directory::new();
    let modules = directory.0.join("executables");
    std::fs::create_dir(&modules).unwrap();
    std::fs::write(modules.join("calculator.wasm"), b"calculator").unwrap();
    let error = bundled::locate(&directory.0).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(error.to_string().contains("weather.wasm"));
}

#[test]
fn empty_bundle_is_an_error() {
    let directory = Directory::new();
    std::fs::create_dir(directory.0.join("executables")).unwrap();
    assert!(bundled::locate(&directory.0).is_err());
}

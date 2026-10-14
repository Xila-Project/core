//! Optional application loading for profiling without startup Cargo invocations.
use std::path::{Path, PathBuf};

pub fn application_path(directory: &Path, name: &str) -> Result<PathBuf, String> {
    let path = directory.join(format!("{name}.wasm"));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "Prebuilt application not found: {}. Run cargo make prepare-profiling first.",
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_prebuilt_directory_requires_application_file() {
        let root = std::env::temp_dir().join(format!("xila-prebuilt-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(application_path(&root, "calculator").is_err());
        let expected = root.join("calculator.wasm");
        std::fs::write(&expected, b"\0asm").unwrap();
        assert_eq!(application_path(&root, "calculator").unwrap(), expected);
        std::fs::remove_dir_all(root).unwrap();
    }
}

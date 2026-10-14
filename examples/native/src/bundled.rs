use std::io;
use std::path::{Path, PathBuf};

/// Released examples ship WASI applications beside the native executable.
/// A missing bundle retains the source-checkout development build path.
pub fn locate(directory: &Path) -> io::Result<Option<[PathBuf; 2]>> {
    let modules = directory.join("executables");
    if !modules.try_exists()? {
        return Ok(None);
    }
    let paths = [
        modules.join("calculator.wasm"),
        modules.join("weather.wasm"),
    ];
    for path in &paths {
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Incomplete native example bundle: {}", path.display()),
            ));
        }
    }
    Ok(Some(paths))
}

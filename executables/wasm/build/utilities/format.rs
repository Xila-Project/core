pub use graphics_bindgen::naming::snake_to_upper_camel_case;
use std::{path::Path, process::Command};

pub fn format_rust(file_path: impl AsRef<Path>) -> Result<(), String> {
    let status = Command::new("rustfmt")
        .arg(
            file_path
                .as_ref()
                .to_str()
                .ok_or("Error converting path to string")?,
        )
        .status()
        .map_err(|code| format!("Error running rustfmt : {code}"))?;

    if !status.success() {
        return Err(format!("rustfmt failed: {status}"));
    }
    Ok(())
}

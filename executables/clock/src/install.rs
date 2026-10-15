#[unsafe(no_mangle)]
pub extern "C" fn __install() {
    let result = (|| -> std::io::Result<()> {
        std::fs::create_dir_all("/configuration/shared/shortcuts")?;
        std::fs::write(
            "/configuration/shared/shortcuts/clock.json",
            r#"{
    "name": "Clock",
    "command": "/binaries/wasm",
    "arguments": ["/binaries/clock"],
    "terminal": false,
    "icon_string": "Cl",
    "icon_color": [59, 130, 246]
}"#,
        )
    })();
    if let Err(error) = result {
        eprintln!("Clock installation failed: {error}");
        std::process::exit(1);
    }
}

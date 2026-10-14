use std::fs;

pub static SHORTCUT: &str = r#"
{
    "name": "Weather",
    "command": "/binaries/wasm",
    "arguments": ["/binaries/weather"],
    "terminal": false,
    "icon_string": "We",
    "icon_color": [33, 150, 243]
}"#;

pub const SHORTCUT_PATH: &str = "/configuration/shared/shortcuts/weather.json";

#[unsafe(no_mangle)]
pub extern "C" fn __install() {
    println!("Installing Weather shortcut at {}", SHORTCUT_PATH);
    fs::write(SHORTCUT_PATH, SHORTCUT).unwrap();
    println!("Weather shortcut installed.");
}

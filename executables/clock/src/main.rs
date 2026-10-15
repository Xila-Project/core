embedded_i18n::track_locales!();

#[cfg(target_arch = "wasm32")]
mod install;
#[cfg(target_arch = "wasm32")]
mod interface;

#[cfg(target_arch = "wasm32")]
fn main() {
    if let Err(error) = interface::run() {
        eprintln!("Clock: graphics error {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("Clock is a Xila Wasm application. Build for wasm32-wasip1.");
}

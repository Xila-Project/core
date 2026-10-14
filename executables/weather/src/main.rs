#[cfg(any(target_arch = "wasm32", test))]
mod install;

#[cfg(target_arch = "wasm32")]
mod interface;

#[cfg(target_arch = "wasm32")]
fn main() {
    let mut interface = interface::Interface::new().unwrap();
    unsafe {
        interface.run();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("This executable is intended to be run in a WASM environment.");
}

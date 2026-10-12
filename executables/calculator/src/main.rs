#[cfg(any(target_arch = "wasm32", test))]
mod evaluator;
#[cfg(any(target_arch = "wasm32", test))]
mod install;
#[cfg(any(target_arch = "wasm32", test))]
mod lexer;
#[cfg(any(target_arch = "wasm32", test))]
mod parser;
#[cfg(any(target_arch = "wasm32", test))]
mod token;

#[cfg(target_arch = "wasm32")]
mod interface;

#[cfg(target_arch = "wasm32")]
fn main() {
    let mut calculator_gui = interface::Interface::new().unwrap();
    unsafe {
        calculator_gui.run().unwrap();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!(
        "{}",
        embedded_i18n::translate!("This executable is intended to be run in a WASM environment.")
    );
}

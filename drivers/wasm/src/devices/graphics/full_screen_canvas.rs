use alloc::string::String;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

use crate::devices::graphics::get_window_size;

pub fn new() -> Result<HtmlCanvasElement, String> {
    let window = web_sys::window().ok_or("Failed to get window")?;

    let document = window.document().ok_or("Failed to get document")?;

    let body = document.body().ok_or("Failed to get body")?;

    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|_| "Failed to create canvas")?
        .dyn_into()
        .map_err(|_| "Failed to cast to HtmlCanvasElement")?;

    let (width, height) = get_window_size(&window).ok_or("Failed to get window size")?;

    // Set body margin to 0 to avoid scrollbars
    body.set_attribute("style", "margin: 0")
        .map_err(|_| "Failed to set body margin")?;

    body.append_child(&canvas)
        .map_err(|_| "Failed to append canvas to body")?;

    // An inline canvas reserves a text baseline below its bitmap, making a
    // viewport-sized screen overflow and allowing the page to scroll.
    canvas
        .set_attribute("style", "display: block")
        .map_err(|_| "Failed to set canvas display style")?;
    canvas.set_id("canvas-screen");
    canvas.set_width(width);
    canvas.set_height(height);

    Ok(canvas)
}

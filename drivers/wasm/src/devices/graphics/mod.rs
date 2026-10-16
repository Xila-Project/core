mod canvas_screen;
mod full_screen_canvas;
mod keyboard;
mod mouse;

use alloc::string::String;
pub use canvas_screen::*;
use graphics::Point;
pub use keyboard::*;
pub use mouse::*;
use wasm_bindgen::JsValue;
use web_sys::{HtmlCanvasElement, Window};

pub struct GraphicsDevices {
    pub screen_device: CanvasScreenDevice,
    pub mouse_device: MouseDevice,
    pub keyboard_device: KeyboardDevice,
    pub canvas: HtmlCanvasElement,
}

pub async fn new() -> Result<GraphicsDevices, String> {
    let canvas = full_screen_canvas::new()?;

    let screen_device = CanvasScreenDevice::new(canvas.clone()).await?;

    let keyboard_device = KeyboardDevice::new(&canvas)?;

    let mouse_device = MouseDevice::new(&canvas)?;

    Ok(GraphicsDevices {
        screen_device,
        mouse_device,
        keyboard_device,
        canvas,
    })
}

pub fn has_touch_screen() -> Result<bool, &'static str> {
    let window = web_sys::window().ok_or("Failed to get window")?;

    // 1. Check if 'ontouchstart' exists in window
    let has_ontouchstart =
        js_sys::Reflect::has(&window, &JsValue::from_str("ontouchstart")).unwrap_or(false);

    // 2. Check maxTouchPoints
    let navigator = window.navigator();
    let max_touch_points = navigator.max_touch_points();

    Ok(has_ontouchstart || max_touch_points > 0)
}

fn get_window_size(window: &Window) -> Option<(u32, u32)> {
    let width = window.inner_width().ok()?.as_f64()? as u32;
    let height = window.inner_height().ok()?.as_f64()? as u32;
    Some((width, height))
}

pub fn get_resolution() -> Result<Point, &'static str> {
    let window = web_sys::window().ok_or("Failed to get window")?;

    let (width, height) = get_window_size(&window).ok_or("Failed to get window size")?;

    Ok(Point::new(width as _, height as _))
}

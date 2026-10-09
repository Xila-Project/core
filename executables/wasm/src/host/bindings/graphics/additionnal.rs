use core::{ffi::CStr, ptr::NonNull};

use wasm_abi_bindings::WasmPointer;
use xila::{
    graphics::{self, Color, Window, lvgl},
    log,
    task::{self},
};

use crate::host::virtual_machine::Translator;

pub unsafe fn object_delete(__translator: &mut Translator, object: WasmPointer) {
    let object = __translator.remove_host_translation(object).unwrap();
    unsafe {
        lvgl::lv_obj_delete(object as *mut _);
    }
}

pub unsafe fn window_create() -> *mut lvgl::lv_obj_t {
    let window = task::block_on(graphics::ffi_manager().create_window()).unwrap();

    let window: NonNull<Window> = window.into();

    window.as_ptr() as *mut lvgl::lv_obj_t
}

pub unsafe fn window_pop_event(
    __translator: &mut Translator,
    window: *mut lvgl::lv_obj_t,
    code: *mut u32,
    target: *mut WasmPointer,
) {
    let mut window = match unsafe { Window::from_raw(window) } {
        Some(window) => window,
        None => return,
    };

    if let Some(event) = unsafe { window.as_mut().pop_event() } {
        unsafe {
            *code = event.code as u32;

            *target = __translator
                .translate_to_guest(event.target, false)
                .unwrap_or(0);
        }
    }
}

pub unsafe fn window_get_event_code(window: *mut lvgl::lv_obj_t) -> u32 {
    let mut window = match unsafe { Window::from_raw(window) } {
        Some(window) => window,
        None => return graphics::EventKind::All as u32,
    };

    if let Some(event) = unsafe { window.as_mut().peek_event() } {
        event.code as u32
    } else {
        graphics::EventKind::All as u32
    }
}

pub unsafe fn window_get_event_target(
    __translator: &mut Translator,
    window: *mut lvgl::lv_obj_t,
) -> WasmPointer {
    let mut window = match unsafe { Window::from_raw(window) } {
        Some(window) => window,
        None => return 0,
    };

    let target = if let Some(event) = unsafe { window.as_mut().peek_event() } {
        event.target
    } else {
        log::warning!("No event available for the window");
        core::ptr::null_mut()
    };

    unsafe { __translator.translate_to_guest(target, false).unwrap() }
}

pub unsafe fn window_next_event(window: *mut lvgl::lv_obj_t) {
    let mut window = match unsafe { Window::from_raw(window) } {
        Some(window) => window,
        None => return,
    };

    unsafe { window.as_mut().pop_event() };
}

pub unsafe fn window_set_icon(
    window: *mut lvgl::lv_obj_t,
    icon_string: *const core::ffi::c_char,
    icon_color: lvgl::lv_color_t,
) {
    let mut window = match unsafe { Window::from_raw(window) } {
        Some(window) => window,
        None => return,
    };

    let icon_string = unsafe { CStr::from_ptr(icon_string).to_str().unwrap() };

    let icon_color = Color::from_lvgl_color(icon_color);
    unsafe { window.as_mut().set_icon(icon_string, icon_color) };
}

pub unsafe fn percentage(value: i32) -> i32 {
    unsafe { lvgl::lv_pct(value) }
}

pub unsafe fn textarea_get_text(
    textarea: *mut lvgl::lv_obj_t,
    buffer: *mut i8,
    buffer_size: usize,
) -> i32 {
    let text = unsafe {
        let text = lvgl::lv_textarea_get_text(textarea);
        if text.is_null() {
            log::warning!("lv_textarea_get_text returned null");
            return 0;
        }
        CStr::from_ptr(text).to_string_lossy()
    };

    let len = core::cmp::min(text.len(), buffer_size - 1);
    unsafe {
        core::ptr::copy_nonoverlapping(text.as_ptr(), buffer as *mut u8, len);
        *buffer.add(len) = 0; // Null-terminate
    }
    len as i32
}

pub unsafe fn textarea_get_text_length(textarea: *mut lvgl::lv_obj_t) -> i32 {
    let text = unsafe {
        let text = lvgl::lv_textarea_get_text(textarea);
        if text.is_null() {
            log::warning!("lv_textarea_get_text returned null");
            return 0;
        }
        CStr::from_ptr(text).to_string_lossy()
    };

    text.len() as i32
}

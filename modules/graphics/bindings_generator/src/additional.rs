use proc_macro2::TokenStream;
use quote::quote;

pub fn get() -> TokenStream {
    quote! {
        extern "C" {
            pub fn object_delete(__translator : &mut Translator, object: u16);

            pub fn window_create() -> *mut lv_obj_t;

            pub fn window_pop_event(
                __translator: &mut Translator,
                window: *mut lv_obj_t,
                code: *mut u32,
                target: *mut WasmPointer
            );

            pub fn window_get_event_code(window: *mut lv_obj_t) -> u32;

            pub fn window_get_event_target(__translator: &mut Translator, window: *mut lv_obj_t) -> u16;

            pub fn window_next_event(window: *mut lv_obj_t);

            pub fn window_set_icon(
                window: *mut lv_obj_t,
                icon_string: *const core::ffi::c_char,
                icon_color: lv_color_t
            );

            pub fn percentage(value: i32) -> i32;

            pub fn textarea_get_text(textarea: *mut lv_obj_t, buffer: *mut i8, buffer_size: usize) -> i32;

            pub fn textarea_get_text_length(textarea: *mut lv_obj_t) -> i32;
        }
    }
}

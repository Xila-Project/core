use core::ffi::CStr;
use core::ptr::null_mut;
use std::string::ToString;
use std::thread::sleep;

use wasm::{
    EventCode, FlexFlow, Object, button_create, label_create, label_set_text, object_create,
    object_set_flex_flow, object_set_flex_grow, object_set_height, object_set_width, percentage,
    size_content, tabview_add_tab, tabview_create, textarea_create, textarea_get_text,
    textarea_get_text_length, textarea_set_one_line, window_create, window_pop_event,
};

use weather::{
    format::{format_current_tab, format_daily_tab, format_hourly_tab, format_meta_tab},
    state::{fetch_weather, map_status_message},
    trigger::should_refresh,
};

pub struct Interface {
    window: *mut Object,
    city_input: *mut Object,
    refresh_button: *mut Object,
    status_label: *mut Object,
    current_label: *mut Object,
    hourly_label: *mut Object,
    daily_label: *mut Object,
    meta_label: *mut Object,
    in_flight: bool,
    buffer: Vec<i8>,
}

impl Interface {
    pub fn new() -> wasm::Result<Self> {
        unsafe {
            let window = window_create()?;
            object_set_flex_flow(window, FlexFlow::Column)?;

            let search_row = object_create(window)?;
            object_set_flex_flow(search_row, FlexFlow::Row)?;
            object_set_width(search_row, percentage(100)?)?;
            object_set_height(search_row, size_content())?;

            let city_input = textarea_create(search_row)?;
            textarea_set_one_line(city_input, true)?;
            object_set_flex_grow(city_input, 1)?;

            let refresh_button = button_create(search_row)?;
            let refresh_label = label_create(refresh_button)?;
            label_set_text(refresh_label, c"Refresh".as_ptr() as *mut _)?;

            let status_label = label_create(window)?;
            label_set_text(status_label, c"Ready".as_ptr() as *mut _)?;

            let tabs = tabview_create(window)?;
            object_set_width(tabs, percentage(100)?)?;
            object_set_flex_grow(tabs, 1)?;

            let current_tab = tabview_add_tab(tabs, c"Current".as_ptr())?;
            let hourly_tab = tabview_add_tab(tabs, c"Hourly".as_ptr())?;
            let daily_tab = tabview_add_tab(tabs, c"Daily".as_ptr())?;
            let meta_tab = tabview_add_tab(tabs, c"Meta".as_ptr())?;

            let current_label = label_create(current_tab)?;
            let hourly_label = label_create(hourly_tab)?;
            let daily_label = label_create(daily_tab)?;
            let meta_label = label_create(meta_tab)?;

            Ok(Self {
                window,
                city_input,
                refresh_button,
                status_label,
                current_label,
                hourly_label,
                daily_label,
                meta_label,
                in_flight: false,
                buffer: Vec::new(),
            })
        }
    }

    fn set_label_text(&self, label: *mut Object, text: &str) {
        let mut owned = text.to_string();
        owned.push('\0');

        unsafe {
            let _ = label_set_text(label, owned.as_ptr() as *mut _);
        }
    }

    fn set_status(&self, text: &str) {
        self.set_label_text(self.status_label, text);
    }

    unsafe fn on_refresh(&mut self) {
        if self.in_flight {
            return;
        }
        self.in_flight = true;

        let text_length = unsafe {
            textarea_get_text_length(self.city_input).expect("Failed to get textarea text length")
        };

        self.buffer.clear();
        self.buffer.reserve(text_length as usize + 1); // +1 for null terminator

        unsafe {
            textarea_get_text(
                self.city_input,
                self.buffer.as_mut_ptr(),
                self.buffer.capacity(),
            )
            .expect("Failed to get textarea text");
        }

        let city = unsafe { CStr::from_ptr(self.buffer.as_ptr()).to_string_lossy() };

        let city = city.trim();
        if city.is_empty() {
            self.set_status("Enter a city name");
            self.in_flight = false;
            return;
        }

        self.set_status("Loading...");

        match fetch_weather(city) {
            Ok(data) => {
                let current = format_current_tab(data.forecast.current.as_ref());
                let hourly = format_hourly_tab(data.forecast.hourly.as_ref());
                let daily = format_daily_tab(data.forecast.daily.as_ref());
                let meta = format_meta_tab(
                    &data.city_label,
                    data.latitude,
                    data.longitude,
                    &data.forecast,
                );

                self.set_label_text(self.current_label, &current);
                self.set_label_text(self.hourly_label, &hourly);
                self.set_label_text(self.daily_label, &daily);
                self.set_label_text(self.meta_label, &meta);
                self.set_status("Updated");
            }
            Err(error) => {
                let message = map_status_message(&error);
                self.set_status(&message);
            }
        }

        self.in_flight = false;
    }

    pub unsafe fn run(&mut self) {
        loop {
            let mut code = EventCode::All;
            let mut target: *mut Object = null_mut();
            let _ = unsafe {
                window_pop_event(
                    self.window,
                    &mut code as *mut _ as *mut _,
                    &mut target as *mut _ as *mut _,
                )
            };

            if should_refresh(
                code as u32,
                target == self.refresh_button,
                target == self.city_input,
            ) {
                unsafe {
                    self.on_refresh();
                }
            } else if code == EventCode::All {
                sleep(core::time::Duration::from_millis(10));
            }
        }
    }
}

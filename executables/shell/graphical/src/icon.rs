use alloc::ffi::CString;
use xila::graphics::{
    Color, Point,
    fonts::{get_font_extra_large, get_font_large},
    lvgl,
};

use crate::error::{Error, Result};

pub unsafe fn create_icon(
    parent: *mut lvgl::lv_obj_t,
    icon_color: Color,
    icon_string: &str,
    size: Point,
) -> Result<*mut lvgl::lv_obj_t> {
    unsafe {
        let icon = lvgl::lv_button_create(parent);

        if icon.is_null() {
            return Err(Error::FailedToCreateObject);
        }

        lvgl::lv_obj_set_size(icon, size.get_x().into(), size.get_y().into());
        lvgl::lv_obj_set_style_pad_all(icon, 0, lvgl::LV_STATE_DEFAULT);

        let radius: i32 = size.get_x() as i32 / 3;

        lvgl::lv_obj_set_style_radius(icon, radius, lvgl::LV_STATE_DEFAULT);

        lvgl::lv_obj_set_style_bg_color(icon, icon_color.into_lvgl_color(), lvgl::LV_STATE_DEFAULT);
        lvgl::lv_obj_set_style_text_color(
            icon,
            Color::WHITE.into_lvgl_color(),
            lvgl::LV_STATE_DEFAULT,
        );

        let label = lvgl::lv_label_create(icon);

        if label.is_null() {
            return Err(Error::FailedToCreateObject);
        }

        if size.get_x() >= 48 {
            lvgl::lv_obj_set_style_text_font(label, get_font_extra_large(), lvgl::LV_STATE_DEFAULT);
        } else {
            lvgl::lv_obj_set_style_text_font(label, get_font_large(), lvgl::LV_STATE_DEFAULT);
        }

        let icon_string = CString::new(icon_string).map_err(Error::NullCharacterInString)?;

        lvgl::lv_label_set_text(label, icon_string.as_ptr());
        lvgl::lv_obj_set_align(label, lvgl::lv_align_t_LV_ALIGN_CENTER);

        Ok(icon)
    }
}

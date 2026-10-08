use core::ptr::null_mut;

use xila::graphics::{self, lvgl};

use crate::{desk::Desk, error::Result};

pub struct Home {
    button: *mut lvgl::lv_obj_t,
}

impl Drop for Home {
    fn drop(&mut self) {
        unsafe {
            lvgl::lv_obj_delete_async(self.button);
        }
    }
}

impl Home {
    pub async fn new(
        graphics_manager: &'static graphics::Manager,
        desk: *mut lvgl::lv_obj_t,
    ) -> Result<Self> {
        let button = graphics::lock!(graphics_manager, {
            unsafe {
                let button = lvgl::lv_obj_create(lvgl::lv_layer_top());

                if button.is_null() {
                    return Err(crate::error::Error::FailedToCreateObject);
                }

                lvgl::lv_obj_set_size(button, lvgl::lv_pct(40), 8);
                lvgl::lv_obj_set_style_bg_color(
                    button,
                    lvgl::lv_color_white(),
                    lvgl::LV_STATE_DEFAULT,
                );
                lvgl::lv_obj_set_style_bg_opa(button, lvgl::LV_OPA_50, lvgl::LV_STATE_DEFAULT);
                lvgl::lv_obj_set_align(button, lvgl::lv_align_t_LV_ALIGN_BOTTOM_MID);
                lvgl::lv_obj_set_y(button, -5);

                lvgl::lv_obj_set_style_pad_all(button, 0, lvgl::LV_STATE_DEFAULT);
                lvgl::lv_obj_set_style_border_width(button, 0, lvgl::LV_STATE_DEFAULT);

                lvgl::lv_obj_remove_flag(button, lvgl::lv_obj_flag_t_LV_OBJ_FLAG_GESTURE_BUBBLE);

                lvgl::lv_obj_add_event_cb(
                    button,
                    Some(handle_pressing),
                    lvgl::lv_event_code_t_LV_EVENT_PRESSING,
                    null_mut(),
                );

                lvgl::lv_obj_add_event_cb(
                    button,
                    Some(handle_released),
                    lvgl::lv_event_code_t_LV_EVENT_RELEASED,
                    desk as *mut core::ffi::c_void,
                );

                button
            }
        });

        Ok(Self { button })
    }
}

unsafe extern "C" fn handle_pressing(event: *mut lvgl::lv_event_t) {
    unsafe {
        let object = lvgl::lv_event_get_target(event) as *mut lvgl::lv_obj_t;

        let input_device = lvgl::lv_indev_active();

        let mut vector = lvgl::lv_point_t::default();

        lvgl::lv_indev_get_vect(input_device, &mut vector as *mut lvgl::lv_point_t);

        let y = lvgl::lv_obj_get_y_aligned(object) + vector.y;

        let y = y.max(-40);

        lvgl::lv_obj_set_y(object, y);
    }
}

unsafe extern "C" fn handle_released(event: *mut lvgl::lv_event_t) {
    unsafe {
        let object = lvgl::lv_event_get_target(event) as *mut lvgl::lv_obj_t;

        let y = lvgl::lv_obj_get_y_aligned(object);

        lvgl::lv_obj_set_y(object, -5);

        if y < -20 {
            let desk = lvgl::lv_event_get_user_data(event) as *mut lvgl::lv_obj_t;

            lvgl::lv_obj_remove_flag(desk, lvgl::lv_obj_flag_t_LV_OBJ_FLAG_HIDDEN);
            lvgl::lv_obj_move_foreground(desk);

            lvgl::lv_obj_send_event(desk, Desk::HOME_EVENT.into_lvgl_code(), null_mut());
        }
    }
}

//! Xila's integration of the shared `lvgl_theme_shadcn` theme.
//!
//! The C theme owns shared styles and follows the display's lifetime. Call theme
//! helpers while holding the graphics manager's LVGL lock.

use crate::{Color, Display, Error, Result, fonts, lvgl};

pub(crate) fn initialize(display: &Display) -> Result<()> {
    unsafe { initialize_display(display.get_lvgl_display()) }
}

unsafe fn initialize_display(display: *mut lvgl::lv_display_t) -> Result<()> {
    let fonts = lvgl::lv_theme_shadcn_fonts_t {
        small: fonts::get_font_small(),
        normal: fonts::get_font_medium(),
        large: fonts::get_font_large(),
        mono: fonts::get_font_monospace_medium(),
    };
    // The theme copies the palette/fonts and releases its state when the display
    // is deleted. No Rust-owned callback, parent theme or leaked Box is needed.
    let theme = unsafe {
        lvgl::lv_theme_shadcn_init(
            display,
            lvgl::lv_theme_shadcn_palette_zinc_dark(),
            &fonts,
            lvgl::lv_theme_shadcn_flags_t_LV_THEME_SHADCN_FLAG_NONE,
        )
    };
    if theme.is_null() {
        return Err(Error::OutOfMemory);
    }
    Ok(())
}

pub(crate) fn update(
    display: &Display,
    primary_color: Color,
    secondary_color: Color,
    is_dark: bool,
) {
    unsafe {
        update_display(
            display.get_lvgl_display(),
            primary_color,
            secondary_color,
            is_dark,
        )
    }
}

unsafe fn update_display(
    display: *mut lvgl::lv_display_t,
    primary_color: Color,
    secondary_color: Color,
    is_dark: bool,
) {
    unsafe {
        let mut palette = if is_dark {
            *lvgl::lv_theme_shadcn_palette_zinc_dark()
        } else {
            *lvgl::lv_theme_shadcn_palette_zinc_light()
        };
        palette.primary = primary_color.into_lvgl_color();
        palette.primary_fg = contrasting_foreground(primary_color).into_lvgl_color();
        palette.ring = palette.primary;
        palette.secondary = secondary_color.into_lvgl_color();
        palette.secondary_fg = contrasting_foreground(secondary_color).into_lvgl_color();
        lvgl::lv_theme_shadcn_set_palette(display, &palette);
    }
}

fn contrasting_foreground(color: Color) -> Color {
    let brightness = 299 * color.get_red() as u32
        + 587 * color.get_green() as u32
        + 114 * color.get_blue() as u32;
    if brightness >= 128_000 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

/// A semantic container surface that follows palette changes.
#[derive(Clone, Copy)]
pub enum Surface {
    Background,
    Card,
    Popover,
    Muted,
}

/// Apply a shared container surface instead of a local background color.
///
/// # Safety
/// `object` must be live and the caller must hold the LVGL lock.
pub unsafe fn set_surface(object: *mut lvgl::lv_obj_t, surface: Surface) {
    let surface = match surface {
        Surface::Background => lvgl::lv_theme_shadcn_surface_t_LV_THEME_SHADCN_SURFACE_BACKGROUND,
        Surface::Card => lvgl::lv_theme_shadcn_surface_t_LV_THEME_SHADCN_SURFACE_CARD,
        Surface::Popover => lvgl::lv_theme_shadcn_surface_t_LV_THEME_SHADCN_SURFACE_POPOVER,
        Surface::Muted => lvgl::lv_theme_shadcn_surface_t_LV_THEME_SHADCN_SURFACE_MUTED,
    };
    unsafe { lvgl::lv_theme_shadcn_set_surface(object, surface) };
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    extern crate abi_definitions;
    use super::*;

    drivers_std::memory::instantiate_global_allocator!();

    #[test]
    fn palette_updates_restyle_existing_widgets_and_surfaces() {
        unsafe {
            lvgl::lv_init();
            let display = lvgl::lv_display_create(320, 240);
            initialize_display(display).unwrap();
            let screen = lvgl::lv_display_get_screen_active(display);
            let button = lvgl::lv_button_create(screen);
            let surface = lvgl::lv_obj_create(screen);
            set_surface(surface, Surface::Muted);
            let dark = *lvgl::lv_theme_shadcn_get_palette(display);
            assert!(dark.dark);
            assert!(lvgl::lv_color_eq(
                lvgl::lv_obj_get_style_bg_color(button, 0),
                dark.primary
            ));
            assert!(lvgl::lv_color_eq(
                lvgl::lv_obj_get_style_bg_color(surface, 0),
                dark.muted
            ));

            let primary = Color::new(0x25, 0x63, 0xeb);
            let secondary = Color::new(0xf4, 0xf4, 0xf5);
            update_display(display, primary, secondary, false);
            let light = *lvgl::lv_theme_shadcn_get_palette(display);
            assert!(!light.dark);
            assert!(lvgl::lv_color_eq(light.primary, primary.into_lvgl_color()));
            assert!(lvgl::lv_color_eq(
                light.secondary,
                secondary.into_lvgl_color()
            ));
            assert!(lvgl::lv_color_eq(
                light.primary_fg,
                Color::WHITE.into_lvgl_color()
            ));
            assert!(lvgl::lv_color_eq(
                light.secondary_fg,
                Color::BLACK.into_lvgl_color()
            ));
            assert!(lvgl::lv_color_eq(
                lvgl::lv_obj_get_style_bg_color(button, 0),
                light.primary
            ));
            assert!(lvgl::lv_color_eq(
                lvgl::lv_obj_get_style_bg_color(surface, 0),
                light.muted
            ));

            lvgl::lv_display_delete(display);
            // Theme state must be gone before display storage can be reused.
            assert!(!lvgl::lv_theme_shadcn_is_active(display));
            lvgl::lv_deinit();
        }
    }
}

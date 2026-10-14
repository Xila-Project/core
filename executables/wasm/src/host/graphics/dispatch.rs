use super::{
    Function, GraphicsState, ResultKind,
    error::{GraphicsError, Result},
    marshal::{ButtonMap, read_c_string},
    objects::Kind,
};
use crate::host::translation::{GuestMemory, GuestPointer, WasmUsize};
use core::ptr::NonNull;
use xila::graphics::{Color, Window, lvgl};

pub struct GraphicsCall {
    pub function: i32,
    pub arguments: [WasmUsize; 7],
    pub count: i32,
    pub result: WasmUsize,
}

// Checked narrowing is required for memory_64 and intentionally a no-op for memory_32.
#[allow(clippy::useless_conversion)]
pub fn dispatch(
    state: &mut GraphicsState,
    memory: &mut GuestMemory<'_>,
    call: GraphicsCall,
) -> Result<()> {
    let function = Function::from_id(call.function).ok_or(GraphicsError::Unsupported)?;
    if function.argument_count() != call.count {
        return Err(GraphicsError::InvalidArgumentsCount);
    }
    let manager = state.manager.ok_or(GraphicsError::Unavailable)?;
    match function.result_kind() {
        ResultKind::None => (),
        ResultKind::Word => {
            memory
                .validate_pointer(GuestPointer::<WasmUsize>::new(call.result))
                .ok_or(GraphicsError::InvalidMemory)?;
        }
        ResultKind::U32 | ResultKind::I32 => {
            memory
                .validate_pointer(GuestPointer::<u32>::new(call.result))
                .ok_or(GraphicsError::InvalidMemory)?;
        }
    }
    let _lock = manager.synchronous_lock();
    state.reap_deleted_maps();
    let [a, b, c, ..] = call.arguments;
    match function {
        Function::WindowCreate
        | Function::ObjectCreate
        | Function::LabelCreate
        | Function::ButtonmatrixCreate => {
            state.ensure_capacity()?;
            let (pointer, kind, parent) = if function == Function::WindowCreate {
                let window = xila::task::block_on(manager.create_window())
                    .map_err(|_| GraphicsError::NativeFailure)?;
                let window: NonNull<Window> = window.into();
                (window.as_ptr().cast(), Kind::Window, None)
            } else {
                let parent = state.object(a, None)?;
                let (pointer, kind) = unsafe {
                    match function {
                        Function::ObjectCreate => (lvgl::lv_obj_create(parent), Kind::Object),
                        Function::LabelCreate => (lvgl::lv_label_create(parent), Kind::Label),
                        Function::ButtonmatrixCreate => {
                            (lvgl::lv_buttonmatrix_create(parent), Kind::ButtonMatrix)
                        }
                        _ => unreachable!(),
                    }
                };
                (pointer, kind, Some(a))
            };
            let handle = state.insert(pointer, kind, parent)?;
            memory
                .write(GuestPointer::new(call.result), handle)
                .expect("validated result");
        }
        Function::ObjectDelete => {
            unsafe { lvgl::lv_obj_delete(state.object(a, None)?) };
            state.reap_deleted_maps();
        }
        Function::WindowSetIcon => {
            let pointer = state.object(a, Some(Kind::Window))?;
            let text = read_c_string(memory, b)?
                .to_str()
                .map_err(|_| GraphicsError::InvalidValue)?;
            let color = u32::try_from(c).map_err(|_| GraphicsError::InvalidValue)?;
            let color = lvgl::lv_color_t {
                red: (color >> 16) as u8,
                green: (color >> 8) as u8,
                blue: color as u8,
            };
            let mut window =
                unsafe { Window::from_raw(pointer) }.ok_or(GraphicsError::InvalidHandle)?;
            unsafe {
                window
                    .as_mut()
                    .set_icon(text, Color::from_lvgl_color(color))
            };
        }
        Function::WindowPopEvent => {
            let pointer = state.object(a, Some(Kind::Window))?;
            memory
                .validate_pointer(GuestPointer::<u32>::new(b))
                .ok_or(GraphicsError::InvalidMemory)?;
            memory
                .validate_pointer(GuestPointer::<WasmUsize>::new(c))
                .ok_or(GraphicsError::InvalidMemory)?;
            let mut window =
                unsafe { Window::from_raw(pointer) }.ok_or(GraphicsError::InvalidHandle)?;
            let (code, target) = unsafe { window.as_mut().pop_event() }.map_or((0, 0), |event| {
                (event.code as u32, state.handle_for(event.target))
            });
            memory
                .write(GuestPointer::new(b), code)
                .expect("validated event code");
            memory
                .write(GuestPointer::new(c), target)
                .expect("validated event target");
        }
        Function::Percentage => {
            memory
                .write(GuestPointer::new(call.result), unsafe {
                    lvgl::lv_pct(a as i32)
                })
                .expect("validated result");
        }
        Function::LabelSetText => {
            let pointer = state.object(a, Some(Kind::Label))?;
            let text = read_c_string(memory, b)?;
            // lv_label_set_text copies synchronously; it does not retain guest bytes.
            unsafe { lvgl::lv_label_set_text(pointer, text.as_ptr()) };
        }
        Function::ButtonmatrixSetMap => {
            state.object(a, Some(Kind::ButtonMatrix))?;
            let map = ButtonMap::decode(memory, b)?;
            state.set_map(a, map);
        }
        Function::ButtonmatrixGetSelectedButton => {
            let pointer = state.object(a, Some(Kind::ButtonMatrix))?;
            let selected = unsafe { lvgl::lv_buttonmatrix_get_selected_button(pointer) };
            memory
                .write(GuestPointer::new(call.result), selected)
                .expect("validated result");
        }
        Function::ObjectSetFlexFlow
        | Function::ObjectSetFlexGrow
        | Function::ObjectSetHeight
        | Function::ObjectSetWidth
        | Function::ObjectAddFlag => {
            let pointer = state.object(a, None)?;
            unsafe {
                match function {
                    Function::ObjectSetFlexFlow => {
                        if !matches!(b, 0 | 1 | 4 | 5 | 8 | 9 | 12 | 13) {
                            return Err(GraphicsError::InvalidValue);
                        }
                        lvgl::lv_obj_set_flex_flow(pointer, b as _);
                    }
                    Function::ObjectSetFlexGrow => lvgl::lv_obj_set_flex_grow(
                        pointer,
                        u8::try_from(b).map_err(|_| GraphicsError::InvalidValue)?,
                    ),
                    Function::ObjectSetHeight => lvgl::lv_obj_set_height(pointer, b as i32),
                    Function::ObjectSetWidth => lvgl::lv_obj_set_width(pointer, b as i32),
                    Function::ObjectAddFlag => lvgl::lv_obj_add_flag(
                        pointer,
                        u32::try_from(b).map_err(|_| GraphicsError::InvalidValue)?,
                    ),
                    _ => unreachable!(),
                }
            }
        }
    }
    Ok(())
}

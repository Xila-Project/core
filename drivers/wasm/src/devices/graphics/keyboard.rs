use alloc::{boxed::Box, string::String};
use file_system::{DirectBaseOperations, DirectCharacterDevice, MountOperations, Size};
use graphics::{InputData, Key, State};
use synchronization::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, channel::Sender,
};
use wasm_bindgen::{JsCast, prelude::Closure};
use web_sys::{ClipboardEvent, KeyboardEvent};

#[derive(Debug)]
pub struct KeyboardDevice(Box<Channel<CriticalSectionRawMutex, (Key, State), 512>>);

impl KeyboardDevice {
    fn handle_key_press_char<const N: usize>(
        sender: Sender<'_, CriticalSectionRawMutex, (Key, State), N>,
        character: char,
        pressed: bool,
    ) {
        let key = Key::Character(character as u8);

        let state = if pressed {
            State::Pressed
        } else {
            State::Released
        };

        if let Err(e) = sender.try_send((key, state)) {
            log::error!("Failed to send key event: {:?}", e);
        }
    }

    fn handle_key_press<const N: usize>(
        sender: Sender<'_, CriticalSectionRawMutex, (Key, State), N>,
        key: &str,
        pressed: bool,
    ) {
        let key = match map_key(key) {
            Some(k) => k,
            None => {
                log::warning!("Unsupported key: {}", key);
                return;
            }
        };

        let state = if pressed {
            State::Pressed
        } else {
            State::Released
        };

        if let Err(e) = sender.try_send((key, state)) {
            log::error!("Failed to send key event: {:?}", e);
        }
    }

    pub fn new() -> Result<Self, String> {
        let window = web_sys::window().ok_or("Failed to get window")?;

        let document = window.document().ok_or("Failed to get document")?;

        let inner = Self(Box::new(Channel::new()));

        let sender = inner.0.sender();

        let key_down_closure = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            // Skip handling Ctrl+V to avoid registering 'V' (paste event will handle it)
            if event.ctrl_key() {
                return;
            }

            Self::handle_key_press(sender, &event.key(), true);
            Self::handle_key_press(sender, &event.key(), false);
        }) as Box<dyn FnMut(KeyboardEvent)>);

        document
            .add_event_listener_with_callback("keydown", key_down_closure.as_ref().unchecked_ref())
            .map_err(|_| "Failed to add keydown event listener")?;

        let sender = inner.0.sender();

        let paste_closure = Closure::wrap(Box::new(move |event: ClipboardEvent| {
            if let Some(clipboard_data) = event.clipboard_data()
                && let Ok(text) = clipboard_data.get_data("text")
            {
                for char in text.chars() {
                    Self::handle_key_press_char(sender, char, true);
                    Self::handle_key_press_char(sender, char, false);
                }
            }
        }) as Box<dyn FnMut(ClipboardEvent)>);

        window
            .add_event_listener_with_callback("paste", paste_closure.as_ref().unchecked_ref())
            .map_err(|_| "Failed to add paste event listener")?;

        key_down_closure.forget(); // Prevent memory leak by keeping the closure alive
        paste_closure.forget(); // Prevent memory leak by keeping the closure alive

        Ok(inner)
    }
}

impl DirectBaseOperations for KeyboardDevice {
    fn read(&self, buffer: &mut [u8], _: Size) -> file_system::Result<usize> {
        let data: &mut InputData = buffer
            .try_into()
            .map_err(|_| file_system::Error::InvalidParameter)?;

        if let Ok((key, state)) = self.0.receiver().try_receive() {
            data.set_key(key);
            data.set_state(state);
        }

        data.set_continue(!self.0.is_empty());

        Ok(buffer.len())
    }

    fn write(&self, _: &[u8], _: Size) -> file_system::Result<usize> {
        Err(file_system::Error::UnsupportedOperation)
    }
}

impl MountOperations for KeyboardDevice {}

impl DirectCharacterDevice for KeyboardDevice {}

fn map_key(key: &str) -> Option<Key> {
    if key.len() == 1 {
        let character = key.chars().next()?;

        if character.is_ascii() {
            return Some(Key::Character(character as u8));
        }
    }

    let key = match key {
        "ArrowUp" => Key::Up,
        "ArrowDown" => Key::Down,
        "ArrowRight" => Key::Right,
        "ArrowLeft" => Key::Left,
        "Escape" => Key::Escape,
        "Delete" => Key::Delete,
        "Backspace" => Key::Backspace,
        "Enter" => Key::Enter,
        "PageDown" => Key::Next,
        "PageUp" => Key::Previous,
        "Home" => Key::Home,
        "End" => Key::End,
        _ => return None,
    };

    Some(key)
}

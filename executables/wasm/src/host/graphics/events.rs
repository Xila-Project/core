//! Translate native LVGL events to Xila's existing guest ABI.
use xila::graphics::EventKind;

pub(super) fn guest_code(event: EventKind) -> u32 {
    match event {
        EventKind::All => 0,
        EventKind::Pressed => 1,
        EventKind::Pressing => 2,
        EventKind::PressLost => 3,
        EventKind::ShortClicked => 4,
        EventKind::SingleClicked => 5,
        EventKind::DoubleClicked => 6,
        EventKind::TripleClicked => 7,
        EventKind::LongPressed => 8,
        EventKind::LongPressedRepeat => 9,
        EventKind::Clicked => 10,
        EventKind::Released => 11,
        EventKind::ScrollBegin => 12,
        EventKind::ScrollThrowBegin => 13,
        EventKind::ScrollEnd => 14,
        EventKind::Scroll => 15,
        EventKind::Gesture => 16,
        EventKind::Key => 17,
        EventKind::Rotary => 18,
        EventKind::Focused => 19,
        EventKind::Defocused => 20,
        EventKind::Leave => 21,
        EventKind::HitTest => 22,
        EventKind::InputDeviceReset => 23,
        EventKind::HoverOver => 24,
        EventKind::HoverLeave => 25,
        EventKind::CoverCheck => 26,
        EventKind::RefreshExtDrawSize => 27,
        EventKind::DrawMainBegin => 28,
        EventKind::DrawMain => 29,
        EventKind::DrawMainEnd => 30,
        EventKind::DrawPostBegin => 31,
        EventKind::DrawPost => 32,
        EventKind::DrawPostEnd => 33,
        EventKind::DrawTaskAdded => 34,
        EventKind::ValueChanged => 35,
        EventKind::Insert => 36,
        EventKind::Refresh => 37,
        EventKind::Ready => 38,
        EventKind::Cancel => 39,
        EventKind::Create => 40,
        EventKind::Delete => 41,
        EventKind::ChildChanged => 42,
        EventKind::ChildCreated => 43,
        EventKind::ChildDeleted => 44,
        EventKind::ScreenUnloadStart => 45,
        EventKind::ScreenLoadStart => 46,
        EventKind::ScreenLoaded => 47,
        EventKind::ScreenUnloaded => 48,
        EventKind::SizeChanged => 49,
        EventKind::StyleChanged => 50,
        EventKind::LayoutChanged => 51,
        EventKind::GetSelfSize => 52,
        EventKind::InvalidateArea => 53,
        EventKind::ResolutionChanged => 54,
        EventKind::ColorFormatChanged => 55,
        EventKind::RefreshRequest => 56,
        EventKind::RefreshStart => 57,
        EventKind::RefreshReady => 58,
        EventKind::RenderStart => 59,
        EventKind::RenderReady => 60,
        EventKind::FlushStart => 61,
        EventKind::FlushFinish => 62,
        EventKind::FlushWaitStart => 63,
        EventKind::FlushWaitFinish => 64,
        EventKind::VerticalSynchronization => 65,
        EventKind::Preprocess => 0x8000,
        _ => 67 + (event as u32 - xila::graphics::lvgl::lv_event_code_t_LV_EVENT_LAST),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_events_keep_the_guest_numbering() {
        assert_eq!(guest_code(EventKind::Clicked), 10);
        assert_eq!(guest_code(EventKind::ValueChanged), 35);
        assert_eq!(guest_code(EventKind::Delete), 41);
        assert_eq!(guest_code(EventKind::Last), 67);
        assert_eq!(guest_code(EventKind::CloseRequested), 70);
        assert_eq!(guest_code(EventKind::Custom32), 99);
        assert_eq!(guest_code(EventKind::Preprocess), 0x8000);
    }
}

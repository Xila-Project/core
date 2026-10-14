pub const EVENT_CLICKED: u32 = 10;
pub const EVENT_READY: u32 = 38;

pub fn should_refresh(
    event_code: u32,
    target_is_refresh_button: bool,
    target_is_city_input: bool,
) -> bool {
    (event_code == EVENT_CLICKED && target_is_refresh_button)
        || (event_code == EVENT_READY && (target_is_refresh_button || target_is_city_input))
}

#[cfg(test)]
mod tests {
    use super::{EVENT_CLICKED, EVENT_READY, should_refresh};

    #[test]
    fn refresh_click_triggers_update() {
        assert!(should_refresh(EVENT_CLICKED, true, false));
    }

    #[test]
    fn city_input_ready_triggers_update() {
        assert!(should_refresh(EVENT_READY, false, true));
    }

    #[test]
    fn unrelated_event_does_not_trigger_update() {
        assert!(!should_refresh(0, false, false));
    }
}

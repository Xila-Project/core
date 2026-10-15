use super::*;

#[test]
fn stopwatch_uses_elapsed_time_and_excludes_pauses() {
    let mut watch = Stopwatch::default();
    watch.start(100);
    watch.start(200); // Starting twice must not lose elapsed time.
    assert_eq!(watch.elapsed(350), 250);
    watch.pause(350);
    assert_eq!(watch.elapsed(1000), 250);
    watch.start(1000);
    assert_eq!(watch.elapsed(1125), 375);
    watch.reset();
    assert_eq!(watch.elapsed(2000), 0);
    assert!(!watch.is_running());
}

#[test]
fn laps_are_bounded_and_capture_total_elapsed_time() {
    let mut watch = Stopwatch::default();
    assert!(!watch.lap(100));
    watch.start(100);
    for index in 0..MAX_LAPS {
        assert!(watch.lap(100 + index as u64 * 20));
    }
    assert!(!watch.lap(9999));
    assert_eq!(watch.laps()[MAX_LAPS - 1], (MAX_LAPS as u64 - 1) * 20);
    watch.reset();
    assert!(watch.laps().is_empty());
}

#[test]
fn timer_pauses_and_fires_once_even_after_a_delayed_poll() {
    let mut timer = Timer::default();
    assert!(!timer.start(0, 0));
    assert!(timer.start(5000, 100));
    timer.pause(1100);
    assert_eq!(timer.remaining(9000), 4000);
    timer.resume(9000);
    assert!(!timer.tick(12999));
    assert!(timer.tick(15000));
    assert!(!timer.tick(16000));
    assert_eq!(timer.remaining(16000), 0);
    assert!(timer.finished());
    timer.restart(20000);
    assert_eq!(timer.remaining(20000), 5000);
    timer.cancel();
    assert!(!timer.tick(30000));
    assert!(!timer.finished());
}

#[test]
fn one_time_alarm_rolls_to_tomorrow_and_survives_delayed_polling() {
    let mut alarm = Alarm::new(7, 30, 0).unwrap();
    alarm.arm(8 * 3600);
    assert_eq!(alarm.next(), Some(86400 + 7 * 3600 + 30 * 60));
    assert!(!alarm.tick(86400 + 7 * 3600));
    assert!(alarm.tick(86400 + 8 * 3600));
    assert!(!alarm.tick(86400 + 8 * 3600 + 1));
    assert!(!alarm.enabled());
}

#[test]
fn recurring_alarm_does_not_ring_twice_after_a_backward_clock_change() {
    let mut alarm = Alarm::new(0, 1, 0x7f).unwrap();
    alarm.arm(0);
    assert!(alarm.tick(61));
    assert!(!alarm.tick(50));
    assert!(!alarm.tick(61));
    assert!(alarm.tick(86461));
}

#[test]
fn weekdays_use_monday_as_bit_zero_and_skip_the_weekend() {
    // 1970-01-01 is Thursday; day 1 is Friday, day 4 is Monday.
    let mut alarm = Alarm::new(8, 0, 0b000_0001).unwrap();
    alarm.arm(86400);
    assert_eq!(alarm.next(), Some(4 * 86400 + 8 * 3600));
    assert!(Alarm::new(24, 0, 0).is_none());
    assert!(Alarm::new(0, 60, 0).is_none());
    assert!(Alarm::new(0, 0, 0x80).is_none());
}

#[test]
fn settings_round_trip_rejects_truncation_and_invalid_values() {
    let mut settings = Settings {
        twelve_hour: true,
        alarm: Some(Alarm::new(23, 59, 0x1f).unwrap()),
    };
    settings.alarm.as_mut().unwrap().arm(0);
    let bytes = settings.encode();
    let decoded = Settings::decode(&bytes).unwrap();
    assert!(decoded.twelve_hour);
    assert_eq!(
        decoded.alarm.unwrap().next(),
        settings.alarm.unwrap().next()
    );
    assert!(Settings::decode(&bytes[..bytes.len() - 1]).is_none());
    let mut bad = bytes;
    bad[0] = 0;
    assert!(Settings::decode(&bad).is_none());
}

#[test]
fn snooze_uses_elapsed_time_and_dismiss_cancels_a_pending_alert() {
    let mut alert = AlarmAlert::default();
    assert!(!alert.snooze(0));
    alert.ring();
    assert!(alert.is_ringing());
    assert!(alert.snooze(1000));
    assert!(!alert.is_ringing());
    assert!(!alert.tick(300_999));
    assert!(alert.tick(301_000));
    assert!(!alert.tick(301_001));
    assert!(alert.snooze(302_000));
    alert.dismiss();
    assert!(!alert.tick(999_999));
    assert!(!alert.is_ringing());
}

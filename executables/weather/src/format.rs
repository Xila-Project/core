use alloc::{format, string::String};

use crate::model::{CurrentWeather, DailyWeather, ForecastResponse, HourlyWeather};

pub fn format_current_tab(current: Option<&CurrentWeather>) -> String {
    let Some(c) = current else {
        return String::from("No current data.");
    };

    format!(
        "Time: {}\nTemperature: {:.1} C\nFeels like: {:.1} C\nHumidity: {}%\nPrecipitation: {:.1} mm\nWeather code: {}\nWind: {:.1} km/h @ {} deg\nWind gusts: {:.1} km/h",
        c.time.as_deref().unwrap_or("?"),
        c.temperature_2m.unwrap_or(0.0),
        c.apparent_temperature.unwrap_or(0.0),
        c.relative_humidity_2m.unwrap_or(0),
        c.precipitation.unwrap_or(0.0),
        c.weather_code.unwrap_or(0),
        c.wind_speed_10m.unwrap_or(0.0),
        c.wind_direction_10m.unwrap_or(0),
        c.wind_gusts_10m.unwrap_or(0.0)
    )
}

pub fn format_hourly_tab(hourly: Option<&HourlyWeather>) -> String {
    let Some(h) = hourly else {
        return String::from("No hourly data.");
    };

    let times = h.time.as_deref().unwrap_or(&[]);
    let temps = h.temperature_2m.as_deref().unwrap_or(&[]);
    let probs = h.precipitation_probability.as_deref().unwrap_or(&[]);
    let winds = h.wind_speed_10m.as_deref().unwrap_or(&[]);

    let mut out = String::from("Next 24h\n");
    let count = core::cmp::min(times.len(), 24);

    for (i, time) in times.iter().enumerate().take(count) {
        let temp = temps.get(i).copied().unwrap_or_default();
        let prob = probs.get(i).copied().unwrap_or_default();
        let wind = winds.get(i).copied().unwrap_or_default();
        out.push_str(&format!(
            "{} | {:.1} C | {}% rain | {:.1} km/h\n",
            time, temp, prob, wind
        ));
    }

    out
}

pub fn format_daily_tab(daily: Option<&DailyWeather>) -> String {
    let Some(d) = daily else {
        return String::from("No daily data.");
    };

    let times = d.time.as_deref().unwrap_or(&[]);
    let maxs = d.temperature_2m_max.as_deref().unwrap_or(&[]);
    let mins = d.temperature_2m_min.as_deref().unwrap_or(&[]);
    let weather = d.weather_code.as_deref().unwrap_or(&[]);
    let precip = d.precipitation_probability_max.as_deref().unwrap_or(&[]);
    let sunrise = d.sunrise.as_deref().unwrap_or(&[]);
    let sunset = d.sunset.as_deref().unwrap_or(&[]);

    let mut out = String::from("Next days\n");
    for (i, time) in times.iter().enumerate().take(10) {
        out.push_str(&format!(
            "{} | min {:.1} / max {:.1} C | code {} | rain {}%\n  sunrise {} sunset {}\n",
            time,
            mins.get(i).copied().unwrap_or_default(),
            maxs.get(i).copied().unwrap_or_default(),
            weather.get(i).copied().unwrap_or_default(),
            precip.get(i).copied().unwrap_or_default(),
            sunrise.get(i).map(|s| s.as_str()).unwrap_or("?"),
            sunset.get(i).map(|s| s.as_str()).unwrap_or("?"),
        ));
    }

    out
}

pub fn format_meta_tab(
    city: &str,
    latitude: f64,
    longitude: f64,
    forecast: &ForecastResponse,
) -> String {
    format!(
        "Location: {}\nLatitude: {:.4}\nLongitude: {:.4}\nTimezone: {} ({})\nElevation: {:.1} m",
        city,
        latitude,
        longitude,
        forecast.timezone.as_deref().unwrap_or("?"),
        forecast.timezone_abbreviation.as_deref().unwrap_or("?"),
        forecast.elevation.unwrap_or(0.0)
    )
}

#[cfg(test)]
mod tests {
    use super::{format_current_tab, format_daily_tab, format_hourly_tab, format_meta_tab};
    use crate::model::{CurrentWeather, ForecastResponse};

    #[test]
    fn current_tab_includes_key_fields() {
        let current = CurrentWeather {
            time: Some("2026-03-29T12:00".into()),
            temperature_2m: Some(18.2),
            apparent_temperature: Some(17.4),
            relative_humidity_2m: Some(47),
            precipitation: Some(0.0),
            weather_code: Some(1),
            wind_speed_10m: Some(12.3),
            wind_direction_10m: Some(230),
            wind_gusts_10m: Some(20.5),
        };
        let text = format_current_tab(Some(&current));
        assert!(text.contains("Temperature"));
        assert!(text.contains("Wind"));
    }

    #[test]
    fn meta_tab_includes_timezone_and_elevation() {
        let forecast = ForecastResponse {
            timezone: Some("Europe/Paris".into()),
            timezone_abbreviation: Some("CEST".into()),
            elevation: Some(36.0),
            current: None,
            hourly: None,
            daily: None,
        };
        let text = format_meta_tab("Paris", 48.85, 2.35, &forecast);
        assert!(text.contains("Europe/Paris"));
        assert!(text.contains("Elevation"));
    }

    #[test]
    fn hourly_and_daily_handle_missing_data() {
        assert!(format_hourly_tab(None).contains("No hourly data"));
        assert!(format_daily_tab(None).contains("No daily data"));
    }
}

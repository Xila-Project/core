use alloc::{format, string::String};

fn encode_city(value: &str) -> String {
    value.replace(' ', "+")
}

pub fn build_geocoding_url(city: &str) -> String {
    format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=10&language=en&format=json",
        encode_city(city)
    )
}

pub fn build_forecast_url(latitude: f64, longitude: f64) -> String {
    let current = "temperature_2m,apparent_temperature,relative_humidity_2m,precipitation,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m";
    let hourly = "temperature_2m,precipitation_probability,wind_speed_10m";
    let daily = "weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,sunrise,sunset";

    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude:.4}&longitude={longitude:.4}&current={current}&hourly={hourly}&daily={daily}&forecast_days=10&timezone=auto"
    )
}

#[cfg(test)]
mod tests {
    use super::{build_forecast_url, build_geocoding_url};

    #[test]
    fn geocoding_url_contains_required_params() {
        let url = build_geocoding_url("Paris");
        assert!(url.starts_with("https://geocoding-api.open-meteo.com/v1/search?"));
        assert!(url.contains("name=Paris"));
        assert!(url.contains("count=10"));
        assert!(url.contains("language=en"));
        assert!(url.contains("format=json"));
    }

    #[test]
    fn forecast_url_contains_demo_rich_fields() {
        let url = build_forecast_url(48.8566, 2.3522);
        assert!(url.starts_with("https://api.open-meteo.com/v1/forecast?"));
        assert!(url.contains("latitude=48.8566"));
        assert!(url.contains("longitude=2.3522"));
        assert!(url.contains("current="));
        assert!(url.contains("hourly="));
        assert!(url.contains("daily="));
        assert!(url.contains("timezone=auto"));
    }
}

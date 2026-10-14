use alloc::{string::String, vec::Vec};
use miniserde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GeocodingResponse {
    pub results: Option<Vec<GeocodingResult>>,
    pub generationtime_ms: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct GeocodingResult {
    pub name: String,
    pub country: Option<String>,
    pub timezone: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Deserialize)]
pub struct ForecastResponse {
    pub timezone: Option<String>,
    pub timezone_abbreviation: Option<String>,
    pub elevation: Option<f64>,
    pub current: Option<CurrentWeather>,
    pub hourly: Option<HourlyWeather>,
    pub daily: Option<DailyWeather>,
}

#[derive(Debug, Deserialize)]
pub struct CurrentWeather {
    pub time: Option<String>,
    pub temperature_2m: Option<f64>,
    pub apparent_temperature: Option<f64>,
    pub relative_humidity_2m: Option<u16>,
    pub precipitation: Option<f64>,
    pub weather_code: Option<u16>,
    pub wind_speed_10m: Option<f64>,
    pub wind_direction_10m: Option<u16>,
    pub wind_gusts_10m: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct HourlyWeather {
    pub time: Option<Vec<String>>,
    pub temperature_2m: Option<Vec<f64>>,
    pub precipitation_probability: Option<Vec<u16>>,
    pub wind_speed_10m: Option<Vec<f64>>,
}

#[derive(Debug, Deserialize)]
pub struct DailyWeather {
    pub time: Option<Vec<String>>,
    pub temperature_2m_max: Option<Vec<f64>>,
    pub temperature_2m_min: Option<Vec<f64>>,
    pub weather_code: Option<Vec<u16>>,
    pub precipitation_probability_max: Option<Vec<u16>>,
    pub sunrise: Option<Vec<String>>,
    pub sunset: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use miniserde::json;

    use super::{ForecastResponse, GeocodingResponse};

    #[test]
    fn parses_geocoding_first_result_fields() {
        let raw = r#"{"results":[{"name":"Paris","latitude":48.8566,"longitude":2.3522,"country":"France","timezone":"Europe/Paris"}],"generationtime_ms":0.7}"#;
        let parsed: GeocodingResponse = json::from_str(raw).unwrap();
        let first = parsed.results.as_ref().unwrap().first().unwrap();
        assert_eq!(first.name, "Paris");
        assert!((first.latitude - 48.8566).abs() < 0.01);
    }

    #[test]
    fn parses_forecast_core_blocks() {
        let raw = r#"{"timezone":"Europe/Paris","timezone_abbreviation":"CEST","elevation":36.0,"current":{"time":"2026-03-29T12:00","temperature_2m":18.2,"relative_humidity_2m":47,"apparent_temperature":17.4,"precipitation":0.0,"weather_code":1,"wind_speed_10m":12.3,"wind_direction_10m":230},"hourly":{"time":["2026-03-29T12:00"],"temperature_2m":[18.2],"precipitation_probability":[5],"wind_speed_10m":[12.3]},"daily":{"time":["2026-03-29"],"temperature_2m_max":[20.0],"temperature_2m_min":[11.0],"weather_code":[1],"sunrise":["2026-03-29T07:19"],"sunset":["2026-03-29T20:08"]}}"#;
        let parsed: ForecastResponse = json::from_str(raw).unwrap();
        assert_eq!(parsed.timezone.as_deref(), Some("Europe/Paris"));
        assert!(parsed.current.is_some());
        assert!(parsed.hourly.is_some());
        assert!(parsed.daily.is_some());
    }
}

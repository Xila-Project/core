#[cfg(test)]
mod tests {
    #[test]
    fn readme_mentions_open_meteo_endpoints() {
        let text = include_str!("../README.md");
        assert!(text.contains("geocoding-api.open-meteo.com"));
        assert!(text.contains("api.open-meteo.com/v1/forecast"));
    }
}

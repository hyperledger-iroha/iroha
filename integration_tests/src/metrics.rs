//! Helpers for parsing Prometheus text-format snapshots in tests.

use std::collections::HashMap;

/// Lightweight Prometheus snapshot reader supporting exact-key lookups and prefix scans.
#[derive(Debug, Clone)]
pub struct MetricsReader {
    map: HashMap<String, f64>,
}

impl MetricsReader {
    /// Parse `{key value}` lines from a Prometheus text snapshot, ignoring comments and blanks.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        let map = raw
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
            .map(|line| {
                let mut iter = line.split_whitespace();
                let key = iter.next().expect("metric key").to_owned();
                let value = iter
                    .next()
                    .expect("metric value")
                    .parse()
                    .expect("numeric metric value");
                assert!(
                    iter.next().is_none(),
                    "unexpected extra fields in metric line: {line}"
                );
                (key, value)
            })
            .collect();
        Self { map }
    }

    /// Fetch a metric value by its exact key, panic if missing.
    #[must_use]
    pub fn get(&self, key: &str) -> f64 {
        *self
            .map
            .get(key)
            .unwrap_or_else(|| panic!("missing metric: {key}"))
    }

    /// Fetch a metric value by its exact key, returning `None` when absent.
    #[must_use]
    pub fn get_optional(&self, key: &str) -> Option<f64> {
        self.map.get(key).copied()
    }

    /// Return the maximum value among metrics sharing a prefix.
    #[must_use]
    pub fn max_with_prefix(&self, prefix: &str) -> Option<f64> {
        self.map
            .iter()
            .filter_map(|(key, value)| key.starts_with(prefix).then_some(*value))
            .reduce(f64::max)
    }

    /// Return the maximum value among metrics sharing a prefix and a label pair.
    #[must_use]
    pub fn max_with_prefix_and_label(&self, prefix: &str, label: &str, value: &str) -> Option<f64> {
        let needle = format!("{label}=\"{value}\"");
        self.map
            .iter()
            .filter_map(|(key, metric_value)| {
                (key.starts_with(prefix) && key.contains(&needle)).then_some(*metric_value)
            })
            .reduce(f64::max)
    }
}

#[cfg(test)]
mod tests {
    use super::MetricsReader;

    #[test]
    fn get_optional_returns_none_for_missing_metric() {
        let reader = MetricsReader::new("metric_a 1.0\nmetric_b 2.0\n");
        assert_eq!(reader.get_optional("metric_a"), Some(1.0));
        assert_eq!(reader.get_optional("metric_missing"), None);
    }

    #[test]
    #[should_panic(expected = "missing metric: metric_missing")]
    fn get_panics_for_missing_metric() {
        let reader = MetricsReader::new("metric_present 1.0\n");
        let _ = reader.get("metric_missing");
    }

    #[test]
    fn max_with_prefix_and_label_selects_matching_metric() {
        let reader = MetricsReader::new(
            "metric{phase=\"propose\",peer=\"a\"} 1.0\nmetric{phase=\"propose\",peer=\"b\"} 2.0\nmetric{phase=\"commit\"} 3.0\n",
        );
        assert_eq!(
            reader.max_with_prefix_and_label("metric{", "phase", "propose"),
            Some(2.0)
        );
    }

    #[test]
    fn max_with_prefix_and_label_returns_none_for_missing_label() {
        let reader = MetricsReader::new("metric{phase=\"commit\"} 1.0\n");
        assert_eq!(
            reader.max_with_prefix_and_label("metric{", "phase", "propose"),
            None
        );
    }
}

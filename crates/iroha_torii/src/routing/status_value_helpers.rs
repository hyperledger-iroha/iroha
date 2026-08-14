#[cfg(feature = "telemetry")]
#[allow(single_use_lifetimes)]
fn json_value_by_segments<'a>(
    mut value: norito::json::Value,
    segments: impl Iterator<Item = &'a str>,
) -> Option<norito::json::Value> {
    for segment in segments {
        value = match value {
            norito::json::Value::Object(map) => map.get(segment)?.clone(),
            norito::json::Value::Array(values) => {
                let index = segment.parse::<usize>().ok()?;
                values.get(index)?.clone()
            }
            _ => return None,
        };
    }
    Some(value)
}

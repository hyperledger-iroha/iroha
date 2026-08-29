#[allow(clippy::cast_precision_loss)]
fn u64_to_f64(value: u64) -> f64 {
    value as f64
}
fn clamp_u32_to_i64(value: u32) -> i64 {
    i64::from(value)
}
fn u128_to_f64(value: u128) -> f64 {
    u64::try_from(value).map_or(f64::MAX, u64_to_f64)
}
fn quantity_to_micro_f64(value: &iroha_data_model::prelude::Quantity) -> f64 {
    let micros = value.as_numeric().to_f64_lossy() * 1_000_000.0;
    if micros.is_finite() { micros } else { f64::MAX }
}
/// Project an exact quantity into the fixed-unit `f64` nano presentation used
/// only by telemetry instruments. This projection never feeds consensus state.
fn quantity_to_nano_f64(value: &Quantity) -> f64 {
    let nanos = value.as_numeric().to_f64_lossy() * 1_000_000_000.0;
    if nanos.is_finite() { nanos } else { f64::MAX }
}

fn render_incentive_prometheus(
    relay_id: RelayId,
    summaries: &[EpochSummary],
    mode: RelayMode,
) -> Result<String, fmt::Error> {
    if summaries.is_empty() {
        return Ok(String::new());
    }
    if summaries.len() > INCENTIVE_MAX_ACTIVE_EPOCHS_V1 {
        return Err(fmt::Error);
    }
    // Five fixed-format series per epoch fit in 1 KiB even with maximal
    // integer fields, a 64-byte relay ID, and the longest mode label.
    let max_bytes = 2_048_usize
        .checked_add(summaries.len().checked_mul(1_024).ok_or(fmt::Error)?)
        .ok_or(fmt::Error)?;
    let mut output = IncentiveMetricsWriter::new(max_bytes)?;
    let mode_label = mode.as_label();
    let relay_hex = hex::encode(relay_id);
    writeln!(
        output,
        "# HELP soranet_relay_uptime_seconds_total Relay uptime observed within the incentive epoch."
    )?;
    writeln!(output, "# TYPE soranet_relay_uptime_seconds_total counter")?;
    for summary in summaries {
        writeln!(
            output,
            "soranet_relay_uptime_seconds_total{{mode=\"{mode}\",relay=\"{relay}\",epoch=\"{epoch}\"}} {uptime}",
            mode = mode_label,
            relay = relay_hex,
            epoch = summary.epoch,
            uptime = summary.uptime_seconds
        )?;
    }
    writeln!(
        output,
        "# HELP soranet_relay_scheduled_seconds_total Expected uptime window for the incentive epoch."
    )?;
    writeln!(
        output,
        "# TYPE soranet_relay_scheduled_seconds_total counter"
    )?;
    for summary in summaries {
        writeln!(
            output,
            "soranet_relay_scheduled_seconds_total{{mode=\"{mode}\",relay=\"{relay}\",epoch=\"{epoch}\"}} {scheduled}",
            mode = mode_label,
            relay = relay_hex,
            epoch = summary.epoch,
            scheduled = summary.scheduled_uptime_seconds
        )?;
    }
    writeln!(
        output,
        "# HELP soranet_relay_bandwidth_verified_bytes_total Verified relay bandwidth contribution for the epoch."
    )?;
    writeln!(
        output,
        "# TYPE soranet_relay_bandwidth_verified_bytes_total counter"
    )?;
    for summary in summaries {
        writeln!(
            output,
            "soranet_relay_bandwidth_verified_bytes_total{{mode=\"{mode}\",relay=\"{relay}\",epoch=\"{epoch}\"}} {bytes}",
            mode = mode_label,
            relay = relay_hex,
            epoch = summary.epoch,
            bytes = summary.verified_bandwidth_bytes
        )?;
    }
    writeln!(
        output,
        "# HELP soranet_relay_measurements_total Accepted blinded measurement proofs per epoch."
    )?;
    writeln!(output, "# TYPE soranet_relay_measurements_total counter")?;
    for summary in summaries {
        writeln!(
            output,
            "soranet_relay_measurements_total{{mode=\"{mode}\",relay=\"{relay}\",epoch=\"{epoch}\"}} {count}",
            mode = mode_label,
            relay = relay_hex,
            epoch = summary.epoch,
            count = summary.measurement_ids.len()
        )?;
    }
    writeln!(
        output,
        "# HELP soranet_relay_confidence_floor_per_mille Minimum measurement confidence per epoch."
    )?;
    writeln!(
        output,
        "# TYPE soranet_relay_confidence_floor_per_mille gauge"
    )?;
    for summary in summaries {
        writeln!(
            output,
            "soranet_relay_confidence_floor_per_mille{{mode=\"{mode}\",relay=\"{relay}\",epoch=\"{epoch}\"}} {confidence}",
            mode = mode_label,
            relay = relay_hex,
            epoch = summary.epoch,
            confidence = u64::from(summary.confidence_floor_per_mille)
        )?;
    }
    Ok(output.output)
}

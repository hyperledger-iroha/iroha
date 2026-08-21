fn four_validator_npos_builder() -> NetworkBuilder {
    const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1024 * 1024 * 1024;

    NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_npos_genesis_bootstrap(SumeragiNposParameters::default().min_self_bond().clone())
        .with_config_layer(|layer| {
            layer.write(
                ["nexus", "storage", "local_budget_bytes"],
                TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
            );
        })
}

fn optional_timeout_view(object: &norito::json::Map, peer: &str) -> Result<Option<u64>> {
    let Some(certificate) = object
        .get("last_timeout_certificate")
        .filter(|value| !value.is_null())
    else {
        return Ok(None);
    };
    let round = certificate
        .as_object()
        .and_then(|certificate| certificate.get("round"))
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("v2 status for {peer} has a malformed timeout-certificate round"))?;
    let view = round
        .get("view")
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("v2 status for {peer} has a timeout certificate without a view"))?;
    Ok(Some(view))
}

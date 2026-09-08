// Source contract for exact deployment-owned runtime forwarding into Torii.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "complete dependency-forwarding contract"
)]
fn standard_launcher_forwards_external_sorafs_runtime_dependencies() {
    let compact_source: String = include_str!("../main.rs")
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    for (field, builder) in [
        (
            "sorafs_stream_token_hardware_client",
            "with_sorafs_stream_token_hardware_client",
        ),
        (
            "sorafs_stream_token_state_observer",
            "with_sorafs_stream_token_state_observer",
        ),
        (
            "sorafs_appeal_finance_checkpoint_runtime",
            "with_sorafs_appeal_finance_checkpoint_runtime",
        ),
        (
            "sorafs_evidence_viewer_webauthn",
            "with_sorafs_evidence_viewer_webauthn",
        ),
        (
            "sorafs_moderation_panel_notification",
            "with_sorafs_moderation_panel_notification",
        ),
        (
            "sorafs_moderation_panel_notification_archive",
            "with_sorafs_moderation_panel_notification_archive",
        ),
        (
            "sorafs_evidence_viewer_grants",
            "with_sorafs_evidence_viewer_grants",
        ),
        (
            "sorafs_evidence_viewer_receipt_signer",
            "with_sorafs_evidence_viewer_receipt_signer",
        ),
        (
            "sorafs_evidence_viewer_erasure",
            "with_sorafs_evidence_viewer_erasure",
        ),
        (
            "sorafs_evidence_viewer_checkpoint_store",
            "with_sorafs_evidence_viewer_checkpoint_store",
        ),
        (
            "sorafs_evidence_viewer_compaction_archive",
            "with_sorafs_evidence_viewer_compaction_archive",
        ),
        (
            "sorafs_evidence_viewer_transparency_publisher",
            "with_sorafs_evidence_viewer_transparency_publisher",
        ),
        (
            "sorafs_gateway_acme_client",
            "with_sorafs_gateway_acme_client",
        ),
        (
            "sorafs_gateway_compliance_feed_transport",
            "with_sorafs_gateway_compliance_feed_transport",
        ),
    ] {
        let clone_from_external = ["runtime_deps.", field, ".clone()"].concat();
        let forward_to_torii = [".", builder, "("].concat();
        assert!(
            compact_source.contains(&clone_from_external),
            "standard launcher must clone external dependency `{field}` before Torii dependency assembly"
        );
        assert!(
            compact_source.contains(&forward_to_torii),
            "standard launcher must forward `{field}` through `{builder}`"
        );
    }
    assert!(compact_source.contains(
        "letsorafs_stream_token_approved_anchor=runtime_deps.sorafs_stream_token_approved_anchor;"
    ));
    assert!(compact_source.contains(".with_sorafs_stream_token_approved_anchor(anchor)"));
    assert!(
        compact_source.contains("runtime_deps.sorafs_pop_credential_provider_registry.clone()"),
        "standard launcher must clone the deployment-owned PoP provider registry"
    );
    assert!(
        compact_source.contains("sorafs_pop_runtime::build("),
        "standard launcher must build the config-bound PoP runtime"
    );
    assert!(
        compact_source.contains(".with_sorafs_pop_credentials("),
        "standard launcher must forward only the qualified PoP runtime to Torii"
    );
    let forbidden_gateway_fallback =
        ["ProductionGatewayComplianceFeedTransport", "::try_new"].concat();
    assert!(
        !compact_source.contains(&forbidden_gateway_fallback),
        "standard launcher must not replace a missing deployment-owned compliance transport with an in-process fallback"
    );
    assert!(
        compact_source.contains(
            "enabledSoraFSgatewaycompliancerequirestheexactdeployment-ownedauthenticatedfeedtransport"
        ),
        "enabled gateway compliance must fail before Torii startup when its deployment transport is absent"
    );
    assert!(
        compact_source.contains(
            "configuredSoraFSgatewayACMEautomationrequirestheexactdeployment-ownedACMEclient"
        ),
        "configured ACME automation must fail before Torii startup when its deployment client is absent"
    );
    assert!(
        compact_source.contains("disabledSoraFSgatewaycompliancerejectsanunexpectedfeedtransport"),
        "disabled gateway compliance must reject an injected transport"
    );
    assert!(
        compact_source.contains("unconfiguredSoraFSgatewayACMEautomationrejectsanunexpectedclient"),
        "unconfigured ACME automation must reject an injected client"
    );
}

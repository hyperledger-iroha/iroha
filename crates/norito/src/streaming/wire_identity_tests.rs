//! Declared identities, stable payloads and feature-specific frame validation.
use super::*;

fn current<T>() -> crate::json::Value
where
    T: crate::NoritoSchema + crate::NoritoSerialize + crate::NoritoDeserialize<'static>,
{
    let nominal = T::nominal_name();
    let serialize_hash = <T as crate::NoritoSerialize>::schema_hash();
    let deserialize_hash = <T as crate::NoritoDeserialize>::schema_hash();
    assert_eq!(
        serialize_hash, deserialize_hash,
        "both directions for {nominal}"
    );
    let declared_hash = crate::schema::identity::frame_hash::<T>();
    // The captures use nominal framing; structural framing has its own active hash.
    #[cfg(not(feature = "schema-structural"))]
    assert_eq!(serialize_hash, declared_hash);
    let declared_hex = declared_hash
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    crate::json!({
        "nominal": nominal,
        "declared_hash": declared_hex,
    })
}

#[test]
fn streaming_declared_identities_match_captured_nominal_hashes() {
    let rows = vec![
        current::<ProfileId>(),
        current::<EntropyMode>(),
        current::<RdoMode>(),
        current::<CapabilityFlags>(),
        current::<BundleAcceleration>(),
        current::<PrivacyCapabilities>(),
        current::<FecParameters>(),
        current::<LayerFeedback>(),
        current::<FeedbackHint>(),
        current::<HpkeSuite>(),
        current::<HpkeSuiteMask>(),
        current::<PrivacyBucketGranularity>(),
        current::<TransportCapabilities>(),
        current::<StorageClass>(),
        current::<FecScheme>(),
        current::<EncryptionSuite>(),
        current::<StreamMetadata>(),
        current::<PrivacyRelay>(),
        current::<SoranetAccessKind>(),
        current::<SoranetStreamTag>(),
        current::<SoranetChannelId>(),
        current::<SoranetRoute>(),
        current::<PrivacyRoute>(),
        current::<NeuralBundle>(),
        current::<TicketCapabilities>(),
        current::<TicketPolicy>(),
        current::<StreamingTicket>(),
        current::<TicketRevocation>(),
        current::<ManifestV1>(),
        current::<SegmentHeader>(),
        current::<ChunkDescriptor>(),
        current::<MerkleProof>(),
        current::<DataAvailabilityProof>(),
        current::<ControlFrame>(),
        current::<ErrorCode>(),
        current::<KeyUpdate>(),
        current::<ContentKeyUpdate>(),
        current::<PrivacyRouteUpdate>(),
        current::<CapabilityReport>(),
        current::<CapabilityAck>(),
        current::<CapabilityRole>(),
        current::<AudioCapability>(),
        current::<Resolution>(),
        current::<AudioLayout>(),
        current::<AudioFrame>(),
        current::<AudioTrackSummary>(),
        current::<SegmentAudio>(),
        current::<FeedbackHintFrame>(),
        current::<ReceiverReport>(),
        current::<TelemetryAuditOutcome>(),
        current::<TelemetryEvent>(),
        current::<ManifestAnnounceFrame>(),
        current::<ChunkRequestFrame>(),
        current::<ChunkAcknowledgeFrame>(),
        current::<TransportCapabilitiesFrame>(),
        current::<PrivacyRouteAckFrame>(),
        current::<ControlErrorFrame>(),
        current::<ResolutionCustom>(),
        current::<TelemetryEncodeStats>(),
        current::<TelemetryDecodeStats>(),
        current::<TelemetryNetworkStats>(),
        current::<SyncDiagnostics>(),
        current::<TelemetrySecurityStats>(),
        current::<TelemetryEnergyStats>(),
        current::<RansGroupTableV1>(),
        current::<RansTablesBodyV1>(),
        current::<RansTablesV1>(),
        current::<SignatureAlgorithm>(),
        current::<RansTablesSignatureV1>(),
        current::<SignedRansTablesV1>(),
        current::<codec::SegmentBundle>(),
        current::<codec::FrameDimensions>(),
        current::<codec::Chroma420Frame>(),
        current::<codec::BundledStats>(),
        current::<codec::RdoTelemetry>(),
        current::<codec::ContextFrequency>(),
        current::<codec::ContextRemapSummary>(),
        current::<codec::BundledTelemetry>(),
        current::<codec::BundledToken>(),
        current::<codec::BundleType>(),
        current::<codec::BundleFlushReason>(),
        current::<codec::BundleContextId>(),
        current::<codec::BundleRecord>(),
        current::<codec::BundleContextStats>(),
    ];
    let expected: Vec<crate::json::Value> = crate::json::from_str(include_str!(
        "../../tests/fixtures/streaming_wire_identities.json"
    ))
    .expect("parse immutable identity fixture");
    let declared_expected: Vec<_> = expected
        .iter()
        .map(|row| {
            assert_eq!(row.get("serialize_hash"), row.get("deserialize_hash"));
            let nominal = row
                .get("nominal")
                .expect("captured nominal identity")
                .clone();
            let declared_hash = row
                .get("serialize_hash")
                .expect("captured nominal hash")
                .clone();
            crate::json!({
                "nominal": nominal,
                "declared_hash": declared_hash,
            })
        })
        .collect();
    assert_eq!(
        rows, declared_expected,
        "all 84 declared nominal identities"
    );
}

#[path = "tests/codec_wire_values.rs"]
mod codec_wire_values;
#[path = "tests/wire_value_fixtures.rs"]
mod wire_value_fixtures;

#[test]
fn streaming_wire_payloads_and_default_frames_match_captures() {
    let mut rows = Vec::new();
    wire_value_fixtures::root_values(&mut rows);
    codec_wire_values::codec_values(&mut rows);
    assert_eq!(rows.len(), 396);
    let expected: Vec<crate::json::Value> = crate::json::from_str(include_str!(
        "../../tests/fixtures/streaming_wire_identity_frames.json"
    ))
    .expect("parse immutable frame fixture");
    #[cfg(not(feature = "schema-structural"))]
    assert_eq!(rows, expected, "all 396 complete nominal-mode frames");
    // Structural framing changes schema headers, while populated payloads, layout
    // flags and alignment must remain identical to the immutable captures.
    // Each observed frame also checks both active hash directions, exact framed
    // roundtrips, wrong-header rejection, truncation and trailing-byte rejection.
    #[cfg(feature = "schema-structural")]
    {
        assert_eq!(rows.len(), expected.len());
        for (actual, captured) in rows.iter().zip(&expected) {
            for key in ["case", "nominal", "bare_hex", "header_flags", "padding_len"] {
                assert_eq!(actual.get(key), captured.get(key), "{key}: {actual:?}");
            }
        }
    }
}

//! Populated values checked against captured streaming wire identities.
//!
//! Values follow the literal patterns in streaming::tests, streaming_ticket_golden,
//! streaming_roundtrip and baseline_and_bundle_tests. They exercise wire records;
//! synthetic signatures and commitments do not assert cryptographic authorization.

use super::*;
use std::fmt::Debug;

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(result, "{byte:02x}").expect("write hex to String");
    }
    result
}

fn observed<T>(case: &str, value: &T) -> crate::json::Value
where
    T: crate::NoritoSchema + crate::NoritoSerialize + for<'a> crate::NoritoDeserialize<'a> + Debug,
{
    let _requested = crate::core::DecodeFlagsGuard::enter(crate::core::default_encode_flags());
    let (bare, flags) = crate::codec::encode_with_header_flags(value);
    let frame = crate::core::frame_bare_with_header_flags::<T>(&bare, flags)
        .unwrap_or_else(|error| panic!("frame {case}: {error}"));
    assert_eq!(
        crate::to_bytes(value).expect("canonical frame"),
        frame,
        "{case}"
    );
    let header = crate::core::Header::read(frame.as_slice()).expect("read emitted header");
    assert_eq!(header.flags, flags, "{case}");
    let padding = crate::core::payload_alignment_padding_for::<T>();
    let payload_start = crate::core::Header::SIZE + padding;
    assert!(
        frame[crate::core::Header::SIZE..payload_start]
            .iter()
            .all(|b| *b == 0),
        "zero alignment padding for {case}"
    );
    assert_eq!(
        header.length,
        u64::try_from(bare.len()).unwrap(),
        "payload length for {case}"
    );
    assert_eq!(&frame[payload_start..], bare, "{case}");
    let _advertised = crate::core::DecodeFlagsGuard::enter(header.flags);
    let decoded: T = crate::decode_from_bytes(&frame)
        .unwrap_or_else(|error| panic!("decode {case}: {error}; value={value:?}"));
    let (decoded_bare, decoded_flags) = crate::codec::encode_with_header_flags(&decoded);
    assert_eq!(decoded_bare, bare, "decoded bare bytes for {case}");
    assert_eq!(decoded_flags, flags, "decoded advertised flags for {case}");
    assert_eq!(
        crate::to_bytes(&decoded).expect("re-encode frame"),
        frame,
        "{case}"
    );
    let serialize_hash = <T as crate::NoritoSerialize>::schema_hash();
    let deserialize_hash = <T as crate::NoritoDeserialize>::schema_hash();
    assert_eq!(
        serialize_hash, deserialize_hash,
        "both codec directions for {case}"
    );
    assert_eq!(header.schema, serialize_hash, "header identity for {case}");
    // These captured declarations describe nominal framing. Structural framing
    // must still agree across both active codec directions, checked above.
    #[cfg(not(feature = "schema-structural"))]
    assert_eq!(
        crate::schema::identity::frame_hash::<T>(),
        serialize_hash,
        "declared identity for {case}"
    );
    let mut wrong_schema = frame.clone();
    wrong_schema[6] ^= 1;
    assert!(
        matches!(
            crate::decode_from_bytes::<T>(&wrong_schema),
            Err(crate::Error::SchemaMismatch)
        ),
        "wrong schema rejected for {case}"
    );
    for end in [0, crate::core::Header::SIZE - 1, frame.len() - 1] {
        assert!(
            crate::decode_from_bytes::<T>(&frame[..end]).is_err(),
            "truncated frame rejected for {case}"
        );
    }
    let mut trailing = frame.clone();
    trailing.push(0);
    assert!(
        crate::decode_from_bytes::<T>(&trailing).is_err(),
        "trailing frame bytes rejected for {case}"
    );
    crate::json!({
        "case": case,
        "nominal": (T::nominal_name()),
        "serialize_hash": (hex(&serialize_hash)),
        "deserialize_hash": (hex(&deserialize_hash)),
        "header_flags": flags,
        "padding_len": padding,
        "frame_hex": (hex(&frame)),
        "bare_hex": (hex(&bare)),
    })
}

pub(super) fn record<T>(rows: &mut Vec<crate::json::Value>, case: &str, value: T)
where
    T: Clone
        + crate::NoritoSchema
        + crate::NoritoSerialize
        + for<'a> crate::NoritoDeserialize<'a>
        + Debug,
{
    rows.push(observed(&format!("{case}/root"), &value));
    rows.push(observed(&format!("{case}/some"), &Some(value.clone())));
    rows.push(observed(
        &format!("{case}/vec"),
        &vec![value.clone(), value],
    ));
}

pub(super) fn root_values(rows: &mut Vec<crate::json::Value>) {
    macro_rules! case {
        ($owner:ident, $label:literal, $value:expr) => {
            record::<$owner>(rows, concat!(stringify!($owner), "/", $label), $value);
        };
    }
    case!(ProfileId, "uhd_ai", ProfileId::UHD_AI);
    case!(EntropyMode, "rans_bundled", EntropyMode::RansBundled);
    case!(RdoMode, "none", RdoMode::None);
    case!(RdoMode, "dynamic_programming", RdoMode::DynamicProgramming);
    case!(RdoMode, "neural", RdoMode::Neural);
    case!(RdoMode, "perceptual", RdoMode::Perceptual);
    let capabilities = CapabilityFlags::from_bits(
        CapabilityFlags::FEATURE_FEEDBACK_HINTS
            | CapabilityFlags::FEATURE_SM_TRANSACTIONS
            | CapabilityFlags::FEATURE_ENTROPY_BUNDLED,
    );
    case!(CapabilityFlags, "populated", capabilities);
    case!(BundleAcceleration, "none", BundleAcceleration::None);
    case!(BundleAcceleration, "cpu_simd", BundleAcceleration::CpuSimd);
    case!(BundleAcceleration, "gpu", BundleAcceleration::Gpu);
    let privacy_capabilities = PrivacyCapabilities::from_bits(0b1010);
    case!(PrivacyCapabilities, "populated", privacy_capabilities);
    let fec = FecParameters {
        scheme: FecScheme::RsWin14_10,
        window_step: Some(2),
        parity_symbols: Some(4),
    };
    case!(FecParameters, "populated", fec);
    let layer = LayerFeedback {
        layer_id: 2,
        min_target_kbps: 1_200,
        max_target_kbps: 2_400,
        storage_hint: Some(StorageClass::Permanent),
    };
    case!(LayerFeedback, "populated", layer);
    let feedback = FeedbackHint {
        layer_hints: vec![layer],
        report_interval_ms: Some(250),
        fec: Some(fec),
    };
    case!(FeedbackHint, "populated", feedback.clone());
    case!(HpkeSuite, "kyber768", HpkeSuite::Kyber768AuthPsk);
    case!(HpkeSuite, "kyber1024", HpkeSuite::Kyber1024AuthPsk);
    let suites =
        HpkeSuiteMask::from_bits(HpkeSuiteMask::KYBER768.bits() | HpkeSuiteMask::KYBER1024.bits());
    case!(HpkeSuiteMask, "both", suites);
    case!(
        PrivacyBucketGranularity,
        "standard_v1",
        PrivacyBucketGranularity::StandardV1
    );
    let transport = TransportCapabilities {
        hpke_suites: suites,
        supports_datagram: true,
        max_segment_datagram_size: 1_200,
        fec_feedback_interval_ms: 250,
        privacy_bucket_granularity: PrivacyBucketGranularity::StandardV1,
    };
    case!(TransportCapabilities, "populated", transport);
    case!(StorageClass, "ephemeral", StorageClass::Ephemeral);
    case!(StorageClass, "permanent", StorageClass::Permanent);
    case!(FecScheme, "rs12_10", FecScheme::Rs12_10);
    case!(FecScheme, "rswin14_10", FecScheme::RsWin14_10);
    case!(FecScheme, "rs18_14", FecScheme::Rs18_14);
    let encryption = EncryptionSuite::Kyber768XChaCha20Poly1305([0x21; 32]);
    case!(
        EncryptionSuite,
        "x25519",
        EncryptionSuite::X25519ChaCha20Poly1305([0x20; 32])
    );
    case!(EncryptionSuite, "kyber768", encryption);
    let metadata = StreamMetadata {
        title: "Test Stream".into(),
        description: Some("Demo manifest".into()),
        access_policy_id: Some([14; 32]),
        tags: vec!["demo".into(), "nsc".into()],
    };
    case!(StreamMetadata, "populated", metadata.clone());
    let entry = PrivacyRelay {
        relay_id: [0x22; 32],
        endpoint: "/dns/soranet.entry/quic".into(),
        key_fingerprint: [0x23; 32],
        capabilities: PrivacyCapabilities::from_bits(0b001),
    };
    let exit = PrivacyRelay {
        relay_id: [0x24; 32],
        endpoint: "/dns/soranet.exit/quic".into(),
        key_fingerprint: [0x25; 32],
        capabilities: PrivacyCapabilities::from_bits(0b010),
    };
    case!(PrivacyRelay, "populated", entry.clone());
    case!(SoranetAccessKind, "read_only", SoranetAccessKind::ReadOnly);
    case!(
        SoranetAccessKind,
        "authenticated",
        SoranetAccessKind::Authenticated
    );
    case!(
        SoranetStreamTag,
        "norito_stream",
        SoranetStreamTag::NoritoStream
    );
    case!(SoranetStreamTag, "kaigi", SoranetStreamTag::Kaigi);
    let channel = SoranetChannelId::new([0x26; 32]);
    case!(SoranetChannelId, "populated", channel);
    let soranet = SoranetRoute {
        channel_id: channel,
        exit_multiaddr: "/dns/torii.exit.example/tcp/8080".into(),
        padding_budget_ms: Some(12),
        access_kind: SoranetAccessKind::Authenticated,
        stream_tag: SoranetStreamTag::Kaigi,
    };
    case!(SoranetRoute, "populated", soranet.clone());
    let route = PrivacyRoute {
        route_id: [0x21; 32],
        entry,
        exit,
        ticket_entry: vec![0x10, 0x11, 0x12],
        ticket_exit: vec![0x20, 0x21, 0x22],
        expiry_segment: 256,
        soranet: Some(soranet.clone()),
    };
    case!(PrivacyRoute, "populated", route.clone());
    let neural = NeuralBundle {
        bundle_id: "bundle-v1".into(),
        weights_sha256: [11; 32],
        activation_scale: vec![123, 234],
        bias: vec![1_000, -42],
        metadata_signature: [12; 64],
        metal_shader_sha256: Some([13; 32]),
        cuda_ptx_sha256: Some([14; 32]),
    };
    case!(NeuralBundle, "populated", neural.clone());
    let ticket_capabilities = TicketCapabilities::from_bits(
        TicketCapabilities::LIVE | TicketCapabilities::HDR | TicketCapabilities::SPATIAL_AUDIO,
    );
    case!(TicketCapabilities, "populated", ticket_capabilities);
    let policy = TicketPolicy {
        max_relays: 4,
        allowed_regions: vec!["us".into(), "jp".into()],
        max_bandwidth_kbps: Some(15_000),
    };
    case!(TicketPolicy, "populated", policy.clone());
    let ticket = StreamingTicket {
        ticket_id: [0x44; 32],
        owner: "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE".to_owned(),
        dsid: 7,
        lane_id: 5,
        settlement_bucket: 2_048,
        start_slot: 21_000,
        expire_slot: 24_000,
        prepaid_teu: 120_000,
        chunk_teu: 64,
        fanout_quota: 12,
        key_commitment: [0x55; 32],
        nonce: 42,
        contract_sig: [0x66; 64],
        commitment: [0x77; 32],
        nullifier: [0x88; 32],
        proof_id: [0x99; 32],
        issued_at: 1_701_234_567,
        expires_at: 1_701_834_567,
        policy: Some(policy),
        capabilities: ticket_capabilities,
    };
    case!(StreamingTicket, "populated", ticket);
    case!(
        TicketRevocation,
        "populated",
        TicketRevocation {
            ticket_id: [0xAA; 32],
            nullifier: [0xBB; 32],
            reason_code: 17,
            revocation_signature: [0xCC; 64],
        }
    );
    case!(AudioLayout, "mono", AudioLayout::Mono);
    case!(AudioLayout, "stereo", AudioLayout::Stereo);
    case!(
        AudioLayout,
        "first_order_ambisonics",
        AudioLayout::FirstOrderAmbisonics
    );
    let audio_summary = AudioTrackSummary {
        sample_rate: 48_000,
        frame_samples: 960,
        frame_duration_ns: 20_000_000,
        frames_per_segment: 2,
        layout: AudioLayout::Stereo,
        fec_level: 1,
    };
    case!(AudioTrackSummary, "populated", audio_summary);
    let audio_frame = AudioFrame {
        sequence: 42,
        timestamp_ns: 1_333_000,
        fec_level: 1,
        channel_layout: AudioLayout::Stereo,
        payload: vec![0x10, 0x20, 0x30, 0x40],
    };
    case!(AudioFrame, "populated", audio_frame.clone());
    let mut next_audio_frame = audio_frame.clone();
    next_audio_frame.sequence += 1;
    next_audio_frame.timestamp_ns += u64::from(audio_summary.frame_duration_ns);
    case!(
        SegmentAudio,
        "populated",
        SegmentAudio {
            summary: audio_summary,
            frames: vec![audio_frame, next_audio_frame],
        }
    );
    let chunk = ChunkDescriptor {
        chunk_id: 1,
        offset: 1_024,
        length: 1_024,
        commitment: [3; 32],
        parity: false,
    };
    case!(ChunkDescriptor, "populated", chunk);
    let manifest = ManifestV1 {
        stream_id: [1; 32],
        protocol_version: 1,
        segment_number: 42,
        published_at: 1_694_000_000,
        profile: ProfileId::BASELINE,
        entropy_mode: EntropyMode::RansBundled,
        entropy_tables_checksum: Some([0x40; 32]),
        da_endpoint: "/ip4/127.0.0.1/udp/9000/quic".into(),
        chunk_root: [2; 32],
        content_key_id: 7,
        nonce_salt: [4; 32],
        chunk_descriptors: vec![chunk],
        transport_capabilities_hash: [11; 32],
        encryption_suite: encryption,
        fec_suite: FecScheme::Rs12_10,
        privacy_routes: vec![route],
        neural_bundle: Some(neural),
        audio_summary: Some(audio_summary),
        public_metadata: metadata,
        capabilities,
        signature: [15; 64],
    };
    case!(ManifestV1, "populated", manifest.clone());
    case!(
        SegmentHeader,
        "populated",
        SegmentHeader {
            segment_number: 42,
            profile: ProfileId::UHD_AI,
            entropy_mode: EntropyMode::RansBundled,
            entropy_tables_checksum: Some([0x40; 32]),
            encryption_suite: encryption,
            layer_bitmap: 0b101,
            chunk_merkle_root: [2; 32],
            chunk_count: 1,
            timeline_start_ns: 1_333_000,
            duration_ns: 40_000_000,
            feedback_hint: feedback,
            content_key_id: 7,
            nonce_salt: [4; 32],
            storage_class: StorageClass::Permanent,
            audio_summary: Some(audio_summary),
            bundle_acceleration: BundleAcceleration::CpuSimd,
        }
    );
    let proof = MerkleProof {
        chunk_id: 1,
        sibling_hashes: vec![[5; 32], [6; 32]],
        directions: vec![false, true],
    };
    case!(MerkleProof, "populated", proof.clone());
    case!(
        DataAvailabilityProof,
        "populated",
        DataAvailabilityProof {
            segment_number: 42,
            chunk_root: [2; 32],
            content_key_id: 7,
            chunk_ids: vec![1],
            merkle_proofs: vec![proof],
            storage_commitment: [7; 32],
            validator_signature: [8; 64],
        }
    );
    case!(ErrorCode, "unknown_chunk", ErrorCode::UnknownChunk);
    case!(ErrorCode, "access_denied", ErrorCode::AccessDenied);
    case!(ErrorCode, "rate_limited", ErrorCode::RateLimited);
    case!(
        ErrorCode,
        "protocol_violation",
        ErrorCode::ProtocolViolation
    );
    let key_update = KeyUpdate {
        session_id: [20; 32],
        suite: encryption,
        protocol_version: 1,
        pub_ephemeral: vec![1, 2, 3, 4],
        key_counter: 3,
        signature: [22; 64],
    };
    case!(KeyUpdate, "populated", key_update.clone());
    let content_key_update = ContentKeyUpdate {
        content_key_id: 7,
        gck_wrapped: vec![0x10, 0x20, 0x30],
        valid_from_segment: 43,
    };
    case!(ContentKeyUpdate, "populated", content_key_update.clone());
    let privacy_update = PrivacyRouteUpdate {
        route_id: [0x44; 32],
        stream_id: [1; 32],
        content_key_id: 7,
        valid_from_segment: 43,
        valid_until_segment: 256,
        exit_token: vec![0xBA, 0xAD, 0xF0, 0x0D],
        soranet: Some(soranet),
    };
    case!(PrivacyRouteUpdate, "populated", privacy_update.clone());
    case!(CapabilityRole, "publisher", CapabilityRole::Publisher);
    case!(CapabilityRole, "viewer", CapabilityRole::Viewer);
    let audio_capability = AudioCapability {
        sample_rates: vec![44_100, 48_000],
        ambisonics: true,
        max_channels: 4,
    };
    case!(AudioCapability, "populated", audio_capability.clone());
    case!(Resolution, "r720p", Resolution::R720p);
    case!(Resolution, "r1080p", Resolution::R1080p);
    case!(Resolution, "r1440p", Resolution::R1440p);
    case!(Resolution, "r2160p", Resolution::R2160p);
    let custom_resolution = ResolutionCustom {
        width: 2_048,
        height: 1_080,
    };
    case!(ResolutionCustom, "populated", custom_resolution);
    case!(Resolution, "custom", Resolution::Custom(custom_resolution));
    let capability_report = CapabilityReport {
        stream_id: [1; 32],
        endpoint_role: CapabilityRole::Publisher,
        protocol_version: 1,
        max_resolution: Resolution::R2160p,
        hdr_supported: true,
        capture_hdr: true,
        neural_bundles: vec!["bundle-v1".into()],
        audio_caps: audio_capability,
        feature_bits: capabilities,
        max_datagram_size: 1_200,
        dplpmtud: true,
    };
    case!(CapabilityReport, "populated", capability_report.clone());
    let capability_ack = CapabilityAck {
        stream_id: [1; 32],
        accepted_version: 1,
        negotiated_features: capabilities,
        max_datagram_size: 1_200,
        dplpmtud: true,
    };
    case!(CapabilityAck, "populated", capability_ack);
    let hint_frame = FeedbackHintFrame {
        stream_id: [1; 32],
        loss_ewma_q16: 1_024,
        latency_gradient_q16: -512,
        observed_rtt_ms: 37,
        report_interval_ms: 250,
        parity_chunks: 2,
    };
    case!(FeedbackHintFrame, "populated", hint_frame);
    let sync = SyncDiagnostics {
        window_ms: 500,
        samples: 96,
        avg_audio_jitter_ms: 4,
        max_audio_jitter_ms: 9,
        avg_av_drift_ms: -3,
        max_av_drift_ms: 11,
        ewma_av_drift_ms: -2,
        violation_count: 1,
    };
    case!(SyncDiagnostics, "populated", sync);
    let receiver = ReceiverReport {
        stream_id: [0xA0; 32],
        latest_segment: 128,
        layer_mask: 0b101,
        measured_throughput_kbps: 2_400,
        rtt_ms: 37,
        loss_percent_x100: 250,
        decoder_buffer_ms: 180,
        active_resolution: Resolution::R1080p,
        hdr_active: true,
        ecn_ce_count: 4,
        jitter_ms: 7,
        delivered_sequence: 9_001,
        parity_applied: 2,
        fec_budget: 3,
        sync_diagnostics: Some(sync),
    };
    case!(ReceiverReport, "populated", receiver);
    let announce = ManifestAnnounceFrame { manifest };
    case!(ManifestAnnounceFrame, "populated", announce.clone());
    let request = ChunkRequestFrame {
        segment: 42,
        chunk_id: 1,
    };
    case!(ChunkRequestFrame, "populated", request);
    let acknowledge = ChunkAcknowledgeFrame {
        segment: 42,
        chunk_id: 1,
    };
    case!(ChunkAcknowledgeFrame, "populated", acknowledge);
    let transport_frame = TransportCapabilitiesFrame {
        endpoint_role: CapabilityRole::Publisher,
        capabilities: transport,
    };
    case!(TransportCapabilitiesFrame, "populated", transport_frame);
    let route_ack = PrivacyRouteAckFrame {
        route_id: [0x44; 32],
    };
    case!(PrivacyRouteAckFrame, "populated", route_ack);
    let error = ControlErrorFrame {
        code: ErrorCode::AccessDenied,
        message: "ticket expired".into(),
    };
    case!(ControlErrorFrame, "populated", error.clone());
    case!(
        ControlFrame,
        "manifest_announce",
        ControlFrame::ManifestAnnounce(Box::new(announce))
    );
    case!(
        ControlFrame,
        "chunk_request",
        ControlFrame::ChunkRequest(request)
    );
    case!(
        ControlFrame,
        "chunk_acknowledge",
        ControlFrame::ChunkAcknowledge(acknowledge)
    );
    case!(
        ControlFrame,
        "transport_capabilities",
        ControlFrame::TransportCapabilities(transport_frame)
    );
    case!(
        ControlFrame,
        "capability_report",
        ControlFrame::CapabilityReport(capability_report)
    );
    case!(
        ControlFrame,
        "capability_ack",
        ControlFrame::CapabilityAck(capability_ack)
    );
    case!(
        ControlFrame,
        "feedback_hint",
        ControlFrame::FeedbackHint(hint_frame)
    );
    case!(
        ControlFrame,
        "receiver_report",
        ControlFrame::ReceiverReport(receiver)
    );
    case!(
        ControlFrame,
        "key_update",
        ControlFrame::KeyUpdate(key_update)
    );
    case!(
        ControlFrame,
        "content_key_update",
        ControlFrame::ContentKeyUpdate(content_key_update)
    );
    case!(
        ControlFrame,
        "privacy_route_update",
        ControlFrame::PrivacyRouteUpdate(privacy_update)
    );
    case!(
        ControlFrame,
        "privacy_route_ack",
        ControlFrame::PrivacyRouteAck(route_ack)
    );
    case!(ControlFrame, "error", ControlFrame::Error(error));
    let audit = TelemetryAuditOutcome {
        trace_id: "trace-42".into(),
        slot_height: 42,
        reviewer: "test-reviewer".into(),
        status: "accepted".into(),
        mitigation_url: Some("https://example.invalid/mitigations/42".into()),
    };
    case!(TelemetryAuditOutcome, "populated", audit.clone());
    let encode = TelemetryEncodeStats {
        segment: 42,
        avg_latency_ms: 7,
        dropped_layers: 0b100,
        avg_audio_jitter_ms: 4,
        max_audio_jitter_ms: 9,
    };
    case!(TelemetryEncodeStats, "populated", encode);
    let decode = TelemetryDecodeStats {
        segment: 42,
        buffer_ms: 180,
        dropped_frames: 2,
        max_decode_queue_ms: 20,
        avg_av_drift_ms: -3,
        max_av_drift_ms: 11,
    };
    case!(TelemetryDecodeStats, "populated", decode);
    let network = TelemetryNetworkStats {
        rtt_ms: 37,
        loss_percent_x100: 250,
        fec_repairs: 5,
        fec_failures: 1,
        datagram_reinjects: 3,
    };
    case!(TelemetryNetworkStats, "populated", network);
    let security = TelemetrySecurityStats {
        suite: encryption,
        rekeys: 3,
        gck_rotations: 2,
        last_content_key_id: Some(7),
        last_content_key_valid_from: Some(43),
    };
    case!(TelemetrySecurityStats, "populated", security);
    let energy = TelemetryEnergyStats {
        segment: 42,
        encoder_milliwatts: 2_400,
        decoder_milliwatts: 1_200,
    };
    case!(TelemetryEnergyStats, "populated", energy);
    case!(TelemetryEvent, "encode", TelemetryEvent::Encode(encode));
    case!(TelemetryEvent, "decode", TelemetryEvent::Decode(decode));
    case!(TelemetryEvent, "network", TelemetryEvent::Network(network));
    case!(
        TelemetryEvent,
        "security",
        TelemetryEvent::Security(security)
    );
    case!(TelemetryEvent, "energy", TelemetryEvent::Energy(energy));
    case!(
        TelemetryEvent,
        "audit_outcome",
        TelemetryEvent::AuditOutcome(audit)
    );
    let group = RansGroupTableV1 {
        width_bits: 2,
        group_size: 4,
        precision_bits: 12,
        frequencies: vec![1_024; 4],
        cumulative: vec![0, 1_024, 2_048, 3_072, 4_096],
    };
    case!(RansGroupTableV1, "populated", group.clone());
    let body = RansTablesBodyV1 {
        seed: 9,
        bundle_width: 2,
        groups: vec![group],
    };
    case!(RansTablesBodyV1, "populated", body.clone());
    // Retain the checksum inside the captured populated value across features.
    // Recomputing it from structural framing would change the fixture payload.
    let checksum: [u8; 32] = [
        0x02, 0x41, 0x69, 0xc5, 0x9b, 0x83, 0x52, 0xfc, 0xad, 0xac, 0x8a, 0xc6, 0xba, 0xdb, 0xbd,
        0x46, 0xb4, 0xbb, 0x7f, 0xb1, 0xaa, 0x51, 0x5a, 0xa6, 0x2b, 0xaa, 0x07, 0x45, 0xec, 0xfa,
        0x01, 0xc8,
    ];
    #[cfg(not(feature = "schema-structural"))]
    {
        use sha2::Digest as _;
        let _flags = crate::core::DecodeFlagsGuard::enter(0);
        let bytes = crate::to_bytes(&body).expect("encode table checksum preimage");
        let computed: [u8; 32] = sha2::Sha256::digest(bytes).into();
        assert_eq!(checksum, computed, "captured nominal-mode table checksum");
    }
    let tables = RansTablesV1 {
        version: 1,
        generated_at: 1_701_234_567,
        generator_commit: "test".into(),
        checksum_sha256: checksum,
        body,
    };
    case!(RansTablesV1, "populated", tables.clone());
    case!(SignatureAlgorithm, "ed25519", SignatureAlgorithm::Ed25519);
    let table_signature = RansTablesSignatureV1 {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key: [0x31; 32],
        signature: [0x32; 64],
    };
    case!(RansTablesSignatureV1, "populated", table_signature);
    case!(
        SignedRansTablesV1,
        "populated",
        SignedRansTablesV1 {
            payload: tables,
            signature: Some(table_signature),
        }
    );
}

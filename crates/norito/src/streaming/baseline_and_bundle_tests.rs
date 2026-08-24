#[test]
#[ignore]
fn dump_baseline_streaming_snapshot() {
    use crate::streaming::chunk;
    let dims = FrameDimensions::new(8, 8);
    let frame_duration_ns = 25_000_000u32;
    let luma = vec![0x55; dims.pixel_count()];
    let frames = vec![
        RawFrame::new(dims, luma.clone()).expect("frame 0"),
        RawFrame::new(dims, luma).expect("frame 1"),
    ];
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frame_duration_ns,
        duration_ns: frame_duration_ns.saturating_mul(frames.len() as u32),
        quantizer: 0,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let segment = encoder
        .encode_segment(5, 1_000_000, 3, &frames, None)
        .expect("encode baseline segment");
    let params = BaselineManifestParams {
        stream_id: demo_hash(0x31),
        protocol_version: 1,
        published_at: 1_703_000_000,
        da_endpoint: "/ip4/127.0.0.1/udp/9100/quic".into(),
        privacy_routes: Vec::new(),
        public_metadata: StreamMetadata {
            title: "NSC Baseline Vector".into(),
            description: Some("Canonical manifest for Norito streaming harness.".into()),
            access_policy_id: None,
            tags: vec!["nsc".into(), "baseline".into()],
        },
        capabilities: CapabilityFlags::from_bits(0b0111),
        signature: demo_signature(0x41),
        fec_suite: FecScheme::Rs12_10,
        neural_bundle: None,
        transport_capabilities_hash: [0u8; 32],
    };
    let manifest = segment.build_manifest(params);
    let chunk_refs: Vec<(u16, &[u8])> = segment
        .descriptors
        .iter()
        .zip(segment.chunks.iter())
        .map(|(descriptor, chunk)| (descriptor.chunk_id, chunk.as_slice()))
        .collect();
    let chunk_commitments = chunk::chunk_commitments(segment.header.segment_number, &chunk_refs);
    let chunk_ids: Vec<u16> = segment
        .descriptors
        .iter()
        .map(|descriptor| descriptor.chunk_id)
        .collect();
    let storage_commitment = chunk::storage_commitment(
        segment.header.segment_number,
        segment.header.content_key_id,
        &segment.header.chunk_merkle_root,
        &chunk_ids,
    )
    .expect("storage commitment");
    let da_root = chunk::data_availability_root(
        segment.header.segment_number,
        segment.header.content_key_id,
        &segment.header.chunk_merkle_root,
        &chunk_ids,
    )
    .expect("da root");
    let manifest_bytes = to_bytes(&manifest).expect("serialize manifest");
    let capabilities = TicketCapabilities::from_bits(
        TicketCapabilities::LIVE | TicketCapabilities::HDR | TicketCapabilities::SPATIAL_AUDIO,
    );
    let ticket_policy = TicketPolicy {
        max_relays: 4,
        allowed_regions: vec!["us".into(), "jp".into()],
        max_bandwidth_kbps: Some(15_000),
    };
    let ticket = StreamingTicket {
        ticket_id: demo_hash(0x44),
        owner: "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE".into(),
        dsid: 7,
        lane_id: 5,
        settlement_bucket: 2_048,
        start_slot: 21_000,
        expire_slot: 24_000,
        prepaid_teu: 120_000,
        chunk_teu: 64,
        fanout_quota: 12,
        key_commitment: demo_hash(0x55),
        nonce: 42,
        contract_sig: demo_signature(0x66),
        commitment: demo_hash(0x77),
        nullifier: demo_hash(0x88),
        proof_id: demo_hash(0x99),
        issued_at: 1_701_234_567,
        expires_at: 1_701_834_567,
        policy: Some(ticket_policy),
        capabilities,
    };
    let ticket_revocation = TicketRevocation {
        ticket_id: demo_hash(0xAA),
        nullifier: demo_hash(0xBB),
        reason_code: 17,
        revocation_signature: demo_signature(0xCC),
    };
    let mut snapshot = norito::json::Map::new();
    snapshot.insert(
        "manifest_template_hex".into(),
        json::to_value(&hex_encode(&manifest_bytes)).expect("json"),
    );
    snapshot.insert(
        "chunk_root".into(),
        json::to_value(&hex_encode(segment.header.chunk_merkle_root)).expect("json"),
    );
    snapshot.insert(
        "chunk_commitments".into(),
        json::to_value(&chunk_commitments.iter().map(hex_encode).collect::<Vec<_>>())
            .expect("json"),
    );
    snapshot.insert(
        "chunk_payloads".into(),
        json::to_value(&segment.chunks.iter().map(hex_encode).collect::<Vec<_>>()).expect("json"),
    );
    snapshot.insert(
        "storage_commitment".into(),
        json::to_value(&hex_encode(storage_commitment)).expect("json"),
    );
    snapshot.insert(
        "da_root".into(),
        json::to_value(&hex_encode(da_root)).expect("json"),
    );
    snapshot.insert(
        "ticket".into(),
        norito::json::Value::from(format!("{ticket:?}")),
    );
    snapshot.insert(
        "ticket_revocation".into(),
        norito::json::Value::from(format!("{ticket_revocation:?}")),
    );
    let json_value = norito::json::Value::Object(snapshot);
    let snapshot_json = json::to_string_pretty(&json_value).expect("json encode");
    println!("{snapshot_json}");
}
#[test]
fn bundle_tables_enforce_max_width() {
    let precision_bits = 12;
    let frequencies_2 = vec![1024u16; 4];
    let cumulative_2 = (0..=4).map(|idx| (idx * 1024) as u32).collect::<Vec<_>>();
    let frequencies_3 = vec![512u16; 8];
    let cumulative_3 = (0..=8).map(|idx| (idx * 512) as u32).collect::<Vec<_>>();
    let body = RansTablesBodyV1 {
        seed: 9,
        bundle_width: 3,
        groups: vec![
            RansGroupTableV1 {
                width_bits: 2,
                group_size: 4,
                precision_bits,
                frequencies: frequencies_2,
                cumulative: cumulative_2,
            },
            RansGroupTableV1 {
                width_bits: 3,
                group_size: 8,
                precision_bits,
                frequencies: frequencies_3,
                cumulative: cumulative_3,
            },
        ],
    };
    let checksum = {
        let _guard = norito_core::DecodeFlagsGuard::enter_with_hint(0, 0);
        let bytes = to_bytes(&body).expect("encode tables");
        let digest = Sha256::digest(bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    };
    let payload = RansTablesV1 {
        version: 1,
        generated_at: 0,
        generator_commit: "test".into(),
        checksum_sha256: checksum,
        body,
    };
    let signed = SignedRansTablesV1 {
        payload,
        signature: None,
    };
    let tables = BundleAnsTables::from_signed_for_tests(&signed).expect("load tables");
    assert_eq!(tables.max_width(), 3);
    assert_eq!(
        tables.freq_len_for_bits_for_tests(2),
        Some(4),
        "2-bit table should have 4 symbols"
    );
    assert_eq!(
        tables.freq_len_for_bits_for_tests(3),
        Some(8),
        "3-bit table should have 8 symbols"
    );
    assert!(
        tables.freq_len_for_bits_for_tests(4).is_none(),
        "should reject widths above bundle limit"
    );
}
fn write_temp_tables_toml(contents: &str) -> std::path::PathBuf {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "norito-bundle-tables-{pid}-{suffix}.toml",
        pid = std::process::id()
    ));
    fs::write(&path, contents).expect("write temp tables toml");
    path
}
#[test]
fn load_bundle_tables_rejects_mangled_payload_string() {
    let payload = r#"{"version""1","generated_at""1765012527","generator_commit""7fa1c4c20a921ae51e6a5fd575cfe8ee06d14877","checksum_sha256""A8C50EF2D4A9E80C0D79AB392B626B62F176B1603963E51CAE1E923B88AE8A06","body":{"bundle_width""3","seed""0","groups":[{"width_bits""2","group_size""4","precision_bits""12","frequencies":["542","1113","1011","1430"],"cumulative":["0","542","1655","2666","4096"]},{"width_bits""3","group_size""8","precision_bits""12","frequencies":["280","262","672","441","818","193","692","738"],"cumulative":["0","280","542","1214","1655","2473","2666","3358","4096"]}]}}"#;
    let toml = format!("payload = '''{payload}'''\nsignature = ''\n");
    let path = write_temp_tables_toml(&toml);
    let result = load_bundle_tables_from_toml(&path);
    std::fs::remove_file(&path).ok();
    assert!(
        result.is_err(),
        "first-release bundle-table syntax must reject repaired or stringified payloads"
    );
}

#[test]
fn load_bundle_tables_rejects_unwrapped_payload_body() {
    let canonical = include_str!("../../../../codec/rans/tables/rans_seed0.toml");
    let unwrapped = canonical
        .replacen("[payload]\n", "", 1)
        .replace("[payload.body]", "[body]")
        .replace("[[payload.body.groups]]", "[[body.groups]]");
    let path = write_temp_tables_toml(&unwrapped);
    let result = load_bundle_tables_from_toml(&path);
    std::fs::remove_file(&path).ok();
    assert!(
        result.is_err(),
        "first-release bundle tables require the SignedRansTablesV1 payload wrapper"
    );
}

#[test]
fn load_bundle_tables_rejects_unknown_v1_fields() {
    let canonical = include_str!("../../../../codec/rans/tables/rans_seed0.toml");
    let extended = canonical.replacen("[payload]\n", "[payload]\nretired_selector = true\n", 1);
    let path = write_temp_tables_toml(&extended);
    let result = load_bundle_tables_from_toml(&path);
    std::fs::remove_file(&path).ok();
    assert!(
        result.is_err(),
        "first-release bundle tables reject unknown payload fields"
    );
}

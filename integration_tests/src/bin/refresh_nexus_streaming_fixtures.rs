//! Regenerate grouped Nexus and streaming golden fixtures from canonical Rust encoders.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use hex::encode as hex_encode;
use iroha_data_model::DomainId;
use iroha_data_model::prelude::{
    AccountId, AssetDefinitionId, AssetId, Burn, InstructionBox, Mint, Numeric, TriggerId,
};
use norito::{
    codec::Encode,
    streaming::{
        CapabilityFlags, EntropyMode, FecScheme, Hash, ManifestV1, Multiaddr, StreamMetadata,
        StreamingTicket, TicketCapabilities, TicketPolicy, TicketRevocation, chunk,
        codec::{
            BaselineEncoder, BaselineEncoderConfig, BaselineManifestParams, EncodedSegment,
            FrameDimensions, RawFrame, load_bundle_tables_from_toml,
        },
    },
    to_bytes,
};

const FIXTURE_PUBLIC_KEY: &str =
    "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";

const BASE_CAPABILITIES: CapabilityFlags = CapabilityFlags::from_bits(
    CapabilityFlags::FEATURE_FEEDBACK_HINTS
        | CapabilityFlags::FEATURE_PRIVACY_PROVIDER
        | CapabilityFlags::FEATURE_ENTROPY_BUNDLED,
);

struct InstructionFixture<'a> {
    file_name: &'a str,
    fixture_id: &'a str,
    description: &'a str,
    instruction: InstructionBox,
}

struct StreamingTestVector {
    manifest: ManifestV1,
    manifest_bytes: Vec<u8>,
    chunk_payloads: Vec<Vec<u8>>,
    chunk_commitments: Vec<Hash>,
    storage_commitment: Hash,
    da_root: Hash,
    ticket: StreamingTicket,
    ticket_revocation: TicketRevocation,
}

impl StreamingTestVector {
    fn snapshot_json(&self) -> Result<String, norito::Error> {
        let manifest_hex = hex_encode(&self.manifest_bytes);
        let chunk_root_hex = hex_encode(self.manifest.chunk_root);
        let chunk_commitments: Vec<String> =
            self.chunk_commitments.iter().map(hex_encode).collect();
        let chunk_payloads: Vec<String> = self.chunk_payloads.iter().map(hex_encode).collect();
        let storage_commitment_hex = hex_encode(self.storage_commitment);
        let da_root_hex = hex_encode(self.da_root);

        let mut map = norito::json::Map::new();
        map.insert(
            "manifest_template_hex".into(),
            norito::json::to_value(&manifest_hex)?,
        );
        map.insert(
            "chunk_root".into(),
            norito::json::to_value(&chunk_root_hex)?,
        );
        map.insert(
            "chunk_commitments".into(),
            norito::json::to_value(&chunk_commitments)?,
        );
        map.insert(
            "chunk_payloads".into(),
            norito::json::to_value(&chunk_payloads)?,
        );
        map.insert(
            "storage_commitment".into(),
            norito::json::to_value(&storage_commitment_hex)?,
        );
        map.insert("da_root".into(), norito::json::to_value(&da_root_hex)?);
        map.insert("ticket".into(), ticket_json_value(&self.ticket)?);
        map.insert(
            "ticket_revocation".into(),
            ticket_revocation_json_value(&self.ticket_revocation),
        );
        norito::json::to_string_pretty(&norito::json::Value::Object(map))
            .map_err(norito::Error::from)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    refresh_norito_instruction_fixtures()?;
    refresh_streaming_snapshot_fixtures()?;
    Ok(())
}

fn refresh_norito_instruction_fixtures() -> Result<(), Box<dyn Error>> {
    for fixture in instruction_fixtures()? {
        let path = norito_instruction_fixture_path(fixture.file_name);
        let document = instruction_fixture_document(&fixture)?;
        write_fixture(&path, &document)?;
        println!("updated {}", path.display());
    }
    Ok(())
}

fn refresh_streaming_snapshot_fixtures() -> Result<(), Box<dyn Error>> {
    let baseline = baseline_test_vector().snapshot_json()?;
    let baseline_path = streaming_fixture_path("baseline.json");
    write_fixture(&baseline_path, &baseline)?;
    println!("updated {}", baseline_path.display());

    let bundled = bundled_test_vector().snapshot_json()?;
    let bundled_path = streaming_fixture_path("bundled.json");
    write_fixture(&bundled_path, &bundled)?;
    println!("updated {}", bundled_path.display());

    Ok(())
}

fn baseline_test_vector() -> StreamingTestVector {
    let segment = baseline_segment(2).1;
    vector_from_segment(&segment)
}

fn bundled_test_vector() -> StreamingTestVector {
    let segment = bundled_segment(2, 4).1;
    vector_from_segment(&segment)
}

fn baseline_segment(frame_count: usize) -> (BaselineEncoderConfig, EncodedSegment, Vec<RawFrame>) {
    assert!(frame_count > 0, "frame_count must be non-zero");
    let dims = FrameDimensions::new(8, 8);
    let frame_duration_ns = 25_000_000u32;
    let mut frames = Vec::with_capacity(frame_count);
    let base_luma = vec![0x55; dims.pixel_count()];
    for _ in 0..frame_count {
        frames.push(RawFrame::new(dims, base_luma.clone()).expect("valid frame"));
    }

    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frame_duration_ns,
        duration_ns: frame_duration_ns
            .saturating_mul(u32::try_from(frame_count).expect("frame count fits u32")),
        quantizer: 0,
        ..BaselineEncoderConfig::default()
    };

    let mut encoder = BaselineEncoder::new(config.clone());
    let segment = encoder
        .encode_segment(5, 1_000_000, 3, &frames, None)
        .expect("encode baseline segment");

    (config, segment, frames)
}

fn bundled_segment(
    frame_count: usize,
    bundle_width: u8,
) -> (BaselineEncoderConfig, EncodedSegment, Vec<RawFrame>) {
    assert!(frame_count > 0, "frame_count must be non-zero");
    let dims = FrameDimensions::new(8, 8);
    let frame_duration_ns = 25_000_000u32;
    let mut frames = Vec::with_capacity(frame_count);
    let base_luma = vec![0x33; dims.pixel_count()];
    for _ in 0..frame_count {
        frames.push(RawFrame::new(dims, base_luma.clone()).expect("valid frame"));
    }

    let bundle_tables =
        load_bundle_tables_from_toml(repo_rans_tables_path()).expect("load bundle tables");
    let max_width = bundle_tables.max_width().max(2);
    let configured_width = bundle_width.clamp(2, max_width);

    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frame_duration_ns,
        duration_ns: frame_duration_ns
            .saturating_mul(u32::try_from(frame_count).expect("frame count fits u32")),
        quantizer: 1,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: configured_width,
        bundle_tables,
        ..BaselineEncoderConfig::default()
    };

    let mut encoder = BaselineEncoder::new(config.clone());
    let segment = encoder
        .encode_segment(6, 2_000_000, 9, &frames, None)
        .expect("encode bundled segment");

    (config, segment, frames)
}

fn vector_from_segment(segment: &EncodedSegment) -> StreamingTestVector {
    let manifest = build_manifest(segment);
    let chunk_refs: Vec<(u16, &[u8])> = segment
        .descriptors
        .iter()
        .zip(segment.chunks.iter())
        .map(|(descriptor, chunk)| (descriptor.chunk_id, chunk.as_slice()))
        .collect();
    let chunk_commitments = chunk::chunk_commitments(segment.header.segment_number, &chunk_refs);

    let chunk_ids: Vec<_> = segment
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

    let manifest_bytes = to_bytes(&manifest).expect("serialize manifest template");
    let capabilities = TicketCapabilities::from_bits(
        TicketCapabilities::LIVE | TicketCapabilities::HDR | TicketCapabilities::SPATIAL_AUDIO,
    );
    let ticket_policy = TicketPolicy {
        max_relays: 4,
        allowed_regions: vec!["us".into(), "jp".into()],
        max_bandwidth_kbps: Some(15_000),
    };
    let ticket = StreamingTicket {
        ticket_id: fill_hash(0x44),
        owner: "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE".into(),
        dsid: 42,
        lane_id: 7,
        capabilities,
        policy: Some(ticket_policy),
        issued_at: 1_703_500_000,
        expires_at: 1_703_800_000,
        settlement_bucket: 1_024,
        start_slot: 10_000,
        expire_slot: 12_000,
        prepaid_teu: 64_000,
        chunk_teu: 32,
        fanout_quota: 4,
        key_commitment: fill_hash(0x45),
        nonce: 42,
        contract_sig: fill_signature(0x46),
        commitment: fill_hash(0x47),
        nullifier: fill_hash(0x48),
        proof_id: fill_hash(0x49),
    };

    let ticket_revocation = TicketRevocation {
        ticket_id: ticket.ticket_id,
        nullifier: ticket.nullifier,
        reason_code: 17,
        revocation_signature: fill_signature(0xCC),
    };

    StreamingTestVector {
        manifest,
        manifest_bytes,
        chunk_payloads: segment.chunks.clone(),
        chunk_commitments,
        storage_commitment,
        da_root,
        ticket,
        ticket_revocation,
    }
}

fn build_manifest(segment: &EncodedSegment) -> ManifestV1 {
    let params = BaselineManifestParams {
        stream_id: fill_hash(0x31),
        protocol_version: 1,
        published_at: 1_703_000_000,
        da_endpoint: Multiaddr::from("/ip4/127.0.0.1/udp/9100/quic"),
        privacy_routes: Vec::new(),
        public_metadata: StreamMetadata {
            title: "NSC Baseline Vector".into(),
            description: Some("Canonical manifest for Norito streaming harness.".into()),
            access_policy_id: None,
            tags: vec!["nsc".into(), "baseline".into()],
        },
        capabilities: BASE_CAPABILITIES,
        signature: fill_signature(0x41),
        fec_suite: FecScheme::Rs12_10,
        neural_bundle: None,
        transport_capabilities_hash: [0u8; 32],
    };

    segment.build_manifest(params)
}

fn instruction_fixture_document(
    fixture: &InstructionFixture<'_>,
) -> Result<String, Box<dyn Error>> {
    let instruction = norito::json::to_value(&fixture.instruction)?;
    let mut bytes = Vec::new();
    fixture.instruction.encode_to(&mut bytes);

    let mut document = norito::json::Map::new();
    document.insert(
        "fixture_id".into(),
        norito::json::Value::from(fixture.fixture_id),
    );
    document.insert(
        "description".into(),
        norito::json::Value::from(fixture.description),
    );
    document.insert("instruction".into(), instruction);
    document.insert(
        "encoded_hex".into(),
        norito::json::Value::from(hex_encode(bytes)),
    );

    Ok(format!(
        "{}\n",
        norito::json::to_string_pretty(&norito::json::Value::Object(document))?
    ))
}

fn ticket_json_value(ticket: &StreamingTicket) -> Result<norito::json::Value, norito::Error> {
    let mut map = norito::json::Map::new();
    map.insert(
        "capabilities".into(),
        norito::json::Value::from(ticket.capabilities.bits()),
    );
    map.insert(
        "chunk_teu".into(),
        norito::json::Value::from(ticket.chunk_teu),
    );
    map.insert("commitment".into(), hex_value(ticket.commitment));
    map.insert("contract_sig".into(), hex_value(ticket.contract_sig));
    map.insert("dsid".into(), norito::json::Value::from(ticket.dsid));
    map.insert(
        "expire_slot".into(),
        norito::json::Value::from(ticket.expire_slot),
    );
    map.insert(
        "expires_at".into(),
        norito::json::Value::from(ticket.expires_at),
    );
    map.insert(
        "fanout_quota".into(),
        norito::json::Value::from(ticket.fanout_quota),
    );
    map.insert(
        "issued_at".into(),
        norito::json::Value::from(ticket.issued_at),
    );
    map.insert("key_commitment".into(), hex_value(ticket.key_commitment));
    map.insert("lane_id".into(), norito::json::Value::from(ticket.lane_id));
    map.insert("nonce".into(), norito::json::Value::from(ticket.nonce));
    map.insert("nullifier".into(), hex_value(ticket.nullifier));
    map.insert(
        "owner".into(),
        norito::json::Value::from(ticket.owner.as_str()),
    );
    map.insert("policy".into(), ticket_policy_json(ticket.policy.as_ref())?);
    map.insert("prepaid_teu".into(), u128_json_value(ticket.prepaid_teu));
    map.insert("proof_id".into(), hex_value(ticket.proof_id));
    map.insert(
        "settlement_bucket".into(),
        norito::json::Value::from(ticket.settlement_bucket),
    );
    map.insert(
        "start_slot".into(),
        norito::json::Value::from(ticket.start_slot),
    );
    map.insert("ticket_id".into(), hex_value(ticket.ticket_id));
    Ok(norito::json::Value::Object(map))
}

fn ticket_policy_json(policy: Option<&TicketPolicy>) -> Result<norito::json::Value, norito::Error> {
    let Some(policy) = policy else {
        return Ok(norito::json::Value::Null);
    };

    let mut map = norito::json::Map::new();
    map.insert(
        "allowed_regions".into(),
        norito::json::to_value(&policy.allowed_regions)?,
    );
    map.insert(
        "max_bandwidth_kbps".into(),
        policy
            .max_bandwidth_kbps
            .map_or(norito::json::Value::Null, |value| {
                norito::json::Value::from(value)
            }),
    );
    map.insert(
        "max_relays".into(),
        norito::json::Value::from(policy.max_relays),
    );
    Ok(norito::json::Value::Object(map))
}

fn ticket_revocation_json_value(revocation: &TicketRevocation) -> norito::json::Value {
    let mut map = norito::json::Map::new();
    map.insert("nullifier".into(), hex_value(revocation.nullifier));
    map.insert(
        "reason_code".into(),
        norito::json::Value::from(revocation.reason_code),
    );
    map.insert(
        "revocation_signature".into(),
        hex_value(revocation.revocation_signature),
    );
    map.insert("ticket_id".into(), hex_value(revocation.ticket_id));
    norito::json::Value::Object(map)
}

fn hex_value(bytes: impl AsRef<[u8]>) -> norito::json::Value {
    norito::json::Value::from(hex_encode(bytes.as_ref()))
}

fn u128_json_value(value: u128) -> norito::json::Value {
    u64::try_from(value).map_or_else(
        |_| norito::json::Value::from(value.to_string()),
        norito::json::Value::from,
    )
}

fn write_fixture(path: &Path, contents: &str) -> Result<(), Box<dyn Error>> {
    let Some(parent) = path.parent() else {
        return Err(format!("fixture path has no parent: {}", path.display()).into());
    };
    fs::create_dir_all(parent)?;
    fs::write(path, contents)?;
    Ok(())
}

fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !dir.join("Cargo.lock").exists() {
        dir = dir
            .parent()
            .expect("workspace root should contain Cargo.lock")
            .to_path_buf();
    }
    dir
}

fn repo_rans_tables_path() -> PathBuf {
    workspace_root().join("codec/rans/tables/rans_seed0.toml")
}

fn repo_root() -> PathBuf {
    workspace_root()
}

fn norito_instruction_fixture_path(file_name: &str) -> PathBuf {
    repo_root()
        .join("fixtures")
        .join("norito_instructions")
        .join(file_name)
}

fn streaming_fixture_path(file_name: &str) -> PathBuf {
    repo_root()
        .join("integration_tests")
        .join("fixtures")
        .join("norito_streaming")
        .join("rans")
        .join(file_name)
}

fn fill_hash(byte: u8) -> Hash {
    [byte; 32]
}

fn fill_signature(byte: u8) -> [u8; 64] {
    [byte; 64]
}

fn instruction_fixtures() -> Result<Vec<InstructionFixture<'static>>, Box<dyn Error>> {
    let asset_id = fixture_asset_id()?;
    let burn_numeric = Numeric::from_str("4")?;
    let burn_fractional = Numeric::from_str("3.1415")?;
    let mint_numeric = Numeric::from_str("4")?;
    let trigger_id = TriggerId::from_str("reconciliation_guard")?;

    Ok(vec![
        InstructionFixture {
            file_name: "burn_asset_numeric.json",
            fixture_id: "burn-asset-numeric-v1",
            description: "Canonical Norito encoding for a Burn::Asset numeric instruction burning 4 units.",
            instruction: Burn::asset_numeric(burn_numeric, asset_id.clone()).into(),
        },
        InstructionFixture {
            file_name: "burn_asset_fractional.json",
            fixture_id: "burn-asset-fractional-v1",
            description: "Canonical Norito encoding for a Burn::Asset fractional instruction burning 3.1415 units.",
            instruction: Burn::asset_numeric(burn_fractional, asset_id.clone()).into(),
        },
        InstructionFixture {
            file_name: "mint_asset_numeric.json",
            fixture_id: "mint-asset-numeric-v1",
            description: "Canonical Norito encoding for a Mint::Asset numeric instruction minting 4 units.",
            instruction: Mint::asset_numeric(mint_numeric, asset_id).into(),
        },
        InstructionFixture {
            file_name: "burn_trigger_repetitions.json",
            fixture_id: "burn-trigger-repetitions-v1",
            description: "Canonical Norito encoding for a Burn::TriggerRepetitions instruction burning 7 repetitions for trigger reconciliation_guard.",
            instruction: Burn::trigger_repetitions(7, trigger_id).into(),
        },
    ])
}

fn fixture_asset_id() -> Result<AssetId, Box<dyn Error>> {
    let public_key = FIXTURE_PUBLIC_KEY.parse()?;
    let account = AccountId::new(public_key);
    let definition = AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal")?,
        "rose".parse()?,
    );
    Ok(AssetId::new(definition, account))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_fixture_document_matches_canonical_hex() {
        let fixture = instruction_fixtures()
            .expect("instruction fixtures")
            .into_iter()
            .find(|fixture| fixture.file_name == "burn_asset_numeric.json")
            .expect("burn fixture present");
        let document = instruction_fixture_document(&fixture).expect("document");
        let value: norito::json::Value = norito::json::from_str(&document).expect("json");
        let object = value.as_object().expect("fixture object");

        let encoded_hex = object
            .get("encoded_hex")
            .and_then(norito::json::Value::as_str)
            .expect("encoded_hex string");
        let mut bytes = Vec::new();
        fixture.instruction.encode_to(&mut bytes);
        assert_eq!(encoded_hex, hex_encode(bytes));
    }

    #[test]
    fn streaming_snapshot_refresh_emits_manifest_template_hex() {
        let snapshot = baseline_test_vector()
            .snapshot_json()
            .expect("baseline snapshot");
        assert!(snapshot.contains("\"manifest_template_hex\""));
        assert!(snapshot.contains("\"chunk_commitments\""));
    }
}

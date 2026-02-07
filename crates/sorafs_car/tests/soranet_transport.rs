use std::sync::Arc;

use sorafs_car::{
    CarBuildPlan, CarWriteStats, CarWriter, chunker_registry,
    gateway::GatewayFetchedManifest,
    multi_fetch::{ChunkReceipt, FetchOutcome, FetchProvider, ProviderId, ProviderReport},
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    ChunkingProfileV1, CouncilSignature, GovernanceProofs, ManifestBuilder, PinPolicy,
};
use sorafs_orchestrator::{
    AnonymityPolicy, FetchSession, ManifestVerificationContext, ManifestVerificationError,
    PolicyReport, PolicyStatus,
};

struct FetchFixture {
    payload: Vec<u8>,
    plan: CarBuildPlan,
    chunker_handle: String,
    manifest: sorafs_manifest::ManifestV1,
    manifest_bytes: Vec<u8>,
    manifest_digest: blake3::Hash,
    car_stats: CarWriteStats,
}

impl FetchFixture {
    fn manifest_id_hex(&self) -> String {
        hex::encode(self.manifest_digest.as_bytes())
    }
}

fn build_fixture() -> FetchFixture {
    let payload_len = 32 * 1024;
    let mut payload = vec![0u8; payload_len];
    for (idx, byte) in payload.iter_mut().enumerate() {
        *byte = idx as u8;
    }

    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("car plan");

    let descriptor = chunker_registry::lookup_by_profile(
        ChunkProfile::DEFAULT,
        chunker_registry::DEFAULT_MULTIHASH_CODE,
    )
    .expect("lookup chunker profile");
    let chunker_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );

    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car bytes");

    let governance = GovernanceProofs {
        council_signatures: vec![CouncilSignature {
            signer: [0x42; 32],
            signature: vec![0x24; 64],
        }],
    };
    let (manifest, manifest_bytes, manifest_digest) =
        build_manifest(&plan, &chunker_handle, &car_stats, governance);

    FetchFixture {
        payload,
        plan,
        chunker_handle,
        manifest,
        manifest_bytes,
        manifest_digest,
        car_stats,
    }
}

fn build_manifest(
    plan: &CarBuildPlan,
    _chunker_handle: &str,
    car_stats: &CarWriteStats,
    governance: GovernanceProofs,
) -> (sorafs_manifest::ManifestV1, Vec<u8>, blake3::Hash) {
    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(sorafs_manifest::DagCodecId(car_stats.dag_codec))
        .chunking_profile(ChunkingProfileV1::from_profile(
            plan.chunk_profile,
            chunker_registry::DEFAULT_MULTIHASH_CODE,
        ))
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: sorafs_manifest::StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(governance)
        .build()
        .expect("manifest");
    let manifest_bytes = manifest.encode().expect("encode manifest");
    let manifest_digest = manifest.digest().expect("manifest digest");
    (manifest, manifest_bytes, manifest_digest)
}

fn build_fetch_session(fixture: &FetchFixture) -> FetchSession {
    let provider_label = "provider-alpha";
    let base_provider = FetchProvider::new(provider_label);
    let provider_id: ProviderId = base_provider.id().clone();
    let provider_arc = Arc::new(base_provider);

    let mut chunks = Vec::with_capacity(fixture.plan.chunks.len());
    let mut receipts = Vec::with_capacity(fixture.plan.chunks.len());
    for (index, chunk) in fixture.plan.chunks.iter().enumerate() {
        let start = chunk.offset as usize;
        let end = start + chunk.length as usize;
        let bytes = fixture.payload[start..end].to_vec();
        receipts.push(ChunkReceipt {
            chunk_index: index,
            provider: provider_id.clone(),
            attempts: 1,
            latency_ms: 10.0,
            bytes: chunk.length,
        });
        chunks.push(bytes);
    }

    let provider_report = ProviderReport {
        provider: provider_arc,
        successes: chunks.len(),
        failures: 0,
        disabled: false,
    };

    let outcome = FetchOutcome {
        chunks,
        chunk_receipts: receipts,
        provider_reports: vec![provider_report],
    };

    let policy_report = PolicyReport {
        policy: AnonymityPolicy::GuardPq,
        effective_policy: AnonymityPolicy::GuardPq,
        total_candidates: 1,
        pq_candidates: 1,
        selected_soranet_total: 1,
        selected_pq: 1,
        status: PolicyStatus::Met,
        fallback_reason: None,
    };

    FetchSession {
        outcome,
        policy_report,
        local_proxy_manifest: None,
        car_verification: None,
        taikai_cache_stats: None,
        taikai_cache_queue: None,
    }
}

#[test]
fn manifest_verification_produces_expected_snapshot() {
    let fixture = build_fixture();
    let mut session = build_fetch_session(&fixture);

    let gateway_manifest = GatewayFetchedManifest {
        manifest_bytes: fixture.manifest_bytes.clone(),
        manifest: fixture.manifest.clone(),
        manifest_digest: fixture.manifest_digest,
        payload_digest: fixture.car_stats.car_payload_digest,
        content_length: fixture.plan.content_length,
        chunk_count: fixture.plan.chunks.len() as u64,
        chunk_profile_handle: fixture.chunker_handle.clone(),
        cache_version: None,
    };

    let context = ManifestVerificationContext::from(&gateway_manifest);
    let verification = session
        .verify_against_manifest(&fixture.plan, context)
        .expect("verification succeeds");

    assert_eq!(
        hex::encode(verification.manifest_digest.as_bytes()),
        fixture.manifest_id_hex()
    );
    assert_eq!(
        hex::encode(verification.manifest_car_digest),
        hex::encode(fixture.car_stats.car_archive_digest.as_bytes())
    );
    assert_eq!(
        verification.manifest_content_length,
        fixture.plan.content_length
    );
    assert_eq!(
        verification.manifest_chunk_count as usize,
        fixture.plan.chunks.len()
    );
    assert_eq!(
        hex::encode(verification.car_stats.car_archive_digest.as_bytes()),
        hex::encode(fixture.car_stats.car_archive_digest.as_bytes())
    );
    assert_eq!(
        verification.manifest_governance.council_signatures.len(),
        1,
        "expected governance proofs to remain intact"
    );
}

#[test]
fn manifest_without_governance_rejects_verification() {
    let fixture = build_fixture();
    let mut session = build_fetch_session(&fixture);

    let (manifest_without_governance, manifest_bytes, manifest_digest) = build_manifest(
        &fixture.plan,
        &fixture.chunker_handle,
        &fixture.car_stats,
        GovernanceProofs::default(),
    );

    let gateway_manifest = GatewayFetchedManifest {
        manifest_bytes,
        manifest: manifest_without_governance,
        manifest_digest,
        payload_digest: fixture.car_stats.car_payload_digest,
        content_length: fixture.plan.content_length,
        chunk_count: fixture.plan.chunks.len() as u64,
        chunk_profile_handle: fixture.chunker_handle.clone(),
        cache_version: None,
    };

    let context = ManifestVerificationContext::from(&gateway_manifest);
    let err = session
        .verify_against_manifest(&fixture.plan, context)
        .expect_err("missing governance must trigger validation failure");
    match err {
        ManifestVerificationError::ManifestValidation(message) => {
            assert!(
                message.contains("council signature"),
                "unexpected validation message: {message}"
            );
        }
        other => panic!("expected manifest validation error, got {other:?}"),
    }
}

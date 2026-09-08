//! DA manifest, commitment, pin-intent, and spool persistence tests.

use super::*;

pub(super) struct ManifestResolutionFixture {
    pub(super) request: DaIngestRequest,
    pub(super) canonical: Vec<u8>,
    pub(super) chunk_store: ChunkStore,
    pub(super) metadata: ExtraMetadata,
    pub(super) rent_policy: DaRentPolicyV1,
}
impl ManifestResolutionFixture {
    pub(super) fn new(request: DaIngestRequest) -> Self {
        let canonical = normalize_payload(&request)
            .expect("normalize payload")
            .into_vec();
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        Self {
            request,
            canonical,
            chunk_store,
            metadata,
            rent_policy: DaRentPolicyV1::default(),
        }
    }
    pub(super) fn resolve(
        &self,
        queued_at_unix: u64,
    ) -> Result<ManifestArtifacts, (StatusCode, String)> {
        self.resolve_with_retention(&self.request.retention_policy, queued_at_unix)
    }
    pub(super) fn resolve_with_retention(
        &self,
        retention_policy: &RetentionPolicy,
        queued_at_unix: u64,
    ) -> Result<ManifestArtifacts, (StatusCode, String)> {
        resolve_manifest(
            &self.request,
            &self.chunk_store,
            self.canonical.as_slice(),
            &self.metadata,
            retention_policy,
            queued_at_unix,
            &self.rent_policy,
        )
    }
}
pub(super) fn resolved_manifest_fixture(
    request: DaIngestRequest,
    queued_at_unix: u64,
    expectation: &str,
) -> (ManifestResolutionFixture, ManifestArtifacts) {
    let fixture = ManifestResolutionFixture::new(request);
    let artifacts = fixture.resolve(queued_at_unix).expect(expectation);
    (fixture, artifacts)
}
fn commitment_record_fixture(
    fixture: &ManifestResolutionFixture,
    manifest: &ManifestArtifacts,
    queued_at_unix: u64,
) -> (Vec<u8>, DaCommitmentRecord) {
    let mut pdp_commitment = sample_pdp_commitment_for_tests();
    pdp_commitment.manifest_digest = *manifest.manifest_hash.as_bytes();
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let receipt = build_receipt(
        &checked_random_keypair(),
        &fixture.request,
        queued_at_unix,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        pdp_bytes.clone(),
        manifest.manifest.rent_quote.clone(),
        stripe_layout_from_manifest(&manifest.manifest),
    )
    .expect("build receipt");
    let record = build_da_commitment_record(
        &fixture.request,
        manifest,
        &fixture.request.retention_policy,
        &receipt.operator_signature,
        &pdp_bytes,
        DaProofScheme::MerkleSha256,
    );
    (pdp_bytes, record)
}
#[test]
fn persist_manifest_for_sorafs_writes_and_is_idempotent() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_000_555, "manifest");
    let request = &fixture.request;
    let first_path = persistence::persist_manifest_for_sorafs(
        manifest_dir,
        &manifest.encoded,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist manifest")
    .expect("spool path");
    let ticket_hex = hex::encode(manifest.storage_ticket.as_bytes());
    assert_eq!(
        first_path,
        manifest_dir
            .join("artifacts")
            .join(&ticket_hex[..2])
            .join(&ticket_hex)
            .join("manifest.norito"),
        "manifest persistence must use the direct sharded ticket index"
    );
    let bytes = fs::read(&first_path).expect("read manifest file");
    assert_eq!(bytes, manifest.encoded);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&first_path)
            .expect("read manifest permissions")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o077,
            0,
            "persisted DA artifacts must not be accessible by group or other users"
        );
        let directory_mode = fs::metadata(first_path.parent().expect("ticket artifact directory"))
            .expect("read ticket directory permissions")
            .permissions()
            .mode();
        assert_eq!(
            directory_mode & 0o077,
            0,
            "ticket artifact directories must not be accessible by group or other users"
        );
    }
    let second_path = persistence::persist_manifest_for_sorafs(
        manifest_dir,
        &manifest.encoded,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist manifest idempotent")
    .expect("spool path");
    assert_eq!(first_path, second_path);
}
#[test]
fn persist_pdp_commitment_writes_and_is_idempotent() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_000_777, "manifest");
    let request = &fixture.request;
    let commitment = compute_pdp_commitment(
        &manifest.manifest_hash,
        &manifest.manifest,
        &fixture.chunk_store,
        fixture.canonical.as_slice(),
        1_701_000_777,
    )
    .expect("commitment");
    let first_path = persistence::persist_pdp_commitment(
        manifest_dir,
        &commitment,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist commitment")
    .expect("spool path");
    let ticket_hex = hex::encode(manifest.storage_ticket.as_bytes());
    assert_eq!(
        first_path,
        manifest_dir
            .join("artifacts")
            .join(&ticket_hex[..2])
            .join(&ticket_hex)
            .join("pdp-commitment.norito"),
        "PDP persistence must use the direct sharded ticket index"
    );
    let bytes = fs::read(&first_path).expect("read commitment file");
    let archived = from_bytes::<PdpCommitmentV1>(&bytes).expect("decode commitment");
    let decoded = PdpCommitmentV1::deserialize(archived);
    assert_eq!(decoded, commitment);
    let second_path = persistence::persist_pdp_commitment(
        manifest_dir,
        &commitment,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist commitment idempotent")
    .expect("spool path");
    assert_eq!(first_path, second_path);
}
#[test]
fn build_da_commitment_record_reflects_artifacts() {
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_500_000, "manifest");
    let request = &fixture.request;
    let (_, record) = commitment_record_fixture(&fixture, &manifest, 1_701_500_000);
    assert_eq!(record.lane_id, request.lane_id);
    assert_eq!(record.epoch, request.epoch);
    assert_eq!(record.sequence, request.sequence);
    assert_eq!(record.client_blob_id, request.client_blob_id);
    assert_eq!(
        record.manifest_hash.as_bytes(),
        manifest.manifest_hash.as_bytes()
    );
    assert_eq!(record.retention_class, request.retention_policy);
    assert_eq!(record.storage_ticket, manifest.storage_ticket);
    assert!(record.proof_digest.is_some(), "expected proof digest");
    assert_eq!(record.proof_scheme, DaProofScheme::MerkleSha256);
}
#[test]
fn persist_da_commitment_record_writes_and_is_idempotent() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_600_000, "manifest");
    let request = &fixture.request;
    let (_, record) = commitment_record_fixture(&fixture, &manifest, 1_701_600_000);
    let first_path = persistence::persist_da_commitment_record(
        manifest_dir,
        &record,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist record")
    .expect("spool path");
    let bytes = fs::read(&first_path).expect("read record file");
    let archived = from_bytes::<DaCommitmentRecord>(&bytes).expect("decode record");
    let decoded = DaCommitmentRecord::deserialize(archived);
    assert_eq!(decoded, record);
    let second_path = persistence::persist_da_commitment_record(
        manifest_dir,
        &record,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist record idempotent")
    .expect("spool path");
    assert_eq!(first_path, second_path);
}
#[test]
fn persist_da_commitment_schedule_entry_writes_bundle() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_600_000, "manifest");
    let request = &fixture.request;
    let (pdp_bytes, record) = commitment_record_fixture(&fixture, &manifest, 1_701_600_000);
    let schedule_path = persistence::persist_da_commitment_schedule_entry(
        manifest_dir,
        &record,
        &pdp_bytes,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist schedule entry")
    .expect("schedule path");
    let bytes = fs::read(&schedule_path).expect("read schedule entry");
    let archived = from_bytes::<persistence::DaCommitmentScheduleEntry>(&bytes)
        .expect("decode schedule entry");
    let decoded = persistence::DaCommitmentScheduleEntry::deserialize(archived);
    assert_eq!(decoded.record, record);
    assert_eq!(decoded.pdp_commitment, pdp_bytes);
}
#[test]
fn persist_da_pin_intent_writes_file() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let mut request = sample_request();
    request.sequence = 42;
    request.metadata.items.push(MetadataEntry::new(
        META_DA_REGISTRY_ALIAS,
        b"sora/docs".to_vec(),
        MetadataVisibility::Public,
    ));
    resign_sample_request(&mut request);
    let (fixture, manifest) = resolved_manifest_fixture(request, 1_701_700_123, "manifest");
    let request = &fixture.request;
    let alias =
        registry_alias_from_metadata(&request.metadata).expect("alias metadata should parse");
    let intent = signed_pin_intent(
        request,
        manifest.storage_ticket,
        ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
        alias,
    );
    let path = persistence::persist_da_pin_intent(
        manifest_dir,
        &intent,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist pin")
    .expect("path");
    let bytes = fs::read(&path).expect("read pin intent");
    let archived = from_bytes::<DaPinIntent>(&bytes).expect("decode pin intent");
    let decoded: DaPinIntent =
        DeserializePayload::try_deserialize(archived).expect("deserialize pin intent");
    assert_eq!(decoded, intent);
    assert_eq!(decoded.alias, Some("sora/docs".to_owned()));
    assert_eq!(decoded.authorization.owner, *ALICE_ID);
    let loaded = persistence::load_da_pin_intent(
        manifest_dir,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("load exact pin intent");
    assert_eq!(loaded, intent);
}

#[test]
fn persist_da_pin_scope_roundtrips_and_rejects_replacement() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_700_124, "manifest");
    let request = &fixture.request;
    let scope = build_da_pin_scope(request, manifest.storage_ticket, manifest.manifest_hash)
        .expect("build exact pin scope");
    let path = persistence::persist_da_pin_scope(
        manifest_dir,
        &scope,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist pin scope")
    .expect("pin-scope path");
    assert!(path.exists());
    let loaded = persistence::load_da_pin_scope(
        manifest_dir,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("load exact pin scope");
    assert_eq!(loaded, scope);

    let mut replacement = scope;
    replacement.alias = Some("forged-replacement".to_owned());
    let error = persistence::persist_da_pin_scope(
        manifest_dir,
        &replacement,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect_err("an existing durable scope cannot be replaced");
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn load_da_pin_intent_rejects_filename_body_tuple_mismatch() {
    let temp_dir = tempdir().expect("temp dir");
    let context = sample_manifest_context_for(BlobClass::TaikaiSegment);
    let request = context.request;
    let manifest = context.artifacts;
    let intent = signed_pin_intent_for_manifest(&request, &manifest);
    let wrong_sequence = request.sequence.saturating_add(1);
    let path = spool_artifact_path_for_key(
        temp_dir.path(),
        "da-pin-intent-",
        request.lane_id,
        request.epoch,
        wrong_sequence,
        &manifest.storage_ticket,
        *manifest.fingerprint.as_bytes(),
    );
    fs::write(&path, to_bytes(&intent).expect("encode pin intent"))
        .expect("write mismatched pin-intent fixture");
    let err = persistence::load_da_pin_intent(
        temp_dir.path(),
        request.lane_id,
        request.epoch,
        wrong_sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect_err("pin-intent filename/body mismatch must reject");
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("does not match its filename"),
        "unexpected pin-intent mismatch error: {err}"
    );
}
fn assert_invalid_input<T>(result: std::io::Result<T>, label: &str) {
    let err = match result {
        Ok(_) => panic!("{label} unexpectedly accepted invalid writer inputs"),
        Err(err) => err,
    };
    assert_eq!(
        err.kind(),
        ErrorKind::InvalidInput,
        "{label} should reject invalid writer inputs: {err}"
    );
}
#[test]
fn persist_spool_artifacts_reject_body_tuple_mismatches() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let request = context.request;
    let manifest = context.artifacts;
    let mut pdp_commitment = sample_pdp_commitment_for_tests();
    pdp_commitment.manifest_digest = *manifest.manifest_hash.as_bytes();
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let record = DaCommitmentRecord::new(
        request.lane_id,
        request.epoch,
        request.sequence,
        request.client_blob_id.clone(),
        ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
        DaProofScheme::MerkleSha256,
        Hash::prehashed(*manifest.chunk_root.as_bytes()),
        Some(Hash::new(&pdp_bytes)),
        request.retention_policy.clone(),
        manifest.storage_ticket,
        Signature::try_from_bytes(&[0x44; 64])
            .expect("checked Torii DA persistence acknowledgement signature fixture"),
    );
    assert_invalid_input(
        persistence::persist_manifest_for_sorafs(
            manifest_dir,
            &manifest.encoded,
            LaneId::new(request.lane_id.as_u32().saturating_add(1)),
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "manifest lane mismatch",
    );
    let mut invalid_pdp = pdp_commitment.clone();
    invalid_pdp.manifest_digest = [0; 32];
    assert_invalid_input(
        persistence::persist_pdp_commitment(
            manifest_dir,
            &invalid_pdp,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "invalid PDP commitment body",
    );
    let mut wrong_fingerprint = *manifest.fingerprint.as_bytes();
    wrong_fingerprint[0] ^= 0xFF;
    assert_invalid_input(
        persistence::persist_pdp_commitment(
            manifest_dir,
            &pdp_commitment,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &ReplayFingerprint::from(wrong_fingerprint),
        ),
        "PDP ticket fingerprint mismatch",
    );
    assert_invalid_input(
        persistence::persist_da_commitment_record(
            manifest_dir,
            &record,
            request.lane_id,
            request.epoch,
            request.sequence.saturating_add(1),
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "commitment sequence mismatch",
    );
    let mut other_pdp = pdp_commitment.clone();
    other_pdp.sealed_at = other_pdp.sealed_at.saturating_add(1);
    let other_pdp_bytes = encode_pdp_commitment_bytes(&other_pdp).expect("encode other PDP");
    assert_invalid_input(
        persistence::persist_da_commitment_schedule_entry(
            manifest_dir,
            &record,
            &other_pdp_bytes,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "schedule PDP digest mismatch",
    );
    let mut wrong_manifest_pdp = pdp_commitment.clone();
    wrong_manifest_pdp.manifest_digest[0] ^= 0xFF;
    let wrong_manifest_pdp_bytes =
        encode_pdp_commitment_bytes(&wrong_manifest_pdp).expect("encode wrong-manifest PDP");
    let mut wrong_manifest_record = record.clone();
    wrong_manifest_record.proof_digest = Some(Hash::new(&wrong_manifest_pdp_bytes));
    assert_invalid_input(
        persistence::persist_da_commitment_schedule_entry(
            manifest_dir,
            &wrong_manifest_record,
            &wrong_manifest_pdp_bytes,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "schedule PDP manifest digest mismatch",
    );
    let intent = signed_pin_intent(
        &request,
        manifest.storage_ticket,
        ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
        None,
    );
    assert_invalid_input(
        persistence::persist_da_pin_intent(
            manifest_dir,
            &intent,
            request.lane_id,
            request.epoch,
            request.sequence.saturating_add(1),
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "pin-intent sequence mismatch",
    );
    assert!(
        fs::read_dir(manifest_dir)
            .expect("read spool dir")
            .next()
            .is_none(),
        "rejected writer inputs must not leave spool artifacts"
    );
}
#[test]
fn persist_spool_artifacts_reject_existing_mismatched_targets() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let request = context.request;
    let manifest = context.artifacts;
    let mut pdp_commitment = sample_pdp_commitment_for_tests();
    pdp_commitment.manifest_digest = *manifest.manifest_hash.as_bytes();
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let signer = checked_random_keypair();
    let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
    let receipt = build_receipt(
        &signer,
        &request,
        1_701_000_999,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        pdp_bytes.clone(),
        manifest.manifest.rent_quote.clone(),
        stripe_layout,
    )
    .expect("build receipt");
    let record = build_da_commitment_record(
        &request,
        &manifest,
        &request.retention_policy,
        &receipt.operator_signature,
        &pdp_bytes,
        DaProofScheme::MerkleSha256,
    );
    let intent = signed_pin_intent(
        &request,
        manifest.storage_ticket,
        ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
        None,
    );
    let fingerprint = *manifest.fingerprint.as_bytes();
    let assert_invalid_data =
        |result: std::io::Result<Option<PathBuf>>, artifact: &str| match result {
            Ok(path) => panic!("{artifact} unexpectedly accepted existing target {path:?}"),
            Err(err) => assert_eq!(
                err.kind(),
                std::io::ErrorKind::InvalidData,
                "{artifact} should reject mismatched existing bytes"
            ),
        };
    let manifest_path = spool_artifact_path_for_key(
        manifest_dir,
        "manifest-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    fs::write(&manifest_path, b"poison-manifest").expect("poison manifest");
    assert_invalid_data(
        persistence::persist_manifest_for_sorafs(
            manifest_dir,
            &manifest.encoded,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "manifest",
    );
    let pdp_path = spool_artifact_path_for_key(
        manifest_dir,
        "pdp-commitment-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    fs::write(&pdp_path, b"poison-pdp").expect("poison pdp");
    assert_invalid_data(
        persistence::persist_pdp_commitment(
            manifest_dir,
            &pdp_commitment,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "pdp",
    );
    let commitment_path = spool_artifact_path_for_key(
        manifest_dir,
        "da-commitment-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    fs::write(&commitment_path, b"poison-commitment").expect("poison commitment");
    assert_invalid_data(
        persistence::persist_da_commitment_record(
            manifest_dir,
            &record,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "commitment",
    );
    let schedule_path = spool_artifact_path_for_key(
        manifest_dir,
        "da-commitment-schedule-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    fs::write(&schedule_path, b"poison-schedule").expect("poison schedule");
    assert_invalid_data(
        persistence::persist_da_commitment_schedule_entry(
            manifest_dir,
            &record,
            &pdp_bytes,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "schedule",
    );
    let pin_path = spool_artifact_path_for_key(
        manifest_dir,
        "da-pin-intent-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    fs::write(&pin_path, b"poison-pin").expect("poison pin");
    assert_invalid_data(
        persistence::persist_da_pin_intent(
            manifest_dir,
            &intent,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "pin",
    );
    let receipt_path = receipt_spool_path(manifest_dir, &receipt, request.sequence, fingerprint);
    fs::write(&receipt_path, b"poison-receipt").expect("poison receipt");
    assert_invalid_data(
        persistence::persist_da_receipt(
            manifest_dir,
            &receipt,
            request.sequence,
            &manifest.fingerprint,
        ),
        "receipt",
    );
}
#[cfg(unix)]
#[test]
fn persist_spool_artifacts_reject_existing_target_symlink() {
    use std::os::unix::fs::symlink;
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let request = context.request;
    let manifest = context.artifacts;
    let fingerprint = *manifest.fingerprint.as_bytes();
    let target = manifest_dir.join("manifest-target.norito");
    fs::write(&target, &manifest.encoded).expect("write symlink target");
    let manifest_path = spool_artifact_path_for_key(
        manifest_dir,
        "manifest-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    symlink(&target, &manifest_path).expect("create manifest artifact symlink");
    let err = persistence::persist_manifest_for_sorafs(
        manifest_dir,
        &manifest.encoded,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect_err("existing manifest target symlink must reject idempotent write");
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert!(
        fs::symlink_metadata(&manifest_path)
            .expect("inspect symlink")
            .file_type()
            .is_symlink(),
        "rejected symlink should be left in place for operator inspection"
    );
    assert_eq!(
        fs::read(&target).expect("read symlink target"),
        manifest.encoded,
        "rejected symlink target should not be modified"
    );
}

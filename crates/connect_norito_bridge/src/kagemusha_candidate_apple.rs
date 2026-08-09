//! Non-shipping physical-Apple-device orchestration for candidate-bound V4 evidence.
//!
//! The two exported phases deliberately call the same candidate C lifecycle
//! boundaries used by the Android lab.  The checkpoint contains public proof
//! results only; note openings remain in the owner-private staged scenario and
//! are decoded again after the XCTest process restart.

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs::{File, OpenOptions},
    io::Read as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::{Path, PathBuf},
    ptr, slice,
    time::Instant,
};

use libc::{c_int, c_uchar, c_ulong};
use norito::json::{Map as JsonMap, Value as JsonValue};
use sha2::{Digest as _, Sha256};

use super::{
    BridgeError, BridgeResult, KAGEMUSHA_RECURSIVE_SPEND_LOCAL_WITNESS_VERSION_V4,
    KagemushaNoteOpeningV2, KagemushaOutputMembershipPathsV4,
    KagemushaRecursiveSpendAppendLocalRequestV4, KagemushaRecursiveSpendInitLocalRequestV4,
    KagemushaRecursiveSpendRedeemLocalRequestV4, KagemushaRecursiveSpendVerifyLocalRequestV4,
    VerifyingKeyId, clear_bridge_output, connect_norito_free,
    connect_norito_kagemusha_recursive_spend_candidate_lab_append_v4,
    connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_begin_v4,
    connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_cancel_v4,
    connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_finalize_v4,
    connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_set_install_v4,
    connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_write_v4,
    connect_norito_kagemusha_recursive_spend_candidate_lab_init_v4,
    connect_norito_kagemusha_recursive_spend_candidate_lab_redeem_v4,
    connect_norito_kagemusha_recursive_spend_candidate_lab_verify_v4,
    decode_canonical_kagemusha_archive, parse_account_id_for_chain,
    require_kagemusha_candidate_evidence_lab_installed_v4,
    validate_kagemusha_recursive_spend_branch_against_installed_v4, write_kagemusha_archive_bridge,
};

use super::kagemusha_candidate_scenario::{
    bytes as scenario_bytes, digest32 as scenario_digest32, load_scenario, positive_decimal,
    read_private_regular, scenario_inventory_sha256,
    validate_kagemusha_candidate_scenario_directory_v1,
};

const APPLE_CHECKPOINT_SCHEMA: &str = "iroha.kagemusha.ios_candidate_lab.checkpoint.v1";
const APPLE_TRANSCRIPT_SCHEMA: &str = "iroha.kagemusha.ios_device_lab.native_transcript.v1";
const APPLE_CHECKPOINT_VERSION: u16 = 1;
const MAX_PATH_BYTES: usize = 4096;
const MAX_CANDIDATE_BYTES: u64 = 1024 * 1024;
const MAX_CHECKPOINT_BYTES: usize = 96 * 1024 * 1024;
const TAIRA_I105_CHAIN_DISCRIMINANT: u16 = 369;
const ARTIFACT_STREAM_BYTES: usize = 1024 * 1024;
const APPLE_RESOURCE_CEILING_BYTES: u64 = 6 * 1024 * 1024 * 1024;
const EXPECTED_DUPLICATE_REJECTION_CODE: c_int = -311;

#[derive(Clone, Debug, norito::Encode, norito::Decode)]
struct AppleCandidateCheckpointV1 {
    schema: String,
    version: u16,
    candidate_record_sha256: [u8; 32],
    candidate_manifest_sha256: [u8; 32],
    native_accepted_identity_sha256: [u8; 32],
    native_accepted_identity: Vec<u8>,
    scenario_inventory_sha256: [u8; 32],
    proof_launch_nonce: [u8; 32],
    proof_process_id: u32,
    resource_ceiling_bytes: u64,
    proof_peak_rss_bytes: u64,
    candidate_install_duration_ns: u64,
    build_init_request_duration_ns: u64,
    init_duration_ns: u64,
    build_append_hop_01_request_duration_ns: u64,
    append_hop_01_duration_ns: u64,
    build_append_hop_02_request_duration_ns: u64,
    append_hop_02_duration_ns: u64,
    init_request: Vec<u8>,
    append_hop_01_request: Vec<u8>,
    append_hop_01_recipient_request: Vec<u8>,
    append_hop_02_request: Vec<u8>,
    append_hop_02_recipient_request: Vec<u8>,
    init_result: Vec<u8>,
    split_hop_01_result: Vec<u8>,
    split_hop_02_result: Vec<u8>,
}

fn fail<T>() -> BridgeResult<T> {
    Err(BridgeError::KagemushaProve)
}

fn checked_duration_ns(started: Instant) -> BridgeResult<u64> {
    u64::try_from(started.elapsed().as_nanos()).map_err(|_| BridgeError::KagemushaProve)
}

fn timed<T>(work: impl FnOnce() -> BridgeResult<T>) -> BridgeResult<(T, u64)> {
    let started = Instant::now();
    let value = work()?;
    Ok((value, checked_duration_ns(started)?))
}

fn peak_rss_bytes() -> BridgeResult<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return fail();
    }
    let raw = unsafe { usage.assume_init() }.ru_maxrss;
    let raw = u64::try_from(raw).map_err(|_| BridgeError::KagemushaProve)?;
    #[cfg(target_vendor = "apple")]
    {
        Ok(raw)
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        raw.checked_mul(1024).ok_or(BridgeError::KagemushaProve)
    }
}

unsafe fn bounded_path(ptr_: *const c_uchar, len: c_ulong) -> BridgeResult<PathBuf> {
    let len = usize::try_from(len).map_err(|_| BridgeError::KagemushaProve)?;
    if ptr_.is_null() || len == 0 || len > MAX_PATH_BYTES {
        return fail();
    }
    let raw = unsafe { slice::from_raw_parts(ptr_, len) };
    if raw.contains(&0) {
        return fail();
    }
    let text = std::str::from_utf8(raw).map_err(|_| BridgeError::KagemushaProve)?;
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return fail();
    }
    Ok(path)
}

unsafe fn nonce32(ptr_: *const c_uchar, len: c_ulong) -> BridgeResult<[u8; 32]> {
    if ptr_.is_null() || len != 32 {
        return fail();
    }
    let nonce: [u8; 32] = unsafe { slice::from_raw_parts(ptr_, 32) }
        .try_into()
        .map_err(|_| BridgeError::KagemushaProve)?;
    if nonce == [0; 32] {
        return fail();
    }
    Ok(nonce)
}

fn decode<T>(payload: &[u8]) -> BridgeResult<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    decode_canonical_kagemusha_archive(payload)
}

fn encode<T: norito::NoritoSerialize>(value: &T) -> BridgeResult<Vec<u8>> {
    norito::to_bytes(value).map_err(|_| BridgeError::KagemushaProve)
}

fn take_ffi_archive<T>(code: c_int, ptr_: *mut c_uchar, len: c_ulong) -> BridgeResult<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if code != 0 || ptr_.is_null() || len == 0 {
        if !ptr_.is_null() {
            connect_norito_free(ptr_);
        }
        return fail();
    }
    let len = usize::try_from(len).map_err(|_| BridgeError::KagemushaProve)?;
    let archive = unsafe { slice::from_raw_parts(ptr_, len) }.to_vec();
    connect_norito_free(ptr_);
    decode(&archive)
}

fn call_init(
    local: &KagemushaRecursiveSpendInitLocalRequestV4,
) -> BridgeResult<iroha_data_model::offline::KagemushaRecursiveSpendInitResultV4> {
    let request = encode(local)?;
    let mut output = ptr::null_mut();
    let mut output_len = 0;
    let code = unsafe {
        connect_norito_kagemusha_recursive_spend_candidate_lab_init_v4(
            request.as_ptr(),
            request.len() as c_ulong,
            &mut output,
            &mut output_len,
        )
    };
    take_ffi_archive(code, output, output_len)
}

fn call_append(
    local: &KagemushaRecursiveSpendAppendLocalRequestV4,
    recipient: &iroha_data_model::offline::KagemushaRecipientPaymentRequestV2,
    verified_at_ms: u64,
) -> BridgeResult<iroha_data_model::offline::KagemushaRecursiveSpendSplitResultV4> {
    let request = encode(local)?;
    let recipient = encode(recipient)?;
    let mut output = ptr::null_mut();
    let mut output_len = 0;
    let code = unsafe {
        connect_norito_kagemusha_recursive_spend_candidate_lab_append_v4(
            request.as_ptr(),
            request.len() as c_ulong,
            recipient.as_ptr(),
            recipient.len() as c_ulong,
            verified_at_ms,
            &mut output,
            &mut output_len,
        )
    };
    take_ffi_archive(code, output, output_len)
}

fn call_verify(
    local: &KagemushaRecursiveSpendVerifyLocalRequestV4,
) -> BridgeResult<iroha_data_model::offline::KagemushaRecursiveSpendVerifyResultV4> {
    let request = encode(local)?;
    let mut output = ptr::null_mut();
    let mut output_len = 0;
    let code = unsafe {
        connect_norito_kagemusha_recursive_spend_candidate_lab_verify_v4(
            request.as_ptr(),
            request.len() as c_ulong,
            &mut output,
            &mut output_len,
        )
    };
    take_ffi_archive(code, output, output_len)
}

fn call_redeem(
    local: &KagemushaRecursiveSpendRedeemLocalRequestV4,
) -> BridgeResult<iroha_data_model::offline::KagemushaRecursiveSpendRedeemBuildResultV4> {
    let request = encode(local)?;
    let mut output = ptr::null_mut();
    let mut output_len = 0;
    let code = unsafe {
        connect_norito_kagemusha_recursive_spend_candidate_lab_redeem_v4(
            request.as_ptr(),
            request.len() as c_ulong,
            &mut output,
            &mut output_len,
        )
    };
    take_ffi_archive(code, output, output_len)
}

fn file_identity(metadata: &std::fs::Metadata) -> (u64, u64, u32, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.nlink(),
        metadata.size(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

fn open_exact_artifact(path: &Path, expected_size: u64) -> BridgeResult<File> {
    let before = std::fs::symlink_metadata(path).map_err(|_| BridgeError::KagemushaProve)?;
    if !before.file_type().is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.size() != expected_size
        || expected_size == 0
    {
        return fail();
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| BridgeError::KagemushaProve)?;
    let opened = file.metadata().map_err(|_| BridgeError::KagemushaProve)?;
    let current = std::fs::symlink_metadata(path).map_err(|_| BridgeError::KagemushaProve)?;
    if file_identity(&before) != file_identity(&opened)
        || file_identity(&before) != file_identity(&current)
    {
        return fail();
    }
    Ok(file)
}

fn cancel_candidate_handles(handles: &[u64]) {
    for handle in handles {
        let _ = connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_cancel_v4(*handle);
    }
}

fn install_candidate_from_directory(
    candidate_path: &Path,
    artifact_root: &Path,
) -> BridgeResult<u64> {
    let started = Instant::now();
    let root_metadata =
        std::fs::symlink_metadata(artifact_root).map_err(|_| BridgeError::KagemushaProve)?;
    if !root_metadata.file_type().is_dir()
        || root_metadata.file_type().is_symlink()
        || artifact_root
            .canonicalize()
            .map_err(|_| BridgeError::KagemushaProve)?
            != artifact_root
    {
        return fail();
    }
    let candidate_bytes = read_private_regular(candidate_path, MAX_CANDIDATE_BYTES)
        .map_err(|_| BridgeError::KagemushaProve)?;
    let candidate: iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4 =
        decode(&candidate_bytes)?;
    candidate
        .validate()
        .map_err(|_| BridgeError::KagemushaProve)?;
    if candidate.manifest.source_repo_dirty
        || candidate.manifest.reviewed_source_closure_descriptor_sha256 == [0; 32]
    {
        return fail();
    }
    let candidate_sha256: [u8; 32] = Sha256::digest(&candidate_bytes).into();
    if candidate
        .sha256()
        .map_err(|_| BridgeError::KagemushaProve)?
        != candidate_sha256
    {
        return fail();
    }
    let descriptors = candidate
        .manifest
        .profiles
        .iter()
        .flat_map(|profile| profile.artifacts.iter())
        .collect::<Vec<_>>();
    if descriptors.len() != 8 {
        return fail();
    }
    let mut handles = Vec::with_capacity(descriptors.len());
    let result = (|| {
        for descriptor in descriptors {
            let file_name = Path::new(&descriptor.file_name);
            if file_name
                .parent()
                .is_some_and(|parent| parent != Path::new(""))
                || file_name.file_name() != Some(OsStr::new(&descriptor.file_name))
            {
                return fail();
            }
            let path = artifact_root.join(file_name);
            if path.parent() != Some(artifact_root) {
                return fail();
            }
            let mut file = open_exact_artifact(&path, descriptor.size_bytes)?;
            let mut handle = 0_u64;
            let begin = unsafe {
                connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_begin_v4(
                    candidate_bytes.as_ptr(),
                    candidate_bytes.len() as c_ulong,
                    candidate_sha256.as_ptr(),
                    candidate_sha256.len() as c_ulong,
                    descriptor.sha256.as_ptr(),
                    descriptor.sha256.len() as c_ulong,
                    &mut handle,
                )
            };
            if begin != 0 || handle == 0 {
                return fail();
            }
            handles.push(handle);
            let mut buffer = vec![0_u8; ARTIFACT_STREAM_BYTES];
            loop {
                let read = file
                    .read(&mut buffer)
                    .map_err(|_| BridgeError::KagemushaProve)?;
                if read == 0 {
                    break;
                }
                let written = unsafe {
                    connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_write_v4(
                        handle,
                        buffer.as_ptr(),
                        read as c_ulong,
                    )
                };
                if written != 0 {
                    return fail();
                }
            }
            let current =
                std::fs::symlink_metadata(&path).map_err(|_| BridgeError::KagemushaProve)?;
            let opened = file.metadata().map_err(|_| BridgeError::KagemushaProve)?;
            if file_identity(&current) != file_identity(&opened) {
                return fail();
            }
            if connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_finalize_v4(handle)
                != 0
            {
                return fail();
            }
        }
        let install = unsafe {
            connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_set_install_v4(
                candidate_bytes.as_ptr(),
                candidate_bytes.len() as c_ulong,
                candidate_sha256.as_ptr(),
                candidate_sha256.len() as c_ulong,
                handles.as_ptr(),
                handles.len() as c_ulong,
            )
        };
        if install != 0 {
            return fail();
        }
        Ok(())
    })();
    if result.is_err() {
        cancel_candidate_handles(&handles);
    }
    result?;
    checked_duration_ns(started)
}

fn candidate_context(
    candidate_path: &Path,
    roster_path: &Path,
    scenario_path: &Path,
) -> BridgeResult<(
    iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4,
    super::kagemusha_candidate_scenario::ScenarioPayloads,
    [u8; 32],
    Vec<u8>,
)> {
    validate_kagemusha_candidate_scenario_directory_v1(
        candidate_path,
        roster_path,
        scenario_path,
        TAIRA_I105_CHAIN_DISCRIMINANT,
    )
    .map_err(|_| BridgeError::KagemushaProve)?;
    let candidate_bytes = read_private_regular(candidate_path, MAX_CANDIDATE_BYTES)
        .map_err(|_| BridgeError::KagemushaProve)?;
    let candidate: iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4 =
        decode(&candidate_bytes)?;
    candidate
        .validate()
        .map_err(|_| BridgeError::KagemushaProve)?;
    if candidate.manifest.source_repo_dirty
        || candidate.manifest.reviewed_source_closure_descriptor_sha256 == [0; 32]
    {
        return fail();
    }
    let candidate_sha: [u8; 32] = Sha256::digest(&candidate_bytes).into();
    if candidate
        .sha256()
        .map_err(|_| BridgeError::KagemushaProve)?
        != candidate_sha
    {
        return fail();
    }
    let installed = require_kagemusha_candidate_evidence_lab_installed_v4()?;
    if installed.candidate != candidate || installed.candidate_sha256 != candidate_sha {
        return fail();
    }
    let files = load_scenario(scenario_path).map_err(|_| BridgeError::KagemushaProve)?;
    let inventory = scenario_inventory_sha256(&files).map_err(|_| BridgeError::KagemushaProve)?;
    let accepted = encode(&installed.accepted_identity)?;
    Ok((candidate, files, inventory, accepted))
}

fn append_local(
    bundle: iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
    provenance: iroha_data_model::offline::KagemushaRecursiveSpendTopUpProvenanceV4,
    opening: KagemushaNoteOpeningV2,
    witness: iroha_data_model::offline::KagemushaNoteMembershipWitnessV2,
    change_opening: KagemushaNoteOpeningV2,
    binding: iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4,
    transfer_commitment: [u8; 32],
    operation_id: [u8; 32],
    block_height: u64,
    output_membership: KagemushaOutputMembershipPathsV4,
) -> KagemushaRecursiveSpendAppendLocalRequestV4 {
    KagemushaRecursiveSpendAppendLocalRequestV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_LOCAL_WITNESS_VERSION_V4,
        previous_inputs: vec![
            iroha_data_model::offline::KagemushaRecursiveSpendAppendInputV4 {
                previous_bundle: bundle,
                topup_provenance: provenance,
            },
        ],
        input_openings: vec![opening],
        input_membership_witnesses: vec![witness],
        change_opening: Some(change_opening),
        output_artifact_binding: binding,
        transfer_verifier_id: VerifyingKeyId::new(
            iroha_core::zk::ZK_BACKEND_HALO2_IPA,
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2,
        ),
        transfer_verifier_commitment: transfer_commitment,
        operation_id,
        block_height,
        output_membership,
    }
}

fn proof_phase(
    candidate_path: &Path,
    roster_path: &Path,
    artifact_root: &Path,
    scenario_path: &Path,
    launch_nonce: [u8; 32],
) -> BridgeResult<Vec<u8>> {
    let candidate_install_duration_ns =
        install_candidate_from_directory(candidate_path, artifact_root)?;
    let (candidate, files, scenario_inventory, accepted_identity) =
        candidate_context(candidate_path, roster_path, scenario_path)?;
    let anchor: iroha_data_model::offline::KagemushaRecursiveSpendTopUpAnchorV4 = decode(
        scenario_bytes(&files, "init-top-up-anchor-v4.norito")
            .map_err(|_| BridgeError::KagemushaProve)?,
    )?;
    let init_local = KagemushaRecursiveSpendInitLocalRequestV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_LOCAL_WITNESS_VERSION_V4,
        request: iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV4 {
            artifact_binding: anchor.artifact_binding.clone(),
            topup_anchor: anchor,
            topup_finality_proof: decode(
                scenario_bytes(&files, "init-top-up-finality-proof-v2.norito")
                    .map_err(|_| BridgeError::KagemushaProve)?,
            )?,
            topup_finality_roster_artifact: decode(
                scenario_bytes(&files, "init-top-up-finality-roster-artifact-v2.norito")
                    .map_err(|_| BridgeError::KagemushaProve)?,
            )?,
        },
        opening: decode(
            scenario_bytes(&files, "init-opening-v2.norito")
                .map_err(|_| BridgeError::KagemushaProve)?,
        )?,
        output_membership: decode(
            scenario_bytes(&files, "init-output-membership-v4.norito")
                .map_err(|_| BridgeError::KagemushaProve)?,
        )?,
    };
    let (init_request, build_init_request_duration_ns) = timed(|| encode(&init_local))?;
    let (init, init_duration_ns) = timed(|| call_init(&init_local))?;
    init.validate_for_request(&init_local.request)
        .map_err(|_| BridgeError::KagemushaProve)?;

    let transfer_commitment = scenario_digest32(&files, "transfer-verifier-commitment-v2.bin")
        .map_err(|_| BridgeError::KagemushaProve)?;
    let request_one: iroha_data_model::offline::KagemushaRecipientPaymentRequestV2 = decode(
        scenario_bytes(&files, "append-hop-01-recipient-request-v2.norito")
            .map_err(|_| BridgeError::KagemushaProve)?,
    )?;
    let append_one = append_local(
        init.bundle.clone(),
        init.topup_provenance.clone(),
        decode(
            scenario_bytes(&files, "init-opening-v2.norito")
                .map_err(|_| BridgeError::KagemushaProve)?,
        )?,
        init.membership_witness.clone(),
        decode(
            scenario_bytes(&files, "append-hop-01-change-opening-v2.norito")
                .map_err(|_| BridgeError::KagemushaProve)?,
        )?,
        init.bundle.statement.artifact_binding.clone(),
        transfer_commitment,
        scenario_digest32(&files, "append-hop-01-operation-id.bin")
            .map_err(|_| BridgeError::KagemushaProve)?,
        positive_decimal(&files, "append-hop-01-block-height.txt")
            .map_err(|_| BridgeError::KagemushaProve)?,
        decode(
            scenario_bytes(&files, "append-hop-01-output-membership-v4.norito")
                .map_err(|_| BridgeError::KagemushaProve)?,
        )?,
    );
    let verified_one = positive_decimal(&files, "append-hop-01-verified-at-ms.txt")
        .map_err(|_| BridgeError::KagemushaProve)?;
    let (
        (append_hop_01_request, append_hop_01_recipient_request),
        build_append_hop_01_request_duration_ns,
    ) = timed(|| Ok((encode(&append_one)?, encode(&request_one)?)))?;
    let (split_one, append_hop_01_duration_ns) =
        timed(|| call_append(&append_one, &request_one, verified_one))?;
    split_one
        .validate_public_binding()
        .map_err(|_| BridgeError::KagemushaProve)?;
    let change_one = split_one
        .change_bundle
        .clone()
        .ok_or(BridgeError::KagemushaProve)?;
    let change_one_witness = split_one
        .change_membership_witness
        .clone()
        .ok_or(BridgeError::KagemushaProve)?;
    let change_one_provenance = split_one
        .change_topup_provenance
        .clone()
        .ok_or(BridgeError::KagemushaProve)?;
    if init.bundle.statement.proof_step_count != 1
        || init.bundle.statement.peer_hop_count != 0
        || split_one.recipient_bundle.statement.proof_step_count != 2
        || split_one.recipient_bundle.statement.peer_hop_count != 1
        || change_one.statement.proof_step_count != 2
        || change_one.statement.peer_hop_count != 1
    {
        return fail();
    }

    let request_two: iroha_data_model::offline::KagemushaRecipientPaymentRequestV2 = decode(
        scenario_bytes(&files, "append-hop-02-recipient-request-v2.norito")
            .map_err(|_| BridgeError::KagemushaProve)?,
    )?;
    let append_two = append_local(
        change_one,
        change_one_provenance,
        decode(
            scenario_bytes(&files, "append-hop-01-change-opening-v2.norito")
                .map_err(|_| BridgeError::KagemushaProve)?,
        )?,
        change_one_witness,
        decode(
            scenario_bytes(&files, "append-hop-02-change-opening-v2.norito")
                .map_err(|_| BridgeError::KagemushaProve)?,
        )?,
        split_one
            .recipient_bundle
            .statement
            .artifact_binding
            .clone(),
        transfer_commitment,
        scenario_digest32(&files, "append-hop-02-operation-id.bin")
            .map_err(|_| BridgeError::KagemushaProve)?,
        positive_decimal(&files, "append-hop-02-block-height.txt")
            .map_err(|_| BridgeError::KagemushaProve)?,
        decode(
            scenario_bytes(&files, "append-hop-02-output-membership-v4.norito")
                .map_err(|_| BridgeError::KagemushaProve)?,
        )?,
    );
    let verified_two = positive_decimal(&files, "append-hop-02-verified-at-ms.txt")
        .map_err(|_| BridgeError::KagemushaProve)?;
    let (
        (append_hop_02_request, append_hop_02_recipient_request),
        build_append_hop_02_request_duration_ns,
    ) = timed(|| Ok((encode(&append_two)?, encode(&request_two)?)))?;
    let (split_two, append_hop_02_duration_ns) =
        timed(|| call_append(&append_two, &request_two, verified_two))?;
    split_two
        .validate_public_binding()
        .map_err(|_| BridgeError::KagemushaProve)?;
    let final_change = split_two
        .change_bundle
        .as_ref()
        .ok_or(BridgeError::KagemushaProve)?;
    if split_two.recipient_bundle.statement.proof_step_count != 3
        || split_two.recipient_bundle.statement.peer_hop_count != 2
        || final_change.statement.proof_step_count != 3
        || final_change.statement.peer_hop_count != 2
    {
        return fail();
    }
    let total = split_one
        .recipient_bundle
        .statement
        .current_note
        .amount
        .atomic_units
        .checked_add(
            split_two
                .recipient_bundle
                .statement
                .current_note
                .amount
                .atomic_units,
        )
        .and_then(|value| {
            value.checked_add(final_change.statement.current_note.amount.atomic_units)
        })
        .ok_or(BridgeError::KagemushaProve)?;
    if total != init.bundle.statement.current_note.amount.atomic_units {
        return fail();
    }
    let proof_peak_rss_bytes = peak_rss_bytes()?;
    if proof_peak_rss_bytes == 0 || proof_peak_rss_bytes > APPLE_RESOURCE_CEILING_BYTES {
        return fail();
    }
    let manifest = encode(&candidate.manifest)?;
    let checkpoint = AppleCandidateCheckpointV1 {
        schema: APPLE_CHECKPOINT_SCHEMA.to_owned(),
        version: APPLE_CHECKPOINT_VERSION,
        candidate_record_sha256: candidate
            .sha256()
            .map_err(|_| BridgeError::KagemushaProve)?,
        candidate_manifest_sha256: Sha256::digest(manifest).into(),
        native_accepted_identity_sha256: Sha256::digest(&accepted_identity).into(),
        native_accepted_identity: accepted_identity,
        scenario_inventory_sha256: scenario_inventory,
        proof_launch_nonce: launch_nonce,
        proof_process_id: std::process::id(),
        resource_ceiling_bytes: APPLE_RESOURCE_CEILING_BYTES,
        proof_peak_rss_bytes,
        candidate_install_duration_ns,
        build_init_request_duration_ns,
        init_duration_ns,
        build_append_hop_01_request_duration_ns,
        append_hop_01_duration_ns,
        build_append_hop_02_request_duration_ns,
        append_hop_02_duration_ns,
        init_request,
        append_hop_01_request,
        append_hop_01_recipient_request,
        append_hop_02_request,
        append_hop_02_recipient_request,
        init_result: encode(&init)?,
        split_hop_01_result: encode(&split_one)?,
        split_hop_02_result: encode(&split_two)?,
    };
    let archive = encode(&checkpoint)?;
    if archive.is_empty() || archive.len() > MAX_CHECKPOINT_BYTES {
        return fail();
    }
    Ok(archive)
}

struct Branch<'a> {
    bundle: &'a iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
    provenance: &'a iroha_data_model::offline::KagemushaRecursiveSpendTopUpProvenanceV4,
    witness: &'a iroha_data_model::offline::KagemushaNoteMembershipWitnessV2,
}

fn verify_request(
    branch: &Branch<'_>,
    request: iroha_data_model::offline::KagemushaRecipientPaymentRequestV2,
    block_height: u64,
    verified_at_ms: u64,
) -> KagemushaRecursiveSpendVerifyLocalRequestV4 {
    KagemushaRecursiveSpendVerifyLocalRequestV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_LOCAL_WITNESS_VERSION_V4,
        request: iroha_data_model::offline::KagemushaRecursiveSpendVerifyRequestV4 {
            bundle: branch.bundle.clone(),
            recipient_request: request,
            topup_provenance: branch.provenance.clone(),
            maximum_hops: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
            artifact_binding: branch.bundle.statement.artifact_binding.clone(),
            block_height,
            verified_at_ms,
        },
    }
}

fn redeem_request(
    branch: &Branch<'_>,
    opening: KagemushaNoteOpeningV2,
    recipient: &iroha_data_model::account::AccountId,
    verifier_commitment: [u8; 32],
    operation_id: [u8; 32],
    block_height: u64,
) -> KagemushaRecursiveSpendRedeemLocalRequestV4 {
    KagemushaRecursiveSpendRedeemLocalRequestV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_LOCAL_WITNESS_VERSION_V4,
        bundle: branch.bundle.clone(),
        topup_provenance: branch.provenance.clone(),
        input_opening: opening,
        input_membership_witness: branch.witness.clone(),
        recipient: recipient.clone(),
        public_amount: branch.bundle.statement.current_note.amount,
        change_opening: None,
        unshield_verifier_id: VerifyingKeyId::new(
            iroha_core::zk::ZK_BACKEND_HALO2_IPA,
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2,
        ),
        unshield_verifier_commitment: verifier_commitment,
        block_height,
        operation_id,
        change_output_membership: None,
    }
}

fn archive_pair(first: &[u8], second: &[u8]) -> BridgeResult<Vec<u8>> {
    let first_len = u64::try_from(first.len()).map_err(|_| BridgeError::KagemushaProve)?;
    let second_len = u64::try_from(second.len()).map_err(|_| BridgeError::KagemushaProve)?;
    let capacity = 16_usize
        .checked_add(first.len())
        .and_then(|value| value.checked_add(second.len()))
        .ok_or(BridgeError::KagemushaProve)?;
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(&first_len.to_le_bytes());
    output.extend_from_slice(first);
    output.extend_from_slice(&second_len.to_le_bytes());
    output.extend_from_slice(second);
    Ok(output)
}

fn json_string(map: JsonMap) -> BridgeResult<Vec<u8>> {
    let mut output =
        norito::json::to_vec(&JsonValue::Object(map)).map_err(|_| BridgeError::KagemushaProve)?;
    output.push(b'\n');
    Ok(output)
}

fn insert_digest(map: &mut JsonMap, key: &str, digest: [u8; 32]) {
    map.insert(key.to_owned(), JsonValue::from(hex::encode(digest)));
}

fn insert_u64(map: &mut JsonMap, key: &str, value: u64) {
    map.insert(key.to_owned(), JsonValue::from(value));
}

fn causal_event(
    sequence: u64,
    phase: &str,
    operation: &str,
    duration_ns: u64,
    input: &[u8],
    output: &[u8],
    rejected: bool,
) -> JsonValue {
    let mut event = JsonMap::new();
    event.insert("sequence".to_owned(), JsonValue::from(sequence));
    event.insert("phase".to_owned(), JsonValue::from(phase));
    event.insert("operation".to_owned(), JsonValue::from(operation));
    event.insert(
        "outcome".to_owned(),
        JsonValue::from(if rejected { "rejected" } else { "succeeded" }),
    );
    event.insert(
        "duration_nanos".to_owned(),
        JsonValue::from(duration_ns.max(1)),
    );
    insert_digest(&mut event, "input_sha256", Sha256::digest(input).into());
    insert_digest(&mut event, "output_sha256", Sha256::digest(output).into());
    insert_u64(
        &mut event,
        "output_size_bytes",
        u64::try_from(output.len()).unwrap_or(u64::MAX),
    );
    event.insert(
        "rejection_classification".to_owned(),
        if rejected {
            JsonValue::from("duplicate_input")
        } else {
            JsonValue::Null
        },
    );
    event.insert("exception_class".to_owned(), JsonValue::Null);
    event.insert(
        "error_message_sha256".to_owned(),
        if rejected {
            JsonValue::from(hex::encode(Sha256::digest(
                b"duplicate input rejected by native ABI-21/V4",
            )))
        } else {
            JsonValue::Null
        },
    );
    JsonValue::Object(event)
}

fn restart_phase(
    candidate_path: &Path,
    roster_path: &Path,
    artifact_root: &Path,
    scenario_path: &Path,
    checkpoint_bytes: &[u8],
    launch_nonce: [u8; 32],
) -> BridgeResult<Vec<u8>> {
    if checkpoint_bytes.is_empty() || checkpoint_bytes.len() > MAX_CHECKPOINT_BYTES {
        return fail();
    }
    let checkpoint: AppleCandidateCheckpointV1 = decode(checkpoint_bytes)?;
    if checkpoint.schema != APPLE_CHECKPOINT_SCHEMA
        || checkpoint.version != APPLE_CHECKPOINT_VERSION
        || checkpoint.proof_launch_nonce == launch_nonce
        || checkpoint.proof_process_id == std::process::id()
        || checkpoint.resource_ceiling_bytes != APPLE_RESOURCE_CEILING_BYTES
        || checkpoint.native_accepted_identity.is_empty()
        || checkpoint.init_request.is_empty()
        || checkpoint.append_hop_01_request.is_empty()
        || checkpoint.append_hop_01_recipient_request.is_empty()
        || checkpoint.append_hop_02_request.is_empty()
        || checkpoint.append_hop_02_recipient_request.is_empty()
        || checkpoint.native_accepted_identity_sha256
            != <[u8; 32]>::from(Sha256::digest(&checkpoint.native_accepted_identity))
    {
        return fail();
    }
    let candidate_reinstall_duration_ns =
        install_candidate_from_directory(candidate_path, artifact_root)?;
    let (candidate, files, scenario_inventory, accepted_identity) =
        candidate_context(candidate_path, roster_path, scenario_path)?;
    let manifest_archive = encode(&candidate.manifest)?;
    if checkpoint.candidate_record_sha256
        != candidate
            .sha256()
            .map_err(|_| BridgeError::KagemushaProve)?
        || checkpoint.candidate_manifest_sha256
            != <[u8; 32]>::from(Sha256::digest(&manifest_archive))
        || checkpoint.scenario_inventory_sha256 != scenario_inventory
        || checkpoint.native_accepted_identity != accepted_identity
    {
        return fail();
    }
    let (init, restore_init_result_ns): (
        iroha_data_model::offline::KagemushaRecursiveSpendInitResultV4,
        u64,
    ) = timed(|| decode(&checkpoint.init_result))?;
    let init_request: KagemushaRecursiveSpendInitLocalRequestV4 = decode(&checkpoint.init_request)?;
    let (split_one, restore_hop_01_result_ns): (
        iroha_data_model::offline::KagemushaRecursiveSpendSplitResultV4,
        u64,
    ) = timed(|| decode(&checkpoint.split_hop_01_result))?;
    let (split_two, restore_hop_02_result_ns): (
        iroha_data_model::offline::KagemushaRecursiveSpendSplitResultV4,
        u64,
    ) = timed(|| decode(&checkpoint.split_hop_02_result))?;
    split_one
        .validate_public_binding()
        .map_err(|_| BridgeError::KagemushaProve)?;
    split_two
        .validate_public_binding()
        .map_err(|_| BridgeError::KagemushaProve)?;
    let hop_one_change_bundle = split_one
        .change_bundle
        .as_ref()
        .ok_or(BridgeError::KagemushaProve)?;
    let hop_one_change_witness = split_one
        .change_membership_witness
        .as_ref()
        .ok_or(BridgeError::KagemushaProve)?;
    let hop_one_change_provenance = split_one
        .change_topup_provenance
        .as_ref()
        .ok_or(BridgeError::KagemushaProve)?;
    let final_change_bundle = split_two
        .change_bundle
        .as_ref()
        .ok_or(BridgeError::KagemushaProve)?;
    let final_change_witness = split_two
        .change_membership_witness
        .as_ref()
        .ok_or(BridgeError::KagemushaProve)?;
    let final_change_provenance = split_two
        .change_topup_provenance
        .as_ref()
        .ok_or(BridgeError::KagemushaProve)?;
    let recipient_one = Branch {
        bundle: &split_one.recipient_bundle,
        provenance: &split_one.recipient_topup_provenance,
        witness: &split_one.recipient_membership_witness,
    };
    let recipient_two = Branch {
        bundle: &split_two.recipient_bundle,
        provenance: &split_two.recipient_topup_provenance,
        witness: &split_two.recipient_membership_witness,
    };
    let hop_one_change = Branch {
        bundle: hop_one_change_bundle,
        provenance: hop_one_change_provenance,
        witness: hop_one_change_witness,
    };
    let final_change = Branch {
        bundle: final_change_bundle,
        provenance: final_change_provenance,
        witness: final_change_witness,
    };
    let opening_one: KagemushaNoteOpeningV2 = decode(
        scenario_bytes(&files, "append-hop-01-recipient-opening-v2.norito")
            .map_err(|_| BridgeError::KagemushaProve)?,
    )?;
    let opening_two: KagemushaNoteOpeningV2 = decode(
        scenario_bytes(&files, "append-hop-02-recipient-opening-v2.norito")
            .map_err(|_| BridgeError::KagemushaProve)?,
    )?;
    let opening_change: KagemushaNoteOpeningV2 = decode(
        scenario_bytes(&files, "append-hop-02-change-opening-v2.norito")
            .map_err(|_| BridgeError::KagemushaProve)?,
    )?;
    let opening_hop_one_change: KagemushaNoteOpeningV2 = decode(
        scenario_bytes(&files, "append-hop-01-change-opening-v2.norito")
            .map_err(|_| BridgeError::KagemushaProve)?,
    )?;
    let block_one = positive_decimal(&files, "append-hop-01-block-height.txt")
        .map_err(|_| BridgeError::KagemushaProve)?;
    let block_two = positive_decimal(&files, "append-hop-02-block-height.txt")
        .map_err(|_| BridgeError::KagemushaProve)?;
    let installed = require_kagemusha_candidate_evidence_lab_installed_v4()?;
    let ((), validate_init_ns) = timed(|| {
        init.validate_for_request(&init_request.request)
            .map_err(|_| BridgeError::KagemushaProve)
    })?;
    let ((), validate_hop_one_change_ns) = timed(|| {
        validate_kagemusha_recursive_spend_branch_against_installed_v4(
            hop_one_change.bundle,
            hop_one_change.provenance,
            hop_one_change.witness,
            &opening_hop_one_change,
            block_one,
            installed.as_ref(),
        )
        .map(|_| ())
    })?;
    let ((), validate_one_ns) = timed(|| {
        validate_kagemusha_recursive_spend_branch_against_installed_v4(
            recipient_one.bundle,
            recipient_one.provenance,
            recipient_one.witness,
            &opening_one,
            block_one,
            installed.as_ref(),
        )
        .map(|_| ())
    })?;
    let ((), validate_two_ns) = timed(|| {
        validate_kagemusha_recursive_spend_branch_against_installed_v4(
            recipient_two.bundle,
            recipient_two.provenance,
            recipient_two.witness,
            &opening_two,
            block_two,
            installed.as_ref(),
        )
        .map(|_| ())
    })?;
    let ((), validate_change_ns) = timed(|| {
        validate_kagemusha_recursive_spend_branch_against_installed_v4(
            final_change.bundle,
            final_change.provenance,
            final_change.witness,
            &opening_change,
            block_two,
            installed.as_ref(),
        )
        .map(|_| ())
    })?;

    let request_one: iroha_data_model::offline::KagemushaRecipientPaymentRequestV2 = decode(
        scenario_bytes(&files, "append-hop-01-recipient-request-v2.norito")
            .map_err(|_| BridgeError::KagemushaProve)?,
    )?;
    let request_two: iroha_data_model::offline::KagemushaRecipientPaymentRequestV2 = decode(
        scenario_bytes(&files, "append-hop-02-recipient-request-v2.norito")
            .map_err(|_| BridgeError::KagemushaProve)?,
    )?;
    let verify_one_request = verify_request(
        &recipient_one,
        request_one,
        block_one,
        positive_decimal(&files, "append-hop-01-verified-at-ms.txt")
            .map_err(|_| BridgeError::KagemushaProve)?,
    );
    let (verify_one_request_archive, build_verify_one_request_ns) =
        timed(|| encode(&verify_one_request))?;
    let (verify_one, verify_one_ns) = timed(|| call_verify(&verify_one_request))?;
    let verify_two_request = verify_request(
        &recipient_two,
        request_two,
        block_two,
        positive_decimal(&files, "append-hop-02-verified-at-ms.txt")
            .map_err(|_| BridgeError::KagemushaProve)?,
    );
    let (verify_two_request_archive, build_verify_two_request_ns) =
        timed(|| encode(&verify_two_request))?;
    let (verify_two, verify_two_ns) = timed(|| call_verify(&verify_two_request))?;
    if !verify_one.valid
        || !verify_one.chain_admissible
        || !verify_one.lineage_redeemable
        || !verify_one.witnessless_redemption_supported
        || !verify_two.valid
        || !verify_two.chain_admissible
        || !verify_two.lineage_redeemable
        || !verify_two.witnessless_redemption_supported
        || verify_one.summary.hop_count != 1
        || verify_two.summary.hop_count != 2
        || verify_one.summary.proof_step_count != 2
        || verify_two.summary.proof_step_count != 3
    {
        return fail();
    }

    let mut duplicate = KagemushaRecursiveSpendAppendLocalRequestV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_LOCAL_WITNESS_VERSION_V4,
        previous_inputs: vec![
            iroha_data_model::offline::KagemushaRecursiveSpendAppendInputV4 {
                previous_bundle: recipient_one.bundle.clone(),
                topup_provenance: recipient_one.provenance.clone(),
            },
        ],
        input_openings: vec![decode(
            scenario_bytes(&files, "append-hop-01-recipient-opening-v2.norito")
                .map_err(|_| BridgeError::KagemushaProve)?,
        )?],
        input_membership_witnesses: vec![recipient_one.witness.clone()],
        change_opening: None,
        output_artifact_binding: recipient_one.bundle.statement.artifact_binding.clone(),
        transfer_verifier_id: VerifyingKeyId::new(
            iroha_core::zk::ZK_BACKEND_HALO2_IPA,
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2,
        ),
        transfer_verifier_commitment: scenario_digest32(
            &files,
            "transfer-verifier-commitment-v2.bin",
        )
        .map_err(|_| BridgeError::KagemushaProve)?,
        operation_id: scenario_digest32(&files, "duplicate-input-operation-id.bin")
            .map_err(|_| BridgeError::KagemushaProve)?,
        block_height: positive_decimal(&files, "duplicate-input-block-height.txt")
            .map_err(|_| BridgeError::KagemushaProve)?,
        output_membership: decode(
            scenario_bytes(&files, "duplicate-input-output-membership-v4.norito")
                .map_err(|_| BridgeError::KagemushaProve)?,
        )?,
    };
    duplicate
        .previous_inputs
        .push(duplicate.previous_inputs[0].clone());
    duplicate
        .input_openings
        .push(duplicate.input_openings[0].clone());
    duplicate
        .input_membership_witnesses
        .push(duplicate.input_membership_witnesses[0].clone());
    let duplicate_recipient = scenario_bytes(&files, "duplicate-input-recipient-request-v2.norito")
        .map_err(|_| BridgeError::KagemushaProve)?;
    let ((duplicate_request, duplicate_request_pair), build_duplicate_request_ns) = timed(|| {
        let request = encode(&duplicate)?;
        let pair = archive_pair(&request, duplicate_recipient)?;
        Ok((request, pair))
    })?;
    let duplicate_started = Instant::now();
    let mut duplicate_output = ptr::null_mut();
    let mut duplicate_output_len = 0;
    let duplicate_code = unsafe {
        connect_norito_kagemusha_recursive_spend_candidate_lab_append_v4(
            duplicate_request.as_ptr(),
            duplicate_request.len() as c_ulong,
            duplicate_recipient.as_ptr(),
            duplicate_recipient.len() as c_ulong,
            positive_decimal(&files, "duplicate-input-verified-at-ms.txt")
                .map_err(|_| BridgeError::KagemushaProve)?,
            &mut duplicate_output,
            &mut duplicate_output_len,
        )
    };
    let duplicate_rejection_ns = checked_duration_ns(duplicate_started)?;
    if !duplicate_output.is_null() {
        connect_norito_free(duplicate_output);
    }
    if duplicate_code != EXPECTED_DUPLICATE_REJECTION_CODE
        || !duplicate_output.is_null()
        || duplicate_output_len != 0
    {
        return fail();
    }

    let recipient_text = scenario_bytes(&files, "redeem-recipient-account-id.txt")
        .map_err(|_| BridgeError::KagemushaProve)?
        .strip_suffix(b"\n")
        .ok_or(BridgeError::KagemushaProve)?;
    let recipient = parse_account_id_for_chain(
        std::str::from_utf8(recipient_text)
            .map_err(|_| BridgeError::KagemushaProve)?
            .to_owned(),
        TAIRA_I105_CHAIN_DISCRIMINANT,
    )?;
    let unshield = scenario_digest32(&files, "unshield-verifier-commitment-v2.bin")
        .map_err(|_| BridgeError::KagemushaProve)?;
    let redeem_one_request = redeem_request(
        &recipient_one,
        opening_one,
        &recipient,
        unshield,
        scenario_digest32(&files, "redeem-hop-01-operation-id.bin")
            .map_err(|_| BridgeError::KagemushaProve)?,
        positive_decimal(&files, "redeem-hop-01-block-height.txt")
            .map_err(|_| BridgeError::KagemushaProve)?,
    );
    let (redeem_one_request_archive, build_redeem_one_request_ns) =
        timed(|| encode(&redeem_one_request))?;
    let (redeem_one, redeem_one_ns) = timed(|| call_redeem(&redeem_one_request))?;
    let redeem_two_request = redeem_request(
        &recipient_two,
        opening_two,
        &recipient,
        unshield,
        scenario_digest32(&files, "redeem-hop-02-operation-id.bin")
            .map_err(|_| BridgeError::KagemushaProve)?,
        positive_decimal(&files, "redeem-hop-02-block-height.txt")
            .map_err(|_| BridgeError::KagemushaProve)?,
    );
    let (redeem_two_request_archive, build_redeem_two_request_ns) =
        timed(|| encode(&redeem_two_request))?;
    let (redeem_two, redeem_two_ns) = timed(|| call_redeem(&redeem_two_request))?;
    let redeem_change_request = redeem_request(
        &final_change,
        opening_change,
        &recipient,
        unshield,
        scenario_digest32(&files, "redeem-sender-change-operation-id.bin")
            .map_err(|_| BridgeError::KagemushaProve)?,
        positive_decimal(&files, "redeem-sender-change-block-height.txt")
            .map_err(|_| BridgeError::KagemushaProve)?,
    );
    let (redeem_change_request_archive, build_redeem_change_request_ns) =
        timed(|| encode(&redeem_change_request))?;
    let (redeem_change, redeem_change_ns) = timed(|| call_redeem(&redeem_change_request))?;
    for result in [&redeem_one, &redeem_two, &redeem_change] {
        result
            .validate_public_binding()
            .map_err(|_| BridgeError::KagemushaProve)?;
        if result.unsigned.offline_change.is_some()
            || result.offline_change_bundle.is_some()
            || result.offline_change_membership_witness.is_some()
            || result.offline_change_topup_provenance.is_some()
            || result.unsigned.amount != result.unsigned.bundle.statement.current_note.amount
        {
            return fail();
        }
    }
    let redeemed_atomic_units = redeem_one
        .unsigned
        .amount
        .atomic_units
        .checked_add(redeem_two.unsigned.amount.atomic_units)
        .and_then(|value| value.checked_add(redeem_change.unsigned.amount.atomic_units))
        .ok_or(BridgeError::KagemushaProve)?;
    if redeemed_atomic_units != init.bundle.statement.current_note.amount.atomic_units {
        return fail();
    }

    let restart_peak_rss_bytes = peak_rss_bytes()?;
    if restart_peak_rss_bytes == 0 || restart_peak_rss_bytes > APPLE_RESOURCE_CEILING_BYTES {
        return fail();
    }
    let accepted: super::KagemushaCandidateEvidenceLabAcceptedIdentityV2 =
        decode(&accepted_identity)?;
    if accepted.production_capability_observed
        || accepted.source_repo_dirty
        || accepted.candidate_record_sha256 != checkpoint.candidate_record_sha256
        || accepted.candidate_manifest_sha256 != checkpoint.candidate_manifest_sha256
        || accepted.source_commit != candidate.manifest.source_commit
        || accepted.source_tree_sha256 != candidate.manifest.source_tree_sha256
        || candidate.manifest.reviewed_source_closure_descriptor_sha256 == [0; 32]
        || accepted.artifacts.len() != 8
    {
        return fail();
    }
    for (expected_role, artifact) in
        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
            .iter()
            .zip(&accepted.artifacts)
    {
        if artifact.role != *expected_role
            || artifact.framed_size_bytes == 0
            || artifact.payload_size_bytes == 0
            || artifact.framed_sha256 == [0; 32]
            || artifact.payload_sha256 == [0; 32]
        {
            return fail();
        }
    }

    let initial_atomic_units = init.bundle.statement.current_note.amount.atomic_units;
    let first_recipient_atomic_units = recipient_one
        .bundle
        .statement
        .current_note
        .amount
        .atomic_units;
    let second_recipient_atomic_units = recipient_two
        .bundle
        .statement
        .current_note
        .amount
        .atomic_units;
    let sender_change_atomic_units = final_change
        .bundle
        .statement
        .current_note
        .amount
        .atomic_units;
    if first_recipient_atomic_units
        .checked_add(second_recipient_atomic_units)
        .and_then(|value| value.checked_add(sender_change_atomic_units))
        != Some(initial_atomic_units)
        || redeemed_atomic_units != initial_atomic_units
    {
        return fail();
    }

    let candidate_record_bytes = read_private_regular(candidate_path, MAX_CANDIDATE_BYTES)
        .map_err(|_| BridgeError::KagemushaProve)?;
    let append_hop_01_request_pair = archive_pair(
        &checkpoint.append_hop_01_request,
        &checkpoint.append_hop_01_recipient_request,
    )?;
    let append_hop_02_request_pair = archive_pair(
        &checkpoint.append_hop_02_request,
        &checkpoint.append_hop_02_recipient_request,
    )?;
    let verify_one_result_archive = encode(&verify_one)?;
    let verify_two_result_archive = encode(&verify_two)?;
    let redeem_one_result_archive = encode(&redeem_one)?;
    let redeem_two_result_archive = encode(&redeem_two)?;
    let redeem_change_result_archive = encode(&redeem_change)?;
    let initial_bundle_archive = encode(&init.bundle)?;
    let hop_one_change_bundle_archive = encode(hop_one_change.bundle)?;
    let recipient_one_bundle_archive = encode(recipient_one.bundle)?;
    let recipient_two_bundle_archive = encode(recipient_two.bundle)?;
    let final_change_bundle_archive = encode(final_change.bundle)?;
    let duplicate_code_bytes = duplicate_code.to_le_bytes();

    let causal_events = vec![
        causal_event(
            1,
            "proof_launch",
            "candidate_install",
            checkpoint.candidate_install_duration_ns,
            &candidate_record_bytes,
            &checkpoint.native_accepted_identity,
            false,
        ),
        causal_event(
            2,
            "proof_launch",
            "build_init_request",
            checkpoint.build_init_request_duration_ns,
            &scenario_inventory,
            &checkpoint.init_request,
            false,
        ),
        causal_event(
            3,
            "proof_launch",
            "init",
            checkpoint.init_duration_ns,
            &checkpoint.init_request,
            &checkpoint.init_result,
            false,
        ),
        causal_event(
            4,
            "proof_launch",
            "build_append_hop_01_request",
            checkpoint.build_append_hop_01_request_duration_ns,
            &checkpoint.init_result,
            &append_hop_01_request_pair,
            false,
        ),
        causal_event(
            5,
            "proof_launch",
            "append_hop_01",
            checkpoint.append_hop_01_duration_ns,
            &append_hop_01_request_pair,
            &checkpoint.split_hop_01_result,
            false,
        ),
        causal_event(
            6,
            "proof_launch",
            "build_append_hop_02_request",
            checkpoint.build_append_hop_02_request_duration_ns,
            &checkpoint.split_hop_01_result,
            &append_hop_02_request_pair,
            false,
        ),
        causal_event(
            7,
            "proof_launch",
            "append_hop_02",
            checkpoint.append_hop_02_duration_ns,
            &append_hop_02_request_pair,
            &checkpoint.split_hop_02_result,
            false,
        ),
        causal_event(
            8,
            "restart_launch",
            "candidate_reinstall_after_process_restart",
            candidate_reinstall_duration_ns,
            &candidate_record_bytes,
            &accepted_identity,
            false,
        ),
        causal_event(
            9,
            "restart_launch",
            "restore_init_result_after_restart",
            restore_init_result_ns,
            checkpoint_bytes,
            &checkpoint.init_result,
            false,
        ),
        causal_event(
            10,
            "restart_launch",
            "restore_hop_01_result_after_restart",
            restore_hop_01_result_ns,
            checkpoint_bytes,
            &checkpoint.split_hop_01_result,
            false,
        ),
        causal_event(
            11,
            "restart_launch",
            "restore_hop_02_result_after_restart",
            restore_hop_02_result_ns,
            checkpoint_bytes,
            &checkpoint.split_hop_02_result,
            false,
        ),
        causal_event(
            12,
            "restart_launch",
            "validate_init_branch_after_restart",
            validate_init_ns,
            &checkpoint.init_result,
            &initial_bundle_archive,
            false,
        ),
        causal_event(
            13,
            "restart_launch",
            "validate_hop_01_change_continuity",
            validate_hop_one_change_ns,
            &checkpoint.split_hop_01_result,
            &hop_one_change_bundle_archive,
            false,
        ),
        causal_event(
            14,
            "restart_launch",
            "validate_hop_01_recipient_branch",
            validate_one_ns,
            &checkpoint.split_hop_01_result,
            &recipient_one_bundle_archive,
            false,
        ),
        causal_event(
            15,
            "restart_launch",
            "validate_hop_02_recipient_branch",
            validate_two_ns,
            &checkpoint.split_hop_02_result,
            &recipient_two_bundle_archive,
            false,
        ),
        causal_event(
            16,
            "restart_launch",
            "validate_sender_change_branch",
            validate_change_ns,
            &checkpoint.split_hop_02_result,
            &final_change_bundle_archive,
            false,
        ),
        causal_event(
            17,
            "restart_launch",
            "build_verify_first_recipient_proof_request",
            build_verify_one_request_ns,
            &recipient_one_bundle_archive,
            &verify_one_request_archive,
            false,
        ),
        causal_event(
            18,
            "restart_launch",
            "verify_first_recipient_proof",
            verify_one_ns,
            &verify_one_request_archive,
            &verify_one_result_archive,
            false,
        ),
        causal_event(
            19,
            "restart_launch",
            "build_verify_multi_hop_recipient_proof_request",
            build_verify_two_request_ns,
            &recipient_two_bundle_archive,
            &verify_two_request_archive,
            false,
        ),
        causal_event(
            20,
            "restart_launch",
            "verify_multi_hop_recipient_proof",
            verify_two_ns,
            &verify_two_request_archive,
            &verify_two_result_archive,
            false,
        ),
        causal_event(
            21,
            "restart_launch",
            "build_duplicate_input_request_from_observed_branch",
            build_duplicate_request_ns,
            &recipient_one_bundle_archive,
            &duplicate_request_pair,
            false,
        ),
        causal_event(
            22,
            "restart_launch",
            "duplicate_input_rejection",
            duplicate_rejection_ns,
            &duplicate_request_pair,
            &duplicate_code_bytes,
            true,
        ),
        causal_event(
            23,
            "restart_launch",
            "build_redeem_first_recipient_request",
            build_redeem_one_request_ns,
            &recipient_one_bundle_archive,
            &redeem_one_request_archive,
            false,
        ),
        causal_event(
            24,
            "restart_launch",
            "redeem_first_recipient",
            redeem_one_ns,
            &redeem_one_request_archive,
            &redeem_one_result_archive,
            false,
        ),
        causal_event(
            25,
            "restart_launch",
            "build_redeem_second_recipient_request",
            build_redeem_two_request_ns,
            &recipient_two_bundle_archive,
            &redeem_two_request_archive,
            false,
        ),
        causal_event(
            26,
            "restart_launch",
            "redeem_second_recipient",
            redeem_two_ns,
            &redeem_two_request_archive,
            &redeem_two_result_archive,
            false,
        ),
        causal_event(
            27,
            "restart_launch",
            "build_redeem_sender_change_request",
            build_redeem_change_request_ns,
            &final_change_bundle_archive,
            &redeem_change_request_archive,
            false,
        ),
        causal_event(
            28,
            "restart_launch",
            "redeem_sender_change",
            redeem_change_ns,
            &redeem_change_request_archive,
            &redeem_change_result_archive,
            false,
        ),
    ];
    if causal_events.len() != 28 {
        return fail();
    }

    let artifact_inventory = accepted
        .artifacts
        .iter()
        .map(|artifact| {
            let mut entry = JsonMap::new();
            entry.insert("role".to_owned(), JsonValue::from(artifact.role.clone()));
            insert_u64(&mut entry, "framed_size_bytes", artifact.framed_size_bytes);
            insert_digest(&mut entry, "framed_sha256", artifact.framed_sha256);
            insert_u64(
                &mut entry,
                "payload_size_bytes",
                artifact.payload_size_bytes,
            );
            insert_digest(&mut entry, "payload_sha256", artifact.payload_sha256);
            JsonValue::Object(entry)
        })
        .collect::<Vec<_>>();

    let mut transcript = BTreeMap::new();
    transcript.insert(
        "schema".to_owned(),
        JsonValue::from(APPLE_TRANSCRIPT_SCHEMA),
    );
    transcript.insert("version".to_owned(), JsonValue::from(1_u64));
    transcript.insert("platform".to_owned(), JsonValue::from("ios"));
    transcript.insert("physical_device_required".to_owned(), JsonValue::Bool(true));
    transcript.insert("simulator_accepted".to_owned(), JsonValue::Bool(false));
    transcript.insert("source_repo_dirty".to_owned(), JsonValue::Bool(false));
    transcript.insert(
        "production_capability_observed".to_owned(),
        JsonValue::Bool(false),
    );
    transcript.insert("process_restart_observed".to_owned(), JsonValue::Bool(true));
    transcript.insert("init_succeeded".to_owned(), JsonValue::Bool(true));
    transcript.insert("two_hop_append_succeeded".to_owned(), JsonValue::Bool(true));
    transcript.insert("all_branches_restored".to_owned(), JsonValue::Bool(true));
    transcript.insert(
        "recipient_proofs_verified".to_owned(),
        JsonValue::Bool(true),
    );
    transcript.insert(
        "all_branches_fully_redeemed".to_owned(),
        JsonValue::Bool(true),
    );
    transcript.insert("duplicate_input_rejected".to_owned(), JsonValue::Bool(true));
    transcript.insert(
        "generation".to_owned(),
        JsonValue::from(accepted.generation.clone()),
    );
    transcript.insert(
        "source_commit".to_owned(),
        JsonValue::from(accepted.source_commit.clone()),
    );
    transcript.insert(
        "bridge_abi_version".to_owned(),
        JsonValue::from(u64::from(accepted.bridge_abi_version)),
    );
    insert_digest(
        &mut transcript,
        "source_tree_sha256",
        accepted.source_tree_sha256,
    );
    insert_digest(
        &mut transcript,
        "reviewed_source_closure_descriptor_sha256",
        candidate.manifest.reviewed_source_closure_descriptor_sha256,
    );
    insert_digest(
        &mut transcript,
        "candidate_record_sha256",
        checkpoint.candidate_record_sha256,
    );
    insert_digest(
        &mut transcript,
        "candidate_manifest_sha256",
        checkpoint.candidate_manifest_sha256,
    );
    insert_digest(
        &mut transcript,
        "native_accepted_inventory_sha256",
        accepted.native_accepted_inventory_sha256,
    );
    insert_digest(
        &mut transcript,
        "scenario_inventory_sha256",
        scenario_inventory,
    );
    insert_digest(
        &mut transcript,
        "checkpoint_sha256",
        Sha256::digest(checkpoint_bytes).into(),
    );
    insert_digest(
        &mut transcript,
        "init_result_sha256",
        Sha256::digest(&checkpoint.init_result).into(),
    );
    insert_digest(
        &mut transcript,
        "split_hop_01_result_sha256",
        Sha256::digest(&checkpoint.split_hop_01_result).into(),
    );
    insert_digest(
        &mut transcript,
        "split_hop_02_result_sha256",
        Sha256::digest(&checkpoint.split_hop_02_result).into(),
    );
    insert_digest(
        &mut transcript,
        "proof_launch_nonce_sha256",
        Sha256::digest(checkpoint.proof_launch_nonce).into(),
    );
    insert_digest(
        &mut transcript,
        "restart_launch_nonce_sha256",
        Sha256::digest(launch_nonce).into(),
    );
    insert_u64(
        &mut transcript,
        "proof_process_id",
        u64::from(checkpoint.proof_process_id),
    );
    insert_u64(
        &mut transcript,
        "restart_process_id",
        u64::from(std::process::id()),
    );
    insert_u64(
        &mut transcript,
        "resource_ceiling_bytes",
        APPLE_RESOURCE_CEILING_BYTES,
    );
    insert_u64(
        &mut transcript,
        "proof_peak_rss_bytes",
        checkpoint.proof_peak_rss_bytes,
    );
    insert_u64(
        &mut transcript,
        "restart_peak_rss_bytes",
        restart_peak_rss_bytes,
    );
    insert_u64(
        &mut transcript,
        "candidate_install_duration_ns",
        checkpoint.candidate_install_duration_ns,
    );
    insert_u64(
        &mut transcript,
        "candidate_reinstall_duration_ns",
        candidate_reinstall_duration_ns,
    );
    insert_u64(
        &mut transcript,
        "init_duration_ns",
        checkpoint.init_duration_ns,
    );
    insert_u64(
        &mut transcript,
        "append_hop_01_duration_ns",
        checkpoint.append_hop_01_duration_ns,
    );
    insert_u64(
        &mut transcript,
        "append_hop_02_duration_ns",
        checkpoint.append_hop_02_duration_ns,
    );
    insert_u64(
        &mut transcript,
        "validate_hop_01_duration_ns",
        validate_one_ns,
    );
    insert_u64(
        &mut transcript,
        "validate_hop_02_duration_ns",
        validate_two_ns,
    );
    insert_u64(
        &mut transcript,
        "validate_change_duration_ns",
        validate_change_ns,
    );
    insert_u64(&mut transcript, "verify_hop_01_duration_ns", verify_one_ns);
    insert_u64(&mut transcript, "verify_hop_02_duration_ns", verify_two_ns);
    insert_u64(&mut transcript, "redeem_hop_01_duration_ns", redeem_one_ns);
    insert_u64(&mut transcript, "redeem_hop_02_duration_ns", redeem_two_ns);
    insert_u64(
        &mut transcript,
        "redeem_change_duration_ns",
        redeem_change_ns,
    );
    insert_u64(
        &mut transcript,
        "duplicate_rejection_duration_ns",
        duplicate_rejection_ns,
    );
    insert_u64(&mut transcript, "proof_hops", 2);
    insert_u64(&mut transcript, "exact_operation_count", 28);
    transcript.insert(
        "initial_atomic_units".to_owned(),
        JsonValue::from(initial_atomic_units.to_string()),
    );
    transcript.insert(
        "first_recipient_atomic_units".to_owned(),
        JsonValue::from(first_recipient_atomic_units.to_string()),
    );
    transcript.insert(
        "second_recipient_atomic_units".to_owned(),
        JsonValue::from(second_recipient_atomic_units.to_string()),
    );
    transcript.insert(
        "sender_change_atomic_units".to_owned(),
        JsonValue::from(sender_change_atomic_units.to_string()),
    );
    transcript.insert(
        "redeemed_atomic_units".to_owned(),
        JsonValue::from(redeemed_atomic_units.to_string()),
    );
    transcript.insert(
        "final_unspent_atomic_units".to_owned(),
        JsonValue::from("0"),
    );
    transcript.insert(
        "asset_scale".to_owned(),
        JsonValue::from(u64::from(init.bundle.statement.asset_scale)),
    );
    transcript.insert(
        "duplicate_error_code".to_owned(),
        JsonValue::from(i64::from(duplicate_code)),
    );
    transcript.insert(
        "artifact_inventory".to_owned(),
        JsonValue::Array(artifact_inventory),
    );
    transcript.insert("causal_events".to_owned(), JsonValue::Array(causal_events));
    json_string(transcript)
}

/// Physical-iOS implementation behind the feature- and target-guarded C ABI.
pub(crate) unsafe fn proof_phase_bridge_v1(
    candidate_path_ptr: *const c_uchar,
    candidate_path_len: c_ulong,
    roster_path_ptr: *const c_uchar,
    roster_path_len: c_ulong,
    artifact_root_path_ptr: *const c_uchar,
    artifact_root_path_len: c_ulong,
    scenario_path_ptr: *const c_uchar,
    scenario_path_len: c_ulong,
    launch_nonce_ptr: *const c_uchar,
    launch_nonce_len: c_ulong,
    out_checkpoint_ptr: *mut *mut c_uchar,
    out_checkpoint_len: *mut c_ulong,
) -> c_int {
    clear_bridge_output(out_checkpoint_ptr, out_checkpoint_len);
    let result = (|| {
        let candidate = unsafe { bounded_path(candidate_path_ptr, candidate_path_len) }?;
        let roster = unsafe { bounded_path(roster_path_ptr, roster_path_len) }?;
        let artifact_root =
            unsafe { bounded_path(artifact_root_path_ptr, artifact_root_path_len) }?;
        let scenario = unsafe { bounded_path(scenario_path_ptr, scenario_path_len) }?;
        let nonce = unsafe { nonce32(launch_nonce_ptr, launch_nonce_len) }?;
        let checkpoint = proof_phase(&candidate, &roster, &artifact_root, &scenario, nonce)?;
        unsafe {
            write_kagemusha_archive_bridge(out_checkpoint_ptr, out_checkpoint_len, &checkpoint)
        }
    })();
    result.map_or_else(|error| error.code(), |()| 0)
}

/// Physical-iOS restart implementation behind the guarded C ABI.
pub(crate) unsafe fn restart_phase_bridge_v1(
    candidate_path_ptr: *const c_uchar,
    candidate_path_len: c_ulong,
    roster_path_ptr: *const c_uchar,
    roster_path_len: c_ulong,
    artifact_root_path_ptr: *const c_uchar,
    artifact_root_path_len: c_ulong,
    scenario_path_ptr: *const c_uchar,
    scenario_path_len: c_ulong,
    checkpoint_ptr: *const c_uchar,
    checkpoint_len: c_ulong,
    launch_nonce_ptr: *const c_uchar,
    launch_nonce_len: c_ulong,
    out_transcript_ptr: *mut *mut c_uchar,
    out_transcript_len: *mut c_ulong,
) -> c_int {
    clear_bridge_output(out_transcript_ptr, out_transcript_len);
    let result = (|| {
        let candidate = unsafe { bounded_path(candidate_path_ptr, candidate_path_len) }?;
        let roster = unsafe { bounded_path(roster_path_ptr, roster_path_len) }?;
        let artifact_root =
            unsafe { bounded_path(artifact_root_path_ptr, artifact_root_path_len) }?;
        let scenario = unsafe { bounded_path(scenario_path_ptr, scenario_path_len) }?;
        let nonce = unsafe { nonce32(launch_nonce_ptr, launch_nonce_len) }?;
        let checkpoint_len =
            usize::try_from(checkpoint_len).map_err(|_| BridgeError::KagemushaProve)?;
        if checkpoint_ptr.is_null() || checkpoint_len == 0 || checkpoint_len > MAX_CHECKPOINT_BYTES
        {
            return fail();
        }
        let checkpoint = unsafe { slice::from_raw_parts(checkpoint_ptr, checkpoint_len) };
        let transcript = restart_phase(
            &candidate,
            &roster,
            &artifact_root,
            &scenario,
            checkpoint,
            nonce,
        )?;
        unsafe {
            write_kagemusha_archive_bridge(out_transcript_ptr, out_transcript_len, &transcript)
        }
    })();
    result.map_or_else(|error| error.code(), |()| 0)
}

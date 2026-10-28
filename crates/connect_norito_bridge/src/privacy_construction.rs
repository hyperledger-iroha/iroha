//! Shared construction-time profile, network and resource validation for SDK bridges.
//!
//! This boundary checks canonical transaction inputs. It does not verify proof cryptography,
//! authenticate Torii transport or grant execution authority; callers retain those obligations.

use iroha_core_privacy::privacy_profiles::compiled_privacy_profile_catalog_v1;
use iroha_data_model::{
    isi::privacy::{SubmitPrivacyProofV1, SubmitPrivacyZkX509CertificateV1},
    privacy::{
        PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1, PrivacyCapabilityArchiveValidationStatusV1,
        PrivacyCompiledProfileResultV1, PrivacyCompiledProfileSnapshotV1, PrivacyConsensusLimitsV1,
        PrivacyExact12CapabilityManifestV1, PrivacyProtocolIdV1, PrivacyProtocolLimitsV1,
        PrivacyStatementV1, validate_privacy_capability_archive_v1,
    },
    transaction::signed_attachments::SIGNED_ATTACHMENTS_MAX_ACTIONS_V1,
};
use std::{
    os::raw::{c_int, c_uchar, c_ulong},
    slice,
};

pub(crate) fn construction_frame_limit() -> usize {
    SubmitPrivacyProofV1::canonical_frame_byte_limit_v1()
        .max(SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1())
}

pub(crate) fn tuple_admitted(archive: &[u8], index: usize, network: &[u8; 32]) -> bool {
    if network[31] & 1 != 1 || !validate_privacy_capability_archive_v1(archive).is_valid() {
        return false;
    }
    let Some(protocol) = PrivacyProtocolIdV1::ALL.get(index).copied() else {
        return false;
    };
    let Ok(manifest) = norito::decode_from_bytes::<PrivacyExact12CapabilityManifestV1>(archive)
    else {
        return false;
    };
    if manifest.network_id.as_bytes() != network {
        return false;
    }
    let Ok(catalog) = compiled_privacy_profile_catalog_v1() else {
        return false;
    };
    let (Some(committed), Some(local)) =
        (manifest.protocols.get(index), catalog.protocols.get(index))
    else {
        return false;
    };
    committed.protocol_id == protocol
        && local.protocol_id == protocol
        && committed.is_network_available()
        && committed.compiled_profile == local.compiled_profile
}

pub(crate) fn request_matches_profile(
    request: &SubmitPrivacyZkX509CertificateV1,
    profile: &PrivacyCompiledProfileSnapshotV1,
    protocol_limits: &PrivacyProtocolLimitsV1,
    limits: &PrivacyConsensusLimitsV1,
    network: &[u8; 32],
) -> bool {
    let protocol = PrivacyProtocolIdV1::IrohaZkX509StarkP256V1;
    let context = &request.statement.context;
    profile.validate().is_ok()
        && profile.protocol_id == protocol
        && profile.proof_system_id == protocol.expected_proof_system()
        && profile.engine_id == protocol.expected_engine()
        && context.network_id.as_bytes() == network
        && context.parameter_id == profile.parameter_id
        && context.parameter_digest == profile.parameter_digest
        && context.verifier_digest == profile.verifier_digest
        && context.statement_schema_digest == profile.statement_schema_digest
        && context.engine_manifest_digest == profile.engine_manifest_digest
        && request.validate_proof_free_v1(limits).is_ok()
        && protocol_limits
            .validate_with_ceiling(&profile.protocol_limits)
            .is_ok()
        && protocol_limits
            .validate_statement(&PrivacyStatementV1::IrohaZkX509StarkP256V1(
                request.statement.clone(),
            ))
            .is_ok()
}

pub(crate) fn construction_admitted(
    manifest_archive: &[u8],
    index: usize,
    instruction_archive: &[u8],
    network: &[u8; 32],
) -> bool {
    if instruction_archive.is_empty()
        || instruction_archive.len() > construction_frame_limit()
        || !tuple_admitted(manifest_archive, index, network)
    {
        return false;
    }
    let Ok(manifest) =
        norito::decode_from_bytes::<PrivacyExact12CapabilityManifestV1>(manifest_archive)
    else {
        return false;
    };
    let Some(committed) = manifest.protocols.get(index) else {
        return false;
    };
    let PrivacyCompiledProfileResultV1::Available(profile) = committed.compiled_profile else {
        return false;
    };
    let Some(protocol_limits) = committed.effective_protocol_limits_v1() else {
        return false;
    };
    if committed.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 {
        if instruction_archive.len()
            > SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1()
        {
            return false;
        }
        let base = norito::canonical_decode_limits(instruction_archive.len());
        let limits = norito::DecodeLimits::new(
            base.max_sequence_elements(),
            base.max_field_bytes(),
            base.max_total_elements(),
            base.max_total_allocated_bytes(),
            64,
        );
        let Ok(request) = norito::decode_canonical_with_limits::<SubmitPrivacyZkX509CertificateV1>(
            instruction_archive,
            limits,
        ) else {
            return false;
        };
        return usize::try_from(request.proof_ref.action_index)
            .is_ok_and(|index| index < SIGNED_ATTACHMENTS_MAX_ACTIONS_V1)
            && request_matches_profile(
                &request,
                &profile,
                &protocol_limits,
                &manifest.consensus_policy.current_limits,
                network,
            );
    }
    if instruction_archive.len() > SubmitPrivacyProofV1::canonical_frame_byte_limit_v1() {
        return false;
    }
    let Ok(instruction) = norito::decode_canonical::<SubmitPrivacyProofV1>(instruction_archive)
    else {
        return false;
    };
    instruction.envelope.protocol_id == committed.protocol_id
        && instruction
            .envelope
            .statement
            .context()
            .network_id
            .as_bytes()
            == network
        && instruction
            .envelope
            .validate_against_profile_v1(
                &profile,
                &protocol_limits,
                &manifest.consensus_policy.current_limits,
            )
            .is_ok()
}

/// Validate canonical construction against the selected local profile and current network policy.
///
/// Returns zero for construction validity, otherwise a capability-validation status. Success
/// does not verify proof cryptography or authenticate the manifest's network transport.
///
/// # Safety
/// Every non-null pointer with an accepted nonzero length must reference that many readable
/// bytes for this call. No pointer is retained; all lengths are bounded before reading inputs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iroha_privacy_validate_exact12_submit_proof_construction_v1(
    manifest_ptr: *const c_uchar,
    manifest_len: c_ulong,
    protocol_index: u32,
    instruction_ptr: *const c_uchar,
    instruction_len: c_ulong,
    expected_network_ptr: *const c_uchar,
    expected_network_len: c_ulong,
) -> c_int {
    use PrivacyCapabilityArchiveValidationStatusV1 as Status;
    if manifest_ptr.is_null() || instruction_ptr.is_null() || expected_network_ptr.is_null() {
        return Status::NullPointer.code();
    }
    let (Ok(manifest_len), Ok(instruction_len), Ok(network_len)) = (
        usize::try_from(manifest_len),
        usize::try_from(instruction_len),
        usize::try_from(expected_network_len),
    ) else {
        return Status::ArchiveTooLarge.code();
    };
    if manifest_len == 0 || instruction_len == 0 {
        return Status::Empty.code();
    }
    if manifest_len > PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1
        || instruction_len > construction_frame_limit()
    {
        return Status::ArchiveTooLarge.code();
    }
    if network_len != 32 || protocol_index as usize >= PrivacyProtocolIdV1::ALL.len() {
        return Status::InvalidManifest.code();
    }
    let manifest = unsafe { slice::from_raw_parts(manifest_ptr, manifest_len) };
    let instruction = unsafe { slice::from_raw_parts(instruction_ptr, instruction_len) };
    let network: &[u8; 32] = unsafe { slice::from_raw_parts(expected_network_ptr, 32) }
        .try_into()
        .expect("fixed network length");
    match std::panic::catch_unwind(|| {
        construction_admitted(manifest, protocol_index as usize, instruction, network)
    }) {
        Ok(true) => 0,
        Ok(false) | Err(_) => Status::InvalidManifest.code(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_ffi_rejects_bad_pointers_lengths_and_network_before_decode() {
        let byte = [1u8];
        let ptr = byte.as_ptr();
        let call = |manifest, manifest_len, instruction_len, network_len| unsafe {
            iroha_privacy_validate_exact12_submit_proof_construction_v1(
                manifest,
                manifest_len,
                0,
                ptr,
                instruction_len,
                ptr,
                network_len,
            )
        };
        assert_ne!(call(std::ptr::null(), 1, 1, 32), 0);
        assert_ne!(call(ptr, 0, 1, 32), 0);
        assert_ne!(call(ptr, 1, 0, 32), 0);
        assert_ne!(
            call(
                ptr,
                (PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1 + 1) as c_ulong,
                1,
                32
            ),
            0
        );
        assert_ne!(
            call(ptr, 1, (construction_frame_limit() + 1) as c_ulong, 32),
            0
        );
        assert_ne!(call(ptr, 1, 1, 31), 0);
        let network = [0xA5; 32];
        assert_ne!(
            unsafe {
                iroha_privacy_validate_exact12_submit_proof_construction_v1(
                    ptr,
                    1,
                    0,
                    ptr,
                    1,
                    network.as_ptr(),
                    32,
                )
            },
            0
        );
    }
}

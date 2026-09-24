//! Android's non-authorizing JNI transport for one testnet State-proof observation.

use super::*;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    kagemusha::{
        KAGEMUSHA_WIRE_VERSION_V1, KagemushaHardwareTransitionSelectionV1, KagemushaOperationKindV1,
    },
};
use sha2::{Digest as _, Sha256};

// Every synthetic identity has this diagnostic domain. None is a release, credential,
// policy root, or monetary transition issued by governance or an enrolled wallet.
const PIXEL6_DIAGNOSTIC_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:pixel6-testnet-diagnostic-selection\0";
const PIXEL6_DIAGNOSTIC_OWNER_MAX_BYTES_V1: usize = 2_048;
const PIXEL6_DIAGNOSTIC_SELECTION_BYTES_V1: usize = 460;
const PIXEL6_DIAGNOSTIC_CONTRACT_V1: [jni::sys::jint; 4] = [
    1,
    32,
    PIXEL6_DIAGNOSTIC_OWNER_MAX_BYTES_V1 as jni::sys::jint,
    PIXEL6_DIAGNOSTIC_SELECTION_BYTES_V1 as jni::sys::jint,
];

fn pixel6_diagnostic_field_v1(network: &[u8; 32], owner: &[u8], tag: u8) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(PIXEL6_DIAGNOSTIC_DOMAIN_V1);
    digest.update([tag]);
    digest.update(network);
    digest.update((owner.len() as u16).to_le_bytes());
    digest.update(owner);
    digest.finalize().into()
}

fn pixel6_diagnostic_selection_v1(
    network: [u8; 32],
    owner: &[u8],
) -> Option<[u8; PIXEL6_DIAGNOSTIC_SELECTION_BYTES_V1]> {
    if network == [0; 32]
        || network[31] & 1 == 0
        || owner.is_empty()
        || owner.len() > PIXEL6_DIAGNOSTIC_OWNER_MAX_BYTES_V1
    {
        return None;
    }
    let field = |tag| pixel6_diagnostic_field_v1(&network, owner, tag);
    let selection = KagemushaHardwareTransitionSelectionV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        release_id: field(1),
        provider_policy_root: field(2),
        app_policy_digest: field(3),
        credential_id: field(4),
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
            network,
        ))),
        lane_commitment: field(5),
        hardware_profile_id: field(6),
        policy_epoch: 1,
        hardware_epoch_id: field(7),
        hardware_epoch_generation: 1,
        operation_kind: KagemushaOperationKindV1::Rotate,
        transition_statement_digest: field(8),
        candidate_envelope_digest: [0; 32],
        terminal_body_commitment: [0; 32],
        secure_index_before: 0,
        secure_index_after: 1,
    };
    selection.canonical_signing_bytes().ok()?.try_into().ok()
}

/// Advertise the bounded, non-authorizing Pixel 6 diagnostic selection constructor.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_probe_Pixel6TestnetDiagnosticSelectionJniV1_nativeContractV1(
    env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jintArray {
    let Ok(output) = env.new_int_array(PIXEL6_DIAGNOSTIC_CONTRACT_V1.len() as jni::sys::jsize)
    else {
        return ptr::null_mut();
    };
    if env
        .set_int_array_region(&output, 0, &PIXEL6_DIAGNOSTIC_CONTRACT_V1)
        .is_err()
    {
        return ptr::null_mut();
    }
    output.into_raw()
}

/// Ask the Rust data model for its exact 460-byte signing preimage, scoped to one app owner.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_probe_Pixel6TestnetDiagnosticSelectionJniV1_nativeCreateV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    network: jni::objects::JByteArray<'_>,
    owner: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let network = bounded_java_archive(&mut env, &network, 32)?;
        let network: [u8; 32] = network.try_into().ok()?;
        let owner = bounded_java_archive(&mut env, &owner, PIXEL6_DIAGNOSTIC_OWNER_MAX_BYTES_V1)?;
        let selection = pixel6_diagnostic_selection_v1(network, &owner)?;
        Some(env.byte_array_from_slice(&selection).ok()?.into_raw())
    }))
    .ok()
    .flatten()
    .unwrap_or(ptr::null_mut())
}

const TESTNET_OBSERVATION_JNI_CONTRACT_V1: [jni::sys::jint; 4] = [
    1,
    KAGEMUSHA_TESTNET_STATE_INPUT_MAX_BYTES_V1 as jni::sys::jint,
    iroha_data_model::kagemusha::KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1 as jni::sys::jint,
    KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1 as jni::sys::jint,
];

fn bounded_nonempty_length(length: jni::sys::jsize, maximum: usize) -> Option<usize> {
    let length = usize::try_from(length).ok()?;
    (length > 0 && length <= maximum).then_some(length)
}

fn bounded_java_archive(
    env: &mut jni::JNIEnv<'_>,
    value: &jni::objects::JByteArray<'_>,
    maximum: usize,
) -> Option<Vec<u8>> {
    let length = bounded_nonempty_length(env.get_array_length(value).ok()?, maximum)?;
    let bytes = env.convert_byte_array(value).ok()?;
    (bytes.len() == length).then_some(bytes)
}

/// Advertise the exact diagnostic JNI ABI; this grants no verifier or monetary authority.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetStateProofObservationJniV1_nativeContractV1(
    env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jintArray {
    let Ok(output) =
        env.new_int_array(TESTNET_OBSERVATION_JNI_CONTRACT_V1.len() as jni::sys::jsize)
    else {
        return ptr::null_mut();
    };
    if env
        .set_int_array_region(&output, 0, &TESTNET_OBSERVATION_JNI_CONTRACT_V1)
        .is_err()
    {
        return ptr::null_mut();
    }
    output.into_raw()
}

/// Return the positive archive length, or the exact negative C ABI error status.
///
/// The caller provides a preallocated direct buffer. The C observer writes its
/// canonical response into that buffer before it reports success. The JNI entry
/// itself makes no Java allocation after the diagnostic trial head advances.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetStateProofObservationJniV1_nativeObserveV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    public_inputs: jni::objects::JByteArray<'_>,
    paired_proof: jni::objects::JByteArray<'_>,
    output: jni::objects::JByteBuffer<'_>,
) -> jni::sys::jint {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(public_inputs) = bounded_java_archive(
            &mut env,
            &public_inputs,
            KAGEMUSHA_TESTNET_STATE_INPUT_MAX_BYTES_V1,
        ) else {
            return ERR_KAGEMUSHA_V1;
        };
        let Some(paired_proof) = bounded_java_archive(
            &mut env,
            &paired_proof,
            iroha_data_model::kagemusha::KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
        ) else {
            return ERR_KAGEMUSHA_V1;
        };
        let Ok(capacity) = env.get_direct_buffer_capacity(&output) else {
            return ERR_KAGEMUSHA_V1;
        };
        if capacity != KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1 {
            return ERR_BUFFER_TOO_SMALL;
        }
        let Ok(output_ptr) = env.get_direct_buffer_address(&output) else {
            return ERR_KAGEMUSHA_V1;
        };
        if output_ptr.is_null() {
            return ERR_KAGEMUSHA_V1;
        }
        let mut output_length = 0;
        let status = unsafe {
            connect_norito_kagemusha_testnet_state_proof_observe_v1(
                public_inputs.as_ptr(),
                public_inputs.len(),
                paired_proof.as_ptr(),
                paired_proof.len(),
                output_ptr,
                capacity,
                &mut output_length,
            )
        };
        if status != 0 {
            return status;
        }
        if output_length == 0 || output_length > capacity {
            return ERR_KAGEMUSHA_V1;
        }
        output_length as jni::sys::jint
    }))
    .unwrap_or(ERR_KAGEMUSHA_V1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_jni_contract_is_exact_and_not_a_monetary_contract() {
        assert_eq!(TESTNET_OBSERVATION_JNI_CONTRACT_V1, [1, 4096, 6528, 256]);
        assert!(
            TESTNET_OBSERVATION_JNI_CONTRACT_V1
                .iter()
                .all(|word| *word > 0)
        );
    }

    #[test]
    fn bounded_jni_archive_length_rejects_empty_negative_and_oversized_arrays() {
        assert_eq!(bounded_nonempty_length(-1, 4096), None);
        assert_eq!(bounded_nonempty_length(0, 4096), None);
        assert_eq!(bounded_nonempty_length(1, 4096), Some(1));
        assert_eq!(bounded_nonempty_length(4096, 4096), Some(4096));
        assert_eq!(bounded_nonempty_length(4097, 4096), None);
    }

    #[test]
    fn pixel6_diagnostic_uses_model_canonical_bytes_and_distinct_scoped_ids() {
        let network = [0x71; 32];
        let owner = b"account\0actor\0runtime\0";
        let frame = pixel6_diagnostic_selection_v1(network, owner).expect("selection");
        assert_eq!(PIXEL6_DIAGNOSTIC_CONTRACT_V1, [1, 32, 2048, 460]);
        assert_eq!(frame.len(), 460);
        assert_eq!(
            &frame[..49],
            b"iroha:kagemusha:v1:hardware-transition-selection\0"
        );
        assert_eq!(&frame[49..57], &403_u64.to_le_bytes());
        assert_eq!(&frame[57..59], &KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes());
        assert_eq!(
            &frame[59..91],
            &pixel6_diagnostic_field_v1(&network, owner, 1)
        );
        assert_eq!(
            &frame[91..123],
            &pixel6_diagnostic_field_v1(&network, owner, 2)
        );
        assert_eq!(
            &frame[123..155],
            &pixel6_diagnostic_field_v1(&network, owner, 3)
        );
        assert_eq!(
            &frame[155..187],
            &pixel6_diagnostic_field_v1(&network, owner, 4)
        );
        assert_eq!(&frame[187..219], &network);
        assert_eq!(
            &frame[219..251],
            &pixel6_diagnostic_field_v1(&network, owner, 5)
        );
        assert_eq!(
            &frame[251..283],
            &pixel6_diagnostic_field_v1(&network, owner, 6)
        );
        assert_eq!(&frame[283..291], &1_u64.to_le_bytes());
        assert_eq!(
            &frame[291..323],
            &pixel6_diagnostic_field_v1(&network, owner, 7)
        );
        assert_eq!(&frame[323..331], &1_u64.to_le_bytes());
        assert_eq!(frame[331], 5);
        assert_eq!(
            &frame[332..364],
            &pixel6_diagnostic_field_v1(&network, owner, 8)
        );
        assert_eq!(&frame[364..428], &[0; 64]);
        assert_eq!(&frame[428..444], &[0; 16]);
        assert_eq!(&frame[444..460], &1_u128.to_le_bytes());
        assert_eq!(
            hex::encode(Sha256::digest(frame)),
            "6c9a8f1aea1d86de62939c1ef3e20fc7ed9832fe833a41fbe12e5e0f60198257"
        );
        assert_ne!(frame[59..91], frame[91..123]);
        assert_ne!(
            frame,
            pixel6_diagnostic_selection_v1(network, b"other\0actor\0runtime\0").unwrap()
        );
        assert!(pixel6_diagnostic_selection_v1([0; 32], owner).is_none());
        assert!(pixel6_diagnostic_selection_v1([0x70; 32], owner).is_none());
        assert!(pixel6_diagnostic_selection_v1(network, &[]).is_none());
    }
}

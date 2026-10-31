//! Client-side construction of final Kaigi authorization and usage artifacts.
//!
//! Core recomputes every public value from authenticated call state, so a client
//! must supply the exact original identities, ledger-owned sequence or segment and
//! current roster root. Each returned proof has passed local native verification.

use crate::{ZK_BACKEND_NATIVE_PIPA_R, hash_vk, native_pipa_r};
use ff::{Field as _, PrimeField as _};
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    confidential::ConfidentialStatus,
    kaigi::{
        KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier,
        authorization::KaigiAuthorizationIdentitiesV1, scalar::KaigiAuthorizationScalarV1,
    },
    proof::{VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use kaigi_zk::{
    Scalar, authorization_v1::compute_authorization_v1, native::NativeRelationV1,
    usage_v1::compute_usage_v1,
};
pub use kaigi_zk::{
    authorization_v1::{
        KaigiAuthorizationActionV1, KaigiAuthorizationContextV1, KaigiAuthorizationWitnessV1,
    },
    usage_v1::KaigiUsageContextV1,
};
use zeroize::Zeroizing;

/// Registry name of the governed Kaigi authorization verifying key.
pub const KAIGI_AUTHORIZATION_VK_NAME_V1: &str = "kaigi_authorization_current";
/// Registry name of the governed Kaigi usage verifying key.
pub const KAIGI_USAGE_VK_NAME_V1: &str = "kaigi_usage_current";
/// Governed proof-size bound of both Kaigi verifying-key records.
const KAIGI_MAX_PROOF_BYTES_V1: u32 = 64 * 1024;
/// Rejection-sampling bound. A working random source fails each attempt with
/// probability below one half, so reaching it means the source is broken.
const BLINDING_SAMPLE_ATTEMPTS: usize = 128;

/// Failure to build Kaigi artifacts on the client.
#[derive(Debug, thiserror::Error)]
pub enum KaigiClientErrorV1 {
    /// The canonical identities could not be framed for hashing.
    #[error("cannot bind canonical Kaigi identities: {0}")]
    Identity(norito::core::Error),
    /// The context violates the relation, for example a wrong role or sequence.
    #[error("invalid Kaigi proof context: {0}")]
    Context(String),
    /// Native key preparation or proving did not complete.
    #[error(transparent)]
    Proof(#[from] native_pipa_r::NativeProofProductionError),
    /// The verifying key could not be prepared or packaged.
    #[error("cannot prepare the Kaigi verifying key: {0}")]
    Key(String),
    /// The generated envelope could not be encoded.
    #[error("cannot encode the Kaigi proof envelope: {0}")]
    Encoding(norito::Error),
    /// A circuit output was not a canonical field encoding.
    #[error("Kaigi circuit produced a non-canonical scalar")]
    NonCanonicalOutput,
    /// The random source failed or never produced a usable scalar.
    #[error("cannot sample a Kaigi blinding: {0}")]
    Randomness(String),
}

/// Public carriers of one authorization action, ready for its instruction.
#[derive(Clone, Debug)]
pub struct KaigiAuthorizationArtifactsV1 {
    /// Commitment to the private opening, stable within one participation.
    pub commitment: KaigiParticipantCommitment,
    /// Action-separated nullifier.
    pub nullifier: KaigiParticipantNullifier,
    /// Exact pre-action roster root the proof binds.
    pub roster_root: Hash,
    /// Canonical Norito `OpenVerifyEnvelope`.
    pub proof: Vec<u8>,
}

/// Public carriers of one usage segment, ready for its instruction.
#[derive(Clone, Debug)]
pub struct KaigiUsageArtifactsV1 {
    /// Commitment to the complete usage context.
    pub usage_commitment: KaigiAuthorizationScalarV1,
    /// Canonical Norito `OpenVerifyEnvelope`.
    pub proof: Vec<u8>,
}

fn canonical_scalar(bytes: [u8; 32]) -> Result<KaigiAuthorizationScalarV1, KaigiClientErrorV1> {
    KaigiAuthorizationScalarV1::from_le_bytes(bytes).ok_or(KaigiClientErrorV1::NonCanonicalOutput)
}

/// Sample a uniform nonzero Pasta Fp blinding by rejection.
///
/// `fill` must draw from a cryptographically secure source. Store the result
/// under owner-only custody: leaving or ending a call needs the same opening.
///
/// # Errors
/// Returns [`KaigiClientErrorV1::Randomness`] if `fill` fails or the source
/// keeps producing values outside the field.
pub fn sample_kaigi_blinding_v1<E: std::fmt::Display>(
    mut fill: impl FnMut(&mut [u8; 32]) -> Result<(), E>,
) -> Result<Zeroizing<[u8; 32]>, KaigiClientErrorV1> {
    let mut candidate = Zeroizing::new([0_u8; 32]);
    for _ in 0..BLINDING_SAMPLE_ATTEMPTS {
        fill(&mut candidate).map_err(|error| KaigiClientErrorV1::Randomness(error.to_string()))?;
        // The modulus lies just above 2^254, so masking the top bit makes most
        // draws canonical while rejection keeps the result uniform over the field.
        candidate[31] &= 0x7f;
        let scalar = Option::<Scalar>::from(Scalar::from_repr(*candidate));
        if scalar.is_some_and(|value| !bool::from(value.is_zero())) {
            return Ok(candidate);
        }
    }
    Err(KaigiClientErrorV1::Randomness(
        "the random source never produced a canonical nonzero scalar".to_owned(),
    ))
}

/// Project explicit ledger values into the authorization relation's public context.
///
/// # Errors
/// Fails if the identities cannot be framed, or if the role, sequence and action
/// are inconsistent with the relation.
pub fn kaigi_authorization_context_v1(
    network: NetworkId,
    call: &KaigiId,
    host: &AccountId,
    subject: &AccountId,
    sequence: u64,
    action: KaigiAuthorizationActionV1,
    pre_roster_root: &Hash,
) -> Result<KaigiAuthorizationContextV1, KaigiClientErrorV1> {
    let identities = KaigiAuthorizationIdentitiesV1::new(network, call, host, subject)
        .map_err(KaigiClientErrorV1::Identity)?;
    let context = KaigiAuthorizationContextV1 {
        network_id: *network.as_bytes(),
        call_id: identities.call_id.words(),
        host_id: identities.host_id.words(),
        subject_id: identities.subject_id.words(),
        participation_sequence: sequence,
        action,
        pre_roster_root: (*pre_roster_root).into(),
    };
    context
        .validate()
        .map_err(|error| KaigiClientErrorV1::Context(error.to_string()))?;
    Ok(context)
}

/// Compute the commitment a witness opens, without the cost of proving.
///
/// Clients compare this with the stored commitment so a wrong secret fails
/// locally, before any proof work or transaction fee.
///
/// # Errors
/// Fails if the context is invalid for the witness.
pub fn kaigi_authorization_commitment_v1(
    context: &KaigiAuthorizationContextV1,
    witness: &KaigiAuthorizationWitnessV1,
) -> Result<KaigiAuthorizationScalarV1, KaigiClientErrorV1> {
    let outputs = compute_authorization_v1(context, witness)
        .map_err(|error| KaigiClientErrorV1::Context(error.to_string()))?;
    canonical_scalar(outputs.commitment.to_repr())
}

/// Prove one authorization action, consuming its private witness.
///
/// # Errors
/// Fails on an invalid context, or if proving or local verification does not complete.
pub fn prove_kaigi_authorization_artifacts_v1(
    context: KaigiAuthorizationContextV1,
    witness: KaigiAuthorizationWitnessV1,
) -> Result<KaigiAuthorizationArtifactsV1, KaigiClientErrorV1> {
    let outputs = compute_authorization_v1(&context, &witness)
        .map_err(|error| KaigiClientErrorV1::Context(error.to_string()))?;
    let pre_roster_root = context.pre_roster_root;
    let envelope = native_pipa_r::prove_kaigi_authorization(context, witness)?;
    Ok(KaigiAuthorizationArtifactsV1 {
        commitment: KaigiParticipantCommitment {
            commitment: canonical_scalar(outputs.commitment.to_repr())?,
        },
        nullifier: KaigiParticipantNullifier {
            digest: canonical_scalar(outputs.nullifier.to_repr())?,
        },
        roster_root: Hash::prehashed(pre_roster_root),
        proof: norito::encode_canonical(&envelope).map_err(KaigiClientErrorV1::Encoding)?,
    })
}

/// Project explicit ledger values into the usage relation's public context.
///
/// # Errors
/// Fails if the identities cannot be framed or the metrics violate the relation.
pub fn kaigi_usage_context_v1(
    network: NetworkId,
    call: &KaigiId,
    host: &AccountId,
    pre_roster_root: &Hash,
    segment_index: u32,
    duration_ms: u64,
    billed_gas: u64,
) -> Result<KaigiUsageContextV1, KaigiClientErrorV1> {
    let identities = KaigiAuthorizationIdentitiesV1::new(network, call, host, host)
        .map_err(KaigiClientErrorV1::Identity)?;
    let context = KaigiUsageContextV1 {
        network_id: *network.as_bytes(),
        call_id: identities.call_id.words(),
        host_id: identities.host_id.words(),
        pre_roster_root: (*pre_roster_root).into(),
        segment_index,
        duration_ms,
        billed_gas,
    };
    context
        .validate()
        .map_err(|error| KaigiClientErrorV1::Context(error.to_string()))?;
    Ok(context)
}

/// Compute the host commitment a usage witness opens, without the cost of proving.
///
/// # Errors
/// Fails if the context is invalid for the witness.
pub fn kaigi_usage_host_commitment_v1(
    context: &KaigiUsageContextV1,
    witness: &KaigiAuthorizationWitnessV1,
) -> Result<KaigiAuthorizationScalarV1, KaigiClientErrorV1> {
    let outputs = compute_usage_v1(context, witness)
        .map_err(|error| KaigiClientErrorV1::Context(error.to_string()))?;
    canonical_scalar(outputs.host_commitment.to_repr())
}

/// Prove one usage segment, consuming the host's private witness.
///
/// # Errors
/// Fails on an invalid context, or if proving or local verification does not complete.
pub fn prove_kaigi_usage_artifacts_v1(
    context: KaigiUsageContextV1,
    witness: KaigiAuthorizationWitnessV1,
) -> Result<KaigiUsageArtifactsV1, KaigiClientErrorV1> {
    let outputs = compute_usage_v1(&context, &witness)
        .map_err(|error| KaigiClientErrorV1::Context(error.to_string()))?;
    let envelope = native_pipa_r::prove_kaigi_usage(context, witness)?;
    Ok(KaigiUsageArtifactsV1 {
        usage_commitment: canonical_scalar(outputs.usage_commitment.to_repr())?,
        proof: norito::encode_canonical(&envelope).map_err(KaigiClientErrorV1::Encoding)?,
    })
}

fn verifying_key_registration(
    kind: NativeRelationV1,
    name: &str,
) -> Result<(VerifyingKeyId, VerifyingKeyRecord), KaigiClientErrorV1> {
    let carrier = native_pipa_r::kaigi_verifying_key(kind)
        .map_err(|error| KaigiClientErrorV1::Key(error.to_string()))?;
    let mut record = VerifyingKeyRecord::new(
        1,
        kind.circuit_id(),
        BackendTag::NativePipaRPasta,
        "vesta",
        Hash::new(native_pipa_r::public_schema(kind.into())).into(),
        hash_vk(&carrier),
    );
    record.vk_len = u32::try_from(carrier.bytes.len())
        .map_err(|_| KaigiClientErrorV1::Key("key length exceeds u32".to_owned()))?;
    record.max_proof_bytes = KAIGI_MAX_PROOF_BYTES_V1;
    record.gas_schedule_id = Some("native_pipa_r_default".to_owned());
    record.key = Some(carrier);
    record.status = ConfidentialStatus::Active;
    name.clone_into(&mut record.namespace);
    Ok((VerifyingKeyId::new(ZK_BACKEND_NATIVE_PIPA_R, name), record))
}

/// Active registry records of the authorization and usage verifying keys, in that order.
///
/// A chain enables private calls by installing these in genesis and selecting them
/// through its committed ZK validity policy; the default executor refuses registration
/// after genesis until a governed verifying-key lifecycle exists.
///
/// # Errors
/// Fails if a compiled key cannot be prepared.
pub fn kaigi_verifying_key_registrations_v1()
-> Result<[(VerifyingKeyId, VerifyingKeyRecord); 2], KaigiClientErrorV1> {
    Ok([
        verifying_key_registration(
            NativeRelationV1::Authorization,
            KAIGI_AUTHORIZATION_VK_NAME_V1,
        )?,
        verifying_key_registration(NativeRelationV1::Usage, KAIGI_USAGE_VK_NAME_V1)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{kaigi::KaigiRecord, proof::ProofBox};
    use std::convert::Infallible;

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new([7; 32])))
    }

    fn call() -> KaigiId {
        norito::json::from_str(r#"{"domain_id":"kaigi.universal","call_name":"client-proof"}"#)
            .unwrap()
    }

    fn witness(seed: u8) -> KaigiAuthorizationWitnessV1 {
        let mut bytes = [0_u8; 32];
        bytes[0] = seed;
        KaigiAuthorizationWitnessV1::take_blinding(&mut bytes).unwrap()
    }

    fn verifies(proof: &[u8]) -> bool {
        let [(_, authorization), (_, usage)] = kaigi_verifying_key_registrations_v1().unwrap();
        let proof_box = ProofBox::new(ZK_BACKEND_NATIVE_PIPA_R.to_owned(), proof.to_vec());
        [authorization, usage].into_iter().any(|record| {
            crate::verify_backend(ZK_BACKEND_NATIVE_PIPA_R, &proof_box, record.key.as_ref())
                .unwrap_or(false)
        })
    }

    #[test]
    fn sampled_blindings_are_canonical_nonzero_witnesses() {
        let mut counter = 0_u8;
        for _ in 0..16 {
            let sampled = sample_kaigi_blinding_v1(|bytes| {
                counter = counter.wrapping_add(0x5b);
                bytes.fill(counter);
                Ok::<(), Infallible>(())
            })
            .unwrap();
            let mut copy = *sampled;
            KaigiAuthorizationWitnessV1::take_blinding(&mut copy).unwrap();
        }
    }

    #[test]
    fn sampling_rejects_zero_and_reports_a_broken_source() {
        let mut draws = 0;
        let sampled = sample_kaigi_blinding_v1(|bytes| {
            draws += 1;
            *bytes = [0; 32];
            if draws == 3 {
                bytes[0] = 1;
            }
            Ok::<(), Infallible>(())
        })
        .unwrap();
        assert_eq!(draws, 3);
        assert_eq!(sampled[0], 1);
        assert!(matches!(
            sample_kaigi_blinding_v1(|bytes| {
                *bytes = [0; 32];
                Ok::<(), Infallible>(())
            }),
            Err(KaigiClientErrorV1::Randomness(_))
        ));
        assert!(matches!(
            sample_kaigi_blinding_v1(|_| Err("no entropy")),
            Err(KaigiClientErrorV1::Randomness(message)) if message == "no entropy"
        ));
    }

    #[test]
    fn host_lifecycle_artifacts_verify_and_bind_the_stored_opening() {
        let host = account(1);
        let empty_root = KaigiRecord::compute_roster_root(&[]);
        let create = kaigi_authorization_context_v1(
            network(),
            &call(),
            &host,
            &host,
            0,
            KaigiAuthorizationActionV1::HostCreate,
            &empty_root,
        )
        .unwrap();
        let created = prove_kaigi_authorization_artifacts_v1(create, witness(9)).unwrap();
        assert_eq!(created.roster_root, empty_root);
        assert!(verifies(&created.proof));
        assert_eq!(
            kaigi_authorization_commitment_v1(&create, &witness(9)).unwrap(),
            created.commitment.commitment
        );
        // A different secret produces a different opening, which clients detect locally.
        assert_ne!(
            kaigi_authorization_commitment_v1(&create, &witness(10)).unwrap(),
            created.commitment.commitment
        );
        let usage =
            kaigi_usage_context_v1(network(), &call(), &host, &empty_root, 0, 1_200, 345).unwrap();
        assert_eq!(
            kaigi_usage_host_commitment_v1(&usage, &witness(9)).unwrap(),
            created.commitment.commitment
        );
        let used = prove_kaigi_usage_artifacts_v1(usage, witness(9)).unwrap();
        assert!(verifies(&used.proof));
    }

    #[test]
    fn inconsistent_roles_fail_before_any_proving() {
        let host = account(1);
        let root = KaigiRecord::compute_roster_root(&[]);
        for (subject, sequence, action) in [
            (account(2), 0, KaigiAuthorizationActionV1::HostCreate),
            (account(2), 0, KaigiAuthorizationActionV1::Join),
            (host.clone(), 1, KaigiAuthorizationActionV1::Join),
            (host.clone(), 1, KaigiAuthorizationActionV1::HostEnd),
        ] {
            assert!(matches!(
                kaigi_authorization_context_v1(
                    network(),
                    &call(),
                    &host,
                    &subject,
                    sequence,
                    action,
                    &root,
                ),
                Err(KaigiClientErrorV1::Context(_))
            ));
        }
        assert!(matches!(
            kaigi_usage_context_v1(network(), &call(), &host, &root, 0, 0, 1),
            Err(KaigiClientErrorV1::Context(_))
        ));
    }

    #[test]
    fn registrations_select_the_two_distinct_compiled_keys() {
        let [(authorization_id, authorization), (usage_id, usage)] =
            kaigi_verifying_key_registrations_v1().unwrap();
        assert_eq!(authorization_id.name, KAIGI_AUTHORIZATION_VK_NAME_V1);
        assert_eq!(usage_id.name, KAIGI_USAGE_VK_NAME_V1);
        assert_ne!(authorization.commitment, usage.commitment);
        for record in [&authorization, &usage] {
            assert_eq!(record.status, ConfidentialStatus::Active);
            assert!(record.max_proof_bytes > 0 && record.gas_schedule_id.is_some());
            assert_eq!(
                record.vk_len as usize,
                record.key.as_ref().unwrap().bytes.len()
            );
        }
    }
}

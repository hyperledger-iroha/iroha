//! Native construction of the final context-bound Kaigi authorization proof.

use halo2_proofs::{
    SerdeFormat,
    halo2curves::{ff::PrimeField as _, pasta::EqAffine},
    plonk::{create_proof, keygen_pk, keygen_vk},
    poly::{
        commitment::ParamsProver,
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::ProverIPA,
        },
    },
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
};
use iroha_core::zk::hash_vk;
use iroha_data_model::{
    account::AccountId,
    domain::DomainId,
    kaigi::{KaigiId, authorization::KaigiAuthorizationIdentitiesV1},
    name::Name,
    proof::{ProofBox, VerifyingKeyBox},
    zk::{BackendTag, OpenVerifyEnvelope},
};
use kaigi_zk::authorization_v1::{
    KAIGI_AUTHORIZATION_CIRCUIT_ID_V1, KAIGI_AUTHORIZATION_CIRCUIT_K_V1,
    KAIGI_AUTHORIZATION_PUBLIC_INPUTS_SCHEMA_V1, KaigiAuthorizationActionV1,
    KaigiAuthorizationCircuitV1, KaigiAuthorizationContextV1, KaigiAuthorizationPublicInputsV1,
    KaigiAuthorizationWitnessV1, compute_authorization_v1,
};
use napi::{
    Env,
    bindgen_prelude::{BigInt, Buffer, JsObjectValue as _, Uint8Array, Uint8ArraySlice},
};
use napi_derive::napi;
use rand_core_06::OsRng;
use std::str::FromStr as _;

const VK_BACKEND: &str = "halo2/ipa";
const ZK1_PREFIX: &[u8] = b"ZK1\0";

/// Exact field outputs and canonical proof for one Kaigi authorization action.
#[napi(object)]
pub struct JsKaigiAuthorizationProofV1 {
    /// Canonical raw Pasta Fp commitment, without Iroha hash-marker mutation.
    pub commitment: Buffer,
    /// Canonical raw Pasta Fp action nullifier.
    pub nullifier: Buffer,
    /// Canonical raw Pasta Fp authorization binding the action and pre-state.
    pub authorization: Buffer,
    /// Exact 32-byte pre-state root supplied to the relation.
    pub pre_roster_root: Buffer,
    /// Canonical Norito OpenVerifyEnvelope for the final V1 circuit.
    pub proof: Buffer,
}

fn invalid(message: impl ToString) -> napi::Error {
    napi::Error::new(napi::Status::InvalidArg, message.to_string())
}

fn failure(message: impl ToString) -> napi::Error {
    napi::Error::new(napi::Status::GenericFailure, message.to_string())
}

fn take_witness(bytes: &mut [u8]) -> napi::Result<KaigiAuthorizationWitnessV1> {
    let mut owned = [0; 32];
    if bytes.len() == owned.len() {
        owned.copy_from_slice(bytes);
    }
    iroha_crypto::zeroize_value_for_confidential_discard(bytes);
    if bytes.len() != owned.len() {
        return Err(invalid(
            "blinding must contain exactly 32 canonical Pasta Fp bytes",
        ));
    }
    KaigiAuthorizationWitnessV1::take_blinding(&mut owned).map_err(invalid)
}

#[allow(clippy::too_many_arguments)] // Fixed context fields mirror the typed N-API boundary.
fn parse_context(
    network: &[u8],
    domain: &str,
    call_name: &str,
    host: &str,
    subject: &str,
    sequence: &BigInt,
    action: &str,
    pre_root: &[u8],
) -> napi::Result<KaigiAuthorizationContextV1> {
    let network = super::parse_transaction_network_id_bytes(network)?;
    let domain_id = DomainId::parse_fully_qualified(domain).map_err(invalid)?;
    let name = Name::from_str(call_name).map_err(invalid)?;
    if domain_id.to_string() != domain || name.as_ref() != call_name {
        return Err(invalid(
            "callId must use canonical domain and call-name text",
        ));
    }
    if host.trim() != host || subject.trim() != subject {
        return Err(invalid(
            "account identities must not contain surrounding whitespace",
        ));
    }
    let host = AccountId::parse_encoded(host).map_err(invalid)?;
    let subject = AccountId::parse_encoded(subject).map_err(invalid)?;
    let identities = KaigiAuthorizationIdentitiesV1::new(
        network,
        &KaigiId::new(domain_id, name),
        &host,
        &subject,
    )
    .map_err(invalid)?;
    let (negative, sequence, lossless) = sequence.get_u64();
    if negative || !lossless {
        return Err(invalid(
            "participationSequence must be an exact unsigned 64-bit bigint",
        ));
    }
    let action = match action {
        "hostCreate" => KaigiAuthorizationActionV1::HostCreate,
        "join" => KaigiAuthorizationActionV1::Join,
        "leave" => KaigiAuthorizationActionV1::Leave,
        "hostEnd" => KaigiAuthorizationActionV1::HostEnd,
        _ => return Err(invalid("unknown Kaigi authorization action")),
    };
    let context = KaigiAuthorizationContextV1 {
        network_id: *identities.network_id.as_bytes(),
        call_id: identities.call_id.words(),
        host_id: identities.host_id.words(),
        subject_id: identities.subject_id.words(),
        participation_sequence: sequence,
        action,
        pre_roster_root: pre_root
            .try_into()
            .map_err(|_| invalid("preRosterRoot must contain exactly 32 bytes"))?,
    };
    context.validate().map_err(invalid)?;
    Ok(context)
}

// These are the former private candidate's ZK1 writers, now owned exclusively
// by the final circuit. The fixed column has 31 rows; no dimension inference,
// alternate order, field reduction, or silent writer failure is accepted.
fn append_tlv(bytes: &mut Vec<u8>, tag: [u8; 4], payload: &[u8]) -> napi::Result<()> {
    let length = u32::try_from(payload.len()).map_err(|_| failure("ZK1 payload exceeds u32"))?;
    bytes.extend_from_slice(&tag);
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(payload);
    Ok(())
}

fn encode_verified_envelope(
    envelope: &OpenVerifyEnvelope,
    key: &VerifyingKeyBox,
) -> napi::Result<Vec<u8>> {
    let encoded = norito::encode_canonical(envelope).map_err(failure)?;
    let proof = ProofBox::new(VK_BACKEND.to_owned(), encoded.clone());
    if !iroha_core::zk::verify_backend(VK_BACKEND, &proof, Some(key)) {
        return Err(failure(
            "generated Kaigi authorization proof failed canonical native verification",
        ));
    }
    Ok(encoded)
}

fn produce(
    context: KaigiAuthorizationContextV1,
    witness: KaigiAuthorizationWitnessV1,
) -> napi::Result<(JsKaigiAuthorizationProofV1, VerifyingKeyBox)> {
    let outputs = compute_authorization_v1(&context, &witness).map_err(invalid)?;
    let instance = KaigiAuthorizationPublicInputsV1 { context, outputs }.instance();
    let circuit = KaigiAuthorizationCircuitV1::new(context, witness).map_err(invalid)?;
    let params: ParamsIPA<EqAffine> = ParamsIPA::new(KAIGI_AUTHORIZATION_CIRCUIT_K_V1);
    let vk = keygen_vk(&params, &KaigiAuthorizationCircuitV1::default()).map_err(failure)?;
    let pk =
        keygen_pk(&params, vk.clone(), &KaigiAuthorizationCircuitV1::default()).map_err(failure)?;
    let mut transcript = Blake2bWrite::<_, EqAffine, Challenge255<EqAffine>>::init(Vec::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        ProverIPA<'_, EqAffine>,
        Challenge255<EqAffine>,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[circuit],
        &[&[&instance]],
        OsRng,
        &mut transcript,
    )
    .map_err(failure)?;

    let mut key_carrier = ZK1_PREFIX.to_vec();
    append_tlv(
        &mut key_carrier,
        *b"IPAK",
        &KAIGI_AUTHORIZATION_CIRCUIT_K_V1.to_le_bytes(),
    )?;
    append_tlv(
        &mut key_carrier,
        *b"CID1",
        KAIGI_AUTHORIZATION_CIRCUIT_ID_V1.as_bytes(),
    )?;
    append_tlv(
        &mut key_carrier,
        *b"H2VK",
        &vk.to_bytes(SerdeFormat::Processed),
    )?;
    let key = VerifyingKeyBox::new(VK_BACKEND.to_owned(), key_carrier);
    let mut proof = ZK1_PREFIX.to_vec();
    append_tlv(&mut proof, *b"PROF", &transcript.finalize())?;
    let mut instance_bytes = Vec::with_capacity(8 + instance.len() * 32);
    instance_bytes.extend_from_slice(&1_u32.to_le_bytes());
    instance_bytes.extend_from_slice(&31_u32.to_le_bytes());
    for scalar in instance {
        instance_bytes.extend_from_slice(scalar.to_repr().as_ref());
    }
    append_tlv(&mut proof, *b"I10P", &instance_bytes)?;
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: KAIGI_AUTHORIZATION_CIRCUIT_ID_V1.to_owned(),
        vk_hash: hash_vk(&key),
        public_inputs: KAIGI_AUTHORIZATION_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        proof_bytes: proof,
        aux: Vec::new(),
    };
    let proof = encode_verified_envelope(&envelope, &key)?;
    let [commitment, nullifier, authorization] = outputs.canonical_bytes();
    Ok((
        JsKaigiAuthorizationProofV1 {
            commitment: commitment.to_vec().into(),
            nullifier: nullifier.to_vec().into(),
            authorization: authorization.to_vec().into(),
            pre_roster_root: context.pre_roster_root.to_vec().into(),
            proof: proof.into(),
        },
        key,
    ))
}

/// Consume a nonzero full-field blinding and prove one complete Kaigi V1 action.
///
/// Accepted blinding buffers are cleared before context parsing, including on
/// failure. Halo2's internal assignment/prover buffers have separate lifetimes.
#[napi(js_name = "buildKaigiAuthorizationProofV1")]
#[allow(clippy::too_many_arguments)] // Keep each final V1 field typed across N-API.
pub fn build_kaigi_authorization_proof_v1(
    env: Env,
    network_id: Uint8Array,
    domain_id: String,
    call_name: String,
    host_id: String,
    subject_id: String,
    participation_sequence: BigInt,
    action: String,
    pre_roster_root: Uint8Array,
    mut blinding: Uint8ArraySlice<'_>,
) -> napi::Result<JsKaigiAuthorizationProofV1> {
    let length =
        u32::try_from(blinding.len()).map_err(|_| invalid("blinding exceeds JS index width"))?;
    let mut owned = [0; 32];
    if length == 32 {
        owned.copy_from_slice(blinding.as_ref());
    }
    // This clears the native stack copy immediately. The witness owns its own
    // zeroizing storage and is dropped even if a subsequent VM operation fails.
    let witness = take_witness(&mut owned);
    let zero = env.create_uint32(0)?;
    // Use the VM's scoped typed-array writes, never an aliased mutable Rust
    // slice into JavaScript-owned memory. No JavaScript callback is invoked.
    for index in 0..length {
        blinding.set_element(index, zero)?;
    }
    if length != 32 {
        return Err(invalid(
            "blinding must contain exactly 32 canonical Pasta Fp bytes",
        ));
    }
    let witness = witness?;
    let context = parse_context(
        network_id.as_ref(),
        &domain_id,
        &call_name,
        &host_id,
        &subject_id,
        &participation_sequence,
        &action,
        pre_roster_root.as_ref(),
    )?;
    produce(context, witness).map(|(artifacts, _)| artifacts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::NetworkId;

    fn account(seed: u8) -> AccountId {
        let key = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key.public_key().clone())
    }

    fn fixture_context() -> KaigiAuthorizationContextV1 {
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new([3; 32])));
        let domain = DomainId::try_new("kaigi", "universal").unwrap();
        parse_context(
            network.as_bytes(),
            &domain.to_string(),
            "native-proof",
            &account(1).to_string(),
            &account(2).to_string(),
            &BigInt::from(1_u64),
            "join",
            &[0x35; 32],
        )
        .unwrap()
    }

    #[test]
    fn generated_envelope_verification_fails_closed_without_returning_bytes() {
        let key = VerifyingKeyBox::new(VK_BACKEND.to_owned(), Vec::new());
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: KAIGI_AUTHORIZATION_CIRCUIT_ID_V1.to_owned(),
            vk_hash: hash_vk(&key),
            public_inputs: KAIGI_AUTHORIZATION_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
            proof_bytes: Vec::new(),
            aux: Vec::new(),
        };
        let error = encode_verified_envelope(&envelope, &key).unwrap_err();
        assert_eq!(error.status, napi::Status::GenericFailure);
        assert!(
            error
                .reason
                .contains("failed canonical native verification")
        );
    }

    #[test]
    fn witness_consumption_clears_valid_invalid_and_wrong_length_buffers() {
        let mut valid = [0; 32];
        valid[0] = 1;
        let witness = take_witness(&mut valid).unwrap();
        assert_eq!(valid, [0; 32]);
        assert!(!format!("{witness:?}").contains("[1,"));
        for mut invalid in [vec![0; 32], vec![0xff; 32], vec![1; 31], vec![1; 33]] {
            assert!(take_witness(&mut invalid).is_err());
            assert!(invalid.iter().all(|&byte| byte == 0));
        }
    }

    #[test]
    fn canonical_context_rejects_lossy_sequence_and_wrong_roles_and_binds_all_identities() {
        let base = fixture_context();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new([3; 32])));
        let domain = DomainId::try_new("kaigi", "universal").unwrap().to_string();
        let host = account(1).to_string();
        let subject = account(2).to_string();
        let context = |sequence: BigInt, action: &str, subject: &str| {
            parse_context(
                network.as_bytes(),
                &domain,
                "native-proof",
                &host,
                subject,
                &sequence,
                action,
                &[0x35; 32],
            )
        };
        assert_eq!(
            context(BigInt::from(1_u64), "join", &subject).unwrap(),
            base
        );
        assert!(
            context(
                BigInt {
                    sign_bit: true,
                    words: vec![1]
                },
                "join",
                &subject
            )
            .is_err()
        );
        assert!(
            context(
                BigInt {
                    sign_bit: false,
                    words: vec![0, 1]
                },
                "join",
                &subject
            )
            .is_err()
        );
        assert!(context(BigInt::from(0_u64), "join", &subject).is_err());
        assert!(context(BigInt::from(1_u64), "hostEnd", &host).is_err());
        assert!(context(BigInt::from(1_u64), "join", &host).is_err());
        assert!(context(BigInt::from(0_u64), "hostCreate", &subject).is_err());
        assert!(context(BigInt::from(1_u64), "JOIN", &subject).is_err());
        let maximum = context(BigInt::from(u64::MAX), "leave", &subject).unwrap();
        assert_eq!(maximum.participation_sequence, u64::MAX);
        let other = context(BigInt::from(1_u64), "join", &account(4).to_string()).unwrap();
        assert_ne!(other.subject_id, base.subject_id);
        use iroha_data_model::account::{MultisigMember, MultisigPolicy};
        let members: Vec<_> = (1..=24)
            .map(|seed| {
                MultisigMember::new(account(seed).expect_single_signatory().clone(), 1).unwrap()
            })
            .collect();
        let policy = MultisigPolicy::new(2, members).unwrap();
        let original = AccountId::new_multisig(policy.clone());
        let original_text = original.canonical_i105().unwrap();
        assert_eq!(AccountId::parse_encoded(&original_text).unwrap(), original);
        let original_context = context(BigInt::from(1_u64), "join", &original_text).unwrap();
        let mut changed_members = policy.members().to_vec();
        let last = changed_members.last_mut().unwrap();
        *last = MultisigMember::new(last.public_key().clone(), 2).unwrap();
        let changed = AccountId::new_multisig(MultisigPolicy::new(2, changed_members).unwrap());
        let changed_context = context(
            BigInt::from(1_u64),
            "join",
            &changed.canonical_i105().unwrap(),
        )
        .unwrap();
        assert_ne!(original_context.subject_id, changed_context.subject_id);
        assert!(
            parse_context(
                &[0; 32],
                &domain,
                "native-proof",
                &host,
                &subject,
                &BigInt::from(1_u64),
                "join",
                &[0x35; 32]
            )
            .is_err()
        );
        assert!(
            parse_context(
                network.as_bytes(),
                &domain,
                "native-proof",
                &host,
                &subject,
                &BigInt::from(1_u64),
                "join",
                &[0x35; 31]
            )
            .is_err()
        );
    }

    #[test]
    fn final_native_proof_verifies_and_rejects_mutated_context_owner_and_outputs() {
        let context = fixture_context();
        let mut secret = [0x11; 32];
        let (artifacts, key) = produce(context, take_witness(&mut secret).unwrap()).unwrap();
        assert_eq!(secret, [0; 32]);
        for output in [
            &artifacts.commitment,
            &artifacts.nullifier,
            &artifacts.authorization,
        ] {
            assert!(
                iroha_data_model::kaigi::scalar::KaigiAuthorizationScalarV1::from_le_bytes(
                    output.as_ref().try_into().unwrap()
                )
                .is_some()
            );
        }
        assert_eq!(artifacts.pre_roster_root.as_ref(), context.pre_roster_root);
        let envelope: OpenVerifyEnvelope =
            norito::decode_canonical(artifacts.proof.as_ref()).unwrap();
        assert_eq!(envelope.circuit_id, KAIGI_AUTHORIZATION_CIRCUIT_ID_V1);
        assert_eq!(
            envelope.public_inputs,
            KAIGI_AUTHORIZATION_PUBLIC_INPUTS_SCHEMA_V1
        );
        assert_eq!(envelope.vk_hash, hash_vk(&key));
        assert!(envelope.aux.is_empty());
        let proof = ProofBox::new(VK_BACKEND.to_owned(), artifacts.proof.as_ref().to_vec());
        assert!(iroha_core::zk::verify_backend(
            VK_BACKEND,
            &proof,
            Some(&key)
        ));
        let first_scalar = envelope.proof_bytes.len() - 31 * 32;
        for row in [0, 4, 10, 16, 22, 23, 24, 28, 29, 30] {
            let mut changed = envelope.clone();
            let range = first_scalar + row * 32..first_scalar + (row + 1) * 32;
            assert!(
                changed.proof_bytes[range.clone()]
                    .iter()
                    .any(|&byte| byte != 0)
            );
            changed.proof_bytes[range].fill(0);
            let proof = ProofBox::new(
                VK_BACKEND.to_owned(),
                norito::encode_canonical(&changed).unwrap(),
            );
            assert!(
                !iroha_core::zk::verify_backend(VK_BACKEND, &proof, Some(&key)),
                "modified row {row}"
            );
        }
    }
}

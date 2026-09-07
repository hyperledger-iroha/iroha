//! Real authorization proofs and governed keys for Kaigi execution tests.

use super::*;
use crate::{state::StateReadOnly, zk::zk1_test_helpers as zk1};
use halo2_proofs::{
    halo2curves::{
        ff::PrimeField as _,
        pasta::{EqAffine, Fp},
    },
    plonk::{ProvingKey, create_proof, keygen_pk, keygen_vk},
    poly::{
        commitment::ParamsProver as _,
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::ProverIPA,
        },
    },
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
};
use iroha_data_model::{
    confidential::ConfidentialStatus,
    prelude::AccountId,
    proof::{VerifyingKeyBox, VerifyingKeyRecord},
};
use kaigi_zk::authorization_v1::{
    KAIGI_AUTHORIZATION_CIRCUIT_K_V1, KAIGI_AUTHORIZATION_PUBLIC_INPUTS_SCHEMA_V1,
    KaigiAuthorizationActionV1, KaigiAuthorizationCircuitV1, KaigiAuthorizationPublicInputsV1,
    KaigiAuthorizationWitnessV1, compute_authorization_v1,
};
use rand_core_06::OsRng;
use std::sync::OnceLock;

struct Key {
    params: ParamsIPA<EqAffine>,
    pk: ProvingKey<EqAffine>,
    carrier: VerifyingKeyBox,
}

fn key() -> &'static Key {
    static KEY: OnceLock<Key> = OnceLock::new();
    KEY.get_or_init(|| {
        let params = ParamsIPA::new(KAIGI_AUTHORIZATION_CIRCUIT_K_V1);
        let empty = KaigiAuthorizationCircuitV1::default();
        let vk = keygen_vk(&params, &empty).unwrap();
        let pk = keygen_pk(&params, vk.clone(), &empty).unwrap();
        let mut bytes = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut bytes, KAIGI_AUTHORIZATION_CIRCUIT_K_V1);
        zk1::wrap_append_circuit_id(&mut bytes, KAIGI_AUTHORIZATION_CIRCUIT_ID_V1);
        zk1::wrap_append_vk_pasta(&mut bytes, &vk);
        Key {
            params,
            pk,
            carrier: VerifyingKeyBox::new("halo2/ipa".into(), bytes),
        }
    })
}

/// Exact artifacts from the final circuit, with a real proof.
pub(crate) struct AuthorizationFixtureV1 {
    /// Canonical private opening commitment.
    pub commitment: KaigiParticipantCommitment,
    /// Canonical action nullifier.
    pub nullifier: KaigiParticipantNullifier,
    /// Authenticated pre-state root.
    pub root: Hash,
    /// Canonical outer envelope.
    pub proof: Vec<u8>,
}

/// Generate a final proof for this ledger context and install its governed key.
pub(crate) fn authorization_fixture_v1(
    state: &mut StateTransaction<'_, '_>,
    record: &KaigiRecord,
    subject: &AccountId,
    sequence: u64,
    action: KaigiAuthorizationActionV1,
    secret: u64,
) -> AuthorizationFixtureV1 {
    let key = key();
    let hash = zk::hash_vk(&key.carrier);
    let mut vk = VerifyingKeyRecord::new(
        1,
        KAIGI_AUTHORIZATION_CIRCUIT_ID_V1,
        BackendTag::Halo2IpaPasta,
        "pallas",
        Hash::new(KAIGI_AUTHORIZATION_PUBLIC_INPUTS_SCHEMA_V1).into(),
        hash,
    );
    vk.status = ConfidentialStatus::Active;
    vk.max_proof_bytes = 64 * 1024;
    vk.gas_schedule_id = Some("kaigi-authorization-v1-tests".into());
    vk.vk_len = key.carrier.bytes.len().try_into().unwrap();
    vk.key = Some(key.carrier.clone());
    let id = VerifyingKeyId::new("halo2/ipa", "kaigi-authorization-v1-tests");
    state.world.verifying_keys.insert(id, vk);
    state.zk.kaigi_authorization_vk = Some(VerifyingKeyRef {
        backend: "halo2/ipa".into(),
        name: "kaigi-authorization-v1-tests".into(),
    });
    let context = authorization_v1::context_from_ledger_v1(
        *state.network_id(),
        &record.id,
        &record.host,
        subject,
        sequence,
        action,
        &record.roster_root(),
    )
    .unwrap();
    let mut secret = Fp::from(secret).to_repr();
    let witness = KaigiAuthorizationWitnessV1::take_blinding(&mut secret).unwrap();
    assert_eq!(secret, [0; 32]);
    let outputs = compute_authorization_v1(&context, &witness).unwrap();
    let columns = KaigiAuthorizationPublicInputsV1 { context, outputs }.instance();
    let circuit = KaigiAuthorizationCircuitV1::new(context, witness).unwrap();
    let mut transcript =
        Blake2bWrite::<Vec<u8>, EqAffine, Challenge255<EqAffine>>::init(Vec::new());
    create_proof::<IPACommitmentScheme<EqAffine>, ProverIPA<_>, _, _, _, _>(
        &key.params,
        &key.pk,
        &[circuit],
        &[&[&columns]],
        OsRng,
        &mut transcript,
    )
    .unwrap();
    let mut carrier = zk1::wrap_start();
    zk1::wrap_append_proof(&mut carrier, &transcript.finalize());
    zk1::wrap_append_instances_pasta_fp_cols(&[&columns], &mut carrier);
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: KAIGI_AUTHORIZATION_CIRCUIT_ID_V1.into(),
        vk_hash: hash,
        public_inputs: KAIGI_AUTHORIZATION_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        proof_bytes: carrier,
        aux: Vec::new(),
    };
    AuthorizationFixtureV1 {
        commitment: KaigiParticipantCommitment {
            commitment: KaigiAuthorizationScalarV1::from_le_bytes(outputs.commitment.to_repr())
                .unwrap(),
        },
        nullifier: KaigiParticipantNullifier {
            digest: KaigiAuthorizationScalarV1::from_le_bytes(outputs.nullifier.to_repr()).unwrap(),
        },
        root: record.roster_root(),
        proof: norito::encode_canonical(&envelope).unwrap(),
    }
}

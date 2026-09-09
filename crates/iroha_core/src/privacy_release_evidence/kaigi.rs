//! Pure real final Kaigi proof fixtures for explicit four-validator release tests.
//!
//! This nonshipping module builds public governed-key instructions and canonical proof bytes.
//! It never reads or mutates StateTransaction, executes an instruction, installs a verifier,
//! or grants authority. Callers must obtain the complete record and NetworkId from the real
//! network and submit every resulting instruction with the correct account signature.
//! Fixed private fixture blindings are unsuitable for production; no witness bytes are exported.
//! TODO: Run and qualify the real four-validator lifecycle against the exact candidate daemon.

use crate::zk::hash_vk;
use halo2_proofs::{
    SerdeFormat,
    halo2curves::{
        ff::PrimeField as _,
        pasta::{EqAffine as Curve, Fp as Scalar},
    },
    plonk::{Circuit, ProvingKey, VerifyingKey, create_proof, keygen_pk, keygen_vk},
    poly::{
        commitment::ParamsProver,
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::ProverIPA,
        },
    },
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
};
use iroha_config::parameters::actual::VerifyingKeyRef;
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    confidential::ConfidentialStatus,
    isi::{
        CreateKaigi, EndKaigi, JoinKaigi, LeaveKaigi, RecordKaigiUsage,
        verifying_keys::RegisterVerifyingKey,
    },
    kaigi::{
        KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiRecord, NewKaigi,
        authorization::KaigiAuthorizationIdentitiesV1, scalar::KaigiAuthorizationScalarV1,
    },
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::{BackendTag, OpenVerifyEnvelope},
};
/// Exact circuit-owned action enum used only to select a real fixture witness relation.
pub use kaigi_zk::authorization_v1::KaigiAuthorizationActionV1;
use kaigi_zk::{
    authorization_v1::{
        KAIGI_AUTHORIZATION_CIRCUIT_ID_V1, KAIGI_AUTHORIZATION_CIRCUIT_K_V1,
        KAIGI_AUTHORIZATION_INSTANCE_ROWS_V1, KAIGI_AUTHORIZATION_PUBLIC_INPUTS_SCHEMA_V1,
        KaigiAuthorizationCircuitV1, KaigiAuthorizationContextV1, KaigiAuthorizationPublicInputsV1,
        KaigiAuthorizationWitnessV1, compute_authorization_v1,
    },
    usage_v1::{
        KAIGI_USAGE_CIRCUIT_ID_V1, KAIGI_USAGE_CIRCUIT_K_V1, KAIGI_USAGE_INSTANCE_ROWS_V1,
        KAIGI_USAGE_PUBLIC_INPUTS_SCHEMA_V1, KaigiUsageCircuitV1, KaigiUsageContextV1,
        KaigiUsagePublicInputsV1, compute_usage_v1,
    },
};
use rand_core_06::OsRng;
use std::sync::OnceLock;

const HOST_BLINDING: [u8; 32] = [0x17; 32];
const PARTICIPANT_BLINDING: [u8; 32] = [0x21; 32];
const AUTHORIZATION_VK_NAME: &str = "kaigi_authorization_current";
const USAGE_VK_NAME: &str = "kaigi_usage_current";

struct GovernedKey {
    params: ParamsIPA<Curve>,
    pk: ProvingKey<Curve>,
    carrier: VerifyingKeyBox,
    circuit_id: &'static str,
    schema: &'static [u8],
    name: &'static str,
}
impl GovernedKey {
    fn new<C: Circuit<Scalar>>(
        circuit: C,
        k: u32,
        circuit_id: &'static str,
        schema: &'static [u8],
        name: &'static str,
    ) -> Self {
        let params = ParamsIPA::new(k);
        let vk = keygen_vk(&params, &circuit).expect("final circuit verifying key");
        let pk = keygen_pk(&params, vk.clone(), &circuit).expect("final circuit proving key");
        let mut wire = zk1_envelope_start();
        zk1_append_ipa_k(&mut wire, k);
        zk1_append_circuit_id(&mut wire, circuit_id);
        zk1_append_vk_pasta(&mut wire, &vk);
        Self {
            params,
            pk,
            carrier: VerifyingKeyBox::new("halo2/ipa".into(), wire),
            circuit_id,
            schema,
            name,
        }
    }
    fn id(&self) -> VerifyingKeyId {
        VerifyingKeyId::new("halo2/ipa", self.name)
    }
    fn record(&self) -> VerifyingKeyRecord {
        let mut record = VerifyingKeyRecord::new(
            1,
            self.circuit_id,
            BackendTag::Halo2IpaPasta,
            "pallas",
            Hash::new(self.schema).into(),
            hash_vk(&self.carrier),
        );
        record.vk_len = self.carrier.bytes.len().try_into().expect("key length");
        record.status = ConfidentialStatus::Active;
        record.max_proof_bytes = 64 * 1024;
        record.gas_schedule_id = Some(self.name.into());
        record.key = Some(self.carrier.clone());
        record
    }
    fn proof<C: Circuit<Scalar>>(&self, circuit: C, instance: &[Scalar]) -> Vec<u8> {
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(Vec::new());
        create_proof::<
            IPACommitmentScheme<Curve>,
            ProverIPA<'_, Curve>,
            Challenge255<Curve>,
            _,
            _,
            _,
        >(
            &self.params,
            &self.pk,
            &[circuit],
            &[&[instance]],
            OsRng,
            &mut transcript,
        )
        .expect("real final IPA proof");
        let mut carrier = zk1_envelope_start();
        zk1_append_proof(&mut carrier, &transcript.finalize());
        zk1_append_instances_cols(&mut carrier, &[instance]);
        norito::encode_canonical(&OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: self.circuit_id.into(),
            vk_hash: hash_vk(&self.carrier),
            public_inputs: self.schema.to_vec(),
            proof_bytes: carrier,
            aux: Vec::new(),
        })
        .expect("canonical final proof envelope")
    }
}
fn authorization_key() -> &'static GovernedKey {
    static KEY: OnceLock<GovernedKey> = OnceLock::new();
    KEY.get_or_init(|| {
        GovernedKey::new(
            KaigiAuthorizationCircuitV1::default(),
            KAIGI_AUTHORIZATION_CIRCUIT_K_V1,
            KAIGI_AUTHORIZATION_CIRCUIT_ID_V1,
            KAIGI_AUTHORIZATION_PUBLIC_INPUTS_SCHEMA_V1,
            AUTHORIZATION_VK_NAME,
        )
    })
}
fn usage_key() -> &'static GovernedKey {
    static KEY: OnceLock<GovernedKey> = OnceLock::new();
    KEY.get_or_init(|| {
        GovernedKey::new(
            KaigiUsageCircuitV1::default(),
            KAIGI_USAGE_CIRCUIT_K_V1,
            KAIGI_USAGE_CIRCUIT_ID_V1,
            KAIGI_USAGE_PUBLIC_INPUTS_SCHEMA_V1,
            USAGE_VK_NAME,
        )
    })
}
fn witness(mut bytes: [u8; 32]) -> KaigiAuthorizationWitnessV1 {
    let witness = KaigiAuthorizationWitnessV1::take_blinding(&mut bytes)
        .expect("canonical nonzero fixture blinding");
    assert_eq!(bytes, [0; 32], "witness takes and erases source bytes");
    witness
}

/// Complete public authorization carrier produced by the final circuit.
#[derive(Clone)]
pub struct KaigiReleaseAuthorizationV1 {
    /// Canonical commitment to the fixture participation opening.
    pub commitment: KaigiParticipantCommitment,
    /// Action-separated canonical nullifier.
    pub nullifier: KaigiParticipantNullifier,
    /// Exact pre-action roster root supplied by the caller.
    pub root: Hash,
    /// Canonical real OpenVerifyEnvelope proof bytes.
    pub proof: Vec<u8>,
}
impl KaigiReleaseAuthorizationV1 {
    /// Build signed-host creation input; this does not submit it.
    #[must_use]
    pub fn create(&self, call: NewKaigi) -> CreateKaigi {
        CreateKaigi {
            call,
            commitment: Some(self.commitment.clone()),
            nullifier: Some(self.nullifier.clone()),
            roster_root: Some(self.root),
            proof: Some(self.proof.clone()),
        }
    }
    /// Build signed-participant join input; this does not submit it.
    #[must_use]
    pub fn join(&self, call_id: &KaigiId, participant: &AccountId) -> JoinKaigi {
        JoinKaigi {
            call_id: call_id.clone(),
            participant: participant.clone(),
            commitment: Some(self.commitment.clone()),
            nullifier: Some(self.nullifier.clone()),
            roster_root: Some(self.root),
            proof: Some(self.proof.clone()),
        }
    }
    /// Build signed-participant leave input; this does not submit it.
    #[must_use]
    pub fn leave(&self, call_id: &KaigiId, participant: &AccountId) -> LeaveKaigi {
        LeaveKaigi {
            call_id: call_id.clone(),
            participant: participant.clone(),
            commitment: Some(self.commitment.clone()),
            nullifier: Some(self.nullifier.clone()),
            roster_root: Some(self.root),
            proof: Some(self.proof.clone()),
        }
    }
    /// Build signed-host termination input; this does not submit it.
    #[must_use]
    pub fn end(&self, call_id: &KaigiId) -> EndKaigi {
        EndKaigi {
            call_id: call_id.clone(),
            ended_at_ms: None,
            commitment: Some(self.commitment.clone()),
            nullifier: Some(self.nullifier.clone()),
            roster_root: Some(self.root),
            proof: Some(self.proof.clone()),
        }
    }
}

/// Ordinary configuration references for the two governed final verifier records.
#[must_use]
pub fn kaigi_release_verifier_references_v1() -> [VerifyingKeyRef; 2] {
    [
        VerifyingKeyRef {
            backend: "halo2/ipa".into(),
            name: AUTHORIZATION_VK_NAME.into(),
        },
        VerifyingKeyRef {
            backend: "halo2/ipa".into(),
            name: USAGE_VK_NAME.into(),
        },
    ]
}

/// Build real governed-key registration instructions without installing either record.
/// The network must grant and exercise normal verifier-governance permission.
#[must_use]
pub fn kaigi_release_verifier_registrations_v1() -> [RegisterVerifyingKey; 2] {
    [authorization_key(), usage_key()].map(|key| RegisterVerifyingKey {
        id: key.id(),
        record: key.record(),
    })
}

/// Prove one final authorization against explicit full network and record context.
/// Fixed fixture blindings are chosen by host versus participant action; no state is trusted implicitly.
#[must_use]
pub fn build_kaigi_release_authorization_v1(
    network: NetworkId,
    record: &KaigiRecord,
    subject: &AccountId,
    sequence: u64,
    action: KaigiAuthorizationActionV1,
) -> KaigiReleaseAuthorizationV1 {
    let blinding = match action {
        KaigiAuthorizationActionV1::HostCreate | KaigiAuthorizationActionV1::HostEnd => {
            HOST_BLINDING
        }
        KaigiAuthorizationActionV1::Join | KaigiAuthorizationActionV1::Leave => {
            PARTICIPANT_BLINDING
        }
    };
    let identity = KaigiAuthorizationIdentitiesV1::new(network, &record.id, &record.host, subject)
        .expect("full canonical identity");
    let context = KaigiAuthorizationContextV1 {
        network_id: *network.as_bytes(),
        call_id: identity.call_id.words(),
        host_id: identity.host_id.words(),
        subject_id: identity.subject_id.words(),
        participation_sequence: sequence,
        action,
        pre_roster_root: record.roster_root().into(),
    };
    let witness = witness(blinding);
    let outputs =
        compute_authorization_v1(&context, &witness).expect("final authorization outputs");
    let instance = KaigiAuthorizationPublicInputsV1 { context, outputs }.instance();
    assert_eq!(instance.len(), KAIGI_AUTHORIZATION_INSTANCE_ROWS_V1);
    KaigiReleaseAuthorizationV1 {
        commitment: KaigiParticipantCommitment {
            commitment: KaigiAuthorizationScalarV1::from_le_bytes(outputs.commitment.to_repr())
                .unwrap(),
        },
        nullifier: KaigiParticipantNullifier {
            digest: KaigiAuthorizationScalarV1::from_le_bytes(outputs.nullifier.to_repr()).unwrap(),
        },
        root: record.roster_root(),
        proof: authorization_key().proof(
            KaigiAuthorizationCircuitV1::new(context, witness).unwrap(),
            &instance,
        ),
    }
}

/// Prove the next fixed-duration usage segment against the complete caller-supplied host context.
#[must_use]
pub fn build_kaigi_release_usage_v1(network: NetworkId, record: &KaigiRecord) -> RecordKaigiUsage {
    let identity =
        KaigiAuthorizationIdentitiesV1::new(network, &record.id, &record.host, &record.host)
            .unwrap();
    let context = KaigiUsageContextV1 {
        network_id: *network.as_bytes(),
        call_id: identity.call_id.words(),
        host_id: identity.host_id.words(),
        pre_roster_root: record.roster_root().into(),
        segment_index: record.segments_recorded,
        duration_ms: 1_200,
        billed_gas: 345,
    };
    let witness = witness(HOST_BLINDING);
    let outputs = compute_usage_v1(&context, &witness).unwrap();
    assert_eq!(
        outputs.host_commitment.to_repr(),
        record
            .host_commitment
            .as_ref()
            .unwrap()
            .commitment
            .to_le_bytes()
    );
    let instance = KaigiUsagePublicInputsV1 { context, outputs }.instance();
    assert_eq!(instance.len(), KAIGI_USAGE_INSTANCE_ROWS_V1);
    RecordKaigiUsage {
        call_id: record.id.clone(),
        duration_ms: context.duration_ms,
        billed_gas: context.billed_gas,
        usage_commitment: Some(
            KaigiAuthorizationScalarV1::from_le_bytes(outputs.usage_commitment.to_repr()).unwrap(),
        ),
        proof: Some(usage_key().proof(
            KaigiUsageCircuitV1::new(context, witness).unwrap(),
            &instance,
        )),
    }
}

fn zk1_envelope_start() -> Vec<u8> {
    b"ZK1\0".to_vec()
}
fn zk1_append_tlv(buf: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    buf.extend_from_slice(tag);
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload);
}
fn zk1_append_ipa_k(buf: &mut Vec<u8>, k: u32) {
    zk1_append_tlv(buf, b"IPAK", &k.to_le_bytes());
}
fn zk1_append_circuit_id(buf: &mut Vec<u8>, circuit_id: &str) {
    zk1_append_tlv(buf, b"CID1", circuit_id.as_bytes());
}
fn zk1_append_vk_pasta(buf: &mut Vec<u8>, vk: &VerifyingKey<Curve>) {
    let bytes = vk.to_bytes(SerdeFormat::Processed);
    zk1_append_tlv(buf, b"H2VK", &bytes);
}
fn zk1_append_proof(buf: &mut Vec<u8>, proof: &[u8]) {
    zk1_append_tlv(buf, b"PROF", proof);
}
fn zk1_append_instances_cols(buf: &mut Vec<u8>, columns: &[&[Scalar]]) {
    assert_eq!(columns.len(), 1, "final Kaigi has one instance column");
    let rows = columns[0].len();
    assert!(rows > 0);
    let mut payload = Vec::with_capacity(8 + rows * columns.len() * core::mem::size_of::<Scalar>());
    payload.extend_from_slice(&(columns.len() as u32).to_le_bytes());
    payload.extend_from_slice(&(rows as u32).to_le_bytes());
    for row in 0..rows {
        for column in columns {
            payload.extend_from_slice(column[row].to_repr().as_ref());
        }
    }
    zk1_append_tlv(buf, b"I10P", &payload);
}

#[cfg(all(test, feature = "zk-halo2-ipa"))]
mod tests;

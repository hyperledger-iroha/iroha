#![doc = "Integration coverage for Kaigi privacy-mode instruction execution."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(
    feature = "zk-tests",
    feature = "zk-halo2",
    feature = "halo2-dev-tests"
))]
use core::num::NonZeroU64;
use halo2_proofs::{
    SerdeFormat,
    halo2curves::{
        ff::PrimeField as _,
        pasta::{EqAffine as Curve, Fp as Scalar},
    },
    plonk::{VerifyingKey, create_proof, keygen_pk, keygen_vk},
    poly::commitment::ParamsProver,
    poly::ipa::{
        commitment::{IPACommitmentScheme, ParamsIPA},
        multiopen::ProverIPA,
    },
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
};
use iroha_config::parameters::actual::VerifyingKeyRef;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, WorldReadOnly},
    zk::hash_vk,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    block::BlockHeader,
    confidential::ConfidentialStatus,
    isi::{CreateKaigi, JoinKaigi, RecordKaigiUsage},
    kaigi::{
        KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
        KaigiRecord, NewKaigi, kaigi_metadata_key,
    },
    metadata::Metadata,
    name::Name,
    prelude::*,
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::{BackendTag, OpenVerifyEnvelope},
};
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use kaigi_zk::{
    KAIGI_ROSTER_BACKEND, KAIGI_ROSTER_CANONICAL_CIRCUIT_ID, KAIGI_ROSTER_CIRCUIT_K,
    KAIGI_ROSTER_PUBLIC_INPUTS_SCHEMA_V1, KAIGI_USAGE_BACKEND, KAIGI_USAGE_CANONICAL_CIRCUIT_ID,
    KAIGI_USAGE_CIRCUIT_K, KAIGI_USAGE_PUBLIC_INPUTS_SCHEMA_V1, KaigiRosterJoinCircuit,
    KaigiUsageCommitmentCircuit, compute_commitment, compute_commitment_bytes, compute_nullifier,
    compute_nullifier_bytes, compute_usage_commitment, compute_usage_commitment_bytes,
    empty_roster_root_hash, roster_root_limbs,
};
use rand_core_06::OsRng;
use std::{str::FromStr, time::Duration};
#[path = "common/world_fixture.rs"]
mod test_world;
const ROSTER_VK_NAME: &str = "kaigi_roster_current";
const USAGE_VK_NAME: &str = "kaigi_usage_current";
struct RosterArtifacts {
    vk_id: VerifyingKeyId,
    vk_record: VerifyingKeyRecord,
    vk_ref: VerifyingKeyRef,
    join_commitment: KaigiParticipantCommitment,
    join_nullifier: KaigiParticipantNullifier,
    join_proof: Vec<u8>,
    join_roster_root: Hash,
}
struct UsageArtifacts {
    vk_id: VerifyingKeyId,
    vk_record: VerifyingKeyRecord,
    vk_ref: VerifyingKeyRef,
    duration_ms: u64,
    billed_gas: u64,
    usage_commitment: Hash,
    proof: Vec<u8>,
}
fn build_roster_artifacts() -> RosterArtifacts {
    let params: ParamsIPA<Curve> = ParamsIPA::new(KAIGI_ROSTER_CIRCUIT_K);
    let vk_base = keygen_vk(&params, &KaigiRosterJoinCircuit::default()).expect("vk");
    let mut vk_env = zk1_envelope_start();
    zk1_append_ipa_k(&mut vk_env, KAIGI_ROSTER_CIRCUIT_K);
    zk1_append_circuit_id(&mut vk_env, KAIGI_ROSTER_CANONICAL_CIRCUIT_ID);
    zk1_append_vk_pasta(&mut vk_env, &vk_base);
    let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vk_env.clone());
    let vk_commitment = hash_vk(&vk_box);
    let schema_hash: [u8; 32] = Hash::new(KAIGI_ROSTER_PUBLIC_INPUTS_SCHEMA_V1).into();
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        KAIGI_ROSTER_BACKEND,
        BackendTag::Halo2IpaPasta,
        "pallas",
        schema_hash,
        vk_commitment,
    );
    vk_record.vk_len = vk_env.len() as u32;
    vk_record.status = ConfidentialStatus::Active;
    vk_record.gas_schedule_id = Some(ROSTER_VK_NAME.to_string());
    vk_record.key = Some(vk_box);
    let vk_id = VerifyingKeyId::new("halo2/ipa", ROSTER_VK_NAME);
    let vk_ref = VerifyingKeyRef {
        backend: "halo2/ipa".to_string(),
        name: ROSTER_VK_NAME.to_string(),
    };
    let account_idx = 11u64;
    let domain_salt = 31u64;
    let join_nullifier_seed = 57u64;
    let commitment_hash = Hash::prehashed(compute_commitment_bytes(account_idx, domain_salt));
    let join_nullifier_hash =
        Hash::prehashed(compute_nullifier_bytes(account_idx, join_nullifier_seed));
    let join_commitment = KaigiParticipantCommitment {
        commitment: commitment_hash,
        alias_tag: Some("participant".to_string()),
    };
    let join_nullifier = KaigiParticipantNullifier {
        digest: join_nullifier_hash,
        issued_at_ms: 1,
    };
    let initial_root = empty_roster_root_hash();
    let join_proof = build_roster_envelope(
        &params,
        &vk_base,
        account_idx,
        domain_salt,
        join_nullifier_seed,
        &initial_root,
        vk_commitment,
    );
    vk_record.max_proof_bytes = join_proof
        .len()
        .try_into()
        .expect("proof size fits into u32");
    RosterArtifacts {
        vk_id,
        vk_record,
        vk_ref,
        join_commitment,
        join_nullifier,
        join_proof,
        join_roster_root: initial_root,
    }
}
fn build_usage_artifacts() -> UsageArtifacts {
    let params: ParamsIPA<Curve> = ParamsIPA::new(KAIGI_USAGE_CIRCUIT_K);
    let vk_base = keygen_vk(&params, &KaigiUsageCommitmentCircuit::default()).expect("vk");
    let mut vk_env = zk1_envelope_start();
    zk1_append_ipa_k(&mut vk_env, KAIGI_USAGE_CIRCUIT_K);
    zk1_append_circuit_id(&mut vk_env, KAIGI_USAGE_CANONICAL_CIRCUIT_ID);
    zk1_append_vk_pasta(&mut vk_env, &vk_base);
    let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vk_env.clone());
    let vk_commitment = hash_vk(&vk_box);
    let schema_hash: [u8; 32] = Hash::new(KAIGI_USAGE_PUBLIC_INPUTS_SCHEMA_V1).into();
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        KAIGI_USAGE_BACKEND,
        BackendTag::Halo2IpaPasta,
        "pallas",
        schema_hash,
        vk_commitment,
    );
    vk_record.vk_len = vk_env.len() as u32;
    vk_record.status = ConfidentialStatus::Active;
    vk_record.gas_schedule_id = Some(USAGE_VK_NAME.to_string());
    vk_record.key = Some(vk_box);
    let vk_id = VerifyingKeyId::new("halo2/ipa", USAGE_VK_NAME);
    let vk_ref = VerifyingKeyRef {
        backend: "halo2/ipa".to_string(),
        name: USAGE_VK_NAME.to_string(),
    };
    let duration_ms = 1_200u64;
    let billed_gas = 345u64;
    let segment_index = 0u64;
    let commitment_hash = Hash::prehashed(compute_usage_commitment_bytes(
        duration_ms,
        billed_gas,
        segment_index,
    ));
    let proof_bytes = build_usage_envelope(
        &params,
        &vk_base,
        duration_ms,
        billed_gas,
        segment_index,
        vk_commitment,
    );
    vk_record.max_proof_bytes = proof_bytes
        .len()
        .try_into()
        .expect("proof size fits into u32");
    UsageArtifacts {
        vk_id,
        vk_record,
        vk_ref,
        duration_ms,
        billed_gas,
        usage_commitment: commitment_hash,
        proof: proof_bytes,
    }
}
fn build_roster_envelope(
    params: &ParamsIPA<Curve>,
    vk: &VerifyingKey<Curve>,
    account_idx: u64,
    domain_salt: u64,
    nullifier_seed: u64,
    roster_root: &Hash,
    vk_commitment: [u8; 32],
) -> Vec<u8> {
    let account_scalar = Scalar::from(account_idx);
    let domain_scalar = Scalar::from(domain_salt);
    let nullifier_scalar = Scalar::from(nullifier_seed);
    let root_scalars = roster_root_limbs(roster_root);
    let circuit = KaigiRosterJoinCircuit::new(
        account_scalar,
        domain_scalar,
        nullifier_scalar,
        root_scalars,
    );
    let pk = keygen_pk(params, vk.clone(), &circuit).expect("pk");
    let commitment_scalar = compute_commitment(account_scalar, domain_scalar);
    let roster_nullifier = compute_nullifier(account_scalar, nullifier_scalar);
    let mut inst_cols = vec![vec![commitment_scalar], vec![roster_nullifier]];
    inst_cols.extend(root_scalars.iter().map(|scalar| vec![*scalar]));
    let inst_refs: Vec<&[Scalar]> = inst_cols.iter().map(Vec::as_slice).collect();
    let proof_instances = vec![inst_refs.as_slice()];
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(Vec::new());
    create_proof::<IPACommitmentScheme<Curve>, ProverIPA<'_, Curve>, Challenge255<Curve>, _, _, _>(
        params,
        &pk,
        &[circuit],
        &proof_instances,
        OsRng,
        &mut transcript,
    )
    .expect("create proof");
    let proof_payload = transcript.finalize();
    let mut zk1 = zk1_envelope_start();
    zk1_append_proof(&mut zk1, &proof_payload);
    zk1_append_instances_cols(&mut zk1, &inst_refs);
    let envelope = iroha_data_model::zk::OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: KAIGI_ROSTER_BACKEND.to_string(),
        vk_hash: vk_commitment,
        public_inputs: KAIGI_ROSTER_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        proof_bytes: zk1,
        aux: Vec::new(),
    };
    norito::encode_canonical(&envelope).expect("serialize canonical roster envelope")
}
fn build_usage_envelope(
    params: &ParamsIPA<Curve>,
    vk: &VerifyingKey<Curve>,
    duration_ms: u64,
    billed_gas: u64,
    segment_index: u64,
    vk_commitment: [u8; 32],
) -> Vec<u8> {
    let duration_scalar = Scalar::from(duration_ms);
    let billed_scalar = Scalar::from(billed_gas);
    let segment_scalar = Scalar::from(segment_index);
    let circuit = KaigiUsageCommitmentCircuit::new(duration_scalar, billed_scalar, segment_scalar);
    let pk = keygen_pk(params, vk.clone(), &circuit).expect("pk");
    let commitment_scalar =
        compute_usage_commitment(duration_scalar, billed_scalar, segment_scalar);
    let inst_cols = vec![vec![commitment_scalar]];
    let inst_refs: Vec<&[Scalar]> = inst_cols.iter().map(Vec::as_slice).collect();
    let proof_instances = vec![inst_refs.as_slice()];
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(Vec::new());
    create_proof::<IPACommitmentScheme<Curve>, ProverIPA<'_, Curve>, Challenge255<Curve>, _, _, _>(
        params,
        &pk,
        &[circuit],
        &proof_instances,
        OsRng,
        &mut transcript,
    )
    .expect("create usage proof");
    let proof_payload = transcript.finalize();
    let mut zk1 = zk1_envelope_start();
    zk1_append_proof(&mut zk1, &proof_payload);
    zk1_append_instances_cols(&mut zk1, &inst_refs);
    let envelope = iroha_data_model::zk::OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: KAIGI_USAGE_BACKEND.to_string(),
        vk_hash: vk_commitment,
        public_inputs: KAIGI_USAGE_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        proof_bytes: zk1,
        aux: Vec::new(),
    };
    norito::encode_canonical(&envelope).expect("serialize canonical usage envelope")
}
fn mutate_open_verify_envelope(
    proof: &[u8],
    mutate: impl FnOnce(&mut OpenVerifyEnvelope),
) -> Vec<u8> {
    let mut envelope: OpenVerifyEnvelope =
        norito::decode_canonical(proof).expect("decode canonical OpenVerifyEnvelope fixture");
    mutate(&mut envelope);
    norito::encode_canonical(&envelope).expect("serialize mutated OpenVerifyEnvelope")
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
    if columns.is_empty() {
        return;
    }
    let rows = columns[0].len();
    if columns.iter().any(|column| column.len() != rows) {
        return;
    }
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
#[test]
fn kaigi_privacy_join_fails_closed_until_authority_is_bound() {
    let roster = build_roster_artifacts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state =
        State::new_for_testing(test_world::world_with_test_accounts(), kura, query_handle);
    state.zk.halo2.enabled = true;
    state.zk.verify_timeout = Duration::ZERO;
    state.zk.kaigi_roster_join_vk = Some(roster.vk_ref.clone());
    state.zk.kaigi_roster_leave_vk = Some(roster.vk_ref.clone());
    let domain_id = DomainId::try_new("kaigi", "universal").expect("domain id");
    let (host, _) = gen_account_in("kaigi");
    let (participant, _) = gen_account_in("kaigi");
    let call_id = KaigiId::new(domain_id.clone(), Name::from_str("privacy").unwrap());
    let header1 = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut tx1 = block1.transaction();
    use iroha_data_model::permission::Permission;
    let manage_vk = Permission::new("CanManageVerifyingKeys".parse().unwrap(), Json::new(()));
    Grant::account_permission(manage_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut tx1)
        .expect("grant vk manage permission");
    iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: roster.vk_id.clone(),
        record: roster.vk_record.clone(),
    }
    .execute(&ALICE_ID, &mut tx1)
    .expect("register roster vk");
    Register::domain(Domain::new(domain_id.clone()))
        .execute(&ALICE_ID, &mut tx1)
        .expect("register domain");
    Register::account(Account::new(host.clone()))
        .execute(&ALICE_ID, &mut tx1)
        .expect("register host");
    Register::account(Account::new(participant.clone()))
        .execute(&ALICE_ID, &mut tx1)
        .expect("register participant");
    let mut template = NewKaigi::with_defaults(call_id.clone(), host.clone());
    template.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
    template.metadata = Metadata::default();
    CreateKaigi {
        call: template,
        commitment: None,
        nullifier: None,
        roster_root: None,
        proof: None,
    }
    .execute(&host, &mut tx1)
    .expect("create kaigi");
    let err = JoinKaigi {
        call_id: call_id.clone(),
        participant: participant.clone(),
        commitment: Some(roster.join_commitment.clone()),
        nullifier: Some(roster.join_nullifier.clone()),
        roster_root: Some(roster.join_roster_root.clone()),
        proof: Some(roster.join_proof.clone()),
    }
    .execute(&participant, &mut tx1)
    .expect_err("unbound private-roster statement must fail closed");
    assert!(
        format!("{err:?}").contains("binds the signed participant authority"),
        "unexpected rejection: {err:?}"
    );
    tx1.apply();
    block1.commit().expect("commit block1");
    let view = state.view();
    let domain = view.world.domain(&domain_id).expect("domain exists");
    let key = kaigi_metadata_key(&call_id.call_name).expect("metadata key");
    let stored = domain
        .metadata()
        .get(&key)
        .cloned()
        .expect("kaigi record stored");
    let record: KaigiRecord = stored.try_into_any_norito().expect("decode record");
    assert!(record.roster_commitments.is_empty());
    assert!(record.nullifier_log.is_empty());
    assert!(record.participants.is_empty());
}
#[test]
fn kaigi_privacy_join_fail_closed_precedes_proof_metadata_dispatch() {
    enum Tamper {
        AuxiliaryBytes,
        ZeroVerifierKeyHash,
    }
    for tamper in [Tamper::AuxiliaryBytes, Tamper::ZeroVerifierKeyHash] {
        let roster = build_roster_artifacts();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_for_testing(test_world::world_with_test_accounts(), kura, query_handle);
        state.zk.halo2.enabled = true;
        state.zk.verify_timeout = Duration::ZERO;
        state.zk.kaigi_roster_join_vk = Some(roster.vk_ref.clone());
        let domain_id = DomainId::try_new("kaigi", "universal").expect("domain id");
        let (host, _) = gen_account_in("kaigi");
        let (participant, _) = gen_account_in("kaigi");
        let call_id = KaigiId::new(domain_id.clone(), Name::from_str("privacy").unwrap());
        let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        use iroha_data_model::permission::Permission;
        let manage_vk = Permission::new("CanManageVerifyingKeys".parse().unwrap(), Json::new(()));
        Grant::account_permission(manage_vk, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut tx)
            .expect("grant vk manage permission");
        iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
            id: roster.vk_id.clone(),
            record: roster.vk_record.clone(),
        }
        .execute(&ALICE_ID, &mut tx)
        .expect("register roster vk");
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut tx)
            .expect("register domain");
        Register::account(Account::new(host.clone()))
            .execute(&ALICE_ID, &mut tx)
            .expect("register host");
        Register::account(Account::new(participant.clone()))
            .execute(&ALICE_ID, &mut tx)
            .expect("register participant");
        let mut template = NewKaigi::with_defaults(call_id.clone(), host.clone());
        template.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
        template.metadata = Metadata::default();
        CreateKaigi {
            call: template,
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .execute(&host, &mut tx)
        .expect("create kaigi");
        let proof = match tamper {
            Tamper::AuxiliaryBytes => mutate_open_verify_envelope(&roster.join_proof, |envelope| {
                envelope.aux = b"ignored-hint".to_vec();
            }),
            Tamper::ZeroVerifierKeyHash => {
                mutate_open_verify_envelope(&roster.join_proof, |envelope| {
                    envelope.vk_hash = [0u8; 32];
                })
            }
        };
        let err = JoinKaigi {
            call_id: call_id.clone(),
            participant: participant.clone(),
            commitment: Some(roster.join_commitment.clone()),
            nullifier: Some(roster.join_nullifier.clone()),
            roster_root: Some(roster.join_roster_root),
            proof: Some(proof),
        }
        .execute(&participant, &mut tx)
        .expect_err("noncanonical privacy proof envelope must be rejected");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("binds the signed participant authority"),
            "unexpected fail-closed rejection: {msg:?}"
        );
    }
}
#[test]
fn usage_summary_emitted_on_record_usage() {
    let roster = build_roster_artifacts();
    let usage = build_usage_artifacts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state =
        State::new_for_testing(test_world::world_with_test_accounts(), kura, query_handle);
    state.zk.halo2.enabled = true;
    state.zk.verify_timeout = Duration::ZERO;
    state.zk.kaigi_roster_join_vk = Some(roster.vk_ref.clone());
    state.zk.kaigi_roster_leave_vk = Some(roster.vk_ref.clone());
    state.zk.kaigi_usage_vk = Some(usage.vk_ref.clone());
    let domain_id = DomainId::try_new("kaigi", "universal").expect("domain id");
    let (host, _) = gen_account_in("kaigi");
    let call_id = KaigiId::new(domain_id.clone(), Name::from_str("usage").unwrap());
    let mut template = NewKaigi::with_defaults(call_id.clone(), host.clone());
    template.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    use iroha_data_model::permission::Permission;
    let manage_vk = Permission::new("CanManageVerifyingKeys".parse().unwrap(), Json::new(()));
    Grant::account_permission(manage_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut tx)
        .expect("grant vk manage");
    iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: roster.vk_id.clone(),
        record: roster.vk_record.clone(),
    }
    .execute(&ALICE_ID, &mut tx)
    .expect("register roster vk");
    iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: usage.vk_id.clone(),
        record: usage.vk_record.clone(),
    }
    .execute(&ALICE_ID, &mut tx)
    .expect("register usage vk");
    Register::domain(Domain::new(domain_id.clone()))
        .execute(&ALICE_ID, &mut tx)
        .expect("register domain");
    Register::account(Account::new(host.clone()))
        .execute(&ALICE_ID, &mut tx)
        .expect("register host");
    CreateKaigi {
        call: template,
        commitment: None,
        nullifier: None,
        roster_root: None,
        proof: None,
    }
    .execute(&host, &mut tx)
    .expect("create kaigi");
    tx.world.take_external_events();
    RecordKaigiUsage {
        call_id: call_id.clone(),
        duration_ms: usage.duration_ms,
        billed_gas: usage.billed_gas,
        usage_commitment: Some(usage.usage_commitment),
        proof: Some(usage.proof.clone()),
    }
    .execute(&host, &mut tx)
    .expect("record usage");
    let events = tx.world.take_external_events();
    let summary = events.iter().find_map(|event| {
        if let EventBox::Data(ev) = event {
            if let DataEvent::Domain(DomainEvent::KaigiUsageSummary(summary)) = ev.as_ref() {
                return Some(summary.clone());
            }
        }
        None
    });
    let summary = summary.expect("usage summary event emitted");
    assert_eq!(summary.call, call_id);
    assert_eq!(summary.total_duration_ms, usage.duration_ms);
    assert_eq!(summary.total_billed_gas, usage.billed_gas);
    assert_eq!(summary.segments_recorded, 1);
}

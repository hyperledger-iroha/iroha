//! Audit coverage for vote tally proofs: ensure the production Halo2/IPA circuit accepts valid envelopes and rejects tampering.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]

#[cfg(all(
    feature = "halo2-dev-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
#[path = "zk_testkit.rs"]
mod zk_testkit;

#[cfg(all(
    feature = "halo2-dev-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use iroha_core::{
        executor::Executor,
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute,
        state::{State, World},
        zk::{self as zk_backend, hash_proof},
    };
    use iroha_data_model::{
        Registrable, ValidationFail,
        account::Account,
        asset::AssetDefinition,
        block::BlockHeader,
        domain::Domain,
        isi::{Grant, verifying_keys, zk as zk_isi},
        permission::Permission,
        prelude::InstructionBox,
        proof::{ProofAttachment, ProofBox, VerifyingKeyId, VerifyingKeyRecord},
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;
    use nonzero_ext::nonzero;

    use super::zk_testkit;

    #[test]
    fn vote_tally_proof_verifies_with_inline_vk() {
        let bundle = zk_testkit::vote_merkle8_bundle();
        let backend = bundle.backend;
        let proof_box = ProofBox::new(backend.into(), bundle.proof_bytes.clone());
        let vk_inline = bundle
            .vk_record
            .key
            .as_ref()
            .expect("bundle must include inline verifying key")
            .clone();

        assert!(
            zk_backend::verify_backend(backend, &proof_box, Some(&vk_inline)),
            "expected untampered proof to verify"
        );
    }

    #[test]
    fn vote_tally_proof_rejects_commit_tampering() {
        let bundle = zk_testkit::vote_merkle8_bundle();
        let backend = bundle.backend;
        let vk_inline = bundle
            .vk_record
            .key
            .as_ref()
            .expect("bundle must include inline verifying key")
            .clone();

        let tampered_bytes = tamper_instance_column(bundle.proof_bytes.clone(), 0);
        let proof_box = ProofBox::new(backend.into(), tampered_bytes);

        assert!(
            !zk_backend::verify_backend(backend, &proof_box, Some(&vk_inline)),
            "commit column tampering must be rejected"
        );
    }

    #[test]
    fn vote_tally_proof_rejects_root_tampering() {
        let bundle = zk_testkit::vote_merkle8_bundle();
        let backend = bundle.backend;
        let vk_inline = bundle
            .vk_record
            .key
            .as_ref()
            .expect("bundle must include inline verifying key")
            .clone();

        let tampered_bytes = tamper_instance_column(bundle.proof_bytes.clone(), 1);
        let proof_box = ProofBox::new(backend.into(), tampered_bytes);

        assert!(
            !zk_backend::verify_backend(backend, &proof_box, Some(&vk_inline)),
            "root column tampering must be rejected"
        );
    }

    #[test]
    fn vote_tally_proof_verifies_with_registered_vk() {
        let state = new_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        grant_manage_vk(&mut block);
        let exec = Executor::default();

        let bundle = zk_testkit::vote_merkle8_bundle();
        register_vk(&exec, &mut block, &bundle.vk_id, &bundle.vk_record);

        let proof = ProofBox::new(bundle.backend.into(), bundle.proof_bytes.clone());
        let attachment =
            ProofAttachment::new_ref(bundle.backend.into(), proof, bundle.vk_id.clone());

        execute_verify_proof(&mut block, attachment, Some(true))
            .expect("canonical vote tally proof should verify");
    }

    #[test]
    fn vote_tally_schema_hash_guard_rejects_commit_tamper() {
        let state = new_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        grant_manage_vk(&mut block);
        let exec = Executor::default();

        let bundle = zk_testkit::vote_merkle8_bundle();
        register_vk(&exec, &mut block, &bundle.vk_id, &bundle.vk_record);

        let tampered = tamper_instance_column(bundle.proof_bytes.clone(), 0);
        let proof = ProofBox::new(bundle.backend.into(), tampered);
        let attachment =
            ProofAttachment::new_ref(bundle.backend.into(), proof, bundle.vk_id.clone());

        let err = execute_verify_proof(&mut block, attachment, Some(true))
            .expect_err("tampered commit must be rejected");
        assert_schema_hash_violation(err);
    }

    #[test]
    fn vote_tally_schema_hash_guard_rejects_root_tamper() {
        let state = new_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        grant_manage_vk(&mut block);
        let exec = Executor::default();

        let bundle = zk_testkit::vote_merkle8_bundle();
        register_vk(&exec, &mut block, &bundle.vk_id, &bundle.vk_record);

        let tampered = tamper_instance_column(bundle.proof_bytes.clone(), 1);
        let proof = ProofBox::new(bundle.backend.into(), tampered);
        let attachment =
            ProofAttachment::new_ref(bundle.backend.into(), proof, bundle.vk_id.clone());

        let err = execute_verify_proof(&mut block, attachment, Some(true))
            .expect_err("tampered root must be rejected");
        assert_schema_hash_violation(err);
    }

    #[test]
    fn vote_tally_schema_hash_guard_rejects_registry_tamper() {
        let state = new_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        grant_manage_vk(&mut block);
        let exec = Executor::default();

        let bundle = zk_testkit::vote_merkle8_bundle();
        let mut record: VerifyingKeyRecord = bundle.vk_record.clone();
        record.public_inputs_schema_hash = [0xEE; 32];
        register_vk(&exec, &mut block, &bundle.vk_id, &record);

        let proof = ProofBox::new(bundle.backend.into(), bundle.proof_bytes.clone());
        let attachment =
            ProofAttachment::new_ref(bundle.backend.into(), proof, bundle.vk_id.clone());

        let err = execute_verify_proof(&mut block, attachment, Some(true))
            .expect_err("registry hash tamper must be rejected");
        assert_schema_hash_violation(err);
    }

    /// Flip the lowest byte of the requested public input column (0 = commit, 1 = root).
    ///
    /// For `halo2/ipa` proofs this tampers both the outer `OpenVerifyEnvelope.public_inputs`
    /// bytes and the inner ZK1 instance column.
    fn tamper_instance_column(envelope: Vec<u8>, column_index: usize) -> Vec<u8> {
        if let Ok(mut env) =
            norito::decode_from_bytes::<iroha_data_model::zk::OpenVerifyEnvelope>(&envelope)
        {
            let offset = column_index
                .checked_mul(32)
                .expect("public input offset must not overflow");
            let target = env
                .public_inputs
                .get_mut(offset)
                .expect("public input column within envelope bounds");
            *target ^= 0x01;
            env.proof_bytes = tamper_zk1_instance_column(env.proof_bytes, column_index);
            return norito::to_bytes(&env)
                .expect("tampered OpenVerifyEnvelope must serialize with Norito");
        }

        tamper_zk1_instance_column(envelope, column_index)
    }

    fn tamper_zk1_instance_column(mut envelope: Vec<u8>, column_index: usize) -> Vec<u8> {
        const TAG: &[u8; 4] = b"I10P";
        let tag_pos = envelope
            .windows(TAG.len())
            .position(|window| window == TAG)
            .expect("I10P TLV not found in proof envelope");
        let length_start = tag_pos + TAG.len();
        let length = u32::from_le_bytes(
            envelope[length_start..length_start + 4]
                .try_into()
                .expect("length bytes"),
        ) as usize;
        let payload_start = length_start + 4;
        let payload_end = payload_start + length;
        assert!(
            payload_end <= envelope.len(),
            "I10P payload truncated: length {}, buffer {}",
            length,
            envelope.len()
        );

        let cols = u32::from_le_bytes(
            envelope[payload_start..payload_start + 4]
                .try_into()
                .expect("columns bytes"),
        ) as usize;
        let rows = u32::from_le_bytes(
            envelope[payload_start + 4..payload_start + 8]
                .try_into()
                .expect("rows bytes"),
        ) as usize;

        assert_eq!(rows, 1, "vote tally bundle should expose a single row");
        assert!(
            column_index < cols,
            "column {} out of bounds (cols = {})",
            column_index,
            cols
        );

        let scalars_start = payload_start + 8;
        let offset = scalars_start + column_index * 32;
        let target = envelope
            .get_mut(offset)
            .expect("scalar column within envelope bounds");
        *target ^= 0x01;

        envelope
    }

    fn assert_schema_hash_violation(err: ValidationFail) {
        match err {
            ValidationFail::InstructionFailed(
                iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(msg),
            ) => assert!(
                msg.contains("schema hash"),
                "expected schema hash violation, got: {msg}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn new_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let alice_id = (*ALICE_ID).clone();
        let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().expect("domain");
        let domain = Domain::new(domain_id.clone()).build(&alice_id);
        let alice = Account::new(alice_id.clone().to_account_id(domain_id)).build(&alice_id);
        let world = World::with([domain], [alice], Vec::<AssetDefinition>::new());
        let mut state = State::new_for_testing(world, kura, query_handle);
        state.zk.halo2.enabled = true;
        state.zk.verify_timeout = std::time::Duration::ZERO;
        state
    }

    #[allow(clippy::disallowed_types)]
    type PreverifiedMap = BTreeMap<[u8; 32], bool>;

    fn grant_manage_vk(block: &mut iroha_core::state::StateBlock<'_>) {
        let mut stx = block.transaction();
        let perm = Permission::new(
            "CanManageVerifyingKeys"
                .parse()
                .expect("permission identifier"),
            Json::new(()),
        );
        Grant::account_permission(perm, ALICE_ID.clone())
            .execute(&ALICE_ID.clone(), &mut stx)
            .expect("grant manage verifying keys");
        stx.apply();
    }

    fn register_vk(
        exec: &Executor,
        block: &mut iroha_core::state::StateBlock<'_>,
        id: &VerifyingKeyId,
        record: &VerifyingKeyRecord,
    ) {
        let mut stx = block.transaction();
        let instr: InstructionBox = verifying_keys::RegisterVerifyingKey {
            id: id.clone(),
            record: record.clone(),
        }
        .into();
        exec.execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
            .expect("register verifying key");
        stx.apply();
    }

    fn execute_verify_proof(
        block: &mut iroha_core::state::StateBlock<'_>,
        attachment: ProofAttachment,
        preverified: Option<bool>,
    ) -> Result<(), ValidationFail> {
        let batch = match preverified {
            Some(result) => {
                let mut map = PreverifiedMap::new();
                map.insert(hash_proof(&attachment.proof), result);
                Arc::new(map)
            }
            None => Arc::new(PreverifiedMap::new()),
        };
        block.set_preverified_batch(batch);
        let mut stx = block.transaction();
        let isi: InstructionBox = zk_isi::VerifyProof::new(attachment).into();
        let res = isi
            .execute(&ALICE_ID.clone(), &mut stx)
            .map_err(ValidationFail::InstructionFailed);
        if res.is_ok() {
            stx.apply();
        }
        res
    }
}

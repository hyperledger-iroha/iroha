//! Stage custody tests use mock archive/hardware and a manually admitted selection. Signed
//! envelopes and native BLS finality are genuine; inner monetary originals and execution
//! rows are synthetic. These tests do not qualify suffix monetary proofs or World execution.
use super::super::ledger_outcome::tests::{
    bytes, certify, genesis, ledger_wallet, original, rejected, restart, signed_action, verified,
};
use super::*;
use crate::kagemusha_wallet_state_v1::tests::{Wallet, fixture};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    sumeragi_finality::test_fixtures::NativeFinalityFixture,
    transaction::{
        ExecutableBatchItem, FeePaymentIntent, TransactionBuilder, executable::ContractInvocation,
    },
};
use iroha_sumeragi::message::Qc;
use std::{num::NonZeroU64, time::Duration};

fn key() -> KeyPair {
    KeyPair::from_seed(vec![87; 32], Algorithm::Ed25519)
}

fn action(wallet: [u8; 32], authority: [u8; 32]) -> KagemushaWalletLedgerActionV1 {
    KagemushaWalletLedgerActionV1::SuffixUnloadBegin {
        wallet,
        authority,
        account: iroha_data_model::account::AccountId::new(key().public_key().clone()),
        ordinal: 0,
        header: vec![1, 2, 3],
        anchor: vec![4, 5, 6],
    }
}

fn stage(w: &mut Wallet) -> SelectedStage {
    let (root, manifest) = w.sync_manifest().unwrap();
    let request_id = [11; 32];
    let authority = [12; 32];
    let instruction = archive::encode(&KagemushaWalletLedgerV1::new(
        w.scheme_id,
        action(w.wallet_id, authority),
    ))
    .unwrap();
    let instruction_digest = object_digest(&instruction);
    let suffix = SuffixArchiveManifestV1 {
        version: 1,
        discriminator: 2,
        scheme_id: w.scheme_id,
        wallet_id: w.wallet_id,
        requests: IndexRoot::default(),
        authorities: IndexRoot::default(),
        operations: IndexRoot::default(),
        source_pins: IndexRoot::default(),
        ledger_attempts: IndexRoot::default(),
        ledger_outcomes: IndexRoot::default(),
        ledger_stages: IndexRoot::default(),
        active_request: Some(request_id),
    };
    let digest = w
        .publish_suffix_checkpoint(root, &manifest, &suffix)
        .unwrap();
    let request = SuffixRequestRecordV1 {
        version: 1,
        discriminator: 3,
        scheme_id: w.scheme_id,
        wallet_id: w.wallet_id,
        request_id,
        request_original: [13; 32],
        source_checkpoint: root,
        source_capsule: [14; 32],
        source_operation: [15; 32],
        source_sequence: 5,
        source_selected_generation: 1,
        source_custody: [16; 32],
        anchor_original: [17; 32],
        anchor_credit_tree: [18; 32],
        anchor_pending: [19; 32],
        anchor_sequence: 4,
        pins: IndexRoot::default(),
        progress: None,
        selection: None,
        released: None,
    };
    let selection = SuffixPublicationSelectionV1 {
        version: 1,
        discriminator: 5,
        scheme_id: w.scheme_id,
        wallet_id: w.wallet_id,
        request_id,
        authority,
        source_checkpoint: root,
        source_capsule: request.source_capsule,
        source_operation: request.source_operation,
        source_sequence: request.source_sequence,
        anchor_original: request.anchor_original,
        header_original: [20; 32],
        begin_instruction_original: instruction_digest,
        chunk_originals: IndexRoot::default(),
        chunk_count: 1,
        stream_bytes: 3,
        step_count: 1,
        pins: IndexRoot::default(),
        native_choices: [21; 32],
        frozen_capsule: [22; 32],
        successor_custody: [23; 32],
        operation_id: [24; 32],
    };
    request.require((w.scheme_id, w.wallet_id)).unwrap();
    selection.require((w.scheme_id, w.wallet_id)).unwrap();
    SelectedStage {
        checkpoint: SelectedCheckpoint {
            digest,
            manifest,
            suffix: Some(suffix),
        },
        request,
        selection,
        instruction,
        instruction_digest,
        ordinal: 0,
    }
}

fn signed(native: &NativeFinalityFixture, stage: &SelectedStage, offset: u64) -> SignedTransaction {
    signed_action(
        native,
        &key(),
        stage.selection.scheme_id,
        action(stage.selection.wallet_id, stage.selection.authority),
        offset,
    )
}

fn refresh(w: &mut Wallet, stage: &mut SelectedStage) {
    stage.checkpoint = w.selected_suffix_checkpoint().unwrap().unwrap();
}

#[test]
fn signed_stage_rejects_foreign_network_instruction_and_noncanonical_wire() {
    let native = NativeFinalityFixture::start("suffix-signed-intake");
    let mut w = ledger_wallet(&native);
    let stage = stage(&mut w);
    let tx = signed(&native, &stage, 0);
    let wire = tx.encode_wire_v1().unwrap();
    signed_stage(&wire, native.network_id().as_bytes(), &stage.instruction).unwrap();
    assert!(signed_stage(&wire, &[9; 32], &stage.instruction).is_err());
    let mut different = stage.instruction.clone();
    different.push(0);
    assert!(signed_stage(&wire, native.network_id().as_bytes(), &different).is_err());
    let mut trailing = wire.clone();
    trailing.push(0);
    for invalid in [
        Vec::new(),
        trailing,
        vec![0; SUFFIX_SIGNED_WIRE_MAX_BYTES_V1 + 1],
    ] {
        assert!(
            signed_stage(&invalid, native.network_id().as_bytes(), &stage.instruction).is_err()
        );
    }
}

#[test]
fn signed_stage_requires_exactly_one_instruction_executable() {
    let native = NativeFinalityFixture::start("suffix-signed-executable");
    let mut w = ledger_wallet(&native);
    let stage = stage(&mut w);
    let expected: iroha_data_model::isi::InstructionBox = KagemushaWalletLedgerV1::new(
        stage.selection.scheme_id,
        action(stage.selection.wallet_id, stage.selection.authority),
    )
    .into();
    let call = ContractInvocation {
        contract_address: "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
            .parse()
            .unwrap(),
        expected_code_hash: iroha_crypto::Hash::new(b"suffix-unrelated-contract"),
        entrypoint: "run".to_owned(),
        arguments: None,
    };
    let invalid_executables = [
        Executable::Instructions(vec![].into()),
        Executable::Instructions(vec![expected.clone(), expected.clone()].into()),
        Executable::Batch(vec![ExecutableBatchItem::Instruction(expected.clone())].into()),
        Executable::Batch(
            vec![
                ExecutableBatchItem::Instruction(expected.clone()),
                ExecutableBatchItem::ContractCall(call.clone()),
            ]
            .into(),
        ),
        Executable::Batch(
            vec![
                ExecutableBatchItem::ContractCall(call),
                ExecutableBatchItem::Instruction(expected),
            ]
            .into(),
        ),
    ];
    for executable in invalid_executables {
        let gas_limit = executable
            .requires_transaction_gas_limit()
            .then(|| NonZeroU64::new(100_000).unwrap());
        let mut builder = TransactionBuilder::new(
            native.network_id(),
            iroha_data_model::account::AccountId::new(key().public_key().clone()),
            FeePaymentIntent::authority(vec![], gas_limit),
        );
        builder.set_creation_time(Duration::from_millis(
            native.next_header().creation_time_ms - 1,
        ));
        let tx = builder
            .with_executable(executable)
            .sign(key().private_key());
        tx.verify_signature().unwrap();
        let wire = tx.encode_wire_v1().unwrap();
        assert!(matches!(
            signed_stage(&wire, native.network_id().as_bytes(), &stage.instruction),
            Err(Error::Invalid("suffix stage exact instruction"))
        ));
    }
}

#[test]
fn retained_attempts_survive_restart_without_a_lifetime_eight_attempt_cap() {
    // Attempt `index` is signed `index` milliseconds before the next proposal. A genesis at
    // 20 ms gives all twenty attempts distinct nonzero creation times (20 down to 1 ms).
    let native = NativeFinalityFixture::start_at("suffix-many-attempts", 20);
    let mut w = ledger_wallet(&native);
    let mut stage = stage(&mut w);
    let mut originals = Vec::new();
    for index in 0..20 {
        let tx = signed(&native, &stage, index);
        let wire = tx.encode_wire_v1().unwrap();
        let entrypoint = ledger_outcome::entrypoint(&tx).unwrap();
        w.retain_suffix_attempt(&mut stage, entrypoint, &wire)
            .unwrap();
        originals.push((entrypoint, wire));
    }
    let before = w.custody.publication_calls;
    w.retain_suffix_attempt(&mut stage, originals[0].0, &originals[0].1)
        .unwrap();
    assert_eq!(w.custody.publication_calls, before);
    let attempts = w.retained_suffix_stage_attempts(&stage).unwrap().unwrap();
    assert_eq!(attempts.attempt_count, 20);
    let mut w = restart(w);
    refresh(&mut w, &mut stage);
    for (index, (entrypoint, wire)) in originals.iter().enumerate() {
        assert_eq!(
            attempts
                .attempts
                .get(&mut w.archive, &suffix_ordinal_key(index as u128))
                .unwrap(),
            Some(entrypoint.to_vec())
        );
        let (_, _, retained, _) = w
            .retained_signed_suffix_attempt(&stage, entrypoint)
            .unwrap();
        assert_eq!(&retained, wire);
    }
    w.archive.hide_object(object_digest(&originals[0].1));
    assert!(matches!(
        w.retained_signed_suffix_attempt(&stage, &originals[0].0),
        Err(Error::WitnessLost(_))
    ));
    w.archive.reveal_object(object_digest(&originals[0].1));
    stage.instruction.push(0);
    assert!(matches!(
        w.retained_signed_suffix_attempt(&stage, &originals[0].0),
        Err(Error::WitnessLost(_))
    ));
}

#[test]
fn readback_reauthenticates_network_even_when_all_archive_addresses_match() {
    let native = NativeFinalityFixture::start("suffix-readback-network");
    let foreign = NativeFinalityFixture::start_at("suffix-foreign-network", 2);
    assert_ne!(native.network_id(), foreign.network_id());
    let mut w = ledger_wallet(&native);
    let mut stage = stage(&mut w);
    let tx = signed(&foreign, &stage, 0);
    let wire = tx.encode_wire_v1().unwrap();
    let entrypoint = ledger_outcome::entrypoint(&tx).unwrap();
    // Fault injection bypasses intake. Even a self-consistent archive cannot make this
    // foreign signed transaction return through the retained-wire reader.
    w.retain_suffix_attempt(&mut stage, entrypoint, &wire)
        .unwrap();
    assert!(matches!(
        w.retained_signed_suffix_attempt(&stage, &entrypoint),
        Err(Error::WitnessLost(_))
    ));
}

#[test]
fn begin_debit_gate_requires_real_finality_and_exact_request_authority_after_restart() {
    let mut native = NativeFinalityFixture::start("suffix-begin-debit-gate");
    let mut w = ledger_wallet(&native);
    let mut stage = stage(&mut w);
    let authority = stage.selection.authority;
    let genesis = genesis(&native);
    assert!(
        !w.accepted_suffix_begin(&stage, &genesis, &authority)
            .unwrap()
    );
    let tx = signed(&native, &stage, 0);
    let wire = tx.encode_wire_v1().unwrap();
    let entrypoint = ledger_outcome::entrypoint(&tx).unwrap();
    let (attempt, digest) = w
        .retain_suffix_attempt(&mut stage, entrypoint, &wire)
        .unwrap();
    assert!(
        !w.accepted_suffix_begin(&stage, &genesis, &authority)
            .unwrap()
    );
    let block = certify(&mut native, std::slice::from_ref(&tx), vec![Ok(vec![])]);
    let original = original(&block, 0);
    let outcome = verified(&native, &original, &tx);
    let held = w
        .retain_verified_suffix_outcome(&stage, &attempt, digest, &outcome, &bytes(&original))
        .unwrap();
    let mut w = restart(w);
    refresh(&mut w, &mut stage);
    let before = w.custody.publication_calls;
    assert!(
        w.accepted_suffix_begin(&stage, &genesis, &authority)
            .unwrap()
    );
    assert!(matches!(
        w.accepted_suffix_begin(&stage, &genesis, &[88; 32]),
        Err(Error::OperationConflict)
    ));
    assert_eq!(w.custody.publication_calls, before);
    w.archive.hide_object(held.finality_original);
    assert!(
        w.accepted_suffix_begin(&stage, &genesis, &authority)
            .is_err()
    );
    w.archive.reveal_object(held.finality_original);
    // Re-signing the archive address is not a substitute for the quorum signature.
    let mut forged = original;
    let mut qc: Qc = norito::decode_canonical(&forged.certificate.commit_qc).unwrap();
    qc.agg_sig.0[0] ^= 1;
    forged.certificate.commit_qc = norito::encode_canonical(&qc).unwrap();
    let forged_bytes = bytes(&forged);
    let mut altered = held;
    altered.finality_original = w
        .archive
        .write_object(&forged_bytes, KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1)
        .unwrap();
    let record = suffix_custody::write_record(&mut w.archive, &altered).unwrap();
    let mut suffix = stage.suffix().clone();
    suffix.ledger_outcomes = suffix
        .ledger_outcomes
        .set(&mut w.archive, entrypoint, &record)
        .unwrap();
    w.publish_suffix_checkpoint(stage.checkpoint.digest, &stage.checkpoint.manifest, &suffix)
        .unwrap();
    refresh(&mut w, &mut stage);
    assert!(
        w.accepted_suffix_begin(&stage, &genesis, &authority)
            .is_err()
    );
}

#[test]
fn rejected_begin_is_not_debit_authority_and_only_finalize_is_a_payout() {
    let mut native = NativeFinalityFixture::start("suffix-rejected-begin");
    let mut w = ledger_wallet(&native);
    let mut stage = stage(&mut w);
    let tx = signed(&native, &stage, 0);
    let entrypoint = ledger_outcome::entrypoint(&tx).unwrap();
    let (attempt, digest) = w
        .retain_suffix_attempt(&mut stage, entrypoint, &tx.encode_wire_v1().unwrap())
        .unwrap();
    let block = certify(&mut native, std::slice::from_ref(&tx), vec![rejected()]);
    let original = original(&block, 0);
    let outcome = verified(&native, &original, &tx);
    let mut held = w
        .retain_verified_suffix_outcome(&stage, &attempt, digest, &outcome, &bytes(&original))
        .unwrap();
    refresh(&mut w, &mut stage);
    assert!(
        !w.accepted_suffix_begin(&stage, &genesis(&native), &stage.selection.authority)
            .unwrap()
    );
    assert!(matches!(
        stage.outcome(&held),
        SuffixStageOutcomeV1::Rejected(_)
    ));
    // Projection check only: do not persist this changed accepted bit.
    held.accepted = true;
    for ordinal in 0..stage.final_ordinal() {
        stage.ordinal = ordinal;
        assert!(matches!(
            stage.outcome(&held),
            SuffixStageOutcomeV1::StageConfirmed(_)
        ));
    }
    stage.ordinal = stage.final_ordinal();
    assert!(matches!(
        stage.outcome(&held),
        SuffixStageOutcomeV1::UnloadConfirmed(_)
    ));
}

#[test]
fn one_append_is_materialized_from_the_exact_released_chunk_and_selected_atomically() {
    let native = NativeFinalityFixture::start("suffix-lazy-append");
    let mut w = ledger_wallet(&native);
    let mut stage = stage(&mut w);
    let claim: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
    let mut public = claim.package.lineage.lineage().unwrap().public;
    public.scheme_id = w.scheme_id;
    public.wallet_id = w.wallet_id;
    let chunk = KagemushaWalletSuffixChunkV1 {
        version: 1,
        chunk_index: 0,
        stream_offset: 0,
        start: public,
        data: KagemushaWalletSuffixChunkV1::stream(&[vec![1, 2, 3]]).unwrap(),
    }
    .to_canonical_bytes()
    .unwrap();
    let chunk_digest =
        kagemusha_wallet_suffix_digest_v1(KagemushaWalletSuffixDigestRoleV1::Chunk, &chunk);
    let original = w
        .archive
        .write_object(&chunk, KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1)
        .unwrap();
    stage.selection.chunk_originals = stage
        .selection
        .chunk_originals
        .set(&mut w.archive, suffix_ordinal_key(0), &original)
        .unwrap();
    let header = KagemushaWalletSuffixHeaderV1 {
        version: 1,
        method: KAGEMUSHA_WALLET_SUFFIX_METHOD_NATIVE_V1,
        network_id: *native.network_id().as_bytes(),
        scheme_id: w.scheme_id,
        wallet_id: w.wallet_id,
        verifier_definition: [2; 32],
        artifact_manifest: [3; 32],
        source_capsule: stage.selection.source_capsule,
        request_id: stage.request.request_id,
        anchor_digest: [4; 32],
        anchor_sequence: 4,
        step_count: 1,
        chunk_count: 1,
        stream_bytes: chunk.len() as u128,
        chunk_digests: vec![chunk_digest],
        tip: claim.package.statement.predecessor,
        tip_sequence: 5,
        tip_adjusted: public,
        statement: claim.package.statement,
        sigma: claim.package.step_proof,
        recovery_witness_digest: [5; 32],
        account: claim.account,
        charge: claim.charge,
    };
    // The caller has already admitted the header; this test isolates exact chunk carriage
    // and checkpoint publication, with synthetic monetary proof fields.
    stage.request.selection = Some([29; 32]);
    let mut released = SuffixReleasedOriginalV1 {
        version: 1,
        discriminator: 6,
        scheme_id: w.scheme_id,
        wallet_id: w.wallet_id,
        request_id: stage.request.request_id,
        authority: stage.selection.authority,
        selection: [29; 32],
        operation_id: stage.selection.operation_id,
        selected_generation: 2,
        capsule_original: [30; 32],
        capsule_custody_digest: [31; 32],
        completion_original: [32; 32],
        completion_digest: [33; 32],
        receipt_original: [34; 32],
        finalize_original: [35; 32],
        instruction_originals: IndexRoot::default(),
        instruction_count: 3,
    };
    let before = w.custody.publication_calls;
    let digest = w
        .materialize_suffix_append(
            &mut stage.checkpoint,
            &mut stage.request,
            &stage.selection,
            &header,
            &mut released,
            1,
        )
        .unwrap();
    assert_eq!(w.custody.publication_calls, before + 1);
    assert_eq!(
        released
            .instruction_originals
            .get(&mut w.archive, &suffix_ordinal_key(1))
            .unwrap(),
        Some(digest.to_vec())
    );
    assert_eq!(
        released
            .instruction_originals
            .get(&mut w.archive, &suffix_ordinal_key(2))
            .unwrap(),
        None
    );
    let encoded = exact_object(&mut w.archive, &digest, SUFFIX_INSTRUCTION_MAX_BYTES_V1).unwrap();
    let instruction: KagemushaWalletLedgerV1 = archive::decode(&encoded).unwrap();
    assert!(
        matches!(instruction.action, KagemushaWalletLedgerActionV1::SuffixUnloadAppend { ordinal: 1, chunk: ref held, .. } if held == &chunk)
    );
    let selected = w.selected_suffix_checkpoint().unwrap().unwrap();
    let (request, _) = indexed_record::<SuffixRequestRecordV1>(
        &mut w.archive,
        selected.suffix.unwrap().requests,
        &stage.request.request_id,
    )
    .unwrap()
    .unwrap();
    assert_eq!(request.released, stage.request.released);
    assert_eq!(
        w.materialize_suffix_append(
            &mut stage.checkpoint,
            &mut stage.request,
            &stage.selection,
            &header,
            &mut released,
            1
        )
        .unwrap(),
        digest
    );
    assert_eq!(w.custody.publication_calls, before + 1);
    let mut changed = header;
    changed.chunk_digests[0] = [99; 32];
    assert!(
        w.materialize_suffix_append(
            &mut stage.checkpoint,
            &mut stage.request,
            &stage.selection,
            &changed,
            &mut released,
            1
        )
        .is_err()
    );
    assert_eq!(w.custody.publication_calls, before + 1);
}

#[test]
fn confirmed_expired_stage_wires_survive_completion_and_ordinary_publication() {
    let mut native = NativeFinalityFixture::start("suffix-confirmed-expired-readback");
    let mut w = ledger_wallet(&native);
    let mut stage = stage(&mut w);
    let mut originals = Vec::new();
    for ordinal in 0..=stage.final_ordinal() {
        let action = match ordinal {
            0 => action(stage.selection.wallet_id, stage.selection.authority),
            1 => KagemushaWalletLedgerActionV1::SuffixUnloadAppend {
                wallet: stage.selection.wallet_id,
                authority: stage.selection.authority,
                account: iroha_data_model::account::AccountId::new(key().public_key().clone()),
                ordinal,
                chunk: vec![7, 8, 9],
            },
            _ => KagemushaWalletLedgerActionV1::SuffixUnloadFinalize {
                wallet: stage.selection.wallet_id,
                authority: stage.selection.authority,
                account: iroha_data_model::account::AccountId::new(key().public_key().clone()),
                ordinal,
                unload: vec![10, 11, 12],
            },
        };
        let instruction = KagemushaWalletLedgerV1::new(stage.selection.scheme_id, action);
        stage.ordinal = ordinal;
        stage.instruction = archive::encode(&instruction).unwrap();
        stage.instruction_digest = object_digest(&stage.instruction);
        let creation = native.next_header().creation_time_ms - 1;
        let mut builder = TransactionBuilder::new(
            native.network_id(),
            iroha_data_model::account::AccountId::new(key().public_key().clone()),
            FeePaymentIntent::authority(vec![], None),
        );
        builder.set_creation_time(Duration::from_millis(creation));
        builder.set_ttl(Duration::from_millis(2));
        let tx = builder
            .with_instructions([instruction])
            .sign(key().private_key());
        let wire = tx.encode_wire_v1().unwrap();
        signed_stage(&wire, native.network_id().as_bytes(), &stage.instruction).unwrap();
        let entrypoint = ledger_outcome::entrypoint(&tx).unwrap();
        let (attempt, digest) = w
            .retain_suffix_attempt(&mut stage, entrypoint, &wire)
            .unwrap();
        // Genuine signed validators certify synthetic execution rows. This isolates custody;
        // these inner monetary originals intentionally do not qualify a real suffix execution.
        let block = certify(&mut native, std::slice::from_ref(&tx), vec![Ok(vec![])]);
        let finality = original(&block, 0);
        let outcome = verified(&native, &finality, &tx);
        let held = w
            .retain_verified_suffix_outcome(&stage, &attempt, digest, &outcome, &bytes(&finality))
            .unwrap();
        refresh(&mut w, &mut stage);
        originals.push((
            ordinal,
            stage.instruction.clone(),
            entrypoint,
            wire,
            held,
            creation + 2,
        ));
    }
    super::super::ledger_outcome::tests::advance(&mut native, 2);
    // Completion clears scheduling ownership only. All stage originals still belong to the
    // permanent selected namespace, including accepted Begin and Append needed by Core repair.
    let mut suffix = stage.suffix().clone();
    suffix.active_request = None;
    let root = w
        .publish_suffix_checkpoint(stage.checkpoint.digest, &stage.checkpoint.manifest, &suffix)
        .unwrap();
    w.publish_manifest(root, &stage.checkpoint.manifest)
        .unwrap();
    let mut w = restart(w);
    refresh(&mut w, &mut stage);
    assert!(stage.suffix().active_request.is_none());
    let publications = w.custody.publication_calls;
    let signatures = w.custody.signatures;
    for (ordinal, instruction, entrypoint, wire, held, expiry) in originals {
        assert!(native.next_header().creation_time_ms > expiry);
        stage.ordinal = ordinal;
        stage.instruction = instruction;
        stage.instruction_digest = object_digest(&stage.instruction);
        let attempts = w.retained_suffix_stage_attempts(&stage).unwrap().unwrap();
        assert_eq!(attempts.attempt_count, 1);
        assert_eq!(attempts.accepted_entrypoint, Some(entrypoint));
        let (attempt, digest, exact, tx) = w
            .retained_signed_suffix_attempt(&stage, &entrypoint)
            .unwrap();
        assert_eq!(exact, wire);
        assert_eq!(tx.time_to_live(), Some(Duration::from_millis(2)));
        let retained = w
            .retained_suffix_outcome(&stage, &attempt, digest)
            .unwrap()
            .unwrap();
        assert_eq!(retained.finality_original, held.finality_original);
        assert_eq!(retained.native_commit_hash, held.native_commit_hash);
        assert!(retained.accepted);
        assert!(matches!(
            (ordinal == stage.final_ordinal(), stage.outcome(&retained)),
            (false, SuffixStageOutcomeV1::StageConfirmed(_))
                | (true, SuffixStageOutcomeV1::UnloadConfirmed(_))
        ));
        // Recovery reads the exact expired signature and first finality. It neither issues
        // a new transaction nor changes custody or the hardware monetary head.
        w.retain_suffix_attempt(&mut stage, entrypoint, &exact)
            .unwrap();
    }
    assert_eq!(w.custody.publication_calls, publications);
    assert_eq!(w.custody.signatures, signatures);
}

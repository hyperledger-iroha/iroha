//! Full-domain strict source projection, private-path independence and resource limits.

use super::*;
use crate::fastpq::{FastpqPublicInputsTemplate, poseidon_preimage_digest};
use iroha_crypto::Hash;
use iroha_data_model::{
    asset::AssetDefinitionId,
    fastpq::{FastpqPublicTransferTranscriptV1, TransferDeltaTranscript, TransferSmtWitness},
};
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};

fn fixture() -> (FastpqPublicInputs, Vec<TransferTranscript>) {
    let amount = Quantity::from(3_u32);
    let before = Quantity::from(u128::MAX);
    let delta = TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        amount: amount.clone(),
        from_balance_before: before.clone(),
        from_balance_after: before.try_sub(&amount).unwrap(),
        to_balance_before: Quantity::zero(),
        to_balance_after: amount,
        from_smt_witness: TransferSmtWitness::default(),
        to_smt_witness: TransferSmtWitness::default(),
    };
    let call = Hash::new(b"full-domain source call");
    let transcript = TransferTranscript {
        batch_hash: call,
        authority_digest: Hash::new(b"source authority"),
        poseidon_preimage_digest: Some(poseidon_preimage_digest(&delta, &call)),
        deltas: vec![delta],
    };
    let inputs = FastpqPublicInputsTemplate {
        dsid: [3; 16],
        slot: 9,
        old_root: [0; 32],
        new_root: [0; 32],
        perm_root: [7; 32],
    }
    .with_tx_set_hash(Hash::new(b"transaction set").into());
    (inputs, vec![transcript])
}

fn build(
    inputs: FastpqPublicInputs,
    transcripts: &[TransferTranscript],
) -> FastpqQuantityStatement {
    quantity_statement_from_finalized_transcripts(
        inputs,
        transcripts,
        PublicTransferLimits::default(),
        TransferSmtBuildLimits::for_update_limit(2).unwrap(),
    )
    .unwrap()
}

#[test]
fn strict_full_domain_statement_preserves_every_original_public_fact() {
    let (inputs, transcripts) = fixture();
    let before = norito::encode_canonical(&transcripts).unwrap();
    let produced = build(inputs, &transcripts);
    let statement = produced.statement();
    assert_eq!(
        statement.transcripts,
        transcripts
            .iter()
            .map(FastpqPublicTransferTranscriptV1::from)
            .collect::<Vec<_>>()
    );
    assert_eq!(statement.public_inputs.slot, inputs.slot);
    assert_eq!(statement.public_inputs.dsid, inputs.dsid);
    assert_eq!(statement.public_inputs.perm_root, inputs.perm_root);
    assert_eq!(statement.public_inputs.tx_set_hash, inputs.tx_set_hash);
    assert_eq!(
        produced.witnesses().roots(),
        (
            statement.public_inputs.old_root,
            statement.public_inputs.new_root
        )
    );
    assert_eq!(statement.transitions.len(), 2);
    assert_eq!(produced.witnesses().pairs().len(), 1);
    for row in &statement.transitions {
        let before = fastpq_prover::gadgets::public_transfer_statement::decode_quantity_units_v1(
            &row.pre_value,
        )
        .unwrap();
        let after = fastpq_prover::gadgets::public_transfer_statement::decode_quantity_units_v1(
            &row.post_value,
        )
        .unwrap();
        assert_eq!(before.scale(), after.scale());
    }
    assert_eq!(norito::encode_canonical(&transcripts).unwrap(), before);
}

#[test]
fn supplied_private_paths_are_ignored_without_changing_inputs() {
    let (inputs, mut transcripts) = fixture();
    let expected = build(inputs, &transcripts);
    transcripts[0].deltas[0]
        .from_smt_witness
        .siblings
        .push([0; 32]);
    transcripts[0].deltas[0].to_smt_witness.path_bits = vec![255];
    let before = norito::encode_canonical(&transcripts).unwrap();
    let actual = build(inputs, &transcripts);
    assert_eq!(
        norito::encode_canonical(actual.statement()).unwrap(),
        norito::encode_canonical(expected.statement()).unwrap()
    );
    assert_eq!(actual.witnesses(), expected.witnesses());
    assert_eq!(norito::encode_canonical(&transcripts).unwrap(), before);
}

#[test]
fn missing_digest_invalid_arithmetic_and_construction_limits_fail_without_repair() {
    let (inputs, transcripts) = fixture();
    for mutation in 0..3 {
        let mut changed = transcripts.clone();
        match mutation {
            0 => changed[0].poseidon_preimage_digest = None,
            1 => changed[0].deltas[0].from_balance_after = Quantity::zero(),
            _ => changed[0].deltas.clear(),
        }
        let before = norito::encode_canonical(&changed).unwrap();
        assert!(
            quantity_statement_from_finalized_transcripts(
                inputs,
                &changed,
                PublicTransferLimits::default(),
                TransferSmtBuildLimits::for_update_limit(2).unwrap()
            )
            .is_err()
        );
        assert_eq!(norito::encode_canonical(&changed).unwrap(), before);
    }
    assert!(
        quantity_statement_from_finalized_transcripts(
            inputs,
            &transcripts,
            PublicTransferLimits {
                max_public_bytes: 0,
                ..PublicTransferLimits::default()
            },
            TransferSmtBuildLimits::for_update_limit(2).unwrap()
        )
        .is_err()
    );
    assert!(
        quantity_statement_from_finalized_transcripts(
            inputs,
            &transcripts,
            PublicTransferLimits::default(),
            TransferSmtBuildLimits::for_update_limit(1).unwrap()
        )
        .is_err()
    );
}

#[test]
fn empty_source_keeps_unchanged_inputs_and_can_be_consumed_without_copies() {
    let (inputs, _) = fixture();
    let produced = build(inputs, &[]);
    assert_eq!(produced.statement().public_inputs, inputs);
    let (statement, witnesses) = produced.into_parts();
    assert!(statement.transitions.is_empty());
    assert!(statement.transcripts.is_empty());
    assert!(witnesses.pairs().is_empty());
    let mut changed = inputs;
    changed.new_root = Hash::new(b"changed empty root").into();
    assert!(
        quantity_statement_from_finalized_transcripts(
            changed,
            &[],
            PublicTransferLimits::default(),
            TransferSmtBuildLimits::for_update_limit(0).unwrap()
        )
        .is_err()
    );
}

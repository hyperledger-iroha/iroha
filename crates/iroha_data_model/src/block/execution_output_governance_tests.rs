//! Governance block-start rows of one block: their place before the execution rows, their
//! order and their partition. These fixtures authenticate no pass.

use super::*;
use crate::{
    block::output_budget::{
        ExecutionOutputLimits, ExecutionOutputPhase, GovernanceStartItemCeilingV1,
        GovernanceStartOutputBudgetErrorV1, GovernanceStartOutputBudgetV1,
    },
    governance::{
        anonymous_ballot::{
            BallotScopeV1, CASTING_CLOCK_ROW_MAX_BYTES_V1, CastingClockEstablishedV1,
            CastingOpenedV1, Digest512, GOVERNANCE_START_OUTPUT_BYTES_V1,
            GOVERNANCE_START_OUTPUT_ROWS_V1, GOVERNANCE_START_PRELUDE_ROWS_V1,
            GOVERNANCE_START_ROW_MAX_BYTES_V1, GovernanceBlockStartEventV1,
            GovernanceBlockStartOutputV1, GovernanceLifecycleItemV1, GovernanceOutputAnchorV1,
            LIFECYCLE_RECEIPT_ROW_MAX_BYTES_V1, LifecycleAppliedV1, MAX_SEALED_SEATS_V1,
            NativeSlotAdvancedV1, SealedCredentialListBytesV1, SealedCredentialListPublishedV1,
            test_fixtures::{anchor, body_context, closure, sealed_list},
        },
        types::{
            BodyInstanceId, GovernanceAttemptId, GovernanceAttemptStatusV1, GovernanceStageV1,
            ParliamentBody,
        },
    },
};

const HEIGHT: u64 = 8;
const TIME_MS: u64 = 80_000;

fn scope(byte: u8) -> BallotScopeV1 {
    BallotScopeV1::new([byte; 64])
}

fn clock(byte: u8) -> GovernanceBlockStartOutputV1 {
    GovernanceBlockStartOutputV1 {
        anchor: GovernanceOutputAnchorV1::CastingClockPrelude {
            height: HEIGHT,
            timestamp_ms: TIME_MS,
        },
        event: GovernanceBlockStartEventV1::CastingClockEstablished(CastingClockEstablishedV1 {
            scope: scope(byte),
            casting_window_ms: 1_000,
            closing_ms: TIME_MS + 1_000,
        }),
    }
}

fn positioned(position: u32, event: GovernanceBlockStartEventV1) -> GovernanceBlockStartOutputV1 {
    GovernanceBlockStartOutputV1 {
        anchor: GovernanceOutputAnchorV1::Pass(anchor(HEIGHT, position, TIME_MS)),
        event,
    }
}

fn receipt(position: u32) -> GovernanceBlockStartOutputV1 {
    positioned(
        position,
        GovernanceBlockStartEventV1::LifecycleApplied(LifecycleAppliedV1 {
            governance_attempt_id: GovernanceAttemptId::new([0x11; 32]),
            item: GovernanceLifecycleItemV1::OpenBallot {
                body: ParliamentBody::PolicyJury,
            },
            status: GovernanceAttemptStatusV1::Active,
            stage: GovernanceStageV1::default(),
            attempt_state_digest: Digest512::new([0x5c; 64]),
            board: None,
        }),
    )
}

fn opened(position: u32) -> GovernanceBlockStartOutputV1 {
    positioned(
        position,
        GovernanceBlockStartEventV1::CastingOpened(CastingOpenedV1 {
            body_context: body_context(),
            body_instance_id: BodyInstanceId::new([0x03; 32]),
            scope: scope(0x44),
            policy_hash: Digest512::new([0xaa; 64]),
            opening_at_ms: TIME_MS,
        }),
    )
}

fn published(position: u32, seats: u16) -> GovernanceBlockStartOutputV1 {
    positioned(
        position,
        GovernanceBlockStartEventV1::SealedCredentialListPublished(
            SealedCredentialListPublishedV1 {
                body_context: body_context(),
                body_instance_id: BodyInstanceId::new([0x03; 32]),
                roster_root: Digest512::new([0x0a; 64]),
                list_digest: Digest512::new([0x99; 64]),
                registration_mapping_digest: Digest512::new([0x04; 64]),
                sealed_list_bytes: SealedCredentialListBytesV1::from_list(&sealed_list(seats))
                    .expect("publish"),
            },
        ),
    )
}

fn row(output: GovernanceBlockStartOutputV1) -> ExecutionOutputV1 {
    ExecutionOutputV1::GovernanceBlockStart(output)
}

fn rows(outputs: impl IntoIterator<Item = GovernanceBlockStartOutputV1>) -> Vec<ExecutionOutputV1> {
    outputs.into_iter().map(row).collect()
}

/// Validate `outputs` as the rows of a block with one input when they hold a Network row,
/// and of a block without inputs otherwise.
fn validate(outputs: &[ExecutionOutputV1]) -> Result<(), String> {
    let inputs: Vec<TransactionEntrypoint> = outputs
        .iter()
        .any(|output| output.network().is_some())
        .then(input)
        .into_iter()
        .collect();
    validate_execution_outputs_v1(outputs, proposal(), HEIGHT, &inputs)
}

#[test]
fn governance_rows_lead_the_block_and_own_no_input_call_or_result() {
    let mut outputs = rows([clock(1), clock(2), receipt(0), opened(0), receipt(3)]);
    outputs.push(network());
    validate(&outputs).unwrap();
    assert_eq!(governance_start_row_count_v1(&outputs), 5);
    assert_eq!(
        validate_governance_start_rows_v1(&outputs, HEIGHT),
        Ok(5),
        "a skipped position belongs to another owner's item"
    );
    for output in &outputs[..5] {
        assert_eq!(
            ExecutionOutputPhase::of(output),
            ExecutionOutputPhase::GovernanceStart
        );
        assert!(output.governance_block_start().is_some());
        assert!(output.network().is_none());
        assert_eq!(output.result().as_ref(), Ok(&Vec::new()));
        assert!(output.completions().is_empty());
        assert!(output.trigger_lifecycle().is_none());
        assert!(!output.is_output_limit_rejection());
        assert!(output.execution_call_hash(proposal(), &[input()]).is_err());
        output.validate_structure(HEIGHT, &[input()]).unwrap();
        let bytes = norito::encode_canonical(output).unwrap();
        assert_eq!(
            norito::decode_canonical::<ExecutionOutputV1>(&bytes).unwrap(),
            *output
        );
        let json = norito::json::to_json(output).unwrap();
        assert_eq!(
            norito::json::from_str::<ExecutionOutputV1>(&json).unwrap(),
            *output
        );
    }
    assert!(outputs[5].governance_block_start().is_none());
    // The variant is appended: its tag follows every earlier one.
    assert_eq!(row(clock(1)).encode()[..4], 6_u32.to_le_bytes());
}

#[test]
fn a_governance_row_after_an_execution_row_is_refused() {
    let outputs = vec![network(), row(receipt(0))];
    assert_eq!(
        validate(&outputs).unwrap_err(),
        "governance block-start output follows an execution row"
    );
    assert_eq!(
        crate::block::output_test_support::limits()
            .validate_outputs(&outputs)
            .unwrap_err(),
        "governance block-start output follows an execution row"
    );
}

#[test]
fn misordered_or_foreign_governance_rows_are_refused() {
    let other_block = GovernanceBlockStartOutputV1 {
        anchor: GovernanceOutputAnchorV1::Pass(anchor(HEIGHT + 1, 0, TIME_MS)),
        ..receipt(0)
    };
    let other_time = GovernanceBlockStartOutputV1 {
        anchor: GovernanceOutputAnchorV1::Pass(anchor(HEIGHT, 1, TIME_MS + 1)),
        ..receipt(1)
    };
    let advanced = |ticket_height| {
        positioned(
            0,
            GovernanceBlockStartEventV1::NativeSlotAdvanced(NativeSlotAdvancedV1 {
                target_height: HEIGHT + 4,
                cursor_height: ticket_height - 1,
                ticket_height,
            }),
        )
    };
    validate(&rows([advanced(HEIGHT), receipt(1)])).unwrap();
    for (outputs, expected) in [
        (
            vec![receipt(0), clock(1)],
            "a casting clock follows a positioned governance output",
        ),
        (
            vec![clock(1), clock(1)],
            "the casting-clock prelude repeats a scope or exceeds K",
        ),
        (
            vec![opened(0)],
            "a positioned governance item must emit its receipt first",
        ),
        (
            vec![receipt(0), receipt(0)],
            "a positioned governance item emits a receipt, then at most one output",
        ),
        (
            vec![receipt(0), opened(0), opened(0)],
            "a positioned governance item emits a receipt, then at most one output",
        ),
        (
            vec![receipt(1), receipt(0)],
            "governance block-start positions decrease",
        ),
        (
            vec![other_block],
            "governance block-start output names another block",
        ),
        (
            vec![receipt(0), other_time],
            "governance block-start outputs name two block times",
        ),
        (
            vec![advanced(HEIGHT + 1)],
            "NativeSlotAdvanced does not record the ticket of its block",
        ),
    ] {
        let outputs = rows(outputs);
        assert_eq!(
            validate_governance_start_rows_v1(&outputs, HEIGHT).unwrap_err(),
            expected
        );
        assert_eq!(validate(&outputs).unwrap_err(), expected);
    }
    // More clocks than K casting windows can exist.
    let clocks = rows(
        (0..=GOVERNANCE_START_PRELUDE_ROWS_V1)
            .map(|index| clock(u8::try_from(index).expect("small"))),
    );
    assert_eq!(
        validate(&clocks).unwrap_err(),
        "the casting-clock prelude repeats a scope or exceeds K"
    );
    validate(&clocks[..GOVERNANCE_START_PRELUDE_ROWS_V1 as usize]).unwrap();
}

#[test]
fn the_partition_bounds_governance_rows_on_their_complete_row_frames() {
    // Four maximal published lists exceed the byte partition; three fit.
    let largest = |position| [receipt(position), published(position, MAX_SEALED_SEATS_V1)];
    let bytes = |outputs: &[ExecutionOutputV1]| -> u64 {
        outputs
            .iter()
            .map(|row| u64::try_from(norito::canonical_frame_len(row).unwrap()).unwrap())
            .sum()
    };
    let one = bytes(&rows(largest(0)));
    assert!(one <= LIFECYCLE_RECEIPT_ROW_MAX_BYTES_V1 + GOVERNANCE_START_ROW_MAX_BYTES_V1);
    let fitting = usize::try_from(GOVERNANCE_START_OUTPUT_BYTES_V1 / one).unwrap();
    let outputs = |items: usize| {
        rows((0..items).flat_map(|position| largest(u32::try_from(position).expect("small"))))
    };
    validate(&outputs(fitting)).unwrap();
    assert_eq!(
        validate(&outputs(fitting + 1)).unwrap_err(),
        "governance block-start rows exceed their byte partition"
    );
    // The row partition.
    let receipts = |count: u32| rows((0..count).map(receipt));
    validate(&receipts(GOVERNANCE_START_OUTPUT_ROWS_V1)).unwrap();
    assert_eq!(
        validate(&receipts(GOVERNANCE_START_OUTPUT_ROWS_V1 + 1)).unwrap_err(),
        "governance block-start rows exceed their partition"
    );

    // The execution-row policy bounds no governance row: a block may hold the whole
    // partition beside every execution row its policy allows.
    let tight = ExecutionOutputLimits {
        max_outputs: 1,
        max_output_bytes: 2_048,
        max_total_output_bytes: 2_048,
        max_executed_wire_bytes: one + 8_192,
    };
    let mut block = rows(largest(0));
    assert!(bytes(&block[1..]) > tight.max_output_bytes);
    tight.validate_outputs(&block).unwrap();
    block.push(network());
    tight.validate_outputs(&block).unwrap();
    // The execution rows still meet their own limits beside the partition.
    assert_eq!(
        ExecutionOutputLimits {
            max_output_bytes: 1,
            max_total_output_bytes: 1,
            ..tight
        }
        .validate_outputs(&block)
        .unwrap_err(),
        "execution output row exceeds applying byte policy"
    );
    block.push(network());
    assert_eq!(
        tight.validate_outputs(&block).unwrap_err(),
        "execution output count exceeds applying policy"
    );
    // The policy check repeats the partition bounds.
    assert_eq!(
        tight.validate_outputs(&outputs(fitting + 1)).unwrap_err(),
        "governance block-start rows exceed their byte partition"
    );
    assert_eq!(
        tight
            .validate_outputs(&receipts(GOVERNANCE_START_OUTPUT_ROWS_V1 + 1))
            .unwrap_err(),
        "governance block-start rows exceed their partition"
    );
}

#[test]
fn the_wire_ceiling_must_hold_the_partition_beside_the_execution_rows() {
    let limits = crate::block::output_test_support::limits();
    limits.validate_governance_start_room().unwrap();
    let exact = ExecutionOutputLimits {
        max_executed_wire_bytes: limits.max_total_output_bytes + GOVERNANCE_START_OUTPUT_BYTES_V1,
        ..limits
    };
    exact.validate_governance_start_room().unwrap();
    assert_eq!(
        ExecutionOutputLimits {
            max_executed_wire_bytes: exact.max_executed_wire_bytes - 1,
            ..limits
        }
        .validate_governance_start_room()
        .unwrap_err(),
        "executed wire ceiling cannot hold the governance-start partition beside the execution rows"
    );
    // An invalid policy has no room to measure.
    assert!(
        ExecutionOutputLimits {
            max_outputs: 0,
            ..limits
        }
        .validate_governance_start_room()
        .is_err()
    );
}

#[test]
fn the_budget_keeps_the_prelude_reserved_until_it_closes() {
    use GovernanceStartItemCeilingV1::{Receipt, ReceiptAndOutput};
    use GovernanceStartOutputBudgetErrorV1::Refused;

    let mut budget = GovernanceStartOutputBudgetV1::new();
    assert!(budget.fits(ReceiptAndOutput), "the first item always fits");
    assert_eq!(
        budget.admit_item(Receipt, vec![receipt(0)]),
        Err(Refused("a positioned item precedes the end of the prelude"))
    );
    assert_eq!(
        budget.admit_prelude(receipt(0)),
        Err(Refused("the prelude admits casting clocks only"))
    );
    for index in 0..GOVERNANCE_START_PRELUDE_ROWS_V1 {
        budget
            .admit_prelude(clock(u8::try_from(index).expect("small")))
            .unwrap();
    }
    assert_eq!(
        budget.admit_prelude(clock(0xff)),
        Err(Refused(
            "the casting-clock prelude exceeds its reserved rows"
        ))
    );
    assert!(
        budget.bytes()
            <= u64::from(GOVERNANCE_START_PRELUDE_ROWS_V1) * CASTING_CLOCK_ROW_MAX_BYTES_V1
    );
    assert!(
        budget.fits(ReceiptAndOutput),
        "every mandatory prelude row leaves room for one maximal item"
    );
    budget.close_prelude();

    // The rows of one item: its receipt, then at most one richer output of the same anchor.
    for (ceiling, outputs) in [
        (Receipt, vec![]),
        (Receipt, vec![opened(0)]),
        (Receipt, vec![receipt(0), opened(0)]),
        (ReceiptAndOutput, vec![receipt(0), receipt(0)]),
        (ReceiptAndOutput, vec![receipt(0), opened(1)]),
        (ReceiptAndOutput, vec![receipt(0), opened(0), opened(0)]),
        (Receipt, vec![clock(0x70)]),
    ] {
        assert!(budget.admit_item(ceiling, outputs).is_err());
    }
    let held = budget.rows().len();
    budget
        .admit_item(ReceiptAndOutput, vec![receipt(0), opened(0)])
        .unwrap();
    budget.admit_item(Receipt, vec![receipt(1)]).unwrap();
    assert_eq!(budget.rows().len(), held + 3);

    // The rows are a valid leading sequence of their block.
    let (rows, bytes) = budget.finish();
    assert_eq!(
        validate_governance_start_rows_v1(&rows, HEIGHT),
        Ok(rows.len())
    );
    assert_eq!(
        bytes,
        rows.iter()
            .map(|row| u64::try_from(norito::canonical_frame_len(row).unwrap()).unwrap())
            .sum::<u64>()
    );
}

#[test]
fn an_item_is_deferred_before_it_mutates_when_its_ceiling_does_not_fit() {
    use GovernanceStartItemCeilingV1::{Receipt, ReceiptAndOutput};
    use GovernanceStartOutputBudgetErrorV1::Refused;

    // Bytes: maximal published lists fill the partition; a receipt still fits when a full
    // item no longer does.
    let mut budget = GovernanceStartOutputBudgetV1::new();
    budget.close_prelude();
    let mut position = 0;
    while budget.fits(ReceiptAndOutput) {
        budget
            .admit_item(
                ReceiptAndOutput,
                vec![receipt(position), published(position, MAX_SEALED_SEATS_V1)],
            )
            .expect("an item that fits is admitted");
        position += 1;
    }
    assert!(position >= 3, "three maximal items fit the partition");
    assert!(budget.bytes() <= GOVERNANCE_START_OUTPUT_BYTES_V1);
    assert!(budget.fits(Receipt));
    let closed = closure(9, 6, 2, 1);
    assert!(
        budget
            .admit_item(
                ReceiptAndOutput,
                vec![
                    receipt(position),
                    positioned(position, GovernanceBlockStartEventV1::BallotClosed(closed)),
                ],
            )
            .is_ok(),
        "admission measures the actual rows, which are below the refused ceiling"
    );

    // Rows: every row counts once the prelude has closed.
    let mut budget = GovernanceStartOutputBudgetV1::new();
    budget.close_prelude();
    for position in 0..GOVERNANCE_START_OUTPUT_ROWS_V1 - 1 {
        assert!(budget.fits(Receipt));
        budget.admit_item(Receipt, vec![receipt(position)]).unwrap();
    }
    assert!(budget.fits(Receipt));
    assert!(!budget.fits(ReceiptAndOutput));
    budget
        .admit_item(Receipt, vec![receipt(GOVERNANCE_START_OUTPUT_ROWS_V1 - 1)])
        .unwrap();
    assert!(!budget.fits(Receipt));
    assert_eq!(
        budget.admit_item(Receipt, vec![receipt(GOVERNANCE_START_OUTPUT_ROWS_V1)]),
        Err(Refused(
            "a positioned item exceeds the capacity its owner checked"
        ))
    );
    // An open prelude keeps its rows: fewer items fit than after it closes.
    let mut reserved = GovernanceStartOutputBudgetV1::new();
    for index in 0..GOVERNANCE_START_PRELUDE_ROWS_V1 - 1 {
        reserved
            .admit_prelude(clock(u8::try_from(index).expect("small")))
            .unwrap();
    }
    let held = reserved.rows().len();
    assert_eq!(
        u32::try_from(held).expect("small") + 1,
        GOVERNANCE_START_PRELUDE_ROWS_V1,
        "one prelude row is still reserved"
    );
    assert!(reserved.fits(ReceiptAndOutput));
}

#[test]
fn payment_proofs_keep_their_coordinates_across_c10_and_governance_outputs() {
    use crate::{
        asset::AssetId,
        block::{BlockSignatures, builder::BlockBuilder, decode_framed_signed_block},
        isi::Transfer,
        ivm_c10::{
            IvmAdmissionErrorC10V1, TransactionRejectionReasonC10V1, TransactionResultC10V1,
            ValidationFailC10V1,
        },
        smart_contract::event::ContractEmissionsV1,
    };
    let source = KeyPair::from_seed(vec![0x37; 32], Algorithm::Ed25519);
    let authority = AccountId::new(source.public_key().clone());
    let recipient = KeyPair::from_seed(vec![0x38; 32], Algorithm::Ed25519);
    let asset = AssetId::new(
        "66owaQmAQMuHxPzxUN3bqZ6FJfDa".parse().unwrap(),
        authority.clone(),
    );
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"mixed output proof",
    )));
    let mut builder = BlockBuilder::new(BlockHeader::new(
        HEIGHT.try_into().unwrap(),
        None,
        None,
        TIME_MS,
        0,
    ));
    for i in 0_u32..3 {
        let mut tx = TransactionBuilder::new(
            network.clone(),
            authority.clone(),
            FeePaymentIntent::authority(vec![], None),
        );
        tx.set_creation_time(std::time::Duration::from_millis(700 + u64::from(i)));
        let tx = if i == 1 {
            tx
        } else {
            tx.with_instructions([Transfer::asset_quantity(
                asset.clone(),
                1 + i,
                AccountId::new(recipient.public_key().clone()),
            )])
        };
        assert_eq!(
            builder.push_transaction(tx.sign(source.private_key())),
            i as usize
        );
    }
    let mut block = builder.build(BlockSignatures::default());
    let ordinary = |input_index| {
        ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
            input_index,
            result: TransactionResult::new(Ok(vec![])),
            completions: vec![],
        })
    };
    let c10 = ExecutionOutputV1::C10(ExecutionOutputC10V1 {
        source: ExecutionSourceC10V1::Network(1),
        result: TransactionResultC10V1(
            Err(TransactionRejectionReasonC10V1::Validation(
                ValidationFailC10V1::IvmAdmission(IvmAdmissionErrorC10V1::MaxCyclesWithoutZk(19)),
            )),
            vec![],
            None,
            ContractEmissionsV1::default(),
        ),
        completions: vec![],
        trigger_lifecycle: None,
    });
    assert_eq!(c10.encode()[..4], 7_u32.to_le_bytes());
    let outputs = vec![row(clock(1)), ordinary(0), c10, ordinary(2)];
    let limits = ExecutionOutputLimits {
        max_outputs: 8,
        max_output_bytes: 64 * 1024,
        max_total_output_bytes: 512 * 1024,
        max_executed_wire_bytes: GOVERNANCE_START_OUTPUT_BYTES_V1 + 1024 * 1024,
    };
    block
        .set_execution_outputs(
            outputs.clone(),
            0,
            Default::default(),
            vec![],
            Default::default(),
            Default::default(),
            &limits,
        )
        .unwrap();
    let restored = decode_framed_signed_block(&block.encode_wire().unwrap()).unwrap();
    restored.validate_execution_result_structure().unwrap();
    restored.validate_output_merkle_cache().unwrap();
    assert_eq!(restored.execution_outputs(), outputs);
    let input_root = restored.network_input_merkle_commitment().unwrap();
    let output_root = restored.output_merkle_commitment().unwrap();
    for i in 0_usize..3 {
        let input = restored.network_entrypoint_at(i).unwrap();
        let proof = restored.network_input_proof(i as u32).unwrap();
        assert!(proof.verify(&input.hash(), &input_root));
        let (index, output) = restored.network_execution_output_at(i as u32).unwrap();
        assert_eq!(index as usize, i + 1);
        assert!(
            restored
                .output_proof(index)
                .unwrap()
                .verify(&HashOf::new(output), &output_root)
        );
        assert!(
            !restored
                .output_proof(index - 1)
                .unwrap()
                .verify(&HashOf::new(output), &output_root)
        );
        assert_eq!(
            restored.network_output_at(i as u32).map(|(index, _)| index),
            (i != 1).then_some(index)
        );
    }
    assert_eq!(
        restored
            .failed_outputs()
            .map(|(index, _)| index)
            .collect::<Vec<_>>(),
        [2]
    );
    let mut reordered = outputs;
    reordered.swap(2, 3);
    assert!(
        block
            .set_execution_outputs(
                reordered,
                0,
                Default::default(),
                vec![],
                Default::default(),
                Default::default(),
                &limits
            )
            .is_err()
    );
}

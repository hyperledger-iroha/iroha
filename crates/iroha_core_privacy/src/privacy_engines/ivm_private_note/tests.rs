//! Native private-note relation, canonical codec, and mutation assertions.

use super::test_fixtures::*;
use super::{
    PRIVATE_PROGRAM_BYTES_V1, PrivateInstructionV1, PrivateNotePlaintextV1, PrivateOpcodeV1,
    PrivateProgramV1,
    codec::decode_private_program_v1,
    derive_note_commitment_v1, derive_note_nullifier_v1, derive_private_program_id_v1,
    encode_private_program_v1,
    relation::{
        IvmPrivateNoteRelationErrorV1, accumulator_leaf_invocation_v1,
        accumulator_node_invocation_v1, preflight_private_note_relation_with_profile_v1,
        validate_private_note_relation_v1, validate_private_note_relation_with_profile_v1,
    },
};
use iroha_data_model::{
    NetworkId,
    privacy::{
        PrivacyNullifierV1, PrivacyParameterDigestV1, PrivacyPoolIdV1, PrivacyRootV1,
        PrivacyTransactionIntentDigestV1, PrivacyValueBalanceDirectionV1, PrivacyValueBalanceV1,
    },
};

#[test]
fn audit_input_openings_bind_each_ordered_private_field_and_value_distribution() {
    let value = three_output_fixture();
    let notes = value
        .witness
        .inputs
        .iter()
        .map(|input| input.note.clone())
        .collect::<Vec<_>>();
    let derive = super::derive_private_note_input_openings_commitment_v1;
    let expected = derive(&notes).expect("two openings");
    assert!(derive(&notes[..1]).is_err());
    for input in 0..2 {
        for field in 0..5 {
            let mut changed = notes.clone();
            match field {
                0 => changed[input].value ^= 1,
                1 => changed[input].spending_authority[0] ^= 1,
                2 => changed[input].rho[0] ^= 1,
                3 => changed[input].blinding[0] ^= 1,
                _ => changed[input].memo_digest[0] ^= 1,
            }
            assert_ne!(derive(&changed).expect("changed opening"), expected);
        }
    }
    let mut changed = notes.clone();
    changed.swap(0, 1);
    assert_ne!(derive(&changed).expect("reordered openings"), expected);
    changed = notes;
    changed[0].value -= 1;
    changed[1].value += 1;
    assert_ne!(
        derive(&changed).expect("same total, different provenance"),
        expected
    );
}

#[test]
fn virtual_zero_input_needs_no_membership_but_positive_input_does() {
    let mut value = three_output_fixture();
    value.witness.inputs[1].authentication_path = [[0xC7; 32]; super::PRIVATE_NOTE_TREE_DEPTH_V1];
    preflight_private_note_relation_with_profile_v1(
        &value.statement,
        &value.witness,
        value.profile,
    )
    .expect("zero cover is independent of the live accumulator");
    value.witness.inputs[0].authentication_path = [[0xC7; 32]; super::PRIVATE_NOTE_TREE_DEPTH_V1];
    assert_eq!(
        preflight_private_note_relation_with_profile_v1(
            &value.statement,
            &value.witness,
            value.profile
        ),
        Err(IvmPrivateNoteRelationErrorV1::Membership)
    );
}
fn rebind_program(value: &mut Fixture, program: PrivateProgramV1) {
    value.witness.program = program;
    value.statement.program_id =
        derive_private_program_id_v1(&value.witness.program).expect("program id");
    let input = &value.witness.inputs[0];
    value.statement.nullifiers[0] = derive_note_nullifier_v1(
        &value.statement,
        &input.spending_secret,
        &input.note.rho,
        value.input_commitment,
    )
    .expect("program-bound nullifier");
    let leaf = accumulator_leaf_invocation_v1(&value.statement, 0, value.input_commitment)
        .expect("program-bound leaf");
    let mut root = leaf.digest;
    let mut position = input.leaf_position;
    for (level, sibling) in input.authentication_path.iter().enumerate() {
        let level = u8::try_from(level).expect("depth fits u8");
        let invocation = if position & 1 == 0 {
            accumulator_node_invocation_v1(0, level, &root, sibling)
        } else {
            accumulator_node_invocation_v1(0, level, sibling, &root)
        }
        .expect("program-bound node");
        root = invocation.digest;
        position >>= 1;
    }
    value.statement.state_root = PrivacyRootV1::new(root);
    redigest(&mut value.statement);
}
#[test]
fn canonical_relation_accepts_and_derives_only_statement_effects() {
    let value = fixture();
    let relation = validate_private_note_relation_v1(&value.statement, &value.witness)
        .expect("canonical relation");
    assert_eq!(relation.input_sum, 10);
    assert_eq!(relation.output_sum, 10);
    assert_eq!(relation.final_registers[6], 10);
    assert_eq!(relation.final_registers[7], 10);
    assert_eq!(relation.final_registers[4], 0);
    assert_eq!(relation.invocations.len(), 38);
}
#[test]
fn exact_three_output_profile_accepts_balanced_cover_geometry_only() {
    let value = three_output_fixture();
    let cover_input = &value.witness.inputs[1];
    let profiled_commitment = cover_input
        .commitment_with_profile_v1(value.profile)
        .expect("profiled zero-valued input commitment");
    assert_eq!(
        cover_input.nullifier_with_profile_v1(&value.statement, value.profile),
        derive_note_nullifier_v1(
            &value.statement,
            &cover_input.spending_secret,
            &cover_input.note.rho,
            profiled_commitment,
        ),
        "the profile-aware input nullifier must use the profile-aware commitment"
    );
    assert_eq!(
        cover_input.commitment_v1(),
        Err(IvmPrivateNoteRelationErrorV1::ZeroWitnessComponent),
        "the canonical IVM private-note commitment helper must reject zero-valued notes"
    );
    assert_eq!(
        cover_input.nullifier_v1(&value.statement),
        Err(IvmPrivateNoteRelationErrorV1::ZeroWitnessComponent),
        "the canonical IVM private-note nullifier helper must reject zero-valued notes"
    );
    let relation = validate_private_note_relation_with_profile_v1(
        &value.statement,
        &value.witness,
        value.profile,
    )
    .expect("exact three-output relation");
    assert_eq!(relation.input_sum, 10);
    assert_eq!(relation.output_sum, 10);
    assert_eq!(value.witness.inputs.len(), 2);
    assert_eq!(value.witness.outputs.len(), 3);
    assert_eq!(value.witness.inputs[1].note.value, 0);
    assert_eq!(value.witness.outputs[1].note.value, 0);
    assert_eq!(value.witness.outputs[2].note.value, 0);
    assert_eq!(relation.final_registers[6], 10);
    assert_eq!(relation.final_registers[7], 10);
    assert_eq!(
        validate_private_note_relation_v1(&value.statement, &value.witness),
        Err(IvmPrivateNoteRelationErrorV1::InvalidStatement),
        "the canonical IVM private-note relation must enforce one-or-two-output geometry"
    );
    assert_eq!(
        PrivateNotePlaintextV1::new(0, bytes(0xC1), bytes(0xC2), bytes(0xC3), bytes(0xC4),),
        Err(IvmPrivateNoteRelationErrorV1::ZeroWitnessComponent),
        "the canonical IVM private-note constructor must reject zero value"
    );
}
#[test]
fn exact_three_output_cover_note_still_requires_nonzero_secret_material() {
    let canonical = three_output_fixture();
    for component in ["authority", "rho", "blinding"] {
        let mut changed = canonical.clone();
        let cover = &mut changed.witness.outputs[2].note;
        match component {
            "authority" => cover.spending_authority = [0; 32],
            "rho" => cover.rho = [0; 32],
            "blinding" => cover.blinding = [0; 32],
            _ => unreachable!(),
        }
        assert_eq!(
            preflight_private_note_relation_with_profile_v1(
                &changed.statement,
                &changed.witness,
                changed.profile,
            ),
            Err(IvmPrivateNoteRelationErrorV1::ZeroWitnessComponent),
            "zero-valued cover output admitted zero {component}"
        );
    }
    let mut changed = canonical;
    changed.witness.inputs[1].authentication_path[7] = [0; 32];
    assert_eq!(
        preflight_private_note_relation_with_profile_v1(
            &changed.statement,
            &changed.witness,
            changed.profile,
        ),
        Err(IvmPrivateNoteRelationErrorV1::ZeroWitnessComponent),
        "the settlement profile must retain the nonzero authentication-path invariant"
    );
}
#[test]
fn program_authority_commitment_and_stable_nullifier_kats_are_pinned() {
    let value = fixture();
    let input = &value.witness.inputs[0];
    assert_eq!(
        value.statement.program_id.as_bytes(),
        &[
            0xc9, 0x46, 0x71, 0x2f, 0x3d, 0xaf, 0xba, 0x7b, 0xc3, 0x61, 0x9e, 0xea, 0x9d, 0x76,
            0x38, 0x8e, 0x54, 0xa6, 0xf8, 0xe2, 0x2b, 0xe9, 0xdf, 0x00, 0x26, 0x0a, 0x9b, 0xac,
            0x0f, 0xf4, 0x85, 0xfc,
        ]
    );
    assert_eq!(
        input.note.spending_authority,
        [
            0x06, 0x74, 0x24, 0xac, 0x7a, 0xc7, 0x81, 0x2b, 0x99, 0xbf, 0x46, 0xd0, 0x7b, 0x90,
            0xcb, 0x35, 0xbb, 0xa8, 0xc0, 0x40, 0xe5, 0x4d, 0x36, 0x9c, 0x42, 0xad, 0xce, 0x98,
            0x76, 0xc3, 0xbb, 0x41,
        ]
    );
    assert_eq!(
        value.input_commitment.as_bytes(),
        &[
            0x69, 0x85, 0x1e, 0x36, 0x11, 0x32, 0x86, 0xe8, 0x88, 0x9d, 0x90, 0x17, 0xf6, 0x6a,
            0xce, 0x11, 0xf5, 0x66, 0xf2, 0xfc, 0xfd, 0x8b, 0x22, 0x51, 0x4b, 0xac, 0x77, 0xcd,
            0xe6, 0xf9, 0x36, 0x0f,
        ]
    );
    assert_eq!(
        value.statement.nullifiers[0].as_bytes(),
        &[
            0x59, 0x88, 0x34, 0x7f, 0x9a, 0xcb, 0x52, 0x8b, 0x8d, 0x9b, 0x00, 0x76, 0xb3, 0x39,
            0x6b, 0x52, 0x15, 0x8c, 0x74, 0x4d, 0x16, 0xfa, 0x39, 0x19, 0xcb, 0x19, 0x34, 0xd8,
            0x84, 0x53, 0xae, 0x2c,
        ]
    );
}
#[test]
fn nullifier_is_stable_across_every_replay_context() {
    let value = fixture();
    let input = &value.witness.inputs[0];
    let canonical = derive_note_nullifier_v1(
        &value.statement,
        &input.spending_secret,
        &input.note.rho,
        value.input_commitment,
    )
    .expect("canonical nullifier");
    let mut replay = value.statement.clone();
    replay.context.network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([0xC2; 32]),
    ));
    replay.context.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new(bytes(0xa1));
    replay.context.action_index = 7;
    replay.root_epoch = 999;
    replay.execution_epoch = 999;
    redigest(&mut replay);
    let replayed = derive_note_nullifier_v1(
        &replay,
        &input.spending_secret,
        &input.note.rho,
        value.input_commitment,
    )
    .expect("replay nullifier");
    assert_eq!(canonical, replayed);
    replay.nullifiers[0] = replayed;
    redigest(&mut replay);
    validate_private_note_relation_v1(&replay, &value.witness)
        .expect("the replay relation retains the same ledger-visible nullifier");
    let different_position = input.leaf_position ^ u32::MAX;
    assert_ne!(different_position, input.leaf_position);
    let position_independent = derive_note_nullifier_v1(
        &value.statement,
        &input.spending_secret,
        &input.note.rho,
        value.input_commitment,
    )
    .expect("position-independent nullifier");
    assert_eq!(canonical, position_independent);
    let mut other_pool = value.statement.clone();
    other_pool.pool_id = PrivacyPoolIdV1::new(bytes(0xa2));
    assert_ne!(
        canonical,
        derive_note_nullifier_v1(
            &other_pool,
            &input.spending_secret,
            &input.note.rho,
            value.input_commitment,
        )
        .expect("pool-separated nullifier")
    );
}
#[test]
fn program_codec_rejects_every_noncanonical_shape() {
    let program = conservation_program();
    let encoded = encode_private_program_v1(&program).expect("canonical program");
    assert_eq!(encoded.len(), PRIVATE_PROGRAM_BYTES_V1);
    assert_eq!(
        decode_private_program_v1(&encoded).expect("decode"),
        program
    );
    for length in 0..encoded.len() {
        assert_eq!(
            decode_private_program_v1(&encoded[..length]),
            Err(IvmPrivateNoteRelationErrorV1::NonCanonicalProgram)
        );
    }
    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_private_program_v1(&trailing),
        Err(IvmPrivateNoteRelationErrorV1::NonCanonicalProgram)
    );
    for index in 0..8 {
        let mut changed = encoded;
        changed[index] ^= 0x80;
        assert_eq!(
            decode_private_program_v1(&changed),
            Err(IvmPrivateNoteRelationErrorV1::NonCanonicalProgram)
        );
    }
    let mut unknown_opcode = encoded;
    unknown_opcode[8] = u8::MAX;
    assert_eq!(
        decode_private_program_v1(&unknown_opcode),
        Err(IvmPrivateNoteRelationErrorV1::NonCanonicalProgram)
    );
    let mut unused_operand = encoded;
    unused_operand[8 + 2 * 8 + 1] = 1;
    assert_eq!(
        decode_private_program_v1(&unused_operand),
        Err(IvmPrivateNoteRelationErrorV1::NonCanonicalProgram)
    );
    let mut post_halt = encoded;
    post_halt[8 + 4 * 8] = PrivateOpcodeV1::MoveImmediate as u8;
    post_halt[8 + 4 * 8 + 1] = 1;
    assert_eq!(
        decode_private_program_v1(&post_halt),
        Err(IvmPrivateNoteRelationErrorV1::NonCanonicalProgram)
    );
}
#[test]
fn relation_rejects_witness_and_membership_mutations() {
    let canonical = fixture();
    let mut changed = canonical.clone();
    changed.witness.inputs[0].spending_secret[0] ^= 1;
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::SpendingAuthorityMismatch)
    );
    let mut changed = canonical.clone();
    changed.witness.inputs[0].note.rho[0] ^= 1;
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::NullifierMismatch)
    );
    let mut changed = canonical.clone();
    changed.witness.inputs[0].authentication_path[17][9] ^= 1;
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::Membership)
    );
    let mut changed = canonical.clone();
    changed.witness.inputs[0].leaf_position ^= 1;
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::Membership)
    );
    let mut changed = canonical.clone();
    changed.witness.outputs[0].note.memo_digest[0] ^= 1;
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::CommitmentMismatch)
    );
    let mut changed = canonical.clone();
    changed.witness.inputs[0].authentication_path[0] = [0; 32];
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::ZeroWitnessComponent)
    );
}
#[test]
fn relation_rejects_public_replays_and_value_attacks() {
    let canonical = fixture();
    let mut changed = canonical.clone();
    changed.statement.context.transaction_intent_digest =
        PrivacyTransactionIntentDigestV1::new(bytes(0xb1));
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::InvalidStatement)
    );
    let mut changed = canonical.clone();
    changed.statement.context.parameter_digest = PrivacyParameterDigestV1::new([0; 32]);
    redigest(&mut changed.statement);
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::InvalidStatement)
    );
    let mut changed = canonical.clone();
    changed.statement.nullifiers[0] = PrivacyNullifierV1::new(bytes(0xb2));
    redigest(&mut changed.statement);
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::NullifierMismatch)
    );
    let mut changed = canonical.clone();
    changed.statement.state_root = PrivacyRootV1::new(bytes(0xb3));
    redigest(&mut changed.statement);
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::Membership)
    );
    let mut changed = canonical.clone();
    changed.statement.execution_epoch += 1;
    redigest(&mut changed.statement);
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::InvalidStatement)
    );
    let mut changed = canonical.clone();
    changed.witness.outputs[0].note.value -= 1;
    changed.statement.output_commitments[0] =
        derive_note_commitment_v1(&changed.witness.outputs[0].note).expect("commitment");
    changed.statement.encrypted_outputs[0].commitment = changed.statement.output_commitments[0];
    redigest(&mut changed.statement);
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::ValueConservation)
    );
    let mut changed = canonical.clone();
    changed.statement.value_balance = PrivacyValueBalanceV1 {
        direction: PrivacyValueBalanceDirectionV1::IntoPool,
        amount: u128::MAX,
    };
    redigest(&mut changed.statement);
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::ValueOverflow)
    );
    let mut changed = canonical.clone();
    changed.statement.encrypted_outputs[0].ciphertext.clear();
    redigest(&mut changed.statement);
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::InvalidStatement)
    );
    let mut changed = canonical.clone();
    changed.witness.inputs.clear();
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::WitnessShape)
    );
}
#[test]
fn relation_rejects_noncanonical_ciphertext_and_binds_canonical_bytes() {
    let canonical = fixture();
    let mut malformed = canonical.clone();
    malformed.statement.encrypted_outputs[0].ciphertext = vec![0xde, 0xad, 0xbe, 0xef];
    redigest(&mut malformed.statement);
    assert_eq!(
        validate_private_note_relation_v1(&malformed.statement, &malformed.witness),
        Err(IvmPrivateNoteRelationErrorV1::InvalidStatement)
    );
    let mut changed = canonical.clone();
    let last = changed.statement.encrypted_outputs[0].ciphertext.len() - 1;
    changed.statement.encrypted_outputs[0].ciphertext[last] ^= 1;
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::InvalidStatement)
    );
    // The relation binds the exact canonical ciphertext through the action
    // digest. Recipient-local AEAD authentication, tested by the wallet codec,
    // deliberately remains outside the arithmetic relation.
    redigest(&mut changed.statement);
    validate_private_note_relation_v1(&changed.statement, &changed.witness)
        .expect("redigested canonical ciphertext is relation-bound");
}
#[test]
fn deterministic_vm_rejects_arithmetic_assertion_and_program_attacks() {
    let canonical = fixture();
    let mut changed = canonical.clone();
    changed.witness.program = PrivateProgramV1 {
        instructions: [PrivateInstructionV1::HALT; 16],
    };
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::ProgramIdMismatch)
    );
    let mut underflow = [PrivateInstructionV1::HALT; 16];
    underflow[0] = PrivateInstructionV1 {
        opcode: PrivateOpcodeV1::SubChecked,
        destination: 6,
        left: 7,
        right: 0,
        immediate: 0,
    };
    let mut changed = canonical.clone();
    rebind_program(
        &mut changed,
        PrivateProgramV1 {
            instructions: underflow,
        },
    );
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::ProgramArithmetic)
    );
    let mut assertion = [PrivateInstructionV1::HALT; 16];
    assertion[0] = PrivateInstructionV1 {
        opcode: PrivateOpcodeV1::AssertLessOrEqual,
        destination: 0,
        left: 0,
        right: 7,
        immediate: 0,
    };
    let mut changed = canonical.clone();
    rebind_program(
        &mut changed,
        PrivateProgramV1 {
            instructions: assertion,
        },
    );
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::ProgramAssertion)
    );
    let mut no_halt = [PrivateInstructionV1::HALT; 16];
    no_halt.fill(PrivateInstructionV1 {
        opcode: PrivateOpcodeV1::MoveImmediate,
        destination: 6,
        left: 0,
        right: 0,
        immediate: 1,
    });
    let mut changed = canonical;
    changed.witness.program = PrivateProgramV1 {
        instructions: no_halt,
    };
    assert_eq!(
        validate_private_note_relation_v1(&changed.statement, &changed.witness),
        Err(IvmPrivateNoteRelationErrorV1::ProgramDoesNotHalt)
    );
}
#[test]
fn relation_rejects_duplicate_and_reused_note_material() {
    let canonical = fixture();
    let mut duplicate = canonical.clone();
    let mut second = duplicate.witness.inputs[0].clone();
    second.note.rho[0] ^= 1;
    second.note.blinding[1] ^= 1;
    let second_commitment = derive_note_commitment_v1(&second.note).expect("second commitment");
    let second_nullifier = derive_note_nullifier_v1(
        &duplicate.statement,
        &second.spending_secret,
        &second.note.rho,
        second_commitment,
    )
    .expect("second nullifier");
    duplicate.witness.inputs.push(second);
    duplicate.statement.nullifiers.push(second_nullifier);
    redigest(&mut duplicate.statement);
    assert_eq!(
        validate_private_note_relation_v1(&duplicate.statement, &duplicate.witness),
        Err(IvmPrivateNoteRelationErrorV1::Duplicate)
    );
    let mut reused = canonical;
    reused.witness.outputs[0].note = reused.witness.inputs[0].note.clone();
    reused.statement.output_commitments[0] = reused.input_commitment;
    reused.statement.encrypted_outputs[0].commitment = reused.input_commitment;
    redigest(&mut reused.statement);
    assert_eq!(
        validate_private_note_relation_v1(&reused.statement, &reused.witness),
        Err(IvmPrivateNoteRelationErrorV1::Duplicate)
    );
}

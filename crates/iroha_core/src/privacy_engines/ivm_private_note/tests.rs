use super::{
    PRIVATE_PROGRAM_BYTES_V1, PrivateInstructionV1, PrivateNotePlaintextV1, PrivateOpcodeV1,
    PrivateProgramV1,
    codec::decode_private_program_v1,
    derive_note_authority_v1, derive_note_commitment_v1, derive_note_nullifier_v1,
    derive_private_program_id_v1, encode_private_program_v1, encrypt_ivm_private_wallet_note_v1,
    ivm_private_recipient_public_key_v1,
    relation::{
        IvmPrivateNoteInputWitnessV1, IvmPrivateNoteOutputWitnessV1, IvmPrivateNoteRelationErrorV1,
        IvmPrivateNoteWitnessV1, PrivateNoteRelationProfileV1, accumulator_leaf_invocation_v1,
        accumulator_node_invocation_v1, derive_profiled_input_commitment_v1,
        derive_profiled_output_commitment_v1, preflight_private_note_relation_with_profile_v1,
        validate_private_note_relation_v1, validate_private_note_relation_with_profile_v1,
    },
};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    domain::DomainId,
    privacy::{
        IrohaIvmPrivateNoteStarkStatementV1, PrivacyActionDigestV1, PrivacyCommitmentV1,
        PrivacyEngineManifestDigestV1, PrivacyNullifierV1, PrivacyParameterDigestV1,
        PrivacyParameterIdV1, PrivacyPoolIdV1, PrivacyRootV1, PrivacyStatementContextV1,
        PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1,
        PrivacyValueBalanceDirectionV1, PrivacyValueBalanceV1, PrivacyVerifierDigestV1,
    },
};
use rand_08::{SeedableRng as _, rngs::StdRng};
use std::str::FromStr as _;
fn bytes(seed: u8) -> [u8; 32] {
    [seed; 32]
}
fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("privacy", "universal").expect("test domain"),
        iroha_data_model::name::Name::from_str("ivmnote").expect("test asset"),
    )
}
fn context() -> PrivacyStatementContextV1 {
    PrivacyStatementContextV1 {
        network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0xC1; 32])
        )),
        action_index: 0,
        transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(bytes(0x31)),
        parameter_id: PrivacyParameterIdV1::new(bytes(0x32)),
        parameter_digest: PrivacyParameterDigestV1::new(bytes(0x33)),
        verifier_digest: PrivacyVerifierDigestV1::new(bytes(0x34)),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(bytes(0x35)),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(bytes(0x36)),
    }
}
fn conservation_program() -> PrivateProgramV1 {
    let mut instructions = [PrivateInstructionV1::HALT; 16];
    instructions[0] = PrivateInstructionV1 {
        opcode: PrivateOpcodeV1::AddChecked,
        destination: 6,
        left: 0,
        right: 2,
        immediate: 0,
    };
    instructions[1] = PrivateInstructionV1 {
        opcode: PrivateOpcodeV1::AddChecked,
        destination: 7,
        left: 1,
        right: 3,
        immediate: 0,
    };
    instructions[2] = PrivateInstructionV1 {
        opcode: PrivateOpcodeV1::AssertEqual,
        destination: 0,
        left: 6,
        right: 7,
        immediate: 0,
    };
    PrivateProgramV1 { instructions }
}
#[derive(Clone)]
pub(super) struct Fixture {
    pub(super) statement: IrohaIvmPrivateNoteStarkStatementV1,
    pub(super) witness: IvmPrivateNoteWitnessV1,
    pub(super) input_commitment: PrivacyCommitmentV1,
}
pub(super) fn fixture() -> Fixture {
    let program = conservation_program();
    let program_id = derive_private_program_id_v1(&program).expect("program id");
    let spending_secret = bytes(0x41);
    let input_note = PrivateNotePlaintextV1 {
        value: 10,
        spending_authority: derive_note_authority_v1(&spending_secret).expect("authority"),
        rho: bytes(0x42),
        blinding: bytes(0x43),
        memo_digest: bytes(0x44),
    };
    let input_commitment = derive_note_commitment_v1(&input_note).expect("input commitment");
    let output_secret = bytes(0x51);
    let output_note = PrivateNotePlaintextV1 {
        value: 10,
        spending_authority: derive_note_authority_v1(&output_secret).expect("output authority"),
        rho: bytes(0x52),
        blinding: bytes(0x53),
        memo_digest: bytes(0x54),
    };
    let output_commitment = derive_note_commitment_v1(&output_note).expect("output commitment");
    let pool_id = PrivacyPoolIdV1::new(bytes(0x61));
    let recipient_public_key =
        ivm_private_recipient_public_key_v1(&bytes(0x71)).expect("recipient public key");
    let encrypted_output = encrypt_ivm_private_wallet_note_v1(
        &mut StdRng::seed_from_u64(0x49_50_4e_45),
        pool_id,
        program_id,
        &output_note,
        recipient_public_key,
    )
    .expect("canonical encrypted output");
    let authentication_path =
        core::array::from_fn(|level| [u8::try_from(level).expect("depth fits u8") + 1; 32]);
    let leaf_position = 0x89ab_cdef;
    let mut statement = IrohaIvmPrivateNoteStarkStatementV1 {
        context: context(),
        asset_definition_id: asset(),
        public_balance_scope: iroha_data_model::asset::AssetBalanceScope::Global,
        pool_id,
        program_id,
        action_digest: PrivacyActionDigestV1::new([0; 32]),
        state_root: PrivacyRootV1::new(bytes(1)),
        root_epoch: 17,
        nullifiers: vec![PrivacyNullifierV1::new(bytes(1))],
        output_commitments: vec![output_commitment],
        encrypted_outputs: vec![encrypted_output],
        value_balance: PrivacyValueBalanceV1::balanced(),
        execution_epoch: 17,
    };
    let leaf =
        accumulator_leaf_invocation_v1(&statement, 0, input_commitment).expect("accumulator leaf");
    let mut root = leaf.digest;
    let mut position = leaf_position;
    for (level, sibling) in authentication_path.iter().enumerate() {
        let level = u8::try_from(level).expect("depth fits u8");
        let invocation = if position & 1 == 0 {
            accumulator_node_invocation_v1(0, level, &root, sibling)
        } else {
            accumulator_node_invocation_v1(0, level, sibling, &root)
        }
        .expect("accumulator node");
        root = invocation.digest;
        position >>= 1;
    }
    assert_eq!(position, 0);
    statement.state_root = PrivacyRootV1::new(root);
    statement.nullifiers[0] = derive_note_nullifier_v1(
        &statement,
        &spending_secret,
        &input_note.rho,
        input_commitment,
    )
    .expect("nullifier");
    statement.action_digest = statement
        .computed_action_digest()
        .expect("canonical action digest");
    let witness = IvmPrivateNoteWitnessV1 {
        program,
        inputs: vec![IvmPrivateNoteInputWitnessV1 {
            note: input_note,
            spending_secret,
            leaf_position,
            authentication_path,
        }],
        outputs: vec![IvmPrivateNoteOutputWitnessV1 { note: output_note }],
    };
    Fixture {
        statement,
        witness,
        input_commitment,
    }
}
#[derive(Clone)]
pub(super) struct ThreeOutputFixture {
    pub(super) statement: IrohaIvmPrivateNoteStarkStatementV1,
    pub(super) witness: IvmPrivateNoteWitnessV1,
    pub(super) profile: PrivateNoteRelationProfileV1,
}
pub(super) fn three_output_fixture() -> ThreeOutputFixture {
    let value = fixture();
    let memo_digests = [bytes(0x54), bytes(0x64), bytes(0x74)];
    let profile = PrivateNoteRelationProfileV1::exact_three_output_balanced(memo_digests);
    let mut statement = value.statement;
    let mut first_input = value.witness.inputs[0].clone();
    let first_input_commitment =
        derive_note_commitment_v1(&first_input.note).expect("first input commitment");
    let second_input_secret = bytes(0x81);
    let second_input_note = PrivateNotePlaintextV1::new_profiled_input_v1(
        0,
        derive_note_authority_v1(&second_input_secret).expect("second input authority"),
        bytes(0x82),
        bytes(0x83),
        bytes(0x84),
        profile,
    )
    .expect("second cover input note");
    let second_input_commitment = derive_profiled_input_commitment_v1(&second_input_note, profile)
        .expect("second input commitment");
    let leaf_0 = accumulator_leaf_invocation_v1(&statement, 0, first_input_commitment)
        .expect("first input leaf")
        .digest;
    let leaf_1 = accumulator_leaf_invocation_v1(&statement, 1, second_input_commitment)
        .expect("second input leaf")
        .digest;
    let mut path_0 = [[0_u8; 32]; super::PRIVATE_NOTE_TREE_DEPTH_V1];
    let mut path_1 = [[0_u8; 32]; super::PRIVATE_NOTE_TREE_DEPTH_V1];
    path_0[0] = leaf_1;
    path_1[0] = leaf_0;
    for level in 1..super::PRIVATE_NOTE_TREE_DEPTH_V1 {
        let seed = u8::try_from(level)
            .expect("tree depth fits u8")
            .wrapping_add(0xA0);
        path_0[level] = [seed; 32];
        path_1[level] = [seed; 32];
    }
    let mut root = accumulator_node_invocation_v1(0, 0, &leaf_0, &leaf_1)
        .expect("sibling input leaves")
        .digest;
    for (level, sibling) in path_0.iter().enumerate().skip(1) {
        root = accumulator_node_invocation_v1(
            0,
            u8::try_from(level).expect("tree depth fits u8"),
            &root,
            sibling,
        )
        .expect("shared upper input path")
        .digest;
    }
    statement.state_root = PrivacyRootV1::new(root);
    first_input.leaf_position = 0;
    first_input.authentication_path = path_0;
    let second_input = IvmPrivateNoteInputWitnessV1::new_with_profile_v1(
        second_input_note,
        second_input_secret,
        1,
        path_1,
        profile,
    )
    .expect("second cover input witness");

    let first_output = IvmPrivateNoteOutputWitnessV1::new_with_profile_v1(
        value.witness.outputs[0].note.clone(),
        0,
        profile,
    )
    .expect("first profiled output");
    let second_output = IvmPrivateNoteOutputWitnessV1::new_with_profile_v1(
        PrivateNotePlaintextV1::new_profiled_output_v1(
            0,
            derive_note_authority_v1(&bytes(0x91)).expect("second output authority"),
            bytes(0x92),
            bytes(0x93),
            memo_digests[1],
            1,
            profile,
        )
        .expect("second output note"),
        1,
        profile,
    )
    .expect("second profiled output");
    let cover_output = IvmPrivateNoteOutputWitnessV1::new_with_profile_v1(
        PrivateNotePlaintextV1::new_profiled_output_v1(
            0,
            derive_note_authority_v1(&bytes(0xA1)).expect("cover output authority"),
            bytes(0xA2),
            bytes(0xA3),
            memo_digests[2],
            2,
            profile,
        )
        .expect("zero-valued cover output note"),
        2,
        profile,
    )
    .expect("cover profiled output");
    let outputs = vec![first_output, second_output, cover_output];
    statement.output_commitments = outputs
        .iter()
        .enumerate()
        .map(|(index, output)| {
            derive_profiled_output_commitment_v1(&output.note, index, profile)
                .expect("profiled output commitment")
        })
        .collect();
    let encrypted_template = statement.encrypted_outputs[0].clone();
    statement.encrypted_outputs = statement
        .output_commitments
        .iter()
        .map(|commitment| {
            let mut encrypted = encrypted_template.clone();
            encrypted.commitment = *commitment;
            encrypted
        })
        .collect();
    statement.nullifiers = vec![
        derive_note_nullifier_v1(
            &statement,
            &first_input.spending_secret,
            &first_input.note.rho,
            first_input_commitment,
        )
        .expect("first input nullifier"),
        derive_note_nullifier_v1(
            &statement,
            &second_input.spending_secret,
            &second_input.note.rho,
            second_input_commitment,
        )
        .expect("second input nullifier"),
    ];
    redigest(&mut statement);
    let witness = IvmPrivateNoteWitnessV1::new_with_profile_v1(
        value.witness.program,
        vec![first_input, second_input],
        outputs,
        profile,
    )
    .expect("three-output profiled witness");
    ThreeOutputFixture {
        statement,
        witness,
        profile,
    }
}
fn redigest(statement: &mut IrohaIvmPrivateNoteStarkStatementV1) {
    statement.action_digest = PrivacyActionDigestV1::new([0; 32]);
    statement.action_digest = statement
        .computed_action_digest()
        .expect("canonical action digest");
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

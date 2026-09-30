//! Pure native private-note statement material and shared relation test fixtures.

#[cfg(test)]
use super::relation::{
    IvmPrivateNoteInputWitnessV1, IvmPrivateNoteOutputWitnessV1, IvmPrivateNoteWitnessV1,
    PrivateNoteRelationProfileV1, derive_profiled_input_commitment_v1,
    derive_profiled_output_commitment_v1,
};
use super::{
    PrivateInstructionV1, PrivateNotePlaintextV1, PrivateOpcodeV1, PrivateProgramV1,
    derive_note_authority_v1, derive_note_commitment_v1, derive_note_nullifier_v1,
    derive_private_program_id_v1, encrypt_ivm_private_wallet_note_v1,
    ivm_private_recipient_public_key_v1,
    relation::{accumulator_leaf_invocation_v1, accumulator_node_invocation_v1},
};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    privacy::{
        IrohaIvmPrivateNoteStarkStatementV1, PrivacyActionDigestV1, PrivacyCommitmentV1,
        PrivacyEngineManifestDigestV1, PrivacyNullifierV1, PrivacyParameterDigestV1,
        PrivacyParameterIdV1, PrivacyPoolIdV1, PrivacyRootV1, PrivacyStatementContextV1,
        PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1, PrivacyValueBalanceV1,
        PrivacyVerifierDigestV1,
    },
};
use iroha_model_base::domain::DomainId;
use rand_08::{SeedableRng as _, rngs::StdRng};
use std::str::FromStr as _;

pub(super) fn bytes(seed: u8) -> [u8; 32] {
    [seed; 32]
}
pub(super) fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("privacy", "universal").expect("test domain"),
        iroha_model_base::name::Name::from_str("ivmnote").expect("test asset"),
    )
}
pub(super) fn context() -> PrivacyStatementContextV1 {
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
pub(super) fn conservation_program() -> PrivateProgramV1 {
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
    #[cfg(test)]
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
    let authentication_path: [[u8; 32]; super::PRIVATE_NOTE_TREE_DEPTH_V1] =
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
    #[cfg(test)]
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
        #[cfg(test)]
        witness,
        input_commitment,
    }
}
#[cfg(test)]
#[derive(Clone)]
pub(super) struct ThreeOutputFixture {
    pub(super) statement: IrohaIvmPrivateNoteStarkStatementV1,
    pub(super) witness: IvmPrivateNoteWitnessV1,
    pub(super) profile: PrivateNoteRelationProfileV1,
}
#[cfg(test)]
pub(super) fn three_output_fixture() -> ThreeOutputFixture {
    let value = fixture();
    let memo_digests = [bytes(0x54), bytes(0x64), bytes(0x74)];
    let profile = PrivateNoteRelationProfileV1::exact_three_output_balanced(memo_digests, [1; 32]);
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
    let profile = PrivateNoteRelationProfileV1::exact_three_output_balanced(
        memo_digests,
        super::derive_private_note_input_openings_commitment_v1(&[
            first_input.note.clone(),
            second_input.note.clone(),
        ])
        .expect("exact audited input openings"),
    );
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
#[cfg(test)]
pub(super) fn redigest(statement: &mut IrohaIvmPrivateNoteStarkStatementV1) {
    statement.action_digest = PrivacyActionDigestV1::new([0; 32]);
    statement.action_digest = statement
        .computed_action_digest()
        .expect("canonical action digest");
}

//! Deterministic complete fixtures for non-shipping tests and release evidence.
use super::{
    IvmPrivateNoteInputWitnessV1, IvmPrivateNoteOutputWitnessV1, IvmPrivateNoteWitnessV1,
    PRIVATE_NOTE_TREE_DEPTH_V1, PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1, PrivateInstructionV1,
    PrivateNotePlaintextV1, PrivateOpcodeV1, PrivateProgramV1, derive_note_authority_v1,
    derive_note_commitment_v1, derive_note_nullifier_v1, derive_private_program_id_v1,
    encrypt_ivm_private_wallet_note_v1, ivm_private_recipient_public_key_v1,
    relation::{accumulator_leaf_invocation_v1, accumulator_node_invocation_v1},
};
use crate::privacy_profiles::{CompiledPrivacyProfileV1, compiled_privacy_profile_v1};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    domain::DomainId,
    privacy::{
        IrohaIvmPrivateNoteStarkStatementV1, PrivacyActionDigestV1, PrivacyNullifierV1,
        PrivacyPoolIdV1, PrivacyProtocolIdV1, PrivacyRootV1, PrivacyStatementContextV1,
        PrivacyTransactionIntentDigestV1, PrivacyValueBalanceV1,
    },
};
use rand_core_06::{CryptoRng, RngCore};
use std::str::FromStr as _;
/// Complete fixture material kept behind `test` or release-evidence cfg.
pub(crate) struct IvmPrivateNoteReleaseFixtureV1 {
    pub(crate) statement: IrohaIvmPrivateNoteStarkStatementV1,
    pub(crate) witness: IvmPrivateNoteWitnessV1,
}
/// Closed fixture-construction failure. Engine diagnostics remain internal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IvmPrivateNoteReleaseFixtureErrorV1;
fn bytes(seed: u8) -> [u8; 32] {
    [seed; 32]
}
fn context_from_compiled_profile_v1(
    profile: &CompiledPrivacyProfileV1,
) -> PrivacyStatementContextV1 {
    PrivacyStatementContextV1 {
        network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0xC1; 32])
        )),
        action_index: 0,
        transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(bytes(0x31)),
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
    }
}
fn context() -> Result<PrivacyStatementContextV1, IvmPrivateNoteReleaseFixtureErrorV1> {
    let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1)
        .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
    Ok(context_from_compiled_profile_v1(&profile))
}
fn conservation_program() -> Result<PrivateProgramV1, IvmPrivateNoteReleaseFixtureErrorV1> {
    let mut instructions = [PrivateInstructionV1::HALT; PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1];
    instructions[0] = PrivateInstructionV1::new(PrivateOpcodeV1::AddChecked, 6, 0, 2, 0)
        .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
    instructions[1] = PrivateInstructionV1::new(PrivateOpcodeV1::AddChecked, 7, 1, 3, 0)
        .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
    instructions[2] = PrivateInstructionV1::new(PrivateOpcodeV1::AssertEqual, 0, 6, 7, 0)
        .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
    PrivateProgramV1::new(instructions).map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)
}
fn note(
    value: u128,
    spending_secret: [u8; 32],
    rho: [u8; 32],
    blinding: [u8; 32],
    memo_digest: [u8; 32],
) -> Result<PrivateNotePlaintextV1, IvmPrivateNoteReleaseFixtureErrorV1> {
    let authority = derive_note_authority_v1(&spending_secret)
        .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
    PrivateNotePlaintextV1::new(value, authority, rho, blinding, memo_digest)
        .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)
}
fn release_asset_definition_id_v1() -> Result<AssetDefinitionId, IvmPrivateNoteReleaseFixtureErrorV1>
{
    Ok(AssetDefinitionId::derive_from_components(
        DomainId::try_new("privacy", "universal")
            .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?,
        iroha_data_model::name::Name::from_str("ivmnote")
            .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?,
    ))
}
fn build_fixture<R: RngCore + CryptoRng>(
    maximum: bool,
    invalid_path: bool,
    canonical_bootstrap: bool,
    pool_id: PrivacyPoolIdV1,
    asset_definition_id: AssetDefinitionId,
    randomness: &mut R,
) -> Result<IvmPrivateNoteReleaseFixtureV1, IvmPrivateNoteReleaseFixtureErrorV1> {
    if canonical_bootstrap && (maximum || invalid_path) {
        return Err(IvmPrivateNoteReleaseFixtureErrorV1);
    }
    let program = conservation_program()?;
    let program_id =
        derive_private_program_id_v1(&program).map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
    let first_spending_secret = bytes(0x41);
    let first_input = note(
        10,
        first_spending_secret,
        bytes(0x42),
        bytes(0x43),
        bytes(0x44),
    )?;
    let first_input_commitment =
        derive_note_commitment_v1(&first_input).map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
    let first_output = note(10, bytes(0x51), bytes(0x52), bytes(0x53), bytes(0x54))?;
    let first_recipient = ivm_private_recipient_public_key_v1(&bytes(0x71))
        .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
    let first_encrypted = encrypt_ivm_private_wallet_note_v1(
        randomness,
        pool_id,
        program_id,
        &first_output,
        first_recipient,
    )
    .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
    let mut statement = IrohaIvmPrivateNoteStarkStatementV1 {
        context: context()?,
        asset_definition_id,
        public_balance_scope: iroha_data_model::asset::AssetBalanceScope::Global,
        pool_id,
        program_id,
        action_digest: PrivacyActionDigestV1::new([0; 32]),
        state_root: PrivacyRootV1::new(bytes(1)),
        root_epoch: 17,
        nullifiers: vec![PrivacyNullifierV1::new(bytes(1))],
        output_commitments: vec![first_encrypted.commitment],
        encrypted_outputs: vec![first_encrypted],
        value_balance: PrivacyValueBalanceV1::balanced(),
        execution_epoch: 17,
    };
    let mut input_notes = vec![(first_input, first_spending_secret)];
    let mut output_notes = vec![first_output];
    let mut input_commitments = vec![first_input_commitment];
    let mut paths = Vec::with_capacity(input_notes.len());
    let mut positions = Vec::with_capacity(input_notes.len());
    if canonical_bootstrap {
        let path = crate::privacy_engines::proof_managed_accumulator::canonical_single_leaf_authentication_path_v1();
        let mut root = accumulator_leaf_invocation_v1(&statement, 0, input_commitments[0])
            .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?
            .digest;
        for (level, sibling) in path.iter().enumerate() {
            root = accumulator_node_invocation_v1(
                0,
                u8::try_from(level).map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?,
                &root,
                sibling,
            )
            .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?
            .digest;
        }
        statement.state_root = PrivacyRootV1::new(root);
        statement.root_epoch = 1;
        statement.execution_epoch = 1;
        paths.push(path);
        positions.push(0);
    } else if maximum {
        let second_spending_secret = bytes(0x81);
        let second_input = note(
            10,
            second_spending_secret,
            bytes(0x82),
            bytes(0x83),
            bytes(0x84),
        )?;
        let second_input_commitment = derive_note_commitment_v1(&second_input)
            .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
        let second_output = note(10, bytes(0x91), bytes(0x92), bytes(0x93), bytes(0x94))?;
        let second_recipient = ivm_private_recipient_public_key_v1(&bytes(0x95))
            .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
        let second_encrypted = encrypt_ivm_private_wallet_note_v1(
            randomness,
            pool_id,
            program_id,
            &second_output,
            second_recipient,
        )
        .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
        statement
            .output_commitments
            .push(second_encrypted.commitment);
        statement.encrypted_outputs.push(second_encrypted);
        input_notes.push((second_input, second_spending_secret));
        output_notes.push(second_output);
        input_commitments.push(second_input_commitment);
    }
    if maximum {
        let first_leaf = accumulator_leaf_invocation_v1(&statement, 0, input_commitments[0])
            .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?
            .digest;
        let second_leaf = accumulator_leaf_invocation_v1(&statement, 1, input_commitments[1])
            .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?
            .digest;
        let mut first_path = [[0_u8; 32]; PRIVATE_NOTE_TREE_DEPTH_V1];
        let mut second_path = [[0_u8; 32]; PRIVATE_NOTE_TREE_DEPTH_V1];
        first_path[0] = second_leaf;
        second_path[0] = first_leaf;
        for level in 1..PRIVATE_NOTE_TREE_DEPTH_V1 {
            let seed = u8::try_from(level)
                .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?
                .wrapping_add(0xa0);
            first_path[level] = [seed; 32];
            second_path[level] = [seed; 32];
        }
        let mut root = accumulator_node_invocation_v1(0, 0, &first_leaf, &second_leaf)
            .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?
            .digest;
        for (level, sibling) in first_path.iter().enumerate().skip(1) {
            root = accumulator_node_invocation_v1(
                0,
                u8::try_from(level).map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?,
                &root,
                sibling,
            )
            .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?
            .digest;
        }
        statement.state_root = PrivacyRootV1::new(root);
        paths.extend([first_path, second_path]);
        positions.extend([0, 1]);
    } else if !canonical_bootstrap {
        let path =
            core::array::from_fn(|level| [u8::try_from(level).expect("depth 32 fits u8") + 1; 32]);
        let position = 0x89ab_cdef;
        let mut root = accumulator_leaf_invocation_v1(&statement, 0, input_commitments[0])
            .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?
            .digest;
        for (level, sibling) in path.iter().enumerate() {
            let level = u8::try_from(level).map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
            let (left, right) = if position & (1_u32 << level) == 0 {
                (&root, sibling)
            } else {
                (sibling, &root)
            };
            root = accumulator_node_invocation_v1(0, level, left, right)
                .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?
                .digest;
        }
        statement.state_root = PrivacyRootV1::new(root);
        paths.push(path);
        positions.push(position);
    }
    statement.nullifiers = input_notes
        .iter()
        .zip(&input_commitments)
        .map(|((note, secret), commitment)| {
            derive_note_nullifier_v1(&statement, secret, note.rho(), *commitment)
                .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    statement.action_digest = statement
        .computed_action_digest()
        .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
    if invalid_path {
        paths[0][17][9] ^= 1;
    }
    let inputs = input_notes
        .into_iter()
        .zip(positions)
        .zip(paths)
        .map(|(((note, secret), position), path)| {
            IvmPrivateNoteInputWitnessV1::new(note, secret, position, path)
                .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outputs = output_notes
        .into_iter()
        .map(|note| {
            IvmPrivateNoteOutputWitnessV1::new(note)
                .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let witness = IvmPrivateNoteWitnessV1::new(program, inputs, outputs)
        .map_err(|_| IvmPrivateNoteReleaseFixtureErrorV1)?;
    Ok(IvmPrivateNoteReleaseFixtureV1 { statement, witness })
}
/// Construct a complete normal or exact two-by-two production fixture.
pub(crate) fn ivm_private_note_release_fixture_v1<R: RngCore + CryptoRng>(
    maximum: bool,
    randomness: &mut R,
) -> Result<IvmPrivateNoteReleaseFixtureV1, IvmPrivateNoteReleaseFixtureErrorV1> {
    build_fixture(
        maximum,
        false,
        false,
        PrivacyPoolIdV1::new(bytes(0x61)),
        release_asset_definition_id_v1()?,
        randomness,
    )
}
/// Construct a normal fixture whose nonzero authentication path misses the root.
pub(crate) fn ivm_private_note_release_invalid_path_fixture_v1<R: RngCore + CryptoRng>(
    randomness: &mut R,
) -> Result<IvmPrivateNoteReleaseFixtureV1, IvmPrivateNoteReleaseFixtureErrorV1> {
    build_fixture(
        false,
        true,
        false,
        PrivacyPoolIdV1::new(bytes(0x61)),
        release_asset_definition_id_v1()?,
        randomness,
    )
}
/// Construct the complete one-input/one-output fixture whose membership path
/// exactly matches a one-leaf authoritative proof-managed pool bootstrap.
pub(crate) fn ivm_private_note_network_fixture_v1<R: RngCore + CryptoRng>(
    pool_id: PrivacyPoolIdV1,
    asset_definition_id: AssetDefinitionId,
    randomness: &mut R,
) -> Result<IvmPrivateNoteReleaseFixtureV1, IvmPrivateNoteReleaseFixtureErrorV1> {
    build_fixture(false, false, true, pool_id, asset_definition_id, randomness)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::ivm_private_note::relation::validate_private_note_relation_v1;
    use rand_08::{SeedableRng as _, rngs::StdRng};
    #[test]
    fn release_context_binds_every_compiled_profile_digest() {
        let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1)
            .expect("compiled IVM private-note profile");
        let baseline =
            norito::encode_canonical(&context_from_compiled_profile_v1(&profile)).expect("context");
        let mutations: [fn(&mut CompiledPrivacyProfileV1); 5] = [
            |profile| profile.parameter_id.0[0] ^= 1,
            |profile| profile.parameter_digest.0[0] ^= 1,
            |profile| profile.verifier_digest.0[0] ^= 1,
            |profile| profile.statement_schema_digest.0[0] ^= 1,
            |profile| profile.engine_manifest_digest.0[0] ^= 1,
        ];
        for mutate in mutations {
            let mut changed = profile;
            mutate(&mut changed);
            assert_ne!(
                norito::encode_canonical(&context_from_compiled_profile_v1(&changed))
                    .expect("changed context"),
                baseline
            );
        }
    }
    #[test]
    fn shared_normal_maximum_and_invalid_path_fixtures_are_exact() {
        let mut normal_rng = StdRng::seed_from_u64(0x49_50_4e_45);
        let normal =
            ivm_private_note_release_fixture_v1(false, &mut normal_rng).expect("normal fixture");
        assert_eq!(
            normal.statement.context,
            context().expect("compiled release context")
        );
        validate_private_note_relation_v1(&normal.statement, &normal.witness)
            .expect("normal relation");
        assert_eq!(normal.witness.inputs().len(), 1);
        assert_eq!(normal.witness.outputs().len(), 1);
        let mut maximum_rng = StdRng::seed_from_u64(0x49_50_4e_45_02);
        let maximum =
            ivm_private_note_release_fixture_v1(true, &mut maximum_rng).expect("maximum fixture");
        validate_private_note_relation_v1(&maximum.statement, &maximum.witness)
            .expect("maximum relation");
        assert_eq!(maximum.witness.inputs().len(), 2);
        assert_eq!(maximum.witness.outputs().len(), 2);
        let mut invalid_rng = StdRng::seed_from_u64(0x49_50_4e_45_03);
        let invalid = ivm_private_note_release_invalid_path_fixture_v1(&mut invalid_rng)
            .expect("invalid-path fixture");
        assert!(validate_private_note_relation_v1(&invalid.statement, &invalid.witness).is_err());
        let pool_id = PrivacyPoolIdV1::new(bytes(0xb1));
        let asset_definition_id = release_asset_definition_id_v1().expect("asset definition");
        let mut network_rng = StdRng::seed_from_u64(0x49_50_4e_45_04);
        let network = ivm_private_note_network_fixture_v1(
            pool_id,
            asset_definition_id.clone(),
            &mut network_rng,
        )
        .expect("network fixture");
        validate_private_note_relation_v1(&network.statement, &network.witness)
            .expect("network relation");
        assert_eq!(network.statement.pool_id, pool_id);
        assert_eq!(network.statement.asset_definition_id, asset_definition_id);
        assert_eq!(network.statement.root_epoch, 1);
        assert_eq!(network.statement.execution_epoch, 1);
        assert_eq!(network.witness.inputs().len(), 1);
        assert_eq!(network.witness.outputs().len(), 1);
    }
}

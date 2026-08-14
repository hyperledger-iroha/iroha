//! Deterministic complete fixtures for non-shipping tests and release evidence.
use super::{
    PQ_MASP_TREE_DEPTH_V1, PqMaspInputWitnessV1, PqMaspNotePlaintextV1, PqMaspOutputWitnessV1,
    PqMaspWitnessV1, derive_pq_masp_authorization_key_digest_v1, derive_pq_masp_note_commitment_v1,
    derive_pq_masp_note_encryption_keys_digest_v1, derive_pq_masp_nullifier_key_digest_v1,
    derive_pq_masp_nullifier_v1, derive_pq_masp_recipient_id_v1, encrypt_pq_masp_note_v1_with_rng,
    relation::{
        accumulator_leaf_invocation_v1, accumulator_node_invocation_v1, namespace_v1,
        validate_pq_masp_relation_v1,
    },
};
use crate::privacy_profiles::{CompiledPrivacyProfileV1, compiled_privacy_profile_v1};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    domain::DomainId,
    name::Name,
    privacy::{
        PqMaspStarkStatementV1, PrivacyAuthorizationKeyDigestV1, PrivacyNoteEncryptionKeyDigestV1,
        PrivacyNullifierV1, PrivacyPoolIdV1, PrivacyPqAuthorizationProfileV1,
        PrivacyPqNoteEncryptionProfileV1, PrivacyProtocolIdV1, PrivacyRecipientIdV1, PrivacyRootV1,
        PrivacyStatementContextV1, PrivacyTransactionIntentDigestV1,
    },
};
use rand::TryCryptoRng;
use sha2::{Digest as _, Sha256};
use soranet_pq::{
    HedgedRngSeed, MlDsaSuite, MlKemSuite, generate_mldsa_keypair_from_seed,
    generate_mlkem_keypair_from_seed,
};
use std::str::FromStr as _;
use zeroize::Zeroizing;
/// Complete fixture material kept behind `test` or release-evidence cfg.
pub(crate) struct PqMaspReleaseFixtureV1 {
    pub(crate) statement: PqMaspStarkStatementV1,
    pub(crate) witness: PqMaspWitnessV1,
    pub(crate) authorization_secret_key: Zeroizing<Vec<u8>>,
}
/// Closed fixture-construction failure. Key and engine diagnostics stay local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PqMaspReleaseFixtureErrorV1;
const FIXTURE_KEYGEN_SEED_DOMAIN_V1: &[u8] =
    b"iroha.privacy.pq-masp.release-fixture.keygen-seed.v1";
fn raw(byte: u8) -> [u8; 32] {
    [byte; 32]
}
fn fixture_keygen_seed_v1(
    master_seed: [u8; 32],
    purpose: &[u8],
    index: usize,
) -> Result<[u8; 32], PqMaspReleaseFixtureErrorV1> {
    let purpose_length = u64::try_from(purpose.len()).map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    let index = u64::try_from(index).map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    let mut hash = Sha256::new();
    hash.update(FIXTURE_KEYGEN_SEED_DOMAIN_V1);
    hash.update(master_seed);
    hash.update(purpose_length.to_be_bytes());
    hash.update(purpose);
    hash.update(index.to_be_bytes());
    let seed: [u8; 32] = hash.finalize().into();
    if seed.iter().all(|byte| *byte == 0) {
        return Err(PqMaspReleaseFixtureErrorV1);
    }
    Ok(seed)
}
fn context_from_compiled_profile_v1(
    profile: &CompiledPrivacyProfileV1,
) -> PrivacyStatementContextV1 {
    PrivacyStatementContextV1 {
        network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0xC2; 32])
        )),
        action_index: 0,
        transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(raw(1)),
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
    }
}
fn context() -> Result<PrivacyStatementContextV1, PqMaspReleaseFixtureErrorV1> {
    let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::PqMaspStarkV0)
        .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    Ok(context_from_compiled_profile_v1(&profile))
}
fn empty_authentication_path_v1()
-> Result<[[u8; 32]; PQ_MASP_TREE_DEPTH_V1], PqMaspReleaseFixtureErrorV1> {
    const EMPTY_LEAF_DOMAIN_V1: &[u8] = b"iroha.privacy.proof-managed-note-tree.empty-leaf.v1";
    let mut path = [[0_u8; 32]; PQ_MASP_TREE_DEPTH_V1];
    let mut empty: [u8; 32] = Sha256::digest(EMPTY_LEAF_DOMAIN_V1).into();
    for (level, sibling) in path.iter_mut().enumerate() {
        *sibling = empty;
        empty = accumulator_node_invocation_v1(
            0,
            u8::try_from(level).map_err(|_| PqMaspReleaseFixtureErrorV1)?,
            &empty,
            &empty,
        )
        .map_err(|_| PqMaspReleaseFixtureErrorV1)?
        .digest;
    }
    Ok(path)
}
fn anchor_for_input_v1(
    statement: &PqMaspStarkStatementV1,
    input: u8,
    commitment: iroha_data_model::privacy::PrivacyCommitmentV1,
    position: u32,
    path: &[[u8; 32]; PQ_MASP_TREE_DEPTH_V1],
) -> Result<PrivacyRootV1, PqMaspReleaseFixtureErrorV1> {
    let mut current = accumulator_leaf_invocation_v1(statement, input, commitment)
        .map_err(|_| PqMaspReleaseFixtureErrorV1)?
        .digest;
    for (level, sibling) in path.iter().enumerate() {
        let level = u8::try_from(level).map_err(|_| PqMaspReleaseFixtureErrorV1)?;
        let (left, right) = if position & (1_u32 << level) == 0 {
            (&current, sibling)
        } else {
            (sibling, &current)
        };
        current = accumulator_node_invocation_v1(input, level, left, right)
            .map_err(|_| PqMaspReleaseFixtureErrorV1)?
            .digest;
    }
    Ok(PrivacyRootV1::new(current))
}
fn note(
    value: u128,
    authorization_key_digest: PrivacyAuthorizationKeyDigestV1,
    recipient_key_digest: PrivacyRecipientIdV1,
    nullifier_secret: [u8; 32],
    rho: [u8; 32],
    blinding: [u8; 32],
    memo_digest: [u8; 32],
) -> Result<PqMaspNotePlaintextV1, PqMaspReleaseFixtureErrorV1> {
    let nullifier_key_digest = derive_pq_masp_nullifier_key_digest_v1(&nullifier_secret)
        .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    PqMaspNotePlaintextV1::new(
        value,
        authorization_key_digest,
        recipient_key_digest,
        nullifier_key_digest,
        rho,
        blinding,
        memo_digest,
    )
    .map_err(|_| PqMaspReleaseFixtureErrorV1)
}
fn recipient_public_key_v1(
    keygen_master_seed: [u8; 32],
    index: usize,
) -> Result<Vec<u8>, PqMaspReleaseFixtureErrorV1> {
    let personalization = match index {
        0 => b"iroha-pq-masp-release-recipient-0-v1".as_slice(),
        1 => b"iroha-pq-masp-release-recipient-1-v1".as_slice(),
        _ => return Err(PqMaspReleaseFixtureErrorV1),
    };
    let entropy = fixture_keygen_seed_v1(keygen_master_seed, b"ml-kem-768-recipient-key", index)?;
    let keys = generate_mlkem_keypair_from_seed(
        MlKemSuite::MlKem768,
        HedgedRngSeed::from_entropy(entropy),
        personalization,
    )
    .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    Ok(keys.public_key().to_vec())
}
fn build_fixture<R: TryCryptoRng + ?Sized>(
    maximum: bool,
    invalid_path: bool,
    keygen_master_seed: [u8; 32],
    randomness: &mut R,
) -> Result<PqMaspReleaseFixtureV1, PqMaspReleaseFixtureErrorV1> {
    let authorization_key_seed =
        fixture_keygen_seed_v1(keygen_master_seed, b"ml-dsa-65-authorization-key", 0)?;
    let authorization_keys = generate_mldsa_keypair_from_seed(
        MlDsaSuite::MlDsa65,
        HedgedRngSeed::from_entropy(authorization_key_seed),
        b"iroha-pq-masp-release-authorization-v1",
    )
    .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    let authorization_key_digest =
        derive_pq_masp_authorization_key_digest_v1(authorization_keys.public_key())
            .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    let authorization_secret_key = Zeroizing::new(authorization_keys.secret_key().to_vec());
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("privacy", "universal").map_err(|_| PqMaspReleaseFixtureErrorV1)?,
        Name::from_str("pq_note").map_err(|_| PqMaspReleaseFixtureErrorV1)?,
    );
    let mut statement = PqMaspStarkStatementV1 {
        context: context()?,
        asset_definition_id,
        pool_id: PrivacyPoolIdV1::new(raw(7)),
        anchor: PrivacyRootV1::new(raw(8)),
        anchor_epoch: 1,
        nullifiers: vec![PrivacyNullifierV1::new(raw(9))],
        output_commitments: Vec::new(),
        encrypted_outputs: Vec::new(),
        authorization_profile: PrivacyPqAuthorizationProfileV1::MlDsa65,
        authorization_key_digest,
        note_encryption_profile: PrivacyPqNoteEncryptionProfileV1::MlKem768XChaCha20Poly1305,
        note_encryption_key_digest: PrivacyNoteEncryptionKeyDigestV1::new(raw(14)),
        authorization_epoch: 1,
    };
    let input_specs: &[(u128, u8, u8, u8, u8, u8)] = if maximum {
        &[(60, 50, 52, 53, 54, 55), (40, 51, 56, 57, 58, 59)]
    } else {
        &[(70, 15, 16, 17, 18, 19)]
    };
    let mut input_notes = Vec::with_capacity(input_specs.len());
    for &(value, secret, recipient, rho, blinding, memo) in input_specs {
        input_notes.push((
            note(
                value,
                authorization_key_digest,
                PrivacyRecipientIdV1::new(raw(recipient)),
                raw(secret),
                raw(rho),
                raw(blinding),
                raw(memo),
            )?,
            raw(secret),
        ));
    }
    let output_specs: &[(u128, u8, u8, u8, u8)] = if maximum {
        &[(55, 62, 63, 64, 65), (45, 68, 69, 70, 71)]
    } else {
        &[(70, 21, 22, 23, 24)]
    };
    let mut output_notes = Vec::with_capacity(output_specs.len());
    for (index, &(value, secret, rho, blinding, memo)) in output_specs.iter().enumerate() {
        let recipient_public_key = recipient_public_key_v1(keygen_master_seed, index)?;
        let recipient = derive_pq_masp_recipient_id_v1(&recipient_public_key)
            .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
        let output_note = note(
            value,
            authorization_key_digest,
            recipient,
            raw(secret),
            raw(rho),
            raw(blinding),
            raw(memo),
        )?;
        let (commitment, encrypted) = encrypt_pq_masp_note_v1_with_rng(
            &statement,
            &output_note,
            &recipient_public_key,
            randomness,
        )
        .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
        statement.output_commitments.push(commitment);
        statement.encrypted_outputs.push(encrypted);
        output_notes.push(output_note);
    }
    statement.note_encryption_key_digest =
        derive_pq_masp_note_encryption_keys_digest_v1(&statement)
            .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    let input_commitments = input_notes
        .iter()
        .map(|(input_note, _)| {
            derive_pq_masp_note_commitment_v1(&statement, input_note)
                .map_err(|_| PqMaspReleaseFixtureErrorV1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut paths = Vec::with_capacity(input_notes.len());
    let mut positions = Vec::with_capacity(input_notes.len());
    if maximum {
        let first_leaf = accumulator_leaf_invocation_v1(&statement, 0, input_commitments[0])
            .map_err(|_| PqMaspReleaseFixtureErrorV1)?
            .digest;
        let second_leaf = accumulator_leaf_invocation_v1(&statement, 1, input_commitments[1])
            .map_err(|_| PqMaspReleaseFixtureErrorV1)?
            .digest;
        let mut first_path = empty_authentication_path_v1()?;
        let mut second_path = first_path;
        first_path[0] = second_leaf;
        second_path[0] = first_leaf;
        let first_anchor =
            anchor_for_input_v1(&statement, 0, input_commitments[0], 0, &first_path)?;
        let second_anchor =
            anchor_for_input_v1(&statement, 1, input_commitments[1], 1, &second_path)?;
        if first_anchor != second_anchor {
            return Err(PqMaspReleaseFixtureErrorV1);
        }
        statement.anchor = first_anchor;
        paths.extend([first_path, second_path]);
        positions.extend([0, 1]);
    } else {
        let path = empty_authentication_path_v1()?;
        statement.anchor = anchor_for_input_v1(&statement, 0, input_commitments[0], 0, &path)?;
        paths.push(path);
        positions.push(0);
    }
    statement.nullifiers = input_notes
        .iter()
        .zip(&input_commitments)
        .map(|((input_note, secret), commitment)| {
            derive_pq_masp_nullifier_v1(&statement, secret, input_note.rho(), *commitment)
                .map_err(|_| PqMaspReleaseFixtureErrorV1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if invalid_path {
        paths[0][7][3] ^= 1;
    }
    let inputs = input_notes
        .into_iter()
        .zip(positions)
        .zip(paths)
        .map(|(((input_note, secret), position), path)| {
            PqMaspInputWitnessV1::new(input_note, secret, position, path)
                .map_err(|_| PqMaspReleaseFixtureErrorV1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outputs = output_notes
        .into_iter()
        .map(|output_note| {
            PqMaspOutputWitnessV1::new(output_note).map_err(|_| PqMaspReleaseFixtureErrorV1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let witness = PqMaspWitnessV1::new(inputs, outputs).map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    Ok(PqMaspReleaseFixtureV1 {
        statement,
        witness,
        authorization_secret_key,
    })
}
/// Construct a complete normal or exact two-by-two production fixture.
pub(crate) fn pq_masp_release_fixture_v1<R: TryCryptoRng + ?Sized>(
    maximum: bool,
    keygen_master_seed: [u8; 32],
    randomness: &mut R,
) -> Result<PqMaspReleaseFixtureV1, PqMaspReleaseFixtureErrorV1> {
    build_fixture(maximum, false, keygen_master_seed, randomness)
}
/// Construct a normal fixture whose nonzero authentication path misses the root.
pub(crate) fn pq_masp_release_invalid_path_fixture_v1<R: TryCryptoRng + ?Sized>(
    keygen_master_seed: [u8; 32],
    randomness: &mut R,
) -> Result<PqMaspReleaseFixtureV1, PqMaspReleaseFixtureErrorV1> {
    build_fixture(false, true, keygen_master_seed, randomness)
}
/// Refresh the normal fixture's consumed-note path against the exact
/// authoritative successor produced by its own output append.
///
/// The returned relation keeps the original stable nullifier but consumes the
/// post-transition root at the next epoch. A network test can therefore prove
/// an independently signed protocol replay that reaches the nullifier gate,
/// instead of failing earlier as a stale-anchor transaction.
pub(crate) fn pq_masp_release_successor_replay_fixture_v1(
    fixture: &PqMaspReleaseFixtureV1,
) -> Result<(PqMaspStarkStatementV1, PqMaspWitnessV1), PqMaspReleaseFixtureErrorV1> {
    let [input] = fixture.witness.inputs.as_slice() else {
        return Err(PqMaspReleaseFixtureErrorV1);
    };
    let [output] = fixture.witness.outputs.as_slice() else {
        return Err(PqMaspReleaseFixtureErrorV1);
    };
    let [output_commitment] = fixture.statement.output_commitments.as_slice() else {
        return Err(PqMaspReleaseFixtureErrorV1);
    };
    if input.leaf_position != 0 {
        return Err(PqMaspReleaseFixtureErrorV1);
    }
    let input_commitment = input
        .commitment_v1(&fixture.statement)
        .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    let output_leaf = accumulator_leaf_invocation_v1(&fixture.statement, 0, *output_commitment)
        .map_err(|_| PqMaspReleaseFixtureErrorV1)?
        .digest;
    let mut successor_path = input.authentication_path;
    successor_path[0] = output_leaf;
    let mut statement = fixture.statement.clone();
    statement.anchor = anchor_for_input_v1(&statement, 0, input_commitment, 0, &successor_path)?;
    let successor_epoch = statement
        .anchor_epoch
        .checked_add(1)
        .ok_or(PqMaspReleaseFixtureErrorV1)?;
    statement.anchor_epoch = successor_epoch;
    statement.authorization_epoch = successor_epoch;
    let origin =
        crate::privacy_engines::proof_managed_accumulator::build_proof_managed_frontier_v1(
            namespace_v1(&statement),
            &[input_commitment],
        )
        .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    if origin.root != fixture.statement.anchor {
        return Err(PqMaspReleaseFixtureErrorV1);
    }
    let successor =
        crate::privacy_engines::proof_managed_accumulator::append_proof_managed_commitments_v1(
            namespace_v1(&statement),
            origin.tree_size,
            origin.leaf,
            &origin.ommers,
            origin.root,
            &[*output_commitment],
        )
        .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    if successor.root != statement.anchor {
        return Err(PqMaspReleaseFixtureErrorV1);
    }
    let replay_input = PqMaspInputWitnessV1::new(
        input.note.clone(),
        input.nullifier_secret,
        input.leaf_position,
        successor_path,
    )
    .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    let witness = PqMaspWitnessV1::new(vec![replay_input], vec![output.clone()])
        .map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    validate_pq_masp_relation_v1(&statement, &witness).map_err(|_| PqMaspReleaseFixtureErrorV1)?;
    Ok((statement, witness))
}
#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng as _, rngs::StdRng};
    #[test]
    fn release_context_binds_every_compiled_profile_digest() {
        let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::PqMaspStarkV0)
            .expect("compiled PQ-MASP profile");
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
        let mut normal_rng = StdRng::from_seed(raw(0xd1));
        let normal =
            pq_masp_release_fixture_v1(false, raw(0xe1), &mut normal_rng).expect("normal fixture");
        assert_eq!(
            normal.statement.context,
            context().expect("compiled release context")
        );
        validate_pq_masp_relation_v1(&normal.statement, &normal.witness).expect("normal relation");
        assert_eq!(normal.witness.inputs().len(), 1);
        assert_eq!(normal.witness.outputs().len(), 1);
        assert!(!normal.authorization_secret_key.is_empty());
        let mut maximum_rng = StdRng::from_seed(raw(0xd2));
        let maximum =
            pq_masp_release_fixture_v1(true, raw(0xe2), &mut maximum_rng).expect("maximum fixture");
        validate_pq_masp_relation_v1(&maximum.statement, &maximum.witness)
            .expect("maximum relation");
        assert_eq!(maximum.witness.inputs().len(), 2);
        assert_eq!(maximum.witness.outputs().len(), 2);
        let mut invalid_rng = StdRng::from_seed(raw(0xd3));
        let invalid = pq_masp_release_invalid_path_fixture_v1(raw(0xe3), &mut invalid_rng)
            .expect("invalid fixture");
        assert!(validate_pq_masp_relation_v1(&invalid.statement, &invalid.witness).is_err());
    }
    #[test]
    fn successor_replay_refreshes_anchor_but_preserves_the_stable_nullifier() {
        let mut rng = StdRng::from_seed(raw(0xd4));
        let fixture =
            pq_masp_release_fixture_v1(false, raw(0xe4), &mut rng).expect("normal fixture");
        let (statement, witness) = pq_masp_release_successor_replay_fixture_v1(&fixture)
            .expect("successor replay fixture");
        assert_ne!(statement.anchor, fixture.statement.anchor);
        assert_eq!(statement.anchor_epoch, fixture.statement.anchor_epoch + 1);
        assert_eq!(
            statement.authorization_epoch,
            fixture.statement.authorization_epoch + 1
        );
        assert_eq!(statement.authorization_epoch, statement.anchor_epoch);
        assert_eq!(statement.nullifiers, fixture.statement.nullifiers);
        assert_eq!(
            statement.output_commitments,
            fixture.statement.output_commitments
        );
        validate_pq_masp_relation_v1(&statement, &witness).expect("refreshed replay relation");
    }
    #[test]
    fn keygen_subseeds_are_deterministic_and_purpose_separated() {
        let master = raw(0xf1);
        let authorization =
            fixture_keygen_seed_v1(master, b"ml-dsa-65-authorization-key", 0).expect("seed");
        assert_eq!(
            authorization,
            fixture_keygen_seed_v1(master, b"ml-dsa-65-authorization-key", 0).expect("same seed")
        );
        let recipient_0 =
            fixture_keygen_seed_v1(master, b"ml-kem-768-recipient-key", 0).expect("seed");
        let recipient_1 =
            fixture_keygen_seed_v1(master, b"ml-kem-768-recipient-key", 1).expect("seed");
        assert_ne!(authorization, recipient_0);
        assert_ne!(authorization, recipient_1);
        assert_ne!(recipient_0, recipient_1);
    }
}

//! Genuine native finality over explicitly synthetic SNS World preimages.

use super::*;
use crate::sumeragi_finality::{
    WorldStateElementKindV1, WorldStateSnapshotEntryV1, test_fixtures::NativeFinalityFixture,
    world_state_value_hash_v1,
};
use iroha_crypto::{Algorithm, KeyPair};

fn certify(native: &mut NativeFinalityFixture, proof: &SnsLeaseProofV1) -> VerifiedSumeragiBlock {
    let header = native.next_header();
    let block = native.block_with_submitted_work(header);
    let certificate = native.certify_with_world_root(block, proof.world.root().unwrap());
    native
        .verifier()
        .verify_retained_decision(&certificate)
        .unwrap()
}
fn bind_record(proof: &mut SnsLeaseProofV1, record: &NameRecordV1) {
    proof.record = record.encode();
    proof.world.entries = vec![WorldStateSnapshotEntryV1 {
        field_id: "world.smart_contract_state".into(),
        kind: WorldStateElementKindV1::Table,
        key_hash: Some(world_state_value_hash_v1(&record_storage_key(&record.selector)).unwrap()),
        value_hash: world_state_value_hash_v1(&proof.record).unwrap(),
    }];
}
fn fixture() -> (
    NativeFinalityFixture,
    SnsLeaseProofV1,
    NameRecordV1,
    VerifiedSumeragiBlock,
) {
    let mut native = NativeFinalityFixture::start("sns-lease-synthetic-world");
    let owner = AccountId::new(
        KeyPair::from_seed(vec![211; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let selector =
        NameSelectorV1::new(super::super::DATASPACE_ALIAS_SUFFIX_ID, "myprivate").unwrap();
    let mut record = NameRecordV1::new(
        selector,
        owner,
        vec![],
        0,
        1,
        60_000,
        70_000,
        80_000,
        Default::default(),
    );
    record.ownership_generation = u64::MAX;
    let mut proof = SnsLeaseProofV1 {
        world: WorldStateSnapshotV1 {
            schema_hash: Hash::new(b"independently-installed-synthetic-schema"),
            entries: vec![],
        },
        record: vec![],
    };
    bind_record(&mut proof, &record);
    let block = certify(&mut native, &proof);
    (native, proof, record, block)
}

#[test]
fn canonical_and_borrowed_projection_preserve_exact_full_width_lease() {
    let (native, proof, record, block) = fixture();
    let bytes = norito::encode_canonical(&proof).unwrap();
    assert_eq!(SnsLeaseProofV1::decode_frame(&bytes).unwrap(), proof);
    assert_eq!(
        norito::encode_canonical(&SnsLeaseProofRefV1::new(&proof.world, &proof.record)).unwrap(),
        bytes
    );
    assert_eq!(
        norito::json::to_vec(&SnsLeaseProofRefV1::new(&proof.world, &proof.record)).unwrap(),
        norito::json::to_vec(&proof).unwrap()
    );
    let verified = proof
        .verify(
            native.network_id(),
            &record.selector,
            &record.owner,
            proof.world.schema_hash,
            &block,
            20_000,
        )
        .unwrap();
    assert_eq!(verified.record(), &record);
    assert_eq!(verified.network_id(), native.network_id());
    assert_eq!(verified.height(), block.height());
    assert_eq!(verified.context_id(), block.context_id());
    assert!(SnsLeaseProofV1::decode_frame(&[bytes.as_slice(), b"suffix"].concat()).is_err());
    let mut value = norito::json::to_value(&proof).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("checkpoint".into(), norito::json::Value::Null);
    assert!(norito::json::from_value::<SnsLeaseProofV1>(value).is_err());
}

#[test]
fn lease_requires_independent_network_schema_selector_owner_and_original_cut() {
    let (native, proof, record, block) = fixture();
    let verify = |proof: &SnsLeaseProofV1| {
        proof.verify(
            native.network_id(),
            &record.selector,
            &record.owner,
            proof.world.schema_hash,
            &block,
            20_000,
        )
    };
    let mut changed = proof.clone();
    changed.record[0] ^= 1;
    assert!(verify(&changed).is_err());
    changed = proof.clone();
    changed.world.entries.clear();
    assert!(verify(&changed).is_err());
    assert!(
        proof
            .verify(
                native.network_id(),
                &record.selector,
                &record.owner,
                Hash::new(b"foreign-schema"),
                &block,
                20_000
            )
            .is_err()
    );
    let other =
        NameSelectorV1::new(super::super::DATASPACE_ALIAS_SUFFIX_ID, "someoneelse").unwrap();
    assert!(
        proof
            .verify(
                native.network_id(),
                &other,
                &record.owner,
                proof.world.schema_hash,
                &block,
                20_000
            )
            .is_err()
    );
    let other = AccountId::new(
        KeyPair::from_seed(vec![212; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    assert!(
        proof
            .verify(
                native.network_id(),
                &record.selector,
                &other,
                proof.world.schema_hash,
                &block,
                20_000
            )
            .is_err()
    );
    let foreign = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        Hash::new(b"different-signed-genesis"),
    ));
    assert!(
        proof
            .verify(
                foreign,
                &record.selector,
                &record.owner,
                proof.world.schema_hash,
                &block,
                20_000
            )
            .is_err()
    );
    assert!(
        proof
            .verify(
                native.network_id(),
                &record.selector,
                &record.owner,
                proof.world.schema_hash,
                &block,
                60_000
            )
            .is_err()
    );
}

#[test]
fn even_certified_malformed_inactive_expired_or_zero_generation_records_are_refused() {
    let (mut native, proof, record, _) = fixture();
    for mutation in 0..6 {
        let mut record = record.clone();
        match mutation {
            0 => record.ownership_generation = 0,
            1 => record.name_hash = [7; 32],
            2 => record.expires_at_ms = 1,
            3 => record.grace_expires_at_ms = 2,
            4 => record.status = NameStatus::GracePeriod,
            _ => record.registered_at_ms = u64::MAX,
        }
        let mut proof = proof.clone();
        bind_record(&mut proof, &record);
        let block = certify(&mut native, &proof);
        assert!(
            proof
                .verify(
                    native.network_id(),
                    &record.selector,
                    &record.owner,
                    proof.world.schema_hash,
                    &block,
                    20_000
                )
                .is_err()
        );
    }
    let mut noncanonical = proof.clone();
    noncanonical.record.push(0);
    noncanonical.world.entries[0].value_hash =
        world_state_value_hash_v1(&noncanonical.record).unwrap();
    let block = certify(&mut native, &noncanonical);
    assert!(
        noncanonical
            .verify(
                native.network_id(),
                &record.selector,
                &record.owner,
                noncanonical.world.schema_hash,
                &block,
                20_000
            )
            .is_err()
    );
}

#[test]
fn old_ownership_generation_cannot_replace_the_current_selected_parent_cut() {
    let (mut native, mut old, mut record, _) = fixture();
    record.ownership_generation = 10;
    bind_record(&mut old, &record);
    let old_cut = certify(&mut native, &old);
    let mut current = old.clone();
    record.ownership_generation = 11;
    bind_record(&mut current, &record);
    let current_cut = certify(&mut native, &current);
    assert!(
        old.verify(
            native.network_id(),
            &record.selector,
            &record.owner,
            old.world.schema_hash,
            &old_cut,
            20_000
        )
        .is_ok()
    );
    assert!(
        old.verify(
            native.network_id(),
            &record.selector,
            &record.owner,
            old.world.schema_hash,
            &current_cut,
            20_000
        )
        .is_err()
    );
    assert_eq!(
        current
            .verify(
                native.network_id(),
                &record.selector,
                &record.owner,
                current.world.schema_hash,
                &current_cut,
                20_000
            )
            .unwrap()
            .record()
            .ownership_generation,
        11
    );
}

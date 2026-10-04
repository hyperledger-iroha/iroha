//! Genuine native owner funding, capacity registration and same-cut pricing projection.

use super::*;
use iroha_data_model::isi::sorafs::{
    DecideSorafsReserveMovement, RegisterCapacityDeclaration, RequestSorafsReserveMovement,
    SetPricingSchedule,
};
use iroha_data_model::sorafs::reserve::ReserveMovementKindV1;
use sorafs_manifest::{
    capacity::{
        CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, CapacityMetadataEntry,
        ChunkerCommitmentV1,
    },
    provider_advert::{CapabilityType, StakePointer},
};

fn declaration(f: &Fixture) -> CapacityDeclarationV1 {
    CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id: *f.provider.as_bytes(),
        stake: StakePointer {
            pool_id: [0x72; 32],
            stake_amount: "1".parse().unwrap(),
        },
        committed_capacity_gib: 1,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: None,
            committed_gib: 1,
            capability_refs: vec![CapabilityType::ToriiGateway],
        }],
        lane_commitments: Vec::new(),
        pricing: None,
        valid_from: 100,
        valid_until: 2_000_000_000,
        metadata: vec![
            CapacityMetadataEntry {
                key: "sorafs.owner_account_id".into(),
                value: f.owner.to_string(),
            },
            CapacityMetadataEntry {
                key: "sorafs.storage_class".into(),
                value: "hot".into(),
            },
            CapacityMetadataEntry {
                key: "note_a".into(),
                value: "a".repeat(3_000),
            },
            CapacityMetadataEntry {
                key: "note_b".into(),
                value: "b".repeat(3_000),
            },
        ],
    }
}

fn funded() -> Fixture {
    let mut f = Fixture::new_with_owner_funds(true);
    f.publish_policy();
    f.register();
    let owner_key = KeyPair::from_seed(vec![0xb4; 32], Algorithm::Ed25519);
    assert_eq!(AccountId::new(owner_key.public_key().clone()), f.owner);
    let digest = f.policy.digest().unwrap();
    let signed = f.chain.sign(
        &owner_key,
        [RequestSorafsReserveMovement::new(
            [0x73; 32],
            f.provider,
            ReserveMovementKindV1::TopUp,
            "1".parse().unwrap(),
            1,
            digest,
        )
        .into()],
        5_000,
    );
    assert_eq!(f.chain.commit(vec![signed]), vec![true]);
    let signed = f.chain.sign(
        &f.key,
        [DecideSorafsReserveMovement::new(
            [0x73; 32],
            2,
            digest,
            true,
            "Native original capacity funding".into(),
        )
        .into()],
        6_000,
    );
    assert_eq!(f.chain.commit(vec![signed]), vec![true]);
    let credit = ProviderCreditRecord::new(
        f.provider,
        0_u32.into(),
        1_u32.into(),
        1_u32.into(),
        0_u32.into(),
        6,
        6,
        iroha_model_base::metadata::Metadata::default(),
    );
    let signed = f.chain.sign(
        &f.key,
        [UpsertProviderCredit::new(None, credit.clone()).into()],
        7_000,
    );
    assert_eq!(f.chain.commit(vec![signed]), vec![true]);
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let proof = f.proof(&budget);
    let current = proof.verify(&f.expected(), &f.verified_tip()).unwrap();
    assert_eq!(
        current.current().unwrap().reserve_balance,
        "1".parse().unwrap()
    );
    assert!(current.current().unwrap().debt_principal.is_zero());
    assert_eq!(current.credit(), Some(&credit));
    assert!(current.capacity().is_none());
    f
}

fn register_capacity(f: &mut Fixture) -> CapacityDeclarationRecord {
    let payload = declaration(f);
    payload.validate().unwrap();
    let bytes = norito::encode_canonical(&payload).unwrap();
    assert!(bytes.len() > 4_096);
    let owner_key = KeyPair::from_seed(vec![0xb4; 32], Algorithm::Ed25519);
    let signed = f.chain.sign(
        &owner_key,
        [RegisterCapacityDeclaration::new(bytes.clone()).into()],
        8_000,
    );
    let original = signed.hash_as_entrypoint();
    assert_eq!(f.chain.commit(vec![signed]), vec![true]);
    let block = f.chain.committed(f.chain.height());
    assert_eq!(block.block().external_transactions().count(), 1);
    assert_eq!(
        block
            .block()
            .external_transactions()
            .next()
            .unwrap()
            .hash_as_entrypoint(),
        original
    );
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let proof = f.proof(&budget);
    let verified = proof.verify(&f.expected(), &f.verified_tip()).unwrap();
    let record = verified.capacity().unwrap();
    assert_eq!(record.provider_id, f.provider);
    assert_eq!(record.declaration, bytes);
    assert_eq!(
        record.committed_capacity_gib,
        payload.committed_capacity_gib
    );
    assert_eq!(record.registered_epoch, block.block_time_ms() / 1_000);
    assert_eq!(
        (record.valid_from_epoch, record.valid_until_epoch),
        (payload.valid_from, payload.valid_until)
    );
    assert!(record.valid_from_epoch > verified.block_time_ms() / 1_000);
    record.clone()
}

#[test]
fn capacity_and_pricing_projection_tracks_real_funding_registration_and_governed_update() {
    let mut f = funded();
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let absent = f.proof(&budget);
    let before = absent.verify(&f.expected(), &f.verified_tip()).unwrap();
    let record = register_capacity(&mut f);
    let proof = f.proof(&budget);
    let tip = f.verified_tip();
    let current = proof.verify(&f.expected(), &tip).unwrap();
    assert_eq!(current.capacity(), Some(&record));
    assert_eq!(current.credit(), before.credit());
    assert_eq!(current.current(), before.current());
    assert_eq!(current.pricing(), before.pricing());
    assert!(absent.verify(&f.expected(), &tip).is_err());
    let mut hidden = proof.clone();
    hidden.capacity = None;
    assert!(hidden.verify(&f.expected(), &tip).is_err());
    let mut pricing = current.pricing().clone();
    pricing.notes = Some("Genuine governed pricing original".into());
    let signed = f.chain.sign(
        &f.key,
        [SetPricingSchedule::new(pricing.clone()).into()],
        9_000,
    );
    assert_eq!(f.chain.commit(vec![signed]), vec![true]);
    let updated = f
        .proof(&budget)
        .verify(&f.expected(), &f.verified_tip())
        .unwrap();
    assert_eq!(updated.pricing(), &pricing);
    assert_eq!(updated.capacity(), Some(&record));
    assert_eq!(updated.current(), before.current());
    assert_eq!(updated.credit(), before.credit());
    assert!(proof.verify(&f.expected(), &f.verified_tip()).is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn capacity_and_pricing_original_tail_substitution_refuses_before_encoding() {
    let mut f = funded();
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let absent = f.proof(&budget);
    let capacity = register_capacity(&mut f);
    let proof = f.proof(&budget);
    let tip = f.chain.committed(f.chain.height());
    let mut world = f.chain.state().world.block();
    let check = |snapshot: &WorldStateSnapshotV1, world: &WorldBlock<'_>| {
        reserve_account_originals(
            snapshot,
            world,
            &f.manager,
            f.provider,
            tip.block_time_ms(),
            tip.height(),
            &budget,
        )
        .map(|_| ())
    };
    check(&proof.world, &world).unwrap();
    assert!(check(&absent.world, &world).is_err());
    world.capacity_declarations.remove(f.provider);
    assert!(
        check(&proof.world, &world)
            .unwrap_err()
            .contains("capacity absence")
    );
    let mut changed = capacity.clone();
    changed.registered_epoch += 1;
    world.capacity_declarations.insert(f.provider, changed);
    assert!(
        check(&proof.world, &world)
            .unwrap_err()
            .contains("typed target")
    );
    world.capacity_declarations.insert(f.provider, capacity);
    let pricing: PricingScheduleRecord = norito::decode_canonical(&proof.pricing).unwrap();
    world.sorafs_pricing.get_mut().notes = Some("uncommitted changed original".into());
    assert!(
        check(&proof.world, &world)
            .unwrap_err()
            .contains("typed target")
    );
    *world.sorafs_pricing.get_mut() = pricing;
    check(&proof.world, &world).unwrap();
    drop(world); // Negative specimens never publish or install trusted state.
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn three_original_frames_retain_exact_peak_pool_and_cumulative_codec_charges() {
    let mut f = funded();
    register_capacity(&mut f);
    let proof = f.proof(&AllocationBudget::new(64 * 1024 * 1024));
    let credit: ProviderCreditRecord =
        norito::decode_canonical(proof.credit.as_ref().unwrap()).unwrap();
    let capacity: CapacityDeclarationRecord =
        norito::decode_canonical(proof.capacity.as_ref().unwrap()).unwrap();
    let pricing: PricingScheduleRecord = norito::decode_canonical(&proof.pricing).unwrap();
    let lengths = [
        norito::canonical_frame_len(&credit).unwrap(),
        norito::canonical_frame_len(&capacity).unwrap(),
        norito::canonical_frame_len(&pricing).unwrap(),
    ];
    let total: usize = lengths.iter().sum();
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, total, 128);
    let budget = AllocationBudget::new(total);
    let (frames, usage) = norito::core::with_decode_limits_measured(limits, || {
        let credit =
            encode_original(&credit, MAX_RESERVE_ACCOUNT_CREDIT_BYTES_V1, &budget).unwrap();
        let capacity =
            encode_original(&capacity, MAX_RESERVE_ACCOUNT_CAPACITY_BYTES_V1, &budget).unwrap();
        let pricing =
            encode_original(&pricing, MAX_RESERVE_ACCOUNT_PRICING_BYTES_V1, &budget).unwrap();
        (credit, capacity, pricing)
    });
    assert_eq!(budget.reserved_bytes(), total);
    assert_eq!(usage.total_allocated_bytes(), total);
    assert_eq!(&frames.0.0, proof.credit.as_ref().unwrap());
    assert_eq!(&frames.1.0, proof.capacity.as_ref().unwrap());
    assert_eq!(&frames.2.0, &proof.pricing);
    drop(frames);
    assert_eq!(budget.reserved_bytes(), 0);
    let short = AllocationBudget::new(total - 1);
    let first = encode_original(&credit, MAX_RESERVE_ACCOUNT_CREDIT_BYTES_V1, &short).unwrap();
    let second = encode_original(&capacity, MAX_RESERVE_ACCOUNT_CAPACITY_BYTES_V1, &short).unwrap();
    assert!(encode_original(&pricing, MAX_RESERVE_ACCOUNT_PRICING_BYTES_V1, &short).is_err());
    assert_eq!(short.reserved_bytes(), lengths[0] + lengths[1]);
    drop((first, second));
    assert_eq!(short.reserved_bytes(), 0);
    norito::core::with_decode_limits_scope(limits, || {
        drop(encode_original(&credit, MAX_RESERVE_ACCOUNT_CREDIT_BYTES_V1, &budget).unwrap());
        drop(encode_original(&capacity, MAX_RESERVE_ACCOUNT_CAPACITY_BYTES_V1, &budget).unwrap());
        drop(encode_original(&pricing, MAX_RESERVE_ACCOUNT_PRICING_BYTES_V1, &budget).unwrap());
        assert!(encode_original(&pricing, MAX_RESERVE_ACCOUNT_PRICING_BYTES_V1, &budget).is_err());
    });
    assert_eq!(budget.reserved_bytes(), 0);
    let (result, usage) = norito::core::with_decode_limits_measured(limits, || {
        encode_original(&capacity, lengths[1] - 1, &budget)
    });
    assert!(result.unwrap_err().contains("response bound"));
    assert_eq!(
        usage.total_allocated_bytes(),
        0,
        "bound refusal precedes any allocation debit"
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

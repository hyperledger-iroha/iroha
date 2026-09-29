//! Exact staged snapshot fields and persisted endorsement append-order controls.

use super::*;
use iroha_data_model::nexus::{DOMAIN_ENDORSEMENT_VERSION_V1, DomainEndorsementScope};

#[test]
fn staged_world_retains_every_committed_field_owned_by_its_snapshot() {
    let world = World::default();
    let committed: json::Value = json::from_str(&json::to_json(&world).unwrap()).unwrap();
    let block = world.block();
    let staged: json::Value = json::from_str(&json::to_json(&block).unwrap()).unwrap();
    let committed = committed.as_object().unwrap();
    let staged = staged.as_object().unwrap();
    assert!(committed.contains_key("external_event_buf"));
    assert!(!staged.contains_key("external_event_buf"));
    // This process-owned cell already has an exact State-level projection.
    // Every other committed World field must be carried by the original block.
    let expected = committed
        .keys()
        .filter(|key| key.as_str() != "external_event_buf")
        .collect::<BTreeSet<_>>();
    assert_eq!(staged.keys().collect::<BTreeSet<_>>(), expected);
    assert!(staged.contains_key("domain_endorsements_by_domain"));
}

fn endorsement(
    domain: &DomainId,
    marker: u8,
    height: u64,
) -> (HashOf<DomainEndorsement>, DomainEndorsementRecord) {
    let endorsement = DomainEndorsement {
        version: DOMAIN_ENDORSEMENT_VERSION_V1,
        domain_id: domain.clone(),
        committee_id: "staged-snapshot-order".to_owned(),
        statement_hash: Hash::new([marker]),
        issued_at_height: height,
        expires_at_height: height + 10,
        scope: DomainEndorsementScope::default(),
        signatures: Vec::new(),
        metadata: Metadata::default(),
    };
    (
        endorsement.body_hash(),
        DomainEndorsementRecord {
            endorsement,
            accepted_at_height: height,
        },
    )
}

#[test]
fn staged_state_snapshot_preserves_nonempty_current_and_predecessor_append_order() {
    let (state, _) = fixture();
    let domain = DomainId::try_new("staged-endorsement", "universal").unwrap();
    let mut earlier = [endorsement(&domain, 1, 0), endorsement(&domain, 2, 0)];
    earlier.sort_by_key(|(hash, _)| std::cmp::Reverse(*hash));
    let before = earlier.iter().map(|(hash, _)| *hash).collect::<Vec<_>>();
    assert!(before[0] > before[1], "retained order is not hash order");
    {
        // Component setup creates exact committed primary/index records. It
        // supplies no consensus endorsement or finalized carrier authority.
        let mut world = state.world.block();
        for (hash, record) in earlier {
            world.domain_endorsements.insert(hash, record);
        }
        world
            .domain_endorsements_by_domain
            .insert(domain.clone(), before.clone());
        world.commit();
    }
    let (third_hash, third) = endorsement(&domain, 3, 1);
    let mut after = before.clone();
    after.push(third_hash);
    let mut block = state.block(BlockHeader::new(
        std::num::NonZeroU64::MIN,
        None,
        None,
        1,
        0,
    ));
    block.world.domain_endorsements.insert(third_hash, third);
    block
        .world
        .domain_endorsements_by_domain
        .insert(domain.clone(), after.clone());
    assert_eq!(
        block
            .world
            .domain_endorsements_by_domain
            .revert_map()
            .get(&domain),
        Some(&Some(before.clone()))
    );
    // Finish these existing deterministic block-boundary owners before taking
    // the expected cut; the publication calls are idempotent afterwards.
    block.finalize_axt_asset_incarnations().unwrap();
    block.finalize_axt_policy_transition_ratchets().unwrap();
    let expected_bytes = crate::snapshot::canonical_staged_state_snapshot_bytes(&block);
    let expected_hash = crate::snapshot::canonical_staged_state_snapshot_hash(&block);
    assert_eq!(expected_hash, Hash::new(&expected_bytes));
    let expected_value: json::Value = json::from_slice(&expected_bytes).unwrap();
    let staged_index = json::from_str::<json::Value>(
        &json::to_json(&block.world.domain_endorsements_by_domain).unwrap(),
    )
    .unwrap();
    assert_eq!(
        expected_value
            .get("world")
            .unwrap()
            .get("domain_endorsements_by_domain"),
        Some(&staged_index)
    );
    block.commit_world_overlay_for_testing().unwrap();
    assert_eq!(
        expected_hash,
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap()
    );
    assert_eq!(
        expected_bytes,
        crate::snapshot::canonical_state_snapshot_bytes_for_tests(&state)
    );
    assert_eq!(
        state
            .world
            .domain_endorsements_by_domain
            .view()
            .get(&domain),
        Some(&after)
    );
    let predecessor = state.world.domain_endorsements_by_domain.block_and_revert();
    assert_eq!(predecessor.get(&domain), Some(&before));
}

//! Real G1 map/array witnesses; these storage tests confer no proof or custody authority.

use super::super::tests::{MemoryArchive, fixture};
use super::*;
use KagemushaWalletOperationKindV1 as K;
use KagemushaWalletPolicyUpdateKindV1 as R;

fn field(x: u128) -> [u8; 32] {
    kagemusha_wallet_field_from_u128_v1(x)
}
fn state() -> KagemushaWalletStateV1 {
    KagemushaWalletStateV1::bootstrap(&fixture("KagemushaWalletCredentialV1"), field(7)).unwrap()
}
fn set_root(state: &mut KagemushaWalletStateV1, role: PreparationMapV1, root: [u8; 32]) {
    match role {
        PreparationMapV1::Consumed => state.core.consumed_credit_root = root,
        PreparationMapV1::Pending => state.core.pending_outgoing_root = root,
        PreparationMapV1::Recovery => state.core.load_redeem_recovery_root = root,
        PreparationMapV1::Fee => state.core.fee_claim_root = root,
        PreparationMapV1::BlacklistHistory => state.rest.blacklist_history_root = root,
    }
}

#[test]
fn exact_state_roots_and_canonical_snapshot_survive_restart() {
    let maps = SourceMapsV1::default();
    let state = state();
    maps.require(&state).unwrap();
    let encoded = archive::encode(&maps).unwrap();
    let decoded: SourceMapsV1 = archive::decode(&encoded).unwrap();
    decoded.require(&state).unwrap();
    assert_eq!(archive::encode(&decoded).unwrap(), encoded);
    for role in PreparationMapV1::ALL {
        let mut wrong = state;
        set_root(&mut wrong, role, field(9));
        assert!(decoded.require(&wrong).is_err());
    }
    let mut wrong = state;
    wrong.core.quota_usage_root = field(9);
    assert!(decoded.require(&wrong).is_err());
    let mut wrong = decoded;
    wrong.version = 2;
    assert!(wrong.require(&state).is_err());
}

#[test]
fn every_operation_can_only_mutate_its_fixed_map_roles() {
    for kind in K::ALL {
        let refresh = (kind == K::RefreshPolicy).then_some(R::Blacklist);
        for role in PreparationMapV1::ALL {
            let mut store = MemoryArchive::new();
            let original = SourceMapsV1::default();
            let state = state();
            let mut view =
                PreparationMapsV1::new(&mut store, &original, &state, kind, refresh).unwrap();
            let allowed = matches!(
                (kind, role),
                (K::Receive, PreparationMapV1::Consumed)
                    | (K::Send, PreparationMapV1::Pending | PreparationMapV1::Fee)
                    | (K::Load | K::Unload, PreparationMapV1::Recovery)
                    | (K::RefreshPolicy, PreparationMapV1::BlacklistHistory)
            );
            let insertion = view.insert(role, field(3), field(4));
            assert_eq!(insertion.is_ok(), allowed, "{kind:?} {role:?}");
            if let Ok(insertion) = insertion {
                let mut successor = state;
                set_root(
                    &mut successor,
                    role,
                    insertion
                        .verify(&role.state_root(&state), &field(3), &field(4))
                        .unwrap(),
                );
                let (leaf, opening) = view.membership(role, &field(3)).unwrap();
                assert_eq!(
                    opening.leaf_root(&leaf).unwrap(),
                    role.state_root(&successor)
                );
                assert!(view.non_membership(role, &field(3)).is_err());
                view.non_membership(role, &field(8)).unwrap();
                view.finish(&successor).unwrap();
                original.require(&state).unwrap();
            } else {
                view.finish(&state).unwrap();
            }
        }
    }
}

#[test]
fn abandoned_draft_never_changes_source_and_wrong_successor_cannot_be_published() {
    let mut store = MemoryArchive::new();
    let original = SourceMapsV1::default();
    let state = state();
    let before = archive::encode(&original).unwrap();
    let mut draft =
        PreparationMapsV1::new(&mut store, &original, &state, K::Receive, None).unwrap();
    draft
        .insert(PreparationMapV1::Consumed, field(3), field(4))
        .unwrap();
    assert!(draft.finish(&state).is_err());
    assert_eq!(archive::encode(&original).unwrap(), before);
    let mut restored =
        PreparationMapsV1::new(&mut store, &original, &state, K::Receive, None).unwrap();
    restored
        .non_membership(PreparationMapV1::Consumed, &field(3))
        .unwrap();
    restored
        .insert(PreparationMapV1::Consumed, field(3), field(4))
        .unwrap();
    drop(restored);
    assert_eq!(archive::encode(&original).unwrap(), before);
}

#[test]
fn only_archive_can_clear_pending_and_its_removed_slot_is_not_reused() {
    let mut store = MemoryArchive::new();
    let maps = SourceMapsV1::default();
    let initial = state();
    let mut send = PreparationMapsV1::new(&mut store, &maps, &initial, K::Send, None).unwrap();
    let insertion = send
        .insert(PreparationMapV1::Pending, field(3), field(4))
        .unwrap();
    let mut sent = initial;
    sent.core.pending_outgoing_root = insertion
        .verify(&initial.core.pending_outgoing_root, &field(3), &field(4))
        .unwrap();
    let maps = send.finish(&sent).unwrap();
    for kind in K::ALL {
        let mut view = PreparationMapsV1::new(
            &mut store,
            &maps,
            &sent,
            kind,
            (kind == K::RefreshPolicy).then_some(R::Blacklist),
        )
        .unwrap();
        assert!(view.remove(PreparationMapV1::Recovery, &field(3)).is_err());
        let removal = view.remove(PreparationMapV1::Pending, &field(3));
        assert_eq!(removal.is_ok(), kind == K::ArchiveSent);
        if let Ok(removal) = removal {
            let mut removed = sent;
            removed.core.pending_outgoing_root = removal
                .verify(&sent.core.pending_outgoing_root, &field(3))
                .unwrap();
            let removed_maps = view.finish(&removed).unwrap();
            let mut next =
                PreparationMapsV1::new(&mut store, &removed_maps, &removed, K::Send, None).unwrap();
            assert_eq!(
                next.insert(PreparationMapV1::Pending, field(3), field(4))
                    .unwrap()
                    .slot_opening
                    .slot,
                2
            );
        }
    }
}

#[test]
fn quota_is_exactly_sixty_four_slots_and_only_send_or_share_refresh_can_change_it() {
    let mut slots = [None; 64];
    slots[0] = Some(KagemushaWalletQuotaUsageLeafV1 {
        window_kind: KagemushaWalletQuotaWindowKindV1::Daily,
        window_start_ms: 10,
        window_end_ms: 20,
        used: 11,
    });
    let usage = KagemushaWalletQuotaUsageArrayV1::from_slots(slots).unwrap();
    for kind in K::ALL {
        let mut store = MemoryArchive::new();
        let maps = SourceMapsV1::default();
        let state = state();
        let mut view = PreparationMapsV1::new(
            &mut store,
            &maps,
            &state,
            kind,
            (kind == K::RefreshPolicy).then_some(R::QuotaShare),
        )
        .unwrap();
        assert_eq!(
            view.quota_usage().unwrap(),
            KagemushaWalletQuotaUsageArrayV1::empty()
        );
        let changed = view.set_quota_usage(&usage);
        assert_eq!(changed.is_ok(), matches!(kind, K::Send | K::RefreshPolicy));
        if changed.is_ok() {
            assert_eq!(view.quota_usage().unwrap(), usage);
            let mut successor = state;
            successor.core.quota_usage_root = usage.root();
            let updated = view.finish(&successor).unwrap();
            let restored: SourceMapsV1 =
                archive::decode(&archive::encode(&updated).unwrap()).unwrap();
            restored.require(&successor).unwrap();
        }
    }
    let mut malformed = SourceMapsV1::default();
    malformed.quota_slots[1] = slots[0];
    assert!(malformed.quota_usage().is_err());
}

#[test]
fn malformed_source_or_refresh_selector_is_rejected_before_map_access() {
    let mut store = MemoryArchive::new();
    let maps = SourceMapsV1::default();
    let mut state = state();
    assert!(
        PreparationMapsV1::new(&mut store, &maps, &state, K::Send, Some(R::Blacklist)).is_err()
    );
    assert!(PreparationMapsV1::new(&mut store, &maps, &state, K::RefreshPolicy, None).is_err());
    state.core.consumed_credit_root = field(1);
    assert!(PreparationMapsV1::new(&mut store, &maps, &state, K::Receive, None).is_err());
}

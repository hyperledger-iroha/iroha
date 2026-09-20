//! Restore both identity indexes from matching authoritative account histories.

use super::*;

type UaidIndex = BTreeMap<UniversalAccountId, AccountId>;
type OpaqueIndex = BTreeMap<OpaqueAccountId, UniversalAccountId>;

fn derive<'a>(
    accounts: impl Iterator<Item = (&'a AccountId, &'a AccountValue)>,
) -> Result<(UaidIndex, OpaqueIndex), String> {
    let mut uaids = BTreeMap::new();
    let mut opaque_ids = BTreeMap::new();
    for (account, value) in accounts {
        let details = value.as_ref();
        let Some(uaid) = details.uaid() else {
            if !details.opaque_ids().is_empty() {
                return Err(format!(
                    "Account {account} defines opaque identifiers without a UAID"
                ));
            }
            continue;
        };
        if let Some(existing) = uaids.insert(*uaid, account.clone()) {
            return Err(format!("UAID {uaid} already bound to account {existing}"));
        }
        let mut seen = BTreeSet::new();
        for opaque in details.opaque_ids() {
            if !seen.insert(*opaque) {
                return Err(format!(
                    "Account {account} contains duplicate opaque identifier {opaque}"
                ));
            }
            if let Some(existing) = opaque_ids.insert(*opaque, *uaid) {
                return Err(format!(
                    "Opaque identifier {opaque} already bound to UAID {existing}"
                ));
            }
        }
    }
    Ok((uaids, opaque_ids))
}

/// Validate both images before atomically replacing the pair of derived indexes.
pub(super) fn rebuild(world: &mut World) -> Result<(), String> {
    let ((uaids, opaque_ids), (prior_uaids, prior_opaque), touched_uaids, touched_opaque) = {
        let history = world.accounts.history();
        let current = derive(history.current().iter())?;
        let previous = derive(history.iter_before_block())?;
        let mut touched_uaids = BTreeSet::new();
        let mut touched_opaque = BTreeSet::new();
        for (key, prior) in history.revert_map().iter() {
            for value in [prior.as_ref(), history.current().get(key)]
                .into_iter()
                .flatten()
            {
                touched_uaids.extend(value.as_ref().uaid().copied());
                touched_opaque.extend(value.as_ref().opaque_ids().iter().copied());
            }
        }
        (current, previous, touched_uaids, touched_opaque)
    };
    let uaid_undo = touched_uaids
        .into_iter()
        .map(|key| (key, prior_uaids.get(&key).cloned()))
        .collect();
    let opaque_undo = touched_opaque
        .into_iter()
        .map(|key| (key, prior_opaque.get(&key).copied()))
        .collect();
    world.uaid_accounts = Storage::from_snapshot_parts(uaids, uaid_undo);
    world.opaque_uaids = Storage::from_snapshot_parts(opaque_ids, opaque_undo);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::account::AccountDetails;

    fn details(uaid: UniversalAccountId, opaque: OpaqueAccountId) -> AccountValue {
        AccountValue::new(AccountDetails::new(
            Metadata::default(),
            None,
            Some(uaid),
            vec![opaque],
        ))
    }

    #[test]
    fn identity_reassignment_and_undo_survive_rebuild_and_replacement() {
        let mut world = World::default();
        let first = iroha_test_samples::ALICE_ID.clone();
        let second = iroha_test_samples::BOB_ID.clone();
        let uaid = UniversalAccountId::from_hash(Hash::new(b"identity restored uaid"));
        let opaque = OpaqueAccountId::from_hash(Hash::new(b"identity restored opaque"));
        world.accounts.insert(first.clone(), details(uaid, opaque));
        rebuild(&mut world).unwrap();
        {
            let mut block = world.accounts.block();
            block.remove(first.clone());
            block.insert(second.clone(), details(uaid, opaque));
            block.commit();
        }
        let accounts = json::to_json(&world.accounts).unwrap();
        rebuild(&mut world).unwrap();
        assert_eq!(world.uaid_accounts.view().get(&uaid), Some(&second));
        let uaids = json::to_json(&world.uaid_accounts).unwrap();
        let opaques = json::to_json(&world.opaque_uaids).unwrap();
        // The unchanged opaque mapping still retains its source touch.
        assert_eq!(
            world.opaque_uaids.snapshot().revert_map().get(&opaque),
            Some(&Some(uaid))
        );
        {
            let replacement = world.block_and_revert();
            assert_eq!(replacement.uaid_accounts.get(&uaid), Some(&first));
            assert_eq!(replacement.opaque_uaids.get(&opaque), Some(&uaid));
            assert!(replacement.accounts.get(&second).is_none());
        }
        assert_eq!(json::to_json(&world.accounts).unwrap(), accounts);
        rebuild(&mut world).unwrap();
        assert_eq!(json::to_json(&world.uaid_accounts).unwrap(), uaids);
        assert_eq!(json::to_json(&world.opaque_uaids).unwrap(), opaques);
        world.block_and_revert().commit();
        assert_eq!(world.uaid_accounts.view().get(&uaid), Some(&first));
    }

    #[test]
    fn duplicate_prior_identity_rejects_before_either_index_changes() {
        let mut world = World::default();
        let first = iroha_test_samples::ALICE_ID.clone();
        let second = iroha_test_samples::BOB_ID.clone();
        let uaid = UniversalAccountId::from_hash(Hash::new(b"duplicate prior identity"));
        let opaque = OpaqueAccountId::from_hash(Hash::new(b"duplicate prior opaque"));
        world.accounts.insert(first.clone(), details(uaid, opaque));
        rebuild(&mut world).unwrap();
        world.accounts.insert(second.clone(), details(uaid, opaque));
        {
            let mut block = world.accounts.block();
            block.remove(second);
            block.commit();
        }
        let accounts = json::to_json(&world.accounts).unwrap();
        let uaids = json::to_json(&world.uaid_accounts).unwrap();
        let opaques = json::to_json(&world.opaque_uaids).unwrap();
        assert!(rebuild(&mut world).unwrap_err().contains("UAID"));
        assert_eq!(json::to_json(&world.accounts).unwrap(), accounts);
        assert_eq!(json::to_json(&world.uaid_accounts).unwrap(), uaids);
        assert_eq!(json::to_json(&world.opaque_uaids).unwrap(), opaques);
    }
}

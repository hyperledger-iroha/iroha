//! A signed Request names exactly one payer action. Duplicate Payment delivery names one
//! receiver action. The selected custody root keeps the exact prepared capsule and input.
use super::custody_archive::{Objects, storage_error};
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletLookupV1 as Lookup, KagemushaWalletSlotIdV1 as Slot,
    KagemushaWalletSlotStatusV1 as Status, kagemusha_wallet_archive_object_digest_v1 as digest,
};
use crate::kagemusha_wallet_state_v1::{IndexRoot, ObjectStore};

const MAX_ORIGINAL: usize = 12_182;

fn bad(field: &'static str) -> ProviderError {
    ProviderError::Invalid { field }
}
fn lost(object: &'static str) -> ProviderError {
    ProviderError::UnavailableCustodyData { object }
}

// Fixed length: mode, archived capsule object, exact frozen witness digest, archived action input.
const RECORD: usize = 97;

fn capsule_action_key(capsule: &[u8; 32]) -> [u8; 32] {
    digest(&[b"future-reserved-capsule-action-v1".as_slice(), capsule].concat())
}

fn selected_child_key(predecessor: &[u8; 32]) -> [u8; 32] {
    digest(&[b"future-actual-selected-child-v1".as_slice(), predecessor].concat())
}

fn selected_operation_key(operation: &[u8; 32]) -> [u8; 32] {
    digest(
        &[
            b"future-selected-operation-capsule-v1".as_slice(),
            operation,
        ]
        .concat(),
    )
}

fn remember_selection(
    store: &mut impl ObjectStore,
    index: IndexRoot,
    key: [u8; 32],
    capsule: &[u8; 32],
) -> Result<IndexRoot, ProviderError> {
    if let Some(selected) = index.get(store, &key).map_err(storage_error)? {
        return if selected == *capsule {
            Ok(index)
        } else {
            Err(lost("conflicting selected transfer history"))
        };
    }
    index.set(store, key, capsule).map_err(storage_error)
}

/// Receive's financial operation identity includes the credit id. Two distinct Payment
/// originals can name the same credit, but only the actually selected capsule owns it.
fn check_receive_selection(
    lookup: &Lookup<native::CompletionRecord>,
    candidate: &[u8; 32],
) -> Result<(), ProviderError> {
    let selected = match lookup {
        Lookup::Retained(row) => &row.capsule_digest,
        Lookup::SelectedUnsigned { capsule_digest } => capsule_digest,
        Lookup::Archived(row) => &row.capsule_digest,
        Lookup::Unknown => return Ok(()),
        Lookup::DeliveryDataLoss => return Err(lost("selected Receive completion")),
    };
    if selected != candidate {
        return Err(bad("credit already selected a different Payment"));
    }
    Ok(())
}

/// The protected index survives loss of historical completion files. A mismatching
/// entry, or an indexed operation whose completion is now unknown, blocks replanning.
fn check_archive_selection(
    lookup: &Lookup<native::CompletionRecord>,
    candidate: &[u8; 32],
    indexed: Option<&[u8]>,
) -> Result<(), ProviderError> {
    if indexed.is_some_and(|digest| digest != candidate || matches!(lookup, Lookup::Unknown)) {
        return Err(lost("ArchiveSent operation selection history"));
    }
    match lookup {
        Lookup::Unknown => Ok(()),
        Lookup::Retained(row) if row.capsule_digest == *candidate => Ok(()),
        Lookup::SelectedUnsigned { capsule_digest } if capsule_digest == candidate => Ok(()),
        Lookup::Archived(_) | Lookup::DeliveryDataLoss => Err(lost("ArchiveSent completion")),
        _ => Err(bad("ArchiveSent already selected another capsule")),
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    /// Read the original receiver Request selected for this verified historical Send.
    pub(super) fn retained_send_request(
        &mut self,
        slot: &Slot,
        sent: &native::PreparedOperation,
    ) -> Result<Option<Vec<u8>>, ProviderError> {
        let capsule = sent.capsule();
        if capsule.kind() != 3 {
            return Err(bad("retained Request requires a Send"));
        }
        let action = sent
            .transfer_replay_key()
            .map_err(provider_error)?
            .ok_or_else(|| lost("released Send action identity"))?;
        let (_, checkpoint) = self.custody_checkpoint(slot, *capsule.wallet())?;
        let encoded = zeroize::Zeroizing::new(capsule.encode().map_err(provider_error)?);
        let selected = digest(&encoded);
        self.provider.with_archive(slot, |archive| {
            let mut store = Objects(archive);
            let record = IndexRoot(checkpoint.transfer_actions)
                .get(&mut store, &action)
                .map_err(storage_error)?;
            let Some(record) = record else {
                return Ok(None);
            };
            if record.len() != RECORD || record[1..33] != selected || record[0] > 1 {
                return Err(lost("released Send action record"));
            }
            if record[0] == 0 {
                return Ok(None);
            }
            let address = record[65..97].try_into().expect("fixed action record");
            let original = store
                .read_object(&address, MAX_ORIGINAL)
                .map_err(storage_error)?;
            if native::wallet_send_replay_key(&original).map_err(provider_error)? != action {
                return Err(lost("released Send Request identity"));
            }
            Ok(Some(original))
        })
    }

    fn check_transfer_selection(
        &mut self,
        slot: &Slot,
        index: IndexRoot,
        prepared: &native::PreparedOperation,
        lookup: &Lookup<native::CompletionRecord>,
    ) -> Result<(), ProviderError> {
        match prepared.capsule().kind() {
            4 => check_receive_selection(lookup, prepared.capsule_digest()),
            5 => self.provider.with_archive(slot, |archive| {
                let key = selected_operation_key(prepared.capsule().operation_id());
                let selected = index
                    .get(&mut Objects(archive), &key)
                    .map_err(storage_error)?;
                check_archive_selection(lookup, prepared.capsule_digest(), selected.as_deref())
            }),
            _ => Ok(()),
        }
    }

    pub(super) fn recover_transfer_action(
        &mut self,
        slot: &Slot,
        expected_kind: u8,
        action: &[u8; 32],
        peer: Option<&[u8]>,
        raw: Option<&[u8]>,
    ) -> Result<Option<native::PreparedOperation>, ProviderError> {
        if !(1..=7).contains(&expected_kind) {
            return Err(bad("wallet action kind"));
        }
        let wallet = self.authorized_wallet_id(slot)?;
        let (_, checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let old = self.provider.with_archive(slot, |archive| {
            let mut objects = Objects(archive);
            let Some(record) = IndexRoot(checkpoint.transfer_actions)
                .get(&mut objects, action)
                .map_err(storage_error)?
            else {
                return Ok(None);
            };
            if record.len() != RECORD || record[0] > 1 {
                return Err(lost("transfer action record"));
            }
            if let Some(peer) = peer {
                if record[0] != 1 {
                    return Err(bad("transfer was bound through another input"));
                }
                let address = record[65..97].try_into().expect("fixed action record");
                let original = objects
                    .read_object(&address, MAX_ORIGINAL + 32)
                    .map_err(storage_error)?;
                if original != peer {
                    return Err(bad("transfer action input changed"));
                }
            }
            let address = record[1..33].try_into().expect("fixed action record");
            let capsule = objects
                .read_object(&address, native::MAX_CUSTODY_FRAME_BYTES)
                .map_err(storage_error)?;
            let frozen: [u8; 32] = record[33..65].try_into().expect("fixed action");
            Ok(Some((zeroize::Zeroizing::new(capsule), frozen)))
        })?;
        let Some((bytes, frozen)) = old else {
            return Ok(None);
        };
        let capsule =
            RecoveryCapsule::decode(&bytes, &self.scheme.scheme_id()).map_err(provider_error)?;
        self.require_account_wallet(capsule.wallet())?;
        if capsule.wallet() != &wallet || capsule.kind() != expected_kind {
            return Err(lost("transfer action capsule scope"));
        }
        let recovered = self
            .engine_for_capsule(slot, &capsule)?
            .resume(&capsule, &self.parameters)
            .map_err(provider_error)?;
        self.require_account(slot, Some(recovered.credential()))?;
        let lookup = self.provider.lookup(slot, capsule.operation_id())?;
        self.check_transfer_selection(
            slot,
            IndexRoot(checkpoint.transfer_actions),
            &recovered,
            &lookup,
        )?;
        match lookup {
            Lookup::Retained(released)
                if released.capsule_digest == *recovered.capsule_digest() => {}
            Lookup::SelectedUnsigned { capsule_digest }
                if capsule_digest == *recovered.capsule_digest() => {}
            Lookup::Archived(_) | Lookup::DeliveryDataLoss => {
                return Err(lost("transfer action exact historical completion"));
            }
            Lookup::Retained(_) | Lookup::SelectedUnsigned { .. } => {
                return Err(lost("transfer action selected a different capsule"));
            }
            Lookup::Unknown => {
                let Status::Released(marker) = self.provider.status(slot)? else {
                    return Err(bad("transfer action selection in progress"));
                };
                let Some((sequence, _, current)) = marker.head() else {
                    return Err(lost("transfer action current released head"));
                };
                let predecessor = *capsule.predecessor_capsule_digest();
                let same_source =
                    sequence.checked_add(1) == Some(capsule.sequence()) && current == predecessor;
                if !same_source {
                    // A lookup miss is insufficient. Prove a different child actually won at
                    // the reservation's predecessor, using its protected marker lineage.
                    let selected = match sequence.cmp(&capsule.sequence()) {
                        std::cmp::Ordering::Equal => match &marker.marker().state {
                            KagemushaWalletMarkerStateV1::Head {
                                predecessor_capsule_digest,
                                ..
                            } if *predecessor_capsule_digest == predecessor => Some(current),
                            _ => None,
                        },
                        std::cmp::Ordering::Greater => {
                            self.provider.with_archive(slot, |archive| {
                                IndexRoot(checkpoint.transfer_actions)
                                    .get(&mut Objects(archive), &selected_child_key(&predecessor))
                                    .map_err(storage_error)?
                                    .map(|value| {
                                        value
                                            .try_into()
                                            .map_err(|_| lost("transfer selected child record"))
                                    })
                                    .transpose()
                            })?
                        }
                        std::cmp::Ordering::Less => None,
                    };
                    match selected {
                        Some(child) if child != *recovered.capsule_digest() => return Ok(None),
                        _ => return Err(lost("transfer action selection history")),
                    }
                }
                if self.engine.revalidate_unselected_time(&recovered).is_err()
                    && capsule.kind() == 3
                {
                    // The positively held predecessor proves no one selected this capsule.
                    // The replacement caller must establish a fresh valid observation.
                    return Ok(None);
                }
                local_blacklist::check_accounts(
                    &mut self.provider,
                    slot,
                    *self.parameters.manifest_digest(),
                    wallet,
                    recovered.policy_counterparties(),
                )?;
            }
        }
        if raw.is_some_and(|bytes| digest(bytes) != frozen) {
            return Err(bad(
                "transfer action already binds a different frozen witness",
            ));
        }
        Ok(Some(recovered))
    }

    pub(super) fn retain_transfer_action(
        &mut self,
        slot: &Slot,
        action: [u8; 32],
        prepared: &native::PreparedOperation,
        peer: Option<&[u8]>,
        raw: &[u8],
    ) -> Result<(), ProviderError> {
        let wallet = self.authorized_wallet_id(slot)?;
        let (_, current) = self.custody_checkpoint(slot, wallet)?;
        if matches!(prepared.capsule().kind(), 4 | 5) {
            let lookup = self
                .provider
                .lookup(slot, prepared.capsule().operation_id())?;
            self.check_transfer_selection(
                slot,
                IndexRoot(current.transfer_actions),
                prepared,
                &lookup,
            )?;
        }
        let previous = self.provider.with_archive(slot, |archive| {
            IndexRoot(current.transfer_actions)
                .get(&mut Objects(archive), &action)
                .map_err(storage_error)
        })?;
        if previous.is_some() {
            let known =
                self.recover_transfer_action(slot, prepared.capsule().kind(), &action, peer, None)?;
            if let Some(known) = known {
                if known.capsule_digest() == prepared.capsule_digest() {
                    return Ok(());
                }
                return Err(bad("transfer action is already reserved or selected"));
            }
        }
        let (expect, mut checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let capsule = zeroize::Zeroizing::new(prepared.capsule().encode().map_err(provider_error)?);
        let payload =
            capsule.len() as u64 + peer.map_or(0, |original| original.len() as u64) + RECORD as u64;
        let needed = crate::kagemusha_wallet_state_v1::transfer_storage_required(payload, 3);
        if self.provider.runtime_resources()?.storage_bytes < needed {
            return Err(ProviderError::NoSpace);
        }
        checkpoint.transfer_actions = self.provider.with_archive(slot, |archive| {
            let mut store = Objects(archive);
            let capsule = store
                .write_object(&capsule, native::MAX_CUSTODY_FRAME_BYTES)
                .map_err(storage_error)?;
            let mut record = [0; RECORD];
            record[1..33].copy_from_slice(&capsule);
            record[33..65].copy_from_slice(&digest(raw));
            if peer.is_none()
                && let Some(previous) = &previous
                && previous.first() == Some(&1)
                && previous.len() == RECORD
            {
                record[0] = 1;
                record[65..97].copy_from_slice(&previous[65..97]);
            }
            if let Some(peer) = peer {
                record[0] = 1;
                let original = store
                    .write_object(peer, MAX_ORIGINAL + 32)
                    .map_err(storage_error)?;
                record[65..97].copy_from_slice(&original);
            }
            let index = IndexRoot(checkpoint.transfer_actions);
            if index.get(&mut store, &action).map_err(storage_error)? != previous {
                return Err(bad("transfer action changed during replacement"));
            }
            let index = index
                .set(&mut store, action, &record)
                .map_err(storage_error)?;
            let reverse = capsule_action_key(prepared.capsule_digest());
            if let Some(bound) = index.get(&mut store, &reverse).map_err(storage_error)?
                && bound != action
            {
                return Err(lost("one capsule already bound to different wallet action"));
            }
            Ok(index
                .set(&mut store, reverse, &action)
                .map_err(storage_error)?
                .0)
        })?;
        self.publish_custody_checkpoint(slot, expect, &checkpoint)
    }

    /// Record only the child already named by the durable financial marker before any
    /// subsequent child can be selected. This lets later honest unselected reviews rebind.
    pub(super) fn check_fresh_transfer_advance(
        &mut self,
        slot: &Slot,
        prepared: &native::PreparedOperation,
    ) -> Result<(), ProviderError> {
        let capsule = prepared.capsule();
        let marker = match self.provider.status(slot)? {
            Status::Released(marker) => marker,
            Status::Pending(marker)
                if marker.head().is_some_and(|(seq, _, current)| {
                    seq.checked_add(1) == Some(capsule.sequence())
                        && current == *capsule.predecessor_capsule_digest()
                }) =>
            {
                // The provider can finish this incumbent during Advance. Seal its actual
                // release before allowing the supplied child to replace its head.
                let _window = self.debit_window(slot, None)?;
                let engine = self.current_reconciliation_engine(slot)?;
                let owner = TransitionOwner::new(&engine, &self.parameters);
                let Status::Released(released) = self.provider.reconcile(slot, &owner)? else {
                    return Err(ProviderError::Unavailable(
                        KagemushaWalletUnavailableV1::Busy,
                    ));
                };
                released
            }
            _ => return Ok(()),
        };
        let Some((sequence, operation, current)) = marker.head() else {
            return Err(lost("wallet transfer lineage marker"));
        };
        if sequence.checked_add(1) != Some(capsule.sequence())
            || &current != capsule.predecessor_capsule_digest()
        {
            return Ok(());
        }
        let wallet = self.authorized_wallet_id(slot)?;
        let (expected, mut checkpoint) = self.custody_checkpoint(slot, wallet)?;
        if capsule.kind() == 5 {
            let lookup = self.provider.lookup(slot, capsule.operation_id())?;
            self.check_transfer_selection(
                slot,
                IndexRoot(checkpoint.transfer_actions),
                prepared,
                &lookup,
            )?;
        }
        let known = prepared.transfer_replay_key().map_err(provider_error)?;
        let reverse = capsule_action_key(prepared.capsule_digest());
        let bound: Option<[u8; 32]> = self.provider.with_archive(slot, |archive| {
            IndexRoot(checkpoint.transfer_actions)
                .get(&mut Objects(archive), &reverse)
                .map_err(storage_error)?
                .map(|v| {
                    v.try_into()
                        .map_err(|_| lost("transfer capsule reservation"))
                })
                .transpose()
        })?;
        if known.is_some() && known != bound {
            return Err(bad("unknown fresh transfer reservation"));
        }
        if let Some(action) = bound {
            let actual = self
                .provider
                .with_archive(slot, |archive| {
                    IndexRoot(checkpoint.transfer_actions)
                        .get(&mut Objects(archive), &action)
                        .map_err(storage_error)
                })?
                .ok_or_else(|| bad("missing fresh transfer action reservation"))?;
            let encoded = zeroize::Zeroizing::new(capsule.encode().map_err(provider_error)?);
            let stored = digest(&encoded);
            if actual.len() != RECORD || actual[1..33] != stored {
                return Err(bad("superseded transfer review"));
            }
        }
        let KagemushaWalletMarkerStateV1::Head {
            predecessor_capsule_digest,
            ..
        } = &marker.marker().state
        else {
            return Err(lost("wallet transfer lineage state"));
        };
        if *predecessor_capsule_digest == [0; 32] {
            return Ok(());
        }
        let existing = IndexRoot(checkpoint.transfer_actions);
        let next = self.provider.with_archive(slot, |archive| {
            let mut store = Objects(archive);
            let children = remember_selection(
                &mut store,
                existing,
                selected_child_key(predecessor_capsule_digest),
                &current,
            )?;
            Ok(remember_selection(
                &mut store,
                children,
                selected_operation_key(&operation),
                &current,
            )?
            .0)
        })?;
        if next != checkpoint.transfer_actions {
            checkpoint.transfer_actions = next;
            self.publish_custody_checkpoint(slot, expected, &checkpoint)?;
        }
        Ok(())
    }

    fn transfer_source(
        &mut self,
        slot: &Slot,
    ) -> Result<super::balance::ReleasedPair, ProviderError> {
        let Status::Released(marker) = self.provider.status(slot)? else {
            return Err(bad("transfer requires a released wallet"));
        };
        self.released_operation(slot, &marker)
    }

    pub(super) fn wallet_prepare_send(
        &mut self,
        slot: &Slot,
        peer: &[u8],
    ) -> Result<native::PreparedOperation, ProviderError> {
        self.require_account(slot, None)?;
        let action = native::wallet_send_replay_key(peer).map_err(provider_error)?;
        if let Some(old) = self.recover_transfer_action(slot, 3, &action, Some(peer), None)? {
            return Ok(old);
        }
        let (source, completed) = self.transfer_source(slot)?;
        let head = completed.head();
        let open = self
            .engine
            .authenticate_open_request_peer(peer, &self.parameters)
            .map_err(provider_error)?;
        let receiver = open.request[162..194]
            .try_into()
            .expect("checked Request account");
        let wallet = self.authorized_wallet_id(slot)?;
        local_blacklist::check(
            &mut self.provider,
            slot,
            *self.parameters.manifest_digest(),
            wallet,
            Some(receiver),
        )?;
        let captured = if source
            .successor_requires_send_time()
            .map_err(provider_error)?
        {
            let op = self
                .engine
                .wallet_send_operation_id(&source, head, peer, &self.parameters)
                .map_err(provider_error)?;
            self.time.release(&wallet, &op);
            self.capture_send_time(slot, op)?;
            Some(op)
        } else {
            None
        };
        let result = self
            .with_transfer_maps(slot, |engine, parameters, maps| {
                let witness = engine.wallet_send_witness(&source, head, peer, maps, parameters)?;
                let raw = zeroize::Zeroizing::new(witness.encode()?);
                let prepared = engine.prepare(&witness, head, parameters)?;
                Ok((prepared, raw))
            })
            .and_then(|(prepared, raw)| {
                self.retain_transfer_action(slot, action, &prepared, Some(peer), &raw)?;
                Ok(prepared)
            });
        if result.is_err()
            && let Some(op) = captured
        {
            self.time.release(&wallet, &op);
        }
        result
    }

    pub(super) fn wallet_prepare_receive(
        &mut self,
        slot: &Slot,
        request_id: &[u8; 32],
        peer: &[u8],
    ) -> Result<native::PreparedOperation, ProviderError> {
        self.require_account(slot, None)?;
        let action = native::wallet_receive_replay_key(peer).map_err(provider_error)?;
        let input = [request_id.as_slice(), peer].concat();
        if let Some(old) = self.recover_transfer_action(slot, 4, &action, Some(&input), None)? {
            return Ok(old);
        }
        let wallet = self.authorized_wallet_id(slot)?;
        let (_, checkpoint) = self.custody_checkpoint(slot, wallet)?;
        if checkpoint.blacklist.is_some() {
            let payer = self
                .engine
                .payment_payer_accounts_for_policy(peer, &self.parameters)
                .map_err(provider_error)?;
            local_blacklist::check_accounts(
                &mut self.provider,
                slot,
                *self.parameters.manifest_digest(),
                wallet,
                [Some(payer[0]), (payer[1] != payer[0]).then_some(payer[1])],
            )?;
        }
        let request = self.issued_request_wire(slot, request_id)?;
        let (source, completed) = self.transfer_source(slot)?;
        let head = completed.head();
        let (prepared, raw) = self.with_transfer_maps(slot, |engine, parameters, maps| {
            let witness =
                engine.wallet_receive_witness(&source, head, peer, &request, maps, parameters)?;
            let raw = zeroize::Zeroizing::new(witness.encode()?);
            let prepared = engine.prepare(&witness, head, parameters)?;
            Ok((prepared, raw))
        })?;
        self.retain_transfer_action(slot, action, &prepared, Some(&input), &raw)?;
        Ok(prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_wallet_advance_v1::KagemushaWalletTombstoneV1;

    #[test]
    fn archive_replanning_requires_absence_in_custody_and_the_operation_index() {
        let capsule = [3; 32];
        let other = [4; 32];
        let pending = Lookup::SelectedUnsigned {
            capsule_digest: capsule,
        };
        let tombstone = Lookup::Archived(Box::new(KagemushaWalletTombstoneV1 {
            version: 1,
            operation_id: [7; 32],
            kind: 5,
            selected_generation: 2,
            capsule_digest: capsule,
            completion_digest: [8; 32],
        }));
        assert!(check_archive_selection(&Lookup::Unknown, &capsule, None).is_ok());
        assert!(check_archive_selection(&Lookup::Unknown, &capsule, Some(&capsule)).is_err());
        assert!(check_archive_selection(&pending, &capsule, Some(&capsule)).is_ok());
        assert!(check_archive_selection(&pending, &other, None).is_err());
        assert!(check_archive_selection(&pending, &capsule, Some(&other)).is_err());
        assert!(check_archive_selection(&tombstone, &capsule, Some(&capsule)).is_err());
        assert!(check_archive_selection(&Lookup::DeliveryDataLoss, &capsule, None).is_err());
    }

    #[test]
    fn another_payment_with_same_credit_is_a_permanent_refusal_even_after_archival() {
        let first = [5; 32];
        let other = [9; 32];
        let tombstone = KagemushaWalletTombstoneV1 {
            version: 1,
            operation_id: [7; 32],
            kind: 4,
            selected_generation: 3,
            capsule_digest: first,
            completion_digest: [8; 32],
        };
        for lookup in [
            Lookup::SelectedUnsigned {
                capsule_digest: first,
            },
            Lookup::Archived(Box::new(tombstone)),
        ] {
            assert!(check_receive_selection(&lookup, &first).is_ok());
            assert!(matches!(
                check_receive_selection(&lookup, &other),
                Err(ProviderError::Invalid { .. })
            ));
        }
        assert!(check_receive_selection(&Lookup::Unknown, &other).is_ok());
        assert!(matches!(
            check_receive_selection(&Lookup::DeliveryDataLoss, &other),
            Err(ProviderError::UnavailableCustodyData { .. })
        ));
    }
}

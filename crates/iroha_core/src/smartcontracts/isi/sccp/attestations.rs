//! `SubmitSccpAttestationsV1` and its admission pre-verification (`specs/sccp.md` §4.8).
//! Owner: ws31.
//!
//! Entries are sorted by `(height, signer_index)` and carry only signatures of the submitting
//! bridge key. Cheap checks run before any cryptography; each stored signature recovers its
//! member's address from `statement_digest(height)`.

use super::{
    Error,
    admission::{SccpAdmissionKeysV1, SccpAdmissionRejectV1, SccpExemptClassV1},
    bridge_keys, store,
    subjects::SccpStatementDigests,
};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::{
    account::AccountId,
    isi::sccp::SubmitSccpAttestationsV1,
    sccp::{
        attestation::SccpAttestationSignatureV1,
        events::{SccpAttestationSignedV1, SccpBlockAttestedV1, SccpEvent},
    },
};
use iroha_sccp::v1::signature::verify_signature;

/// Error text of a batch that stores no new signature (§4.8).
pub const ALL_RECORDED: &str = "SccpAttestationsAllRecorded";

/// A validated entry: the member address and generation it signs for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CheckedEntry {
    entry: SccpAttestationSignatureV1,
    address: [u8; 20],
    generation: u64,
    threshold: u8,
}

/// Run the §4.8 checks 1–3 of `instruction` from `authority` against `world` at
/// `current_height`, verifying each signature against `digests`.
fn check(
    world: &(impl WorldReadOnly + ?Sized),
    digests: &(impl SccpStatementDigests + ?Sized),
    current_height: u64,
    instruction: &SubmitSccpAttestationsV1,
    authority: &AccountId,
) -> Result<Vec<CheckedEntry>, String> {
    let params = store::parameters::get(world)
        .as_ref()
        .ok_or_else(|| "SCCP does not exist on this network".to_owned())?;
    // Check 1: bounds and strict ordering.
    let count = instruction.entries.len();
    let max = usize::try_from(params.max_attestation_entries_per_instruction).unwrap_or(usize::MAX);
    if count == 0 || count > max {
        return Err(format!(
            "attestation batch holds {count} entries; 1..={max} allowed"
        ));
    }
    if !instruction.entries_strictly_ascending() {
        return Err(
            "attestation entries are not strictly ascending by (height, signer_index)".into(),
        );
    }
    let authority_address = bridge_keys::bridge_key_address_of(authority)
        .ok_or_else(|| "authority is not a bridge key's account".to_owned())?;
    // Check 2: every entry names an existing earlier subject and this authority's slot.
    let mut checked = Vec::with_capacity(count);
    for entry in &instruction.entries {
        if entry.height >= current_height {
            return Err(format!("height {} is not yet committed", entry.height));
        }
        let subject = store::attestation_subjects::get(world, &entry.height)
            .ok_or_else(|| format!("no attestation subject at height {}", entry.height))?;
        let roster = store::rosters::get(world, &subject.generation)
            .ok_or_else(|| format!("generation {} is unknown", subject.generation))?;
        let member = roster
            .members
            .get(usize::from(entry.signer_index))
            .filter(|member| member.is_nonzero())
            .ok_or_else(|| {
                format!(
                    "signer index {} is not a keyed member of generation {}",
                    entry.signer_index, subject.generation
                )
            })?;
        if member.address != authority_address {
            return Err(format!(
                "entry for height {} is not this authority's slot",
                entry.height
            ));
        }
        checked.push(CheckedEntry {
            entry: *entry,
            address: member.address,
            generation: subject.generation,
            threshold: roster.threshold,
        });
    }
    // Check 3: signatures, in order.
    for checked_entry in &checked {
        let digest = digests
            .statement_digest(checked_entry.entry.height)
            .ok_or_else(|| format!("no statement at height {}", checked_entry.entry.height))?;
        verify_signature(
            &digest,
            &checked_entry.entry.signature,
            &checked_entry.address,
        )
        .map_err(|error| {
            format!(
                "signature for height {} does not verify: {error}",
                checked_entry.entry.height
            )
        })?;
    }
    Ok(checked)
}

/// Execute `SubmitSccpAttestationsV1` (§4.8).
///
/// Stores each new signature, sets its bitmap bit, updates `sccp_member_last_signed` and emits
/// `SccpAttestationSigned`; the first time a subject's bitmap reaches its generation's
/// threshold, sets `attested_at_height` and emits `SccpBlockAttested`.
///
/// # Errors
///
/// Fails on the first invalid entry (checks 1–3), and with [`ALL_RECORDED`] when no entry stored
/// a new signature.
pub fn execute_submit_attestations(
    instruction: SubmitSccpAttestationsV1,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let current_height = state_transaction._curr_block.height().get();
    let checked = check(
        &*state_transaction.world,
        &*state_transaction,
        current_height,
        &instruction,
        authority,
    )
    .map_err(|reason| Error::InvariantViolation(format!("SCCP attestation: {reason}").into()))?;
    let mut stored_any = false;
    for CheckedEntry {
        entry,
        address,
        generation,
        threshold,
    } in checked
    {
        let mut status = store::attestation_status::get(&*state_transaction.world, &entry.height)
            .copied()
            .unwrap_or_default();
        if !status.record_signer(entry.signer_index) {
            continue;
        }
        stored_any = true;
        store::attestation_signatures::insert(
            state_transaction,
            (entry.height, entry.signer_index),
            entry.signature,
        )?;
        let last = store::member_last_signed::get(&*state_transaction.world, &address).copied();
        if last.is_none_or(|last| last < entry.height) {
            store::member_last_signed::insert(state_transaction, address, entry.height)?;
        }
        state_transaction
            .world
            .emit_events(Some(SccpEvent::AttestationSigned(
                SccpAttestationSignedV1 {
                    height: entry.height,
                    generation,
                    signer_index: entry.signer_index,
                    address,
                },
            )));
        if status.attested_at_height.is_none() && status.signer_count() >= u32::from(threshold) {
            status.attested_at_height = Some(current_height);
            state_transaction
                .world
                .emit_events(Some(SccpEvent::BlockAttested(SccpBlockAttestedV1 {
                    height: entry.height,
                    generation,
                    signer_bitmap: status.signer_bitmap,
                })));
        }
        store::attestation_status::insert(state_transaction, entry.height, status)?;
    }
    if stored_any {
        Ok(())
    } else {
        Err(Error::InvariantViolation(ALL_RECORDED.into()))
    }
}

/// Pre-verify one attestation batch against committed state at `next_block_height` and return
/// its admission keys (one pending batch per authority; one content key per entry).
///
/// # Errors
///
/// Rejects a batch that fails §4.8 checks 1–3, or whose every entry is already recorded.
pub fn preverify(
    world: &(impl WorldReadOnly + ?Sized),
    digests: &(impl SccpStatementDigests + ?Sized),
    next_block_height: u64,
    instruction: &SubmitSccpAttestationsV1,
    authority: &AccountId,
) -> Result<SccpAdmissionKeysV1, SccpAdmissionRejectV1> {
    let checked = check(world, digests, next_block_height, instruction, authority)
        .map_err(SccpAdmissionRejectV1::new)?;
    let fresh: Vec<_> = checked
        .iter()
        .filter(|checked| {
            store::attestation_status::get(world, &checked.entry.height)
                .is_none_or(|status| !status.has_signer(checked.entry.signer_index))
        })
        .collect();
    if fresh.is_empty() {
        return Err(SccpAdmissionRejectV1::new(ALL_RECORDED));
    }
    Ok(fresh.into_iter().fold(
        SccpAdmissionKeysV1::new(SccpExemptClassV1::Attestation).with_exclusive(authority),
        |keys, checked| keys.with_content(&(checked.entry.height, checked.entry.signer_index)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{authority, blank_state, header};
    use iroha_data_model::sccp::{
        attestation::SccpAttestationSubjectV1,
        params::SccpParametersV1,
        roster::{SccpBridgeRosterV1, SccpRosterMemberV1},
    };
    use iroha_sccp::v1::key_file::SccpBridgeKeyFileV1;

    struct FixedDigests([u8; 32]);
    impl SccpStatementDigests for FixedDigests {
        fn statement_digest(&self, _height: u64) -> Option<[u8; 32]> {
            Some(self.0)
        }
        fn taira_network_id(&self) -> iroha_data_model::NetworkId {
            iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new([7; 32])),
            )
        }
        fn committed_time_ms(&self) -> u64 {
            0
        }
    }

    struct Fixture {
        keys: Vec<SccpBridgeKeyFileV1>,
    }

    fn key(seed: u8) -> SccpBridgeKeyFileV1 {
        SccpBridgeKeyFileV1::new([seed; 32], 0).expect("valid secret")
    }

    fn account(key: &SccpBridgeKeyFileV1) -> AccountId {
        bridge_keys::account_of(&key.public_key().expect("public key")).expect("account")
    }

    fn setup(stx: &mut StateTransaction<'_, '_>) -> Fixture {
        store::parameters::set(stx, Some(SccpParametersV1::taira_default()));
        let keys: Vec<_> = (1..=4).map(key).collect();
        let mut addresses: Vec<[u8; 20]> = keys
            .iter()
            .map(|key| key.address().expect("address"))
            .collect();
        addresses.sort_unstable();
        let roster = SccpBridgeRosterV1 {
            generation: 1,
            valid_from_ms: 0,
            valid_until_ms: 1_000_000,
            activation_height: 1,
            handoff_height: None,
            members: addresses
                .iter()
                .map(|address| SccpRosterMemberV1 {
                    address: *address,
                    peer: None,
                })
                .collect(),
            threshold: 3,
            digest: [5; 32],
        };
        store::rosters::insert(stx, 1, roster).expect("roster");
        store::attestation_subjects::insert(
            stx,
            3,
            SccpAttestationSubjectV1 {
                height: 3,
                epoch: 0,
                timestamp_ms: 12_000,
                sccp_root: [0; 32],
                message_count: 0,
                history_root: [0; 32],
                history_size: 0,
                generation: 1,
                roster_digest: [5; 32],
                next_roster_digest: [0; 32],
            },
        )
        .expect("subject");
        let mut ordered = keys;
        ordered.sort_by_key(|key| key.address().expect("address"));
        Fixture { keys: ordered }
    }

    fn entry(fixture: &Fixture, index: u8, digest: &[u8; 32]) -> SccpAttestationSignatureV1 {
        SccpAttestationSignatureV1 {
            height: 3,
            signer_index: index,
            signature: fixture.keys[usize::from(index)]
                .sign_digest(digest)
                .expect("signature"),
        }
    }

    #[test]
    fn valid_signatures_reach_the_threshold_once() {
        let state = blank_state();
        let mut block = state.block(header(5));
        let mut stx = block.transaction();
        let fixture = setup(&mut stx);
        let digest = [9; 32];
        let digests = FixedDigests(digest);
        for index in 0..3_u8 {
            let batch = SubmitSccpAttestationsV1 {
                entries: vec![entry(&fixture, index, &digest)],
            };
            let author = account(&fixture.keys[usize::from(index)]);
            check(&*stx.world, &digests, 5, &batch, &author).expect("valid batch");
            let keys = preverify(&*stx.world, &digests, 5, &batch, &author).expect("admissible");
            assert_eq!(keys.class, SccpExemptClassV1::Attestation);
            assert_eq!(keys.content.len(), 1);
        }
        let mut status = store::attestation_status::get(&*stx.world, &3)
            .copied()
            .unwrap_or_default();
        assert!(status.record_signer(0));
        assert!(!status.record_signer(0), "a bit is set once");
    }

    #[test]
    fn foreign_slots_future_heights_and_bad_signatures_are_rejected() {
        let state = blank_state();
        let mut block = state.block(header(5));
        let mut stx = block.transaction();
        let fixture = setup(&mut stx);
        let digest = [9; 32];
        let digests = FixedDigests(digest);
        let batch = SubmitSccpAttestationsV1 {
            entries: vec![entry(&fixture, 0, &digest)],
        };
        let foreign = account(&fixture.keys[1]);
        assert!(
            check(&*stx.world, &digests, 5, &batch, &foreign)
                .unwrap_err()
                .contains("not this authority's slot")
        );
        let owner = account(&fixture.keys[0]);
        assert!(
            check(&*stx.world, &digests, 3, &batch, &owner)
                .unwrap_err()
                .contains("not yet committed")
        );
        let wrong = FixedDigests([8; 32]);
        assert!(
            check(&*stx.world, &wrong, 5, &batch, &owner)
                .unwrap_err()
                .contains("does not verify")
        );
        assert!(
            check(&*stx.world, &digests, 5, &batch, &authority(1))
                .unwrap_err()
                .contains("not a bridge key")
        );
        let empty = SubmitSccpAttestationsV1 { entries: vec![] };
        assert!(check(&*stx.world, &digests, 5, &empty, &owner).is_err());
    }
}

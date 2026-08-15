//! Authority-neutral durability precursor for one direct RKG1 publication pair.
//!
//! `PublicationUnknown` is deliberately not a reservation, evidence that no
//! publication occurred, or permission to retry publication.  After its marker
//! becomes durable, a crash may have happened before H0 staging; during H0
//! writes, sealing, publication, lookup, or readback; after H0 completed but
//! before H1 started; during the corresponding H1 steps; after either publish
//! acknowledgement was lost; after both move-only publication receipts existed
//! but before the journal transition; or after that transition became durable
//! but before its acknowledgement returned.
//!
//! `PublishedUnbound` records the two pointers already retained by an opaque
//! publication owner. Recovery validates their stored encodings only to classify
//! journal bytes: the recovered handle contains no pointer and cannot recreate a
//! publication receipt or read authority. It does not prove object availability
//! or authorize a proof, contribution, aggregate, or later round. The publication
//! identity is retained only as a corroborating session label; it is excluded
//! from transaction identity and carries no authority.
//! The record footer is a canonical corruption check, not authentication or an
//! authority-bearing claim.
//! The complete state machine is `Absent -> PublicationUnknown ->
//! PublishedUnbound`; there is no reverse or further transition here.

// TODO: Wire the durable store consumer to establish or recover PublicationUnknown before H0
// staging, then persist PublishedUnbound before any proof, admission, aggregate, or release gate.

#![expect(dead_code, reason = "durable orphan-journal consumer is not wired yet")]

use super::super::super::super::{
    MKHE_VERSION_V1, ZkAmsMkheErrorV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1, ZkAmsMkheDirectObjectKindV1,
        ZkAmsMkheDirectObjectPointerV1,
    },
};
use super::{
    DirectRkgOnePublicationOwnerV1, DirectRkgOnePublicationScopeV1, RKG_ONE_POLYNOMIAL_BYTES_V1,
    direct_rkg_one_publication_scope_v1, validate_publication_pair_v1,
};
use crate::vega::sponge::Keccak256;

const DIRECT_RKG_ONE_ORPHAN_TRANSACTION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-rkg-one-orphan-transaction";
const DIRECT_RKG_ONE_ORPHAN_RECORD_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-rkg-one-orphan-record";
const DIRECT_RKG_ONE_ORPHAN_RECORD_TAG_V1: [u8; 4] = *b"R1OJ";
const DIRECT_RKG_ONE_RELINEARIZATION_TARGET_TAG_V1: u8 = 0;
const DIRECT_RKG_ONE_PUBLICATION_UNKNOWN_TAG_V1: u8 = 0;
const DIRECT_RKG_ONE_PUBLISHED_UNBOUND_TAG_V1: u8 = 1;

pub(super) const DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1: usize = 334;
const DIRECT_RKG_ONE_ORPHAN_RECORD_PREFIX_BYTES_V1: usize = 302;
const DIRECT_RKG_ONE_ORPHAN_RECORD_BUFFER_CAP_BYTES_V1: usize =
    2 * DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1;
const TRANSACTION_ID_RANGE_V1: core::ops::Range<usize> = 16..48;
const CONTEXT_DIGEST_RANGE_V1: core::ops::Range<usize> = 48..80;
const PARTY_ID_RANGE_V1: core::ops::Range<usize> = 82..114;
const PUBLICATION_IDENTITY_RANGE_V1: core::ops::Range<usize> = 114..146;
const H0_POINTER_RANGE_V1: core::ops::Range<usize> = 146..224;
const H1_POINTER_RANGE_V1: core::ops::Range<usize> = 224..302;
const RECORD_DIGEST_RANGE_V1: core::ops::Range<usize> = 302..334;

const _: () = {
    assert!(ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1 == 78);
    assert!(DIRECT_RKG_ONE_ORPHAN_RECORD_PREFIX_BYTES_V1 + 32 == 334);
    assert!(DIRECT_RKG_ONE_ORPHAN_RECORD_BUFFER_CAP_BYTES_V1 == 668);
};

#[derive(Clone, Copy, PartialEq, Eq)]
struct DirectRkgOneOrphanTransactionIdV1([u8; 32]);

/// Move-only observation that external publication state is unknown.
pub(super) struct DirectRkgOnePublicationUnknownV1 {
    id: DirectRkgOneOrphanTransactionIdV1,
    scope: DirectRkgOnePublicationScopeV1,
}

/// Move-only, unauthoritative observation that both publication pointers were journaled.
pub(super) struct DirectRkgOnePublishedUnboundV1 {
    id: DirectRkgOneOrphanTransactionIdV1,
    scope: DirectRkgOnePublicationScopeV1,
}

/// Exact durable state recovered under freshly validated typed scope.
pub(super) enum DirectRkgOneRecoveredOrphanV1 {
    PublicationUnknown(DirectRkgOnePublicationUnknownV1),
    PublishedUnbound(DirectRkgOnePublishedUnboundV1),
}

/// Atomic durable storage for one exact fixed-width orphan-journal record.
///
/// Every method addresses the canonical, internally derived raw storage key.
/// `load_exact_v1` must bypass caches. `Ok(true)` overwrites all 334 output
/// bytes with the durable value; `Ok(false)` overwrites all 334 bytes with zero.
/// An error makes the output unspecified and the caller discards it.
/// `put_if_absent_exact_v1` never overwrites an existing key.
/// `compare_exchange_exact_v1` replaces only a byte-for-byte expected record.
/// Both mutations are atomic, linearizable, and crash-durable before success.
/// Their acknowledgement can still be lost, so the adapter performs exactly
/// one cache-bypassing load after every returned `Ok` or `Err`.
pub(super) trait DirectRkgOneOrphanJournalStoreV1 {
    fn load_exact_v1(
        &mut self,
        storage_key: &[u8; 32],
        record: &mut [u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1],
    ) -> Result<bool, ZkAmsMkheErrorV1>;

    fn put_if_absent_exact_v1(
        &mut self,
        storage_key: &[u8; 32],
        record: &[u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1],
    ) -> Result<(), ZkAmsMkheErrorV1>;

    fn compare_exchange_exact_v1(
        &mut self,
        storage_key: &[u8; 32],
        expected: &[u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1],
        replacement: &[u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1],
    ) -> Result<(), ZkAmsMkheErrorV1>;
}

/// Durably establish or recover the sole authority-neutral state for typed scope.
pub(super) fn establish_or_recover_direct_rkg_one_publication_unknown_v1<S>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    party_index: usize,
    store: &mut S,
) -> Result<DirectRkgOneRecoveredOrphanV1, ZkAmsMkheErrorV1>
where
    S: DirectRkgOneOrphanJournalStoreV1 + ?Sized,
{
    let scope = direct_rkg_one_publication_scope_v1(roster, context, party_index)?;
    establish_or_recover_scope_v1(scope, store)
}

/// Atomically journal both opaque-owner pointers without changing their authority.
pub(super) fn persist_direct_rkg_one_published_unbound_v1<S>(
    publication_unknown: DirectRkgOnePublicationUnknownV1,
    publication: &DirectRkgOnePublicationOwnerV1,
    store: &mut S,
) -> Result<DirectRkgOnePublishedUnboundV1, ZkAmsMkheErrorV1>
where
    S: DirectRkgOneOrphanJournalStoreV1 + ?Sized,
{
    validate_publication_pair_v1(publication)?;
    if publication_unknown.scope != publication.scope {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let h0 = &publication.h0.publication;
    let h1 = &publication.h1.publication;
    if h0.publication_identity() != h1.publication_identity() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    persist_published_axes_v1(
        publication_unknown,
        DirectRkgOnePublishedAxesV1 {
            publication_identity: h0.publication_identity(),
            h0: h0.pointer(),
            h1: h1.pointer(),
        },
        store,
    )
}

#[derive(Clone, Copy)]
struct DirectRkgOnePublishedAxesV1 {
    publication_identity: [u8; 32],
    h0: ZkAmsMkheDirectObjectPointerV1,
    h1: ZkAmsMkheDirectObjectPointerV1,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectRkgOneDecodedJournalStateV1 {
    PublicationUnknown,
    PublishedUnbound,
}

fn validate_scope_fields_v1(scope: DirectRkgOnePublicationScopeV1) -> Result<(), ZkAmsMkheErrorV1> {
    if scope.context_digest == [0; 32]
        || usize::from(scope.party_index) >= 8
        || usize::from(scope.digit_index) >= 38
        || scope.party.to_bytes() == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn transaction_id_v1(
    scope: DirectRkgOnePublicationScopeV1,
) -> Result<DirectRkgOneOrphanTransactionIdV1, ZkAmsMkheErrorV1> {
    validate_scope_fields_v1(scope)?;
    let mut hash = Keccak256::new();
    hash.update(DIRECT_RKG_ONE_ORPHAN_TRANSACTION_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&scope.context_digest);
    hash.update(&[
        DIRECT_RKG_ONE_RELINEARIZATION_TARGET_TAG_V1,
        scope.digit_index,
        scope.party_index,
    ]);
    hash.update(&scope.party.to_bytes());
    Ok(DirectRkgOneOrphanTransactionIdV1(hash.finalize()))
}

fn validate_published_axes_v1(axes: DirectRkgOnePublishedAxesV1) -> Result<(), ZkAmsMkheErrorV1> {
    if axes.publication_identity == [0; 32]
        || axes.h0.kind() != ZkAmsMkheDirectObjectKindV1::RkgH0
        || axes.h1.kind() != ZkAmsMkheDirectObjectKindV1::RkgH1
        || axes.h0.payload_bytes() != RKG_ONE_POLYNOMIAL_BYTES_V1
        || axes.h1.payload_bytes() != RKG_ONE_POLYNOMIAL_BYTES_V1
        || axes.h0 == axes.h1
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn encode_record_v1(
    scope: DirectRkgOnePublicationScopeV1,
    id: DirectRkgOneOrphanTransactionIdV1,
    published: Option<DirectRkgOnePublishedAxesV1>,
    record: &mut [u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1],
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_scope_fields_v1(scope)?;
    if transaction_id_v1(scope)? != id {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    record.fill(0);
    record[..4].copy_from_slice(&DIRECT_RKG_ONE_ORPHAN_RECORD_TAG_V1);
    record[4] = MKHE_VERSION_V1;
    if let Some(axes) = published {
        validate_published_axes_v1(axes)?;
        record[5] = DIRECT_RKG_ONE_PUBLISHED_UNBOUND_TAG_V1;
        record[8..16].copy_from_slice(&1_u64.to_be_bytes());
        record[PUBLICATION_IDENTITY_RANGE_V1].copy_from_slice(&axes.publication_identity);
        record[H0_POINTER_RANGE_V1].copy_from_slice(&axes.h0.encode());
        record[H1_POINTER_RANGE_V1].copy_from_slice(&axes.h1.encode());
    } else {
        record[5] = DIRECT_RKG_ONE_PUBLICATION_UNKNOWN_TAG_V1;
    }
    record[TRANSACTION_ID_RANGE_V1].copy_from_slice(&id.0);
    record[CONTEXT_DIGEST_RANGE_V1].copy_from_slice(&scope.context_digest);
    record[80] = scope.party_index;
    record[81] = scope.digit_index;
    record[PARTY_ID_RANGE_V1].copy_from_slice(&scope.party.to_bytes());
    let digest = record_digest_v1(record);
    record[RECORD_DIGEST_RANGE_V1].copy_from_slice(&digest);
    Ok(())
}

fn record_digest_v1(record: &[u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_RKG_ONE_ORPHAN_RECORD_DOMAIN_V1);
    hash.update(&record[..DIRECT_RKG_ONE_ORPHAN_RECORD_PREFIX_BYTES_V1]);
    hash.finalize()
}

fn decode_record_v1(
    scope: DirectRkgOnePublicationScopeV1,
    id: DirectRkgOneOrphanTransactionIdV1,
    record: &[u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1],
) -> Result<DirectRkgOneDecodedJournalStateV1, ZkAmsMkheErrorV1> {
    validate_scope_fields_v1(scope)?;
    if transaction_id_v1(scope)? != id
        || record[..4] != DIRECT_RKG_ONE_ORPHAN_RECORD_TAG_V1
        || record[4] != MKHE_VERSION_V1
        || record[6..8] != [0; 2]
        || record[TRANSACTION_ID_RANGE_V1] != id.0
        || record[CONTEXT_DIGEST_RANGE_V1] != scope.context_digest
        || record[80] != scope.party_index
        || record[81] != scope.digit_index
        || record[PARTY_ID_RANGE_V1] != scope.party.to_bytes()
        || record[RECORD_DIGEST_RANGE_V1] != record_digest_v1(record)
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut generation = [0_u8; 8];
    generation.copy_from_slice(&record[8..16]);
    match record[5] {
        DIRECT_RKG_ONE_PUBLICATION_UNKNOWN_TAG_V1 => {
            if u64::from_be_bytes(generation) != 0 || record[114..302] != [0; 188] {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            Ok(DirectRkgOneDecodedJournalStateV1::PublicationUnknown)
        }
        DIRECT_RKG_ONE_PUBLISHED_UNBOUND_TAG_V1 => {
            if u64::from_be_bytes(generation) != 1
                || record[PUBLICATION_IDENTITY_RANGE_V1] == [0; 32]
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            let h0 = ZkAmsMkheDirectObjectPointerV1::decode_exact(
                ZkAmsMkheDirectObjectKindV1::RkgH0,
                &record[H0_POINTER_RANGE_V1],
            )?;
            let h1 = ZkAmsMkheDirectObjectPointerV1::decode_exact(
                ZkAmsMkheDirectObjectKindV1::RkgH1,
                &record[H1_POINTER_RANGE_V1],
            )?;
            validate_published_axes_v1(DirectRkgOnePublishedAxesV1 {
                publication_identity: record[PUBLICATION_IDENTITY_RANGE_V1]
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                h0,
                h1,
            })?;
            Ok(DirectRkgOneDecodedJournalStateV1::PublishedUnbound)
        }
        _ => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
    }
}

fn load_state_v1<S>(
    scope: DirectRkgOnePublicationScopeV1,
    id: DirectRkgOneOrphanTransactionIdV1,
    store: &mut S,
    record: &mut [u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1],
) -> Result<Option<DirectRkgOneDecodedJournalStateV1>, ZkAmsMkheErrorV1>
where
    S: DirectRkgOneOrphanJournalStoreV1 + ?Sized,
{
    record.fill(0xa5);
    if !store.load_exact_v1(&id.0, record)? {
        if *record != [0; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1] {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        return Ok(None);
    }
    decode_record_v1(scope, id, record).map(Some)
}

fn recovered_state_v1(
    scope: DirectRkgOnePublicationScopeV1,
    id: DirectRkgOneOrphanTransactionIdV1,
    state: DirectRkgOneDecodedJournalStateV1,
) -> DirectRkgOneRecoveredOrphanV1 {
    match state {
        DirectRkgOneDecodedJournalStateV1::PublicationUnknown => {
            DirectRkgOneRecoveredOrphanV1::PublicationUnknown(DirectRkgOnePublicationUnknownV1 {
                id,
                scope,
            })
        }
        DirectRkgOneDecodedJournalStateV1::PublishedUnbound => {
            DirectRkgOneRecoveredOrphanV1::PublishedUnbound(DirectRkgOnePublishedUnboundV1 {
                id,
                scope,
            })
        }
    }
}

fn establish_or_recover_scope_v1<S>(
    scope: DirectRkgOnePublicationScopeV1,
    store: &mut S,
) -> Result<DirectRkgOneRecoveredOrphanV1, ZkAmsMkheErrorV1>
where
    S: DirectRkgOneOrphanJournalStoreV1 + ?Sized,
{
    let id = transaction_id_v1(scope)?;
    let mut publication_unknown = [0_u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1];
    encode_record_v1(scope, id, None, &mut publication_unknown)?;
    let mut observed = [0_u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1];
    if let Some(state) = load_state_v1(scope, id, store, &mut observed)? {
        return Ok(recovered_state_v1(scope, id, state));
    }

    let mutation = store.put_if_absent_exact_v1(&id.0, &publication_unknown);
    let loaded = load_state_v1(scope, id, store, &mut observed);
    match loaded {
        Ok(Some(state)) => Ok(recovered_state_v1(scope, id, state)),
        Ok(None) => Err(mutation
            .err()
            .unwrap_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)),
        Err(error) => Err(error),
    }
}

fn persist_published_axes_v1<S>(
    publication_unknown: DirectRkgOnePublicationUnknownV1,
    axes: DirectRkgOnePublishedAxesV1,
    store: &mut S,
) -> Result<DirectRkgOnePublishedUnboundV1, ZkAmsMkheErrorV1>
where
    S: DirectRkgOneOrphanJournalStoreV1 + ?Sized,
{
    let scope = publication_unknown.scope;
    let id = transaction_id_v1(scope)?;
    if publication_unknown.id != id {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut expected = [0_u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1];
    encode_record_v1(scope, id, None, &mut expected)?;
    let mut desired = [0_u8; DIRECT_RKG_ONE_ORPHAN_RECORD_BYTES_V1];
    encode_record_v1(scope, id, Some(axes), &mut desired)?;
    let mutation = store.compare_exchange_exact_v1(&id.0, &expected, &desired);
    let loaded = load_state_v1(scope, id, store, &mut expected);
    match loaded {
        Ok(Some(DirectRkgOneDecodedJournalStateV1::PublishedUnbound)) if expected == desired => {
            Ok(DirectRkgOnePublishedUnboundV1 { id, scope })
        }
        Ok(_) => Err(mutation
            .err()
            .unwrap_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "direct_rkg_one_orphan_journal_v1_tests.rs"]
mod tests;

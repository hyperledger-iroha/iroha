//! Versioned, authority-neutral durability for one direct RKG1 candidate lifecycle.
//!
//! A recovered record is observation only. In particular, recovered or ambiguously inserted
//! `Fresh` bytes never recreate the private permit required before H0 staging. The checksum detects
//! noncanonical/corrupt bytes; it is not authentication, a proof receipt, or release authority.

#![expect(dead_code, reason = "V2 creator corridor remains private and unconnected")]

use super::{
    DirectRkgOnePublicationOwnerV1, DirectRkgOnePublicationScopeV1,
    direct_rkg_one_publication_scope_v1,
};
use crate::vega::zk_ams::mkhe::{
    ZK_AMS_MKHE_DIRECT_RKG_ONE_LEGACY_RECORD_BYTES_V1,
    ZK_AMS_MKHE_DIRECT_RKG_ONE_LIFECYCLE_RECORD_BYTES_V2, ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2,
    ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2, ZkAmsMkheDirectRkgOneLifecycleStoreV2,
    ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2, ZkAmsMkheErrorV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::{DirectRelationPublicObjectsV1, PublishedDirectRkgOneProofOwnerV2},
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
    direct_object_transport::ZkAmsMkheDirectObjectReadAtProviderV1,
};

#[path = "direct_rkg_one_lifecycle_record_v2.rs"]
mod record_v2;
use record_v2::{
    DecodedStateV2, LEGACY_RECORD_BYTES_V1, PublishedAxesV2, RECORD_BYTES_V2, RecordV2,
};

const _: () = {
    assert!(record_v2::LEGACY_RECORD_BYTES_V1 == ZK_AMS_MKHE_DIRECT_RKG_ONE_LEGACY_RECORD_BYTES_V1);
    assert!(record_v2::RECORD_BYTES_V2 == ZK_AMS_MKHE_DIRECT_RKG_ONE_LIFECYCLE_RECORD_BYTES_V2);
};

/// Durable state classification with no publication, proof, verifier, or retry authority.
pub(in super::super) enum DirectRkgOneLifecycleObservationV2 {
    LegacyV1Quarantined,
    FreshQuarantined,
    PublishedUnbound,
    ProofPublishedUnverified,
}

/// Sole live permit accepted before H0 staging; never returned by recovery.
pub(in super::super) struct DirectRkgOneFreshPublishPermitV2 {
    scope: DirectRkgOnePublicationScopeV1,
    storage_key: [u8; 32],
    record: RecordV2,
}

/// Move-only owner of the exact durable H0/H1 publication state.
pub(in super::super) struct DirectRkgOnePublishedUnboundOwnerV2 {
    scope: DirectRkgOnePublicationScopeV1,
    storage_key: [u8; 32],
    record: RecordV2,
    axes: PublishedAxesV2,
}

/// Move-only owner of the exact durable published-proof state.
pub(in super::super) struct DirectRkgOneProofPublishedUnverifiedOwnerV2<'a> {
    proof_owner: PublishedDirectRkgOneProofOwnerV2<'a>,
    publication_owner: DirectRkgOnePublicationOwnerV1,
    _scope: DirectRkgOnePublicationScopeV1,
    _storage_key: [u8; 32],
    _record: RecordV2,
}

struct PostSemanticDirectRkgOneLifecycleOwnerV2<S> {
    _proof_owner: S,
    _publication_owner: DirectRkgOnePublicationOwnerV1,
    _scope: DirectRkgOnePublicationScopeV1,
    _storage_key: [u8; 32],
    _record: RecordV2,
}

impl<'a> DirectRkgOneProofPublishedUnverifiedOwnerV2<'a> {
    pub(in super::super) fn statement_objects_v2(
        &self,
    ) -> Result<DirectRelationPublicObjectsV1, ZkAmsMkheErrorV1> {
        self.publication_owner.statement_objects_v1()
    }

    pub(in super::super) fn verify_semantic_candidate_v2<P>(
        self,
        context: ZkAmsMkheDirectCeremonyContextV1,
        objects: DirectRelationPublicObjectsV1,
        provider: &mut P,
    ) -> Result<impl Sized + use<'a, P>, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        let Self {
            proof_owner,
            publication_owner,
            _scope,
            _storage_key,
            _record,
        } = self;
        let proof_owner = proof_owner.verify_semantic_candidate_v1(context, objects, provider)?;
        Ok(PostSemanticDirectRkgOneLifecycleOwnerV2 {
            _proof_owner: proof_owner,
            _publication_owner: publication_owner,
            _scope,
            _storage_key,
            _record,
        })
    }
}

pub(in super::super) enum DirectRkgOneFreshReservationOutcomeV2 {
    Reserved(DirectRkgOneFreshPublishPermitV2),
    Quarantined(DirectRkgOneLifecycleObservationV2),
}

/// Reserve the stable key. Only an unambiguous insertion by this call can mint the H0 permit.
pub(in super::super) fn reserve_direct_rkg_one_fresh_v2<S>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    party_index: usize,
    store: &mut S,
) -> Result<DirectRkgOneFreshReservationOutcomeV2, ZkAmsMkheErrorV1>
where
    S: ZkAmsMkheDirectRkgOneLifecycleStoreV2 + ?Sized,
{
    let scope = direct_rkg_one_publication_scope_v1(roster, context, party_index)?;
    let storage_key = record_v2::stable_storage_key_v2(scope)?;
    let mut observed = [0; RECORD_BYTES_V2];
    match load_v2(scope, storage_key, store, &mut observed)? {
        LoadedV2::Absent => {}
        state => {
            return Ok(DirectRkgOneFreshReservationOutcomeV2::Quarantined(
                observation_v2(state)?,
            ));
        }
    }
    let mut fresh = [0; RECORD_BYTES_V2];
    record_v2::encode_fresh_v2(scope, storage_key, &mut fresh)?;
    let mutation = store.put_if_absent_exact_v2(&storage_key, &fresh);
    let reloaded = load_v2(scope, storage_key, store, &mut observed);
    match mutation {
        Ok(ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2::InsertedByThisCall) => match reloaded? {
            LoadedV2::Lifecycle(DecodedStateV2::Fresh) if observed == fresh => {
                Ok(DirectRkgOneFreshReservationOutcomeV2::Reserved(
                    DirectRkgOneFreshPublishPermitV2 {
                        scope,
                        storage_key,
                        record: fresh,
                    },
                ))
            }
            _ => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        },
        Ok(ZkAmsMkheDirectRkgOneLifecyclePutOutcomeV2::AlreadyPresent) => Ok(
            DirectRkgOneFreshReservationOutcomeV2::Quarantined(observation_v2(reloaded?)?),
        ),
        Err(error) => {
            let _ = reloaded?;
            Err(error)
        }
    }
}

/// Recover only an authority-neutral classification; no recovered state can resume creation.
pub(in super::super) fn recover_direct_rkg_one_lifecycle_v2<S>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    party_index: usize,
    store: &mut S,
) -> Result<Option<DirectRkgOneLifecycleObservationV2>, ZkAmsMkheErrorV1>
where
    S: ZkAmsMkheDirectRkgOneLifecycleStoreV2 + ?Sized,
{
    let scope = direct_rkg_one_publication_scope_v1(roster, context, party_index)?;
    let storage_key = record_v2::stable_storage_key_v2(scope)?;
    let mut record = [0; RECORD_BYTES_V2];
    match load_v2(scope, storage_key, store, &mut record)? {
        LoadedV2::Absent => Ok(None),
        state => observation_v2(state).map(Some),
    }
}

pub(super) fn validate_fresh_publish_permit_v2(
    permit: &DirectRkgOneFreshPublishPermitV2,
    scope: DirectRkgOnePublicationScopeV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if permit.scope != scope
        || record_v2::stable_storage_key_v2(scope)? != permit.storage_key
        || record_v2::decode_record_v2(scope, permit.storage_key, &permit.record)?
            != DecodedStateV2::Fresh
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

/// Consume the live Fresh permit and persist all receipt-derived H0/H1 axes.
pub(super) fn persist_direct_rkg_one_published_unbound_v2<S>(
    permit: DirectRkgOneFreshPublishPermitV2,
    publication: &DirectRkgOnePublicationOwnerV1,
    store: &mut S,
) -> Result<DirectRkgOnePublishedUnboundOwnerV2, ZkAmsMkheErrorV1>
where
    S: ZkAmsMkheDirectRkgOneLifecycleStoreV2 + ?Sized,
{
    validate_fresh_publish_permit_v2(&permit, publication.scope)?;
    let axes = record_v2::published_axes_v2(publication)?;
    let mut desired = [0; RECORD_BYTES_V2];
    record_v2::encode_published_v2(permit.scope, permit.storage_key, axes, &mut desired)?;
    let mutation = store.compare_exchange_exact_v2(
        &permit.storage_key,
        &permit.record,
        &desired,
    );
    let mut observed = [0; RECORD_BYTES_V2];
    let reloaded = load_v2(permit.scope, permit.storage_key, store, &mut observed);
    match mutation {
        Ok(ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::ExchangedByThisCall) => match reloaded? {
            LoadedV2::Lifecycle(DecodedStateV2::PublishedUnbound(found))
                if observed == desired && found == axes =>
            {
                Ok(DirectRkgOnePublishedUnboundOwnerV2 {
                    scope: permit.scope,
                    storage_key: permit.storage_key,
                    record: desired,
                    axes,
                })
            }
            _ => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        },
        Ok(
            ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::ExactReplay
            | ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::Conflict,
        ) => {
            let _ = reloaded?;
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        }
        Err(error) => {
            let _ = reloaded?;
            Err(error)
        }
    }
}

/// Consume PublishedUnbound and persist the retained exact proof-publication receipt axes.
pub(in super::super) fn persist_direct_rkg_one_proof_published_unverified_v2<'a, S>(
    published: DirectRkgOnePublishedUnboundOwnerV2,
    publication_owner: DirectRkgOnePublicationOwnerV1,
    proof_owner: PublishedDirectRkgOneProofOwnerV2<'a>,
    store: &mut S,
) -> Result<DirectRkgOneProofPublishedUnverifiedOwnerV2<'a>, ZkAmsMkheErrorV1>
where
    S: ZkAmsMkheDirectRkgOneLifecycleStoreV2 + ?Sized,
{
    if record_v2::published_axes_v2(&publication_owner)? != published.axes {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let proof =
        record_v2::proof_axes_v2(published.axes, proof_owner.publication_receipt_v2())?;
    let mut desired = [0; RECORD_BYTES_V2];
    record_v2::encode_proof_v2(
        published.scope,
        published.storage_key,
        published.axes,
        proof,
        &mut desired,
    )?;
    let mutation = store.compare_exchange_exact_v2(
        &published.storage_key,
        &published.record,
        &desired,
    );
    let mut observed = [0; RECORD_BYTES_V2];
    let reloaded = load_v2(
        published.scope,
        published.storage_key,
        store,
        &mut observed,
    );
    match mutation {
        Ok(ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::ExchangedByThisCall) => match reloaded? {
            LoadedV2::Lifecycle(DecodedStateV2::ProofPublishedUnverified(found, proof_found))
                if observed == desired && found == published.axes && proof_found == proof =>
            {
                Ok(DirectRkgOneProofPublishedUnverifiedOwnerV2 {
                    proof_owner,
                    publication_owner,
                    _scope: published.scope,
                    _storage_key: published.storage_key,
                    _record: desired,
                })
            }
            _ => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        },
        Ok(
            ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::ExactReplay
            | ZkAmsMkheDirectRkgOneLifecycleCasOutcomeV2::Conflict,
        ) => {
            let _ = reloaded?;
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        }
        Err(error) => {
            let _ = reloaded?;
            Err(error)
        }
    }
}

enum LoadedV2 {
    Absent,
    Legacy,
    Lifecycle(DecodedStateV2),
}

fn load_v2<S>(
    scope: DirectRkgOnePublicationScopeV1,
    storage_key: [u8; 32],
    store: &mut S,
    record: &mut RecordV2,
) -> Result<LoadedV2, ZkAmsMkheErrorV1>
where
    S: ZkAmsMkheDirectRkgOneLifecycleStoreV2 + ?Sized,
{
    record.fill(0xa5);
    match store.load_exact_v2(&storage_key, record)? {
        ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2::Absent
            if *record == [0; RECORD_BYTES_V2] =>
        {
            Ok(LoadedV2::Absent)
        }
        ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2::Legacy334
            if record[LEGACY_RECORD_BYTES_V1..] == [0; RECORD_BYTES_V2 - LEGACY_RECORD_BYTES_V1] =>
        {
            Ok(LoadedV2::Legacy)
        }
        ZkAmsMkheDirectRkgOneLifecycleStoredWidthV2::Lifecycle640 => {
            record_v2::decode_record_v2(scope, storage_key, record).map(LoadedV2::Lifecycle)
        }
        _ => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
    }
}

fn observation_v2(
    loaded: LoadedV2,
) -> Result<DirectRkgOneLifecycleObservationV2, ZkAmsMkheErrorV1> {
    match loaded {
        LoadedV2::Legacy => Ok(DirectRkgOneLifecycleObservationV2::LegacyV1Quarantined),
        LoadedV2::Lifecycle(DecodedStateV2::Fresh) => {
            Ok(DirectRkgOneLifecycleObservationV2::FreshQuarantined)
        }
        LoadedV2::Lifecycle(DecodedStateV2::PublishedUnbound(_)) => {
            Ok(DirectRkgOneLifecycleObservationV2::PublishedUnbound)
        }
        LoadedV2::Lifecycle(DecodedStateV2::ProofPublishedUnverified(_, _)) => {
            Ok(DirectRkgOneLifecycleObservationV2::ProofPublishedUnverified)
        }
        LoadedV2::Absent => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
    }
}

#[cfg(test)]
#[path = "direct_rkg_one_lifecycle_v2_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "direct_rkg_one_lifecycle_v2_kats.rs"]
mod kats;

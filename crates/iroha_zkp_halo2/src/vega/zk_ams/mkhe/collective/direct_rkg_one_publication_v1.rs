//! Sequential H0/H1 publication owners for one direct RKG1 candidate.

use super::super::super::{
    ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::{
        CompletedDirectRkgOneCreatorV1, DirectPolynomialObjectV1, DirectRelationPublicObjectsV1,
        DirectRkgOneCreatorH0ReadyV1, DirectRkgOneCreatorH1ReadyV1, RkgH0ObjectRoleV1,
        RkgH1ObjectRoleV1,
    },
    direct_collective_eval_ceremony::{
        ZkAmsMkheDirectCeremonyContextV1, ZkAmsMkheDirectEvaluatedKeyTargetV1,
        ZkAmsMkheDirectPolynomialRoleV1, ZkAmsMkheDirectPolynomialStreamReceiptV1,
        ZkAmsMkheDirectPolynomialStreamV1,
    },
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1, ZkAmsMkheDirectObjectCasPublicationV1,
        ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPublicationReceiptV1,
        ZkAmsMkheDirectObjectPublicationTransactionV1,
    },
    manifest::{
        ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1, ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1,
    },
};

#[path = "direct_rkg_one_publication_v1/direct_rkg_one_orphan_journal_v1.rs"]
mod direct_rkg_one_orphan_journal_v1;

const RKG_ONE_POLYNOMIAL_BYTES_V1: u64 = 39_845_888;
const RKG_ONE_LIMBS_V1: usize = 38;
const RESIDUES_PER_WRITE_V1: usize = ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / 8;

const _: () = {
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == 131_072);
    assert!(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 == 8);
    assert!(RKG_ONE_LIMBS_V1 == 38);
    assert!(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 == 8_192);
    assert!(RESIDUES_PER_WRITE_V1 == 1_024);
};

/// Typed H0 owner retaining both statement authentication and CAS/readback.
pub(in crate::vega::zk_ams::mkhe) struct DirectRkgOneH0PublicationV1 {
    stream: ZkAmsMkheDirectPolynomialStreamReceiptV1,
    publication: ZkAmsMkheDirectObjectPublicationReceiptV1,
}

/// Typed H1 owner retaining both statement authentication and CAS/readback.
pub(in crate::vega::zk_ams::mkhe) struct DirectRkgOneH1PublicationV1 {
    stream: ZkAmsMkheDirectPolynomialStreamReceiptV1,
    publication: ZkAmsMkheDirectObjectPublicationReceiptV1,
}

/// Move-only paired publication owner. H1 cannot exist without completed H0.
pub(in crate::vega::zk_ams::mkhe) struct DirectRkgOnePublicationOwnerV1 {
    scope: DirectRkgOnePublicationScopeV1,
    h0: DirectRkgOneH0PublicationV1,
    h1: DirectRkgOneH1PublicationV1,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct DirectRkgOnePublicationScopeV1 {
    context_digest: [u8; 32],
    party_index: u8,
    digit_index: u8,
    party: ZkAmsMkhePartyIdV1,
}

fn direct_rkg_one_publication_scope_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    party_index: usize,
) -> Result<DirectRkgOnePublicationScopeV1, ZkAmsMkheErrorV1> {
    if context.target() != ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization
        || context.evaluated_key_ordinal() != 0
        || context.galois_exponent() != 0
        || usize::from(context.digit_index()) >= RKG_ONE_LIMBS_V1
        || party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || context.digest() == [0; 32]
        || context.profile_digest() != roster.profile_digest()
        || context.roster_digest() != roster.roster_digest()
        || context.key_material_digest() != roster.key_material_digest()
        || context.epoch() != roster.epoch()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let participant = roster
        .participants()
        .get(party_index)
        .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
    Ok(DirectRkgOnePublicationScopeV1 {
        context_digest: context.digest(),
        party_index: u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
        digit_index: context.digit_index(),
        party: participant.party(),
    })
}

impl DirectRkgOnePublicationOwnerV1 {
    pub(in crate::vega::zk_ams::mkhe) const fn h0_stream(
        &self,
    ) -> &ZkAmsMkheDirectPolynomialStreamReceiptV1 {
        &self.h0.stream
    }

    pub(in crate::vega::zk_ams::mkhe) const fn h1_stream(
        &self,
    ) -> &ZkAmsMkheDirectPolynomialStreamReceiptV1 {
        &self.h1.stream
    }

    pub(in crate::vega::zk_ams::mkhe) fn statement_objects_v1(
        &self,
    ) -> Result<DirectRelationPublicObjectsV1, ZkAmsMkheErrorV1> {
        Ok(DirectRelationPublicObjectsV1::RkgRoundOne {
            h0: DirectPolynomialObjectV1::<RkgH0ObjectRoleV1>::new(
                self.h0.stream.polynomial_digest(),
                self.h0.publication.pointer(),
            )?,
            h1: DirectPolynomialObjectV1::<RkgH1ObjectRoleV1>::new(
                self.h1.stream.polynomial_digest(),
                self.h1.publication.pointer(),
            )?,
        })
    }
}

pub(in crate::vega::zk_ams::mkhe) fn publish_direct_rkg_one_h0_h1_v1<'a, P>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    h0_ready: DirectRkgOneCreatorH0ReadyV1<'a>,
    publisher: &mut P,
) -> Result<
    (
        CompletedDirectRkgOneCreatorV1<'a>,
        DirectRkgOnePublicationOwnerV1,
    ),
    ZkAmsMkheErrorV1,
>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    let (context, party_index) = h0_ready.stream_axes_v1();
    let scope = direct_rkg_one_publication_scope_v1(roster, context, party_index)?;
    let mut h0_stream = ZkAmsMkheDirectPolynomialStreamV1::begin_rkg_one_creator_v1(
        roster,
        context,
        party_index,
        ZkAmsMkheDirectPolynomialRoleV1::RkgH0,
    )?;
    let mut h0_publication = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
        ZkAmsMkheDirectObjectKindV1::RkgH0,
        RKG_ONE_POLYNOMIAL_BYTES_V1,
        publisher,
    )?;
    let mut h0_replay = h0_ready.begin_h0_v1()?;
    let mut common_a = zeroed_limb_v1()?;
    for limb in 0..RKG_ONE_LIMBS_V1 {
        let h0 = h0_replay.derive_next_limb_v1(limb, &mut common_a)?;
        h0_stream.admit_limb(limb, &h0)?;
        write_residue_limb_v1(&mut h0_publication, &h0)?;
    }
    let h1_ready = h0_replay.finish_h0_v1()?;
    let h0 = DirectRkgOneH0PublicationV1 {
        stream: h0_stream.finish()?,
        publication: h0_publication.finish()?,
    };
    validate_publication_v1(
        &h0.stream,
        &h0.publication,
        ZkAmsMkheDirectObjectKindV1::RkgH0,
    )?;

    let completed_and_h1 = publish_h1_v1(roster, h1_ready, publisher, &mut common_a)?;
    let owner = DirectRkgOnePublicationOwnerV1 {
        scope,
        h0,
        h1: completed_and_h1.1,
    };
    validate_publication_pair_v1(&owner)?;
    Ok((completed_and_h1.0, owner))
}

fn publish_h1_v1<'a, P>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    h1_ready: DirectRkgOneCreatorH1ReadyV1<'a>,
    publisher: &mut P,
    common_a: &mut [u64],
) -> Result<
    (
        CompletedDirectRkgOneCreatorV1<'a>,
        DirectRkgOneH1PublicationV1,
    ),
    ZkAmsMkheErrorV1,
>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    let (context, party_index) = h1_ready.stream_axes_v1();
    let mut stream = ZkAmsMkheDirectPolynomialStreamV1::begin_rkg_one_creator_v1(
        roster,
        context,
        party_index,
        ZkAmsMkheDirectPolynomialRoleV1::RkgH1,
    )?;
    let mut publication = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
        ZkAmsMkheDirectObjectKindV1::RkgH1,
        RKG_ONE_POLYNOMIAL_BYTES_V1,
        publisher,
    )?;
    let mut replay = h1_ready.begin_h1_v1()?;
    for limb in 0..RKG_ONE_LIMBS_V1 {
        let h1 = replay.derive_next_limb_v1(limb, common_a)?;
        stream.admit_limb(limb, &h1)?;
        write_residue_limb_v1(&mut publication, &h1)?;
    }
    let completed = replay.finish_h1_v1()?;
    let h1 = DirectRkgOneH1PublicationV1 {
        stream: stream.finish()?,
        publication: publication.finish()?,
    };
    validate_publication_v1(
        &h1.stream,
        &h1.publication,
        ZkAmsMkheDirectObjectKindV1::RkgH1,
    )?;
    Ok((completed, h1))
}

fn zeroed_limb_v1() -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
    let mut limb = Vec::new();
    limb.try_reserve_exact(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    limb.resize(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1, 0);
    Ok(limb)
}

fn write_residue_limb_v1<P>(
    transaction: &mut ZkAmsMkheDirectObjectPublicationTransactionV1<'_, P>,
    residues: &[u64],
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    if residues.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut bytes = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    for chunk in residues.chunks_exact(RESIDUES_PER_WRITE_V1) {
        for (encoded, residue) in bytes.chunks_exact_mut(8).zip(chunk) {
            encoded.copy_from_slice(&residue.to_be_bytes());
        }
        transaction.write_exact(&bytes)?;
    }
    Ok(())
}

fn validate_publication_v1(
    stream: &ZkAmsMkheDirectPolynomialStreamReceiptV1,
    publication: &ZkAmsMkheDirectObjectPublicationReceiptV1,
    kind: ZkAmsMkheDirectObjectKindV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let pointer = publication.pointer();
    if stream.polynomial_digest() == [0; 32]
        || stream.canonical_bytes() != RKG_ONE_POLYNOMIAL_BYTES_V1
        || pointer.kind() != kind
        || pointer.payload_bytes() != RKG_ONE_POLYNOMIAL_BYTES_V1
        || publication.post_publish_read_receipt().canonical_bytes() != RKG_ONE_POLYNOMIAL_BYTES_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let profile = release_profile_v1();
    if profile.moduli.len() != RKG_ONE_LIMBS_V1
        || profile.ring_degree != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(())
}

fn validate_publication_pair_v1(
    owner: &DirectRkgOnePublicationOwnerV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let h0 = &owner.h0.publication;
    let h1 = &owner.h1.publication;
    let h0_binding = h0.published_binding();
    let h1_binding = h1.published_binding();
    let h0_snapshot = h0.post_publish_read_receipt().snapshot();
    let h1_snapshot = h1.post_publish_read_receipt().snapshot();
    validate_publication_axes_v1(DirectRkgOnePublicationAxesV1 {
        publication: [h0.publication_identity(), h1.publication_identity()],
        staging: [h0.staging_identity(), h1.staging_identity()],
        seal: [h0.seal_identity(), h1.seal_identity()],
        object: [
            h0_binding.published_object_identity(),
            h1_binding.published_object_identity(),
        ],
        provider: [
            h0_snapshot.provider_identity(),
            h1_snapshot.provider_identity(),
        ],
        snapshot: [
            h0_snapshot.snapshot_identity(),
            h1_snapshot.snapshot_identity(),
        ],
    })
}

#[derive(Clone, Copy)]
struct DirectRkgOnePublicationAxesV1 {
    publication: [[u8; 32]; 2],
    staging: [[u8; 32]; 2],
    seal: [[u8; 32]; 2],
    object: [[u8; 32]; 2],
    provider: [[u8; 32]; 2],
    snapshot: [[u8; 32]; 2],
}

fn validate_publication_axes_v1(
    axes: DirectRkgOnePublicationAxesV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let both_nonzero = |values: &[[u8; 32]; 2]| values.iter().all(|value| *value != [0; 32]);
    if !both_nonzero(&axes.publication)
        || axes.publication[0] != axes.publication[1]
        || !both_nonzero(&axes.staging)
        || axes.staging[0] == axes.staging[1]
        || !both_nonzero(&axes.seal)
        || axes.seal[0] == axes.seal[1]
        || !both_nonzero(&axes.object)
        || axes.object[0] == axes.object[1]
        || !both_nonzero(&axes.provider)
        || axes.provider[0] != axes.provider[1]
        || !both_nonzero(&axes.snapshot)
        || axes.snapshot[0] != axes.snapshot[1]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

#[cfg(test)]
#[path = "direct_rkg_one_publication_v1_tests.rs"]
mod tests;

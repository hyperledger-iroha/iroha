//! Canonical 764-byte RKG1 statement core and exact final trailer.

use super::super::super::super::{
    ZkAmsMkheErrorV1, collective::DirectRkgOnePublicationOwnerV1,
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
};
use super::super::super::{
    FinalizedDirectRkgOneCapabilityV1, PersistentDirectRelationV1,
    PreparedDirectRkgOneCreatorPermitV1,
};
use super::super::{
    DIRECT_RELATION_CODEC_VERSION_V1, DIRECT_RELATION_STATEMENT_MAGIC_V1,
    FINAL_STATEMENT_DOMAIN_V1, OBJECT_ENTRY_BYTES_V1, RELATION_CORE_DOMAIN_V1,
    RKG_ONE_STATEMENT_BYTES_V1, STATEMENT_PREFIX_BYTES_V1,
};
use super::{CanonicalObjectEntryV1, DirectRelationPublicObjectsV1, domain_hash, put};

pub(in crate::vega::zk_ams::mkhe) const PREPARED_RKG_ONE_STATEMENT_CORE_BYTES_V1: usize = 764;

const _: () = {
    assert!(PREPARED_RKG_ONE_STATEMENT_CORE_BYTES_V1 == STATEMENT_PREFIX_BYTES_V1 + 2 * 110);
    assert!(RKG_ONE_STATEMENT_BYTES_V1 == PREPARED_RKG_ONE_STATEMENT_CORE_BYTES_V1 + 64);
};

pub(in crate::vega::zk_ams::mkhe) struct PreparedDirectRkgOneStatementCoreV1 {
    bytes: [u8; PREPARED_RKG_ONE_STATEMENT_CORE_BYTES_V1],
    core_digest: [u8; 32],
}

pub(in crate::vega::zk_ams::mkhe) struct FinalizedDirectRkgOneStatementV1 {
    bytes: [u8; RKG_ONE_STATEMENT_BYTES_V1],
    core_digest: [u8; 32],
    statement_digest: [u8; 32],
}

impl PreparedDirectRkgOneStatementCoreV1 {
    pub(in crate::vega::zk_ams::mkhe) fn new(
        context: ZkAmsMkheDirectCeremonyContextV1,
        permit: &PreparedDirectRkgOneCreatorPermitV1<'_>,
        publications: &DirectRkgOnePublicationOwnerV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if permit.context() != context {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let objects = publications.statement_objects_v1()?;
        let mut bytes = [0_u8; PREPARED_RKG_ONE_STATEMENT_CORE_BYTES_V1];
        let core_end = build_statement_core_v1(
            &mut bytes,
            context,
            PersistentDirectRelationV1::RkgRoundOne,
            context.initial_round_digest(),
            true,
            objects,
            |output| permit.write_statement_authority_v1(output),
        )?;
        if core_end != PREPARED_RKG_ONE_STATEMENT_CORE_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let core_digest = domain_hash(RELATION_CORE_DOMAIN_V1, &bytes);
        if core_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self { bytes, core_digest })
    }

    pub(in crate::vega::zk_ams::mkhe) const fn core_digest(&self) -> [u8; 32] {
        self.core_digest
    }

    pub(in crate::vega::zk_ams::mkhe) fn finalize(
        self,
        finalized: &FinalizedDirectRkgOneCapabilityV1<'_>,
    ) -> Result<FinalizedDirectRkgOneStatementV1, ZkAmsMkheErrorV1> {
        let mut bytes = [0_u8; RKG_ONE_STATEMENT_BYTES_V1];
        bytes[..PREPARED_RKG_ONE_STATEMENT_CORE_BYTES_V1].copy_from_slice(&self.bytes);
        let trailer: &mut [u8; 64] = bytes[PREPARED_RKG_ONE_STATEMENT_CORE_BYTES_V1..]
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        finalized.write_statement_trailer_v1(trailer)?;
        let statement_digest = domain_hash(FINAL_STATEMENT_DOMAIN_V1, &bytes);
        if statement_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(FinalizedDirectRkgOneStatementV1 {
            bytes,
            core_digest: self.core_digest,
            statement_digest,
        })
    }
}

impl FinalizedDirectRkgOneStatementV1 {
    pub(in crate::vega::zk_ams::mkhe) const fn bytes(&self) -> &[u8; RKG_ONE_STATEMENT_BYTES_V1] {
        &self.bytes
    }

    pub(in crate::vega::zk_ams::mkhe) const fn core_digest(&self) -> [u8; 32] {
        self.core_digest
    }

    pub(in crate::vega::zk_ams::mkhe) const fn statement_digest(&self) -> [u8; 32] {
        self.statement_digest
    }
}

pub(super) fn build_statement_core_v1(
    bytes: &mut [u8],
    context: ZkAmsMkheDirectCeremonyContextV1,
    relation: PersistentDirectRelationV1,
    prior_round_digest: [u8; 32],
    has_ephemeral: bool,
    objects: DirectRelationPublicObjectsV1,
    write_authority: impl FnOnce(&mut [u8]) -> Result<(), ZkAmsMkheErrorV1>,
) -> Result<usize, ZkAmsMkheErrorV1> {
    let bytes_len = relation.statement_bytes();
    let (entries, entry_count) = objects.entries();
    let core_end = STATEMENT_PREFIX_BYTES_V1
        .checked_add(
            entry_count
                .checked_mul(OBJECT_ENTRY_BYTES_V1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if prior_round_digest == [0; 32]
        || objects.relation() != relation
        || entry_count != relation.object_count()
        || bytes.len() < core_end
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    put(bytes, 0, &DIRECT_RELATION_STATEMENT_MAGIC_V1);
    bytes[4] = DIRECT_RELATION_CODEC_VERSION_V1;
    bytes[5] = relation as u8;
    bytes[6] = entry_count as u8;
    bytes[7] = u8::from(has_ephemeral);
    put(bytes, 8, &(bytes_len as u32).to_be_bytes());
    put(bytes, 12, &context.profile_digest());
    put(bytes, 44, &context.roster_digest());
    put(bytes, 76, &context.key_material_digest());
    put(bytes, 108, &context.epoch().to_be_bytes());
    put(bytes, 116, &context.transcript_digest());
    put(bytes, 148, &context.digest());
    put(bytes, 180, &prior_round_digest);
    write_authority(bytes)?;
    for (index, entry) in entries.into_iter().take(entry_count).enumerate() {
        let CanonicalObjectEntryV1 {
            statement_digest,
            pointer,
        } = entry.ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let offset = STATEMENT_PREFIX_BYTES_V1 + index * OBJECT_ENTRY_BYTES_V1;
        put(bytes, offset, &statement_digest);
        put(bytes, offset + 32, &pointer.encode());
    }
    Ok(core_end)
}

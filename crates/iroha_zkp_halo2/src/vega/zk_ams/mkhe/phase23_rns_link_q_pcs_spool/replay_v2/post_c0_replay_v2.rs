//! Exact two-pass coefficient replay after the initial qPCS root.

use super::*;

const CQ_POST_ROOT_CONTEXT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.cq-post-root.context\0";

pub(super) fn cq_bound_layout_v2(
    parameter_digest: [u8; 32],
    logical_length: u64,
    columns: u16,
    values_per_block: u16,
) -> Result<StorageLayoutDescriptorV2, QPcsSpoolErrorV2> {
    let digest = mapping_digest_for_layout_v2(
        StorageRoleV2::CqColumnStage,
        0,
        logical_length,
        columns,
        values_per_block,
        parameter_digest,
        None,
    )?;
    checked_layout_v2(
        StorageRoleV2::CqColumnStage,
        0,
        logical_length,
        columns,
        values_per_block,
        digest,
    )
}

pub(super) fn cq_post_root_context_digest_v2(
    descriptor: StorageLayoutDescriptorV2,
    context: PublicSpoolContextV2,
    parameter_digest: [u8; 32],
    initial_root: [u8; 32],
    evaluation_transcript: [u8; 32],
) -> Result<[u8; 32], QPcsSpoolErrorV2> {
    if descriptor.role != StorageRoleV2::CqColumnStage
        || descriptor.layer != 0
        || parameter_digest == [0; 32]
        || initial_root == [0; 32]
        || evaluation_transcript == [0; 32]
    {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    if cq_bound_layout_v2(
        parameter_digest,
        descriptor.logical_length,
        descriptor.columns,
        descriptor.values_per_block,
    )? != descriptor
    {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    context.validate_v2()?;
    let mut hash = Keccak256::new();
    hash.update(CQ_POST_ROOT_CONTEXT_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&descriptor.mapping_digest);
    hash.update(&context.sealed_source_transcript_digest);
    hash.update(&context.source_algebra_binding_digest);
    hash.update(&initial_root);
    hash.update(&evaluation_transcript);
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    Ok(digest)
}

pub(super) struct QPcsC0StoredV2 {
    coefficient: ConfidentialSpoolSnapshotV1,
    lde: ConfidentialSpoolSnapshotV1,
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
    coefficient_context_digest: [u8; 32],
    lde_context_digest: [u8; 32],
    snapshot_binding_digest: [u8; 32],
}

pub(super) struct PostC0CoefficientReplayV2 {
    snapshot: Option<QPcsSpoolSnapshotV2>,
    next_purpose: u16,
    pass: u8,
}

pub(super) struct PostC0ReplayBoundaryV2 {
    snapshot: Option<QPcsSpoolSnapshotV2>,
    completed_passes: u8,
}

pub(super) struct QPcsC0PostReplayCompleteV2 {
    snapshot: QPcsSpoolSnapshotV2,
}

pub(super) struct PostC0CoefficientRowV2 {
    owner: Option<PostC0CoefficientReplayV2>,
    pair: u16,
    component: CoefficientComponentV2,
    next_block: u64,
}

impl QPcsC0CompleteV2 {
    pub(super) fn begin_post_c0_coefficient_replay_v2(
        self,
    ) -> Result<PostC0CoefficientReplayV2, QPcsSpoolErrorV2> {
        if self.snapshot.live.is_none() {
            return Err(QPcsSpoolErrorV2::Poisoned);
        }
        Ok(PostC0CoefficientReplayV2 {
            snapshot: Some(self.snapshot),
            next_purpose: 0,
            pass: 0,
        })
    }
}

impl PostC0ReplayBoundaryV2 {
    pub(super) fn begin_second_replay_v2(
        mut self,
    ) -> Result<PostC0CoefficientReplayV2, QPcsSpoolErrorV2> {
        let snapshot = self.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if self.completed_passes != 1 {
            return Err(QPcsSpoolErrorV2::InvalidStoragePhase);
        }
        Ok(PostC0CoefficientReplayV2 {
            snapshot: Some(snapshot),
            next_purpose: 0,
            pass: 1,
        })
    }

    pub(super) fn finish_v2(mut self) -> Result<QPcsC0PostReplayCompleteV2, QPcsSpoolErrorV2> {
        let snapshot = self.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if self.completed_passes != 2 {
            return Err(QPcsSpoolErrorV2::InvalidStoragePhase);
        }
        Ok(QPcsC0PostReplayCompleteV2 { snapshot })
    }
}

impl QPcsC0PostReplayCompleteV2 {
    pub(super) fn separate_replay_permit_v2(
        self,
    ) -> Result<(QPcsC0StoredV2, AuthenticatedReplayPermitV2), QPcsSpoolErrorV2> {
        let mut snapshot = self.snapshot;
        let live = snapshot.live.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        Ok((
            QPcsC0StoredV2 {
                coefficient: live.coefficient,
                lde: live.lde,
                geometry: snapshot.geometry,
                parameter_digest: snapshot.parameter_digest,
                coefficient_context_digest: snapshot.coefficient_context_digest,
                lde_context_digest: snapshot.lde_context_digest,
                snapshot_binding_digest: snapshot.snapshot_binding_digest,
            },
            live.replay_permit,
        ))
    }
}

impl PostC0CoefficientReplayV2 {
    pub(super) fn geometry_v2(&self) -> Result<SpoolGeometryV2, QPcsSpoolErrorV2> {
        Ok(self
            .snapshot
            .as_ref()
            .ok_or(QPcsSpoolErrorV2::Poisoned)?
            .geometry)
    }

    pub(super) fn begin_next_row_v2(mut self) -> Result<PostC0CoefficientRowV2, QPcsSpoolErrorV2> {
        let snapshot = self.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let purpose_count = u16::from(snapshot.geometry.limb_count_v2()?)
            .checked_mul(u16::from(OPENING_REPETITIONS_V2))
            .and_then(|value| value.checked_mul(u16::from(COEFFICIENT_COMPONENTS_V2)))
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        if self.next_purpose >= purpose_count {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        let pair = self.next_purpose / u16::from(COEFFICIENT_COMPONENTS_V2);
        let component = match self.next_purpose % u16::from(COEFFICIENT_COMPONENTS_V2) {
            0 => CoefficientComponentV2::ProductLow,
            1 => CoefficientComponentV2::ProductHighWithTopZero,
            2 => CoefficientComponentV2::QuotientWithTopZero,
            _ => return Err(QPcsSpoolErrorV2::InvalidGeometry),
        };
        self.snapshot = Some(snapshot);
        Ok(PostC0CoefficientRowV2 {
            owner: Some(self),
            pair,
            component,
            next_block: 0,
        })
    }

    pub(super) fn complete_v2(mut self) -> Result<PostC0ReplayBoundaryV2, QPcsSpoolErrorV2> {
        let snapshot = self.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let expected = u16::from(snapshot.geometry.limb_count_v2()?)
            .checked_mul(u16::from(OPENING_REPETITIONS_V2))
            .and_then(|value| value.checked_mul(u16::from(COEFFICIENT_COMPONENTS_V2)))
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        if self.next_purpose != expected {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        let completed_passes = self
            .pass
            .checked_add(1)
            .ok_or(QPcsSpoolErrorV2::InvalidStoragePhase)?;
        if completed_passes > 2 {
            return Err(QPcsSpoolErrorV2::InvalidStoragePhase);
        }
        Ok(PostC0ReplayBoundaryV2 {
            snapshot: Some(snapshot),
            completed_passes,
        })
    }
}

impl PostC0CoefficientRowV2 {
    pub(super) fn geometry_v2(&self) -> Result<SpoolGeometryV2, QPcsSpoolErrorV2> {
        self.owner
            .as_ref()
            .ok_or(QPcsSpoolErrorV2::Poisoned)?
            .geometry_v2()
    }

    pub(super) fn read_next_block_v2(
        &mut self,
    ) -> Result<AuthenticatedReplayChunkV2, QPcsSpoolErrorV2> {
        let mut owner = self.owner.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let mut snapshot = owner.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let mut live = snapshot.live.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let blocks = snapshot.geometry.coefficient_blocks_per_component_v2()?;
        if self.next_block >= blocks {
            return Err(QPcsSpoolErrorV2::ExtraCoefficientBlock);
        }
        let component = match self.component {
            CoefficientComponentV2::ProductLow => 0,
            CoefficientComponentV2::ProductHighWithTopZero => 1,
            CoefficientComponentV2::QuotientWithTopZero => 2,
        };
        let slot = (u64::from(self.pair) * blocks + self.next_block)
            * u64::from(COEFFICIENT_COMPONENTS_V2)
            + component;
        let coordinate = coefficient_coordinate_v2(snapshot.geometry, slot)?;
        let chunk = live
            .coefficient
            .read_slot_v1(slot, snapshot.coefficient_context_digest)?;
        validate_coefficient_chunk_v2(snapshot.geometry, coordinate, &chunk)?;
        self.next_block += 1;
        snapshot.live = Some(live);
        owner.snapshot = Some(snapshot);
        self.owner = Some(owner);
        Ok(AuthenticatedReplayChunkV2 { chunk })
    }

    pub(super) fn complete_v2(mut self) -> Result<PostC0CoefficientReplayV2, QPcsSpoolErrorV2> {
        let mut owner = self.owner.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let snapshot = owner.snapshot.as_ref().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if self.next_block != snapshot.geometry.coefficient_blocks_per_component_v2()? {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        owner.next_purpose = owner
            .next_purpose
            .checked_add(1)
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        Ok(owner)
    }

    #[cfg(test)]
    fn panic_after_take_for_test_v2(&mut self) {
        let _owner = self.owner.take().expect("live post-C0 replay row");
        panic!("intentional post-C0 replay unwind test");
    }
}

pub(super) fn bind_cq_post_root_replay_v2(
    snapshot: ConfidentialSpoolSnapshotV1,
    descriptor: StorageLayoutDescriptorV2,
    context: PublicSpoolContextV2,
    parameter_digest: [u8; 32],
    initial_root: [u8; 32],
    evaluation_transcript: [u8; 32],
    replay_permit: AuthenticatedReplayPermitV2,
) -> Result<QPcsDerivedReplayV2, QPcsSpoolErrorV2> {
    let context_digest = cq_post_root_context_digest_v2(
        descriptor,
        context,
        parameter_digest,
        initial_root,
        evaluation_transcript,
    )?;
    if snapshot.slot_count_v1() != descriptor.slot_count
        || snapshot.plaintext_len_v1() != descriptor.plaintext_bytes
        || snapshot.file_len_v1() != descriptor.file_bytes
    {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    Ok(QPcsDerivedReplayV2 {
        snapshot: Some(snapshot),
        descriptor,
        context_digest,
        next_unit: 0,
        replay_permit,
    })
}

#[cfg(test)]
#[path = "post_c0_replay_v2_tests.rs"]
mod tests;

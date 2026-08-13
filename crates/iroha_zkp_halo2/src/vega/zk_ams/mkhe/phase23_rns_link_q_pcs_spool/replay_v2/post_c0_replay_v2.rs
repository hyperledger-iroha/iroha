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

pub(super) struct C0BatchReplayV2 {
    stored: Option<QPcsC0StoredV2>,
    next_slot: u64,
}

pub(super) struct CqBatchReplayV2 {
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    replay_permit: Option<AuthenticatedReplayPermitV2>,
    pre_quotient_transcript: [u8; 32],
    next_slot: u64,
}

pub(super) struct QPcsCqStoredV2 {
    snapshot: ConfidentialSpoolSnapshotV1,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    snapshot_digest: [u8; 32],
    pre_quotient_transcript: [u8; 32],
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

impl QPcsC0StoredV2 {
    pub(super) fn begin_c0_batch_replay_v2(
        self,
        context: PublicSpoolContextV2,
    ) -> Result<C0BatchReplayV2, QPcsSpoolErrorV2> {
        self.geometry.validate_v2()?;
        if parameter_digest_v2(self.geometry)? != self.parameter_digest {
            return Err(QPcsSpoolErrorV2::InvalidGeometry);
        }
        let coefficient_context = context_digest_v2(
            SpoolRoleV2::Coefficients,
            self.parameter_digest,
            mapping_digest_v2(self.geometry, self.parameter_digest, true)?,
            context,
        )?;
        let lde_context = context_digest_v2(
            SpoolRoleV2::Lde,
            self.parameter_digest,
            mapping_digest_v2(self.geometry, self.parameter_digest, false)?,
            context,
        )?;
        let coefficient_file = self
            .geometry
            .coefficient_slot_count_v2()?
            .checked_mul(
                self.geometry
                    .coefficient_block_bytes_v2()?
                    .checked_add(AUTHENTICATION_TAG_BYTES_V2)
                    .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?,
            )
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        let lde_file = self
            .geometry
            .lde_slot_count_v2()?
            .checked_mul(
                self.geometry
                    .lde_block_bytes_v2()?
                    .checked_add(AUTHENTICATION_TAG_BYTES_V2)
                    .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?,
            )
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        if coefficient_context != self.coefficient_context_digest
            || lde_context != self.lde_context_digest
            || self.coefficient.slot_count_v1() != self.geometry.coefficient_slot_count_v2()?
            || self.coefficient.plaintext_len_v1() != self.geometry.coefficient_block_bytes_v2()?
            || self.coefficient.file_len_v1() != coefficient_file
            || self.lde.slot_count_v1() != self.geometry.lde_slot_count_v2()?
            || self.lde.plaintext_len_v1() != self.geometry.lde_block_bytes_v2()?
            || self.lde.file_len_v1() != lde_file
            || snapshot_binding_digest_v2(
                self.parameter_digest,
                coefficient_context,
                lde_context,
                *self.coefficient.snapshot_digest_v1(),
                *self.lde.snapshot_digest_v1(),
            )? != self.snapshot_binding_digest
        {
            return Err(QPcsSpoolErrorV2::InvalidGeometry);
        }
        Ok(C0BatchReplayV2 {
            stored: Some(self),
            next_slot: 0,
        })
    }
}

impl C0BatchReplayV2 {
    pub(super) fn read_next_v2(
        &mut self,
        block: u64,
        column: u16,
    ) -> Result<AuthenticatedReplayChunkV2, QPcsSpoolErrorV2> {
        let mut stored = self.stored.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let columns = stored.geometry.lde_column_count_v2()?;
        let blocks = stored.geometry.lde_blocks_per_column_v2()?;
        if block >= blocks
            || u64::from(column) >= columns
            || self.next_slot != block * columns + u64::from(column)
        {
            return Err(QPcsSpoolErrorV2::InvalidStoragePhase);
        }
        let chunk = stored
            .lde
            .read_slot_v1(self.next_slot, stored.lde_context_digest)?;
        validate_lde_chunk_v2(
            stored.geometry,
            lde_coordinate_v2(stored.geometry, self.next_slot)?,
            &chunk,
        )?;
        self.next_slot += 1;
        self.stored = Some(stored);
        Ok(AuthenticatedReplayChunkV2 { chunk })
    }

    pub(super) fn complete_v2(mut self) -> Result<QPcsC0StoredV2, QPcsSpoolErrorV2> {
        let stored = self.stored.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if self.next_slot != stored.geometry.lde_slot_count_v2()? {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        Ok(stored)
    }
}

fn validate_exhausted_cq_batch_boundary_v2(
    descriptor: StorageLayoutDescriptorV2,
    next_unit: u64,
    parameter_digest: [u8; 32],
) -> Result<(), QPcsSpoolErrorV2> {
    if parameter_digest_v2(SpoolGeometryV2::release_v2())? != parameter_digest {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    let exact = cq_bound_layout_v2(
        parameter_digest,
        REPLAY_DOMAIN_VALUES_V2,
        REPLAY_COLUMNS_V2,
        REPLAY_BLOCK_VALUES_V2,
    )?;
    if descriptor != exact
        || descriptor.blocks_per_column != REPLAY_BLOCKS_PER_COLUMN_V2
        || next_unit != descriptor.blocks_per_column
    {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    Ok(())
}

impl QPcsDerivedReplayV2 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn begin_cq_batch_replay_v2(
        mut self,
        context: PublicSpoolContextV2,
        parameter_digest: [u8; 32],
        initial_root: [u8; 32],
        pre_quotient_transcript: [u8; 32],
    ) -> Result<CqBatchReplayV2, QPcsSpoolErrorV2> {
        let snapshot = self.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        validate_exhausted_cq_batch_boundary_v2(self.descriptor, self.next_unit, parameter_digest)?;
        if cq_post_root_context_digest_v2(
            self.descriptor,
            context,
            parameter_digest,
            initial_root,
            pre_quotient_transcript,
        )? != self.context_digest
            || snapshot.slot_count_v1() != self.descriptor.slot_count
            || snapshot.plaintext_len_v1() != self.descriptor.plaintext_bytes
            || snapshot.file_len_v1() != self.descriptor.file_bytes
        {
            return Err(QPcsSpoolErrorV2::InvalidGeometry);
        }
        Ok(CqBatchReplayV2 {
            snapshot: Some(snapshot),
            descriptor: self.descriptor,
            context_digest: self.context_digest,
            replay_permit: Some(self.replay_permit),
            pre_quotient_transcript,
            next_slot: 0,
        })
    }
}

impl CqBatchReplayV2 {
    pub(super) fn read_next_v2(
        &mut self,
        block: u64,
        column: u16,
    ) -> Result<AuthenticatedReplayChunkV2, QPcsSpoolErrorV2> {
        let mut snapshot = self.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let flat = block
            .checked_mul(u64::from(self.descriptor.columns))
            .and_then(|value| value.checked_add(u64::from(column)))
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        if block >= self.descriptor.blocks_per_column
            || column >= self.descriptor.columns
            || flat != self.next_slot
        {
            return Err(QPcsSpoolErrorV2::InvalidStoragePhase);
        }
        let slot = u64::from(column) * self.descriptor.blocks_per_column + block;
        let chunk = snapshot.read_slot_v1(slot, self.context_digest)?;
        self.next_slot += 1;
        self.snapshot = Some(snapshot);
        Ok(AuthenticatedReplayChunkV2 { chunk })
    }

    pub(super) fn complete_v2(
        mut self,
    ) -> Result<(QPcsCqStoredV2, AuthenticatedReplayPermitV2), QPcsSpoolErrorV2> {
        let snapshot = self.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let permit = self
            .replay_permit
            .take()
            .ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if self.next_slot != self.descriptor.slot_count {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        let snapshot_digest = *snapshot.snapshot_digest_v1();
        Ok((
            QPcsCqStoredV2 {
                snapshot,
                descriptor: self.descriptor,
                context_digest: self.context_digest,
                snapshot_digest,
                pre_quotient_transcript: self.pre_quotient_transcript,
            },
            permit,
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

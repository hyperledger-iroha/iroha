//! Purpose-bound authenticated column staging for the initial C0 pass.
use super::*;
use crate::vega::sponge::Keccak256;
use iroha_crypto::confidential_spool::{
    CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1, CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1,
    CONFIDENTIAL_SPOOL_MAX_SLOTS_V1, ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1,
    ConfidentialSpoolSnapshotV1, ConfidentialSpoolWriterV1,
};
use std::path::Path;
const INITIAL_C0_COLUMN_MAPPING_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.initial-c0-column-stage.mapping\0";
const INITIAL_C0_COLUMN_CONTEXT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.initial-c0-column-stage.context\0";
const INITIAL_C0_COLUMN_FORMULA_V2: &[u8] = b"column=limb*10+repetition*2+role;slot=column*blocks_per_column+block;first_index=block*values_per_block;role=p:0,h:1";
const INITIAL_C0_ENCODING_V2: &[u8] = b"canonical Fq2=(c0,c1), each canonical big-endian u64";
const RELEASE_INITIAL_C0_COLUMN_FILE_BYTES_V2: u64 = 3_190_784_000;
const _: () = {
    assert!(RELEASE_INITIAL_C0_COLUMN_FILE_BYTES_V2 == RELEASE_LDE_FILE_BYTES_V2);
};
#[derive(Clone, Copy)]
pub(super) struct InitialColumnDescriptorV2 {
    pub(super) domain_size: u64,
    pub(super) columns: u16,
    pub(super) values_per_block: u16,
    pub(super) blocks_per_column: u64,
    pub(super) slot_count: u64,
    pub(super) plaintext_bytes: u64,
    pub(super) file_bytes: u64,
    pub(super) mapping_digest: [u8; 32],
}
fn initial_column_descriptor_v2(
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
) -> Result<InitialColumnDescriptorV2, ProverPrerequisiteErrorV2> {
    geometry.validate_v2()?;
    if parameter_digest_v2(geometry)? != parameter_digest {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
    }
    let domain_size = geometry.domain_size_v2()?;
    let columns = u16::try_from(geometry.lde_column_count_v2()?)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let values_per_block = geometry.lde_values_per_block;
    let blocks_per_column = geometry.lde_blocks_per_column_v2()?;
    let slot_count = u64::from(columns)
        .checked_mul(blocks_per_column)
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let plaintext_bytes = u64::from(values_per_block)
        .checked_mul(FQ2_WIRE_BYTES_V2)
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let file_bytes = slot_count
        .checked_mul(
            plaintext_bytes
                .checked_add(AUTHENTICATION_TAG_BYTES_V2)
                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    if slot_count > CONFIDENTIAL_SPOOL_MAX_SLOTS_V1
        || plaintext_bytes > CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1
        || file_bytes > CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1
    {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    let release = SpoolGeometryV2::release_v2();
    if geometry.ring_degree == release.ring_degree
        && geometry.domain_log == release.domain_log
        && geometry.query_count == release.query_count
        && geometry.lde_values_per_block == release.lde_values_per_block
        && geometry.moduli == release.moduli
        && file_bytes != RELEASE_INITIAL_C0_COLUMN_FILE_BYTES_V2
    {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    let mut hash = Keccak256::new();
    hash.update(INITIAL_C0_COLUMN_MAPPING_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&geometry.ring_degree.to_be_bytes());
    hash.update(&[geometry.domain_log, geometry.limb_count_v2()?]);
    hash.update(&geometry.query_count.to_be_bytes());
    hash.update(&domain_size.to_be_bytes());
    hash.update(&columns.to_be_bytes());
    hash.update(&values_per_block.to_be_bytes());
    hash.update(&blocks_per_column.to_be_bytes());
    hash.update(&slot_count.to_be_bytes());
    hash.update(&plaintext_bytes.to_be_bytes());
    hash.update(&file_bytes.to_be_bytes());
    hash.update(INITIAL_C0_COLUMN_FORMULA_V2);
    hash.update(INITIAL_C0_ENCODING_V2);
    hash.update(&slot_count.to_be_bytes());
    for slot in 0..slot_count {
        let column = slot / blocks_per_column;
        let block = slot % blocks_per_column;
        let row = column % u64::from(FIXED_ROW_COUNT_V2);
        hash.update(&slot.to_be_bytes());
        hash.update(
            &u16::try_from(column)
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?
                .to_be_bytes(),
        );
        hash.update(&[
            u8::try_from(column / u64::from(FIXED_ROW_COUNT_V2))
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
            u8::try_from(row / u64::from(ROWS_PER_REPETITION_V2))
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
            u8::from(!row.is_multiple_of(u64::from(ROWS_PER_REPETITION_V2))),
        ]);
        hash.update(&block.to_be_bytes());
        hash.update(&(block * u64::from(values_per_block)).to_be_bytes());
    }
    let mapping_digest = hash.finalize();
    if mapping_digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    Ok(InitialColumnDescriptorV2 {
        domain_size,
        columns,
        values_per_block,
        blocks_per_column,
        slot_count,
        plaintext_bytes,
        file_bytes,
        mapping_digest,
    })
}
fn initial_column_context_v2(
    descriptor: InitialColumnDescriptorV2,
    parameter_digest: [u8; 32],
    context: PublicSpoolContextV2,
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    context.validate_v2()?;
    let mut hash = Keccak256::new();
    hash.update(INITIAL_C0_COLUMN_CONTEXT_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&descriptor.mapping_digest);
    hash.update(&context.sealed_source_transcript_digest);
    hash.update(&context.source_algebra_binding_digest);
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
    }
    Ok(digest)
}
struct LiveInitialColumnWriterV2 {
    writer: ConfidentialSpoolWriterV1,
    next_slot: u64,
}
pub(super) struct InitialColumnWriterV2 {
    live: Option<LiveInitialColumnWriterV2>,
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
    pub(super) descriptor: InitialColumnDescriptorV2,
    pub(super) context_digest: [u8; 32],
}
impl InitialColumnWriterV2 {
    pub(super) fn create_v2(
        directory: &Path,
        geometry: SpoolGeometryV2,
        parameter_digest: [u8; 32],
        context: PublicSpoolContextV2,
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        let descriptor = initial_column_descriptor_v2(geometry, parameter_digest)?;
        let context_digest = initial_column_context_v2(descriptor, parameter_digest, context)?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(
            descriptor.slot_count,
            descriptor.plaintext_bytes,
            context_digest,
        )?;
        if layout.file_len_v1() != descriptor.file_bytes {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
        }
        let writer = ConfidentialSpoolWriterV1::create_in_v1(directory, layout)?;
        Ok(Self {
            live: Some(LiveInitialColumnWriterV2 {
                writer,
                next_slot: 0,
            }),
            geometry,
            parameter_digest,
            descriptor,
            context_digest,
        })
    }
    pub(super) fn expect_next_column_v2(
        &self,
        expected: u16,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        let next = self
            .live
            .as_ref()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?
            .next_slot
            / self.descriptor.blocks_per_column;
        if next != u64::from(expected) {
            return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
        }
        Ok(())
    }
    pub(super) fn push_next_block_v2(
        &mut self,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if live.next_slot >= self.descriptor.slot_count {
            return Err(ProverPrerequisiteErrorV2::Spool(
                QPcsSpoolErrorV2::ExtraLdeBlock,
            ));
        }
        let column = live.next_slot / self.descriptor.blocks_per_column;
        let block = live.next_slot % self.descriptor.blocks_per_column;
        let row = column % u64::from(FIXED_ROW_COUNT_V2);
        let coordinate = LdeCoordinateV2 {
            limb: u8::try_from(column / u64::from(FIXED_ROW_COUNT_V2))
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
            repetition: u8::try_from(row / u64::from(ROWS_PER_REPETITION_V2))
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
            role: if row.is_multiple_of(u64::from(ROWS_PER_REPETITION_V2)) {
                LdeRowRoleV2::Product
            } else {
                LdeRowRoleV2::Quotient
            },
            block,
        };
        validate_lde_chunk_v2(self.geometry, coordinate, &chunk)?;
        live.writer.write_slot_v1(live.next_slot, chunk)?;
        live.next_slot = live
            .next_slot
            .checked_add(1)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        self.live = Some(live);
        Ok(())
    }
    pub(super) fn seal_v2(mut self) -> Result<InitialColumnSnapshotV2, ProverPrerequisiteErrorV2> {
        let live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if live.next_slot != self.descriptor.slot_count {
            return Err(ProverPrerequisiteErrorV2::Spool(
                QPcsSpoolErrorV2::MissingLdeBlocks,
            ));
        }
        Ok(InitialColumnSnapshotV2 {
            snapshot: Some(live.writer.seal_v1()?),
            geometry: self.geometry,
            parameter_digest: self.parameter_digest,
            descriptor: self.descriptor,
            context_digest: self.context_digest,
        })
    }
    #[cfg(test)]
    pub(super) fn panic_after_take_for_test_v2(&mut self) {
        let _live = self.live.take().expect("live initial column writer");
        panic!("intentional initial column writer unwind");
    }
}
pub(super) struct InitialColumnSnapshotV2 {
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
    descriptor: InitialColumnDescriptorV2,
    context_digest: [u8; 32],
}
struct LiveInitialTransposeV2 {
    snapshot: ConfidentialSpoolSnapshotV1,
    stage: QPcsCoefficientReplayStageV2,
}
pub(super) struct InitialTransposeV2 {
    live: Option<LiveInitialTransposeV2>,
    pub(super) descriptor: InitialColumnDescriptorV2,
    context_digest: [u8; 32],
    next_block: u64,
    next_column: u16,
}
impl InitialColumnSnapshotV2 {
    pub(super) fn begin_transpose_v2(
        mut self,
        stage: QPcsCoefficientReplayStageV2,
        context: PublicSpoolContextV2,
    ) -> Result<InitialTransposeV2, ProverPrerequisiteErrorV2> {
        let snapshot = self
            .snapshot
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if stage.parameter_digest != self.parameter_digest
            || stage.geometry.ring_degree != self.geometry.ring_degree
            || stage.geometry.domain_log != self.geometry.domain_log
            || stage.geometry.query_count != self.geometry.query_count
            || stage.geometry.coefficient_values_per_block
                != self.geometry.coefficient_values_per_block
            || stage.geometry.lde_values_per_block != self.geometry.lde_values_per_block
            || stage.geometry.moduli != self.geometry.moduli
            || initial_column_context_v2(self.descriptor, self.parameter_digest, context)?
                != self.context_digest
            || snapshot.slot_count_v1() != self.descriptor.slot_count
            || snapshot.plaintext_len_v1() != self.descriptor.plaintext_bytes
            || snapshot.file_len_v1() != self.descriptor.file_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        Ok(InitialTransposeV2 {
            live: Some(LiveInitialTransposeV2 { snapshot, stage }),
            descriptor: self.descriptor,
            context_digest: self.context_digest,
            next_block: 0,
            next_column: 0,
        })
    }
}
impl InitialTransposeV2 {
    pub(super) fn copy_next_block_v2(&mut self) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_block >= self.descriptor.blocks_per_column
            || self.next_column >= self.descriptor.columns
        {
            return Err(ProverPrerequisiteErrorV2::Spool(
                QPcsSpoolErrorV2::ExtraLdeBlock,
            ));
        }
        let slot =
            u64::from(self.next_column) * self.descriptor.blocks_per_column + self.next_block;
        let chunk = live.snapshot.read_slot_v1(slot, self.context_digest)?;
        live.stage.push_lde_block_v2(chunk)?;
        self.next_column += 1;
        if self.next_column == self.descriptor.columns {
            self.next_column = 0;
            self.next_block += 1;
        }
        self.live = Some(live);
        Ok(())
    }
    pub(super) fn complete_v2(
        mut self,
    ) -> Result<QPcsCoefficientReplayStageV2, ProverPrerequisiteErrorV2> {
        let live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_block != self.descriptor.blocks_per_column || self.next_column != 0 {
            return Err(ProverPrerequisiteErrorV2::Spool(
                QPcsSpoolErrorV2::ReplayIncomplete,
            ));
        }
        Ok(live.stage)
    }
    #[cfg(test)]
    pub(super) fn panic_after_take_for_test_v2(&mut self) {
        let _live = self.live.take().expect("live initial transpose");
        panic!("intentional initial transpose unwind");
    }
}

//! Exact-purpose authenticated replay and temporary-storage geometry for qPCS V2.

use super::*;

#[path = "replay_v2/post_c0_replay_v2.rs"]
mod post_c0_replay_v2;
use post_c0_replay_v2::*;
const REPLAY_COLUMNS_V2: u16 = 380;
const REPLAY_BLOCK_VALUES_V2: u16 = 1_024;
const REPLAY_DOMAIN_VALUES_V2: u64 = 1 << 19;
const REPLAY_BLOCKS_PER_COLUMN_V2: u64 = 512;
const REPLAY_FRI_LAYERS_V2: u8 = 18;
const REPLAY_FRI_TOTAL_FILE_BYTES_V2: u64 = 6_381_586_240;
const CQ_COLUMN_FILE_BYTES_V2: u64 = 3_190_784_000;
const ROW_SCRATCH_FILE_BYTES_V2: u64 = 8_396_800;
const ROW_SCRATCH_MAX_SNAPSHOTS_V2: u8 = 2;
const TRANSPOSE_LEAF_BYTES_V2: u64 = 6_080;
const TRANSPOSE_WINDOW_PLAINTEXT_BYTES_V2: u64 = 6_225_920;
const CQ_MAPPING_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.cq-column-stage.mapping\0";
const ROW_SCRATCH_MAPPING_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.row-scratch.mapping\0";
const FRI_MAPPING_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.fri-layer.mapping\0";
const DERIVED_CONTEXT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.derived-storage.context\0";
const CQ_SLOT_FORMULA_V2: &[u8] =
    b"column=limb*10+repetition*2+role;slot=column*512+block;first_index=block*1024";
const BLOCK_MAJOR_SLOT_FORMULA_V2: &[u8] =
    b"slot=block*380+column;first_index=block*values_per_block";
const ROW_SCRATCH_SLOT_FORMULA_V2: &[u8] =
    b"slot=block;first_index=block*1024;role-limb-repetition-pass-orientation-tile-bound-in-header";
const FQ2_ENCODING_V2: &[u8] = b"canonical Fq2=(c0,c1), each canonical big-endian u64";
const FRI_RELEASE_FILES_V2: [u64; 18] = [
    3_190_784_000,
    1_595_392_000,
    797_696_000,
    398_848_000,
    199_424_000,
    99_712_000,
    49_856_000,
    24_928_000,
    12_464_000,
    6_232_000,
    3_119_040,
    1_562_560,
    784_320,
    395_200,
    200_640,
    103_360,
    54_720,
    30_400,
];

const _: () = {
    assert!(REPLAY_COLUMNS_V2 as u64 == RELEASE_LDE_COLUMNS_V2);
    assert!(REPLAY_DOMAIN_VALUES_V2 == 1_u64 << RELEASE_DOMAIN_LOG_V2);
    assert!(REPLAY_BLOCKS_PER_COLUMN_V2 == RELEASE_LDE_BLOCKS_PER_COLUMN_V2);
    assert!(CQ_COLUMN_FILE_BYTES_V2 == RELEASE_LDE_FILE_BYTES_V2);
    assert!(ROW_SCRATCH_FILE_BYTES_V2 == 512 * (16_384 + AUTHENTICATION_TAG_BYTES_V2));
    assert!(TRANSPOSE_LEAF_BYTES_V2 == 380 * 16);
    assert!(TRANSPOSE_WINDOW_PLAINTEXT_BYTES_V2 == 1_024 * TRANSPOSE_LEAF_BYTES_V2);
    assert!(!SOURCE_AGGREGATION_COMPLETE_V2);
    assert!(!SOURCE_ALGEBRA_VERIFIED_V2);
    assert!(!Q_PCS_MASKING_INTEGRATED_V2);
    assert!(!Q_PCS_COMMITMENT_INTEGRATED_V2);
    assert!(!Q_PCS_PROOF_INTEGRATED_V2);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V2);
    assert!(!RELEASE_READY_V2);
    assert!(!RELEASE_COMPLETE_V2);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StorageRoleV2 {
    CqColumnStage = 1,
    RowScratch = 2,
    FriLayer = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScratchOrientationV2 {
    Rows = 1,
    Columns = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RowScratchAxesV2 {
    limb: u8,
    repetition: u8,
    role: LdeRowRoleV2,
    pass: u8,
    orientation: ScratchOrientationV2,
    tile: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StorageLayoutDescriptorV2 {
    role: StorageRoleV2,
    layer: u8,
    logical_length: u64,
    columns: u16,
    values_per_block: u16,
    blocks_per_column: u64,
    slot_count: u64,
    plaintext_bytes: u64,
    file_bytes: u64,
    mapping_digest: [u8; 32],
}

fn fixed_row_column_v2(
    limb: u8,
    repetition: u8,
    role: LdeRowRoleV2,
) -> Result<u16, QPcsSpoolErrorV2> {
    if limb >= RELEASE_LIMB_COUNT_V2 || repetition >= OPENING_REPETITIONS_V2 {
        return Err(QPcsSpoolErrorV2::InvalidReplayPurpose);
    }
    let role = match role {
        LdeRowRoleV2::Product => 0_u16,
        LdeRowRoleV2::Quotient => 1_u16,
    };
    Ok(u16::from(limb) * u16::from(FIXED_ROW_COUNT_V2)
        + u16::from(repetition) * u16::from(ROWS_PER_REPETITION_V2)
        + role)
}

fn checked_layout_v2(
    role: StorageRoleV2,
    layer: u8,
    logical_length: u64,
    columns: u16,
    values_per_block: u16,
    mapping_digest: [u8; 32],
) -> Result<StorageLayoutDescriptorV2, QPcsSpoolErrorV2> {
    if logical_length == 0
        || columns == 0
        || values_per_block == 0
        || logical_length % u64::from(values_per_block) != 0
        || mapping_digest == [0; 32]
    {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    let blocks_per_column = logical_length / u64::from(values_per_block);
    let slot_count = blocks_per_column
        .checked_mul(u64::from(columns))
        .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
    let plaintext_bytes = u64::from(values_per_block)
        .checked_mul(FQ2_WIRE_BYTES_V2)
        .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
    let file_bytes = slot_count
        .checked_mul(
            plaintext_bytes
                .checked_add(AUTHENTICATION_TAG_BYTES_V2)
                .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?,
        )
        .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
    if slot_count > CONFIDENTIAL_SPOOL_MAX_SLOTS_V1
        || plaintext_bytes > CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1
        || file_bytes > CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1
    {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    Ok(StorageLayoutDescriptorV2 {
        role,
        layer,
        logical_length,
        columns,
        values_per_block,
        blocks_per_column,
        slot_count,
        plaintext_bytes,
        file_bytes,
        mapping_digest,
    })
}

fn cq_column_layout_v2(
    parameter_digest: [u8; 32],
) -> Result<StorageLayoutDescriptorV2, QPcsSpoolErrorV2> {
    let digest = mapping_digest_for_layout_v2(
        StorageRoleV2::CqColumnStage,
        0,
        REPLAY_DOMAIN_VALUES_V2,
        REPLAY_COLUMNS_V2,
        REPLAY_BLOCK_VALUES_V2,
        parameter_digest,
        None,
    )?;
    let layout = checked_layout_v2(
        StorageRoleV2::CqColumnStage,
        0,
        REPLAY_DOMAIN_VALUES_V2,
        REPLAY_COLUMNS_V2,
        REPLAY_BLOCK_VALUES_V2,
        digest,
    )?;
    if layout.file_bytes != CQ_COLUMN_FILE_BYTES_V2 {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    Ok(layout)
}

fn row_scratch_layout_v2(
    parameter_digest: [u8; 32],
    axes: RowScratchAxesV2,
) -> Result<StorageLayoutDescriptorV2, QPcsSpoolErrorV2> {
    let digest = mapping_digest_for_layout_v2(
        StorageRoleV2::RowScratch,
        0,
        REPLAY_DOMAIN_VALUES_V2,
        1,
        REPLAY_BLOCK_VALUES_V2,
        parameter_digest,
        Some(axes),
    )?;
    let layout = checked_layout_v2(
        StorageRoleV2::RowScratch,
        0,
        REPLAY_DOMAIN_VALUES_V2,
        1,
        REPLAY_BLOCK_VALUES_V2,
        digest,
    )?;
    if layout.file_bytes != ROW_SCRATCH_FILE_BYTES_V2 {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    Ok(layout)
}

fn fri_layer_layout_v2(
    parameter_digest: [u8; 32],
    layer: u8,
) -> Result<StorageLayoutDescriptorV2, QPcsSpoolErrorV2> {
    if layer >= REPLAY_FRI_LAYERS_V2 {
        return Err(QPcsSpoolErrorV2::InvalidFriLayer);
    }
    let logical_length = REPLAY_DOMAIN_VALUES_V2 >> layer;
    let values_per_block = u16::try_from(logical_length.min(u64::from(REPLAY_BLOCK_VALUES_V2)))
        .map_err(|_| QPcsSpoolErrorV2::InvalidGeometry)?;
    let digest = mapping_digest_for_layout_v2(
        StorageRoleV2::FriLayer,
        layer,
        logical_length,
        REPLAY_COLUMNS_V2,
        values_per_block,
        parameter_digest,
        None,
    )?;
    let layout = checked_layout_v2(
        StorageRoleV2::FriLayer,
        layer,
        logical_length,
        REPLAY_COLUMNS_V2,
        values_per_block,
        digest,
    )?;
    if layout.file_bytes != FRI_RELEASE_FILES_V2[usize::from(layer)] {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    Ok(layout)
}

#[allow(clippy::too_many_arguments)]
fn mapping_digest_for_layout_v2(
    role: StorageRoleV2,
    layer: u8,
    logical_length: u64,
    columns: u16,
    values_per_block: u16,
    parameter_digest: [u8; 32],
    scratch: Option<RowScratchAxesV2>,
) -> Result<[u8; 32], QPcsSpoolErrorV2> {
    if parameter_digest == [0; 32] {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    let release = (
        REPLAY_DOMAIN_VALUES_V2,
        REPLAY_COLUMNS_V2,
        REPLAY_BLOCK_VALUES_V2,
    );
    #[cfg(test)]
    let tiny_cq_test_geometry = logical_length != 0
        && columns != 0
        && values_per_block != 0
        && logical_length.is_multiple_of(u64::from(values_per_block));
    #[cfg(not(test))]
    let tiny_cq_test_geometry = false;
    match role {
        StorageRoleV2::CqColumnStage
            if layer == 0
                && ((logical_length, columns, values_per_block) == release
                    || tiny_cq_test_geometry)
                && scratch.is_none() => {}
        StorageRoleV2::RowScratch
            if layer == 0
                && (logical_length, columns, values_per_block)
                    == (REPLAY_DOMAIN_VALUES_V2, 1, REPLAY_BLOCK_VALUES_V2)
                && scratch.is_some() => {}
        StorageRoleV2::FriLayer
            if layer < REPLAY_FRI_LAYERS_V2
                && logical_length == REPLAY_DOMAIN_VALUES_V2 >> layer
                && columns == REPLAY_COLUMNS_V2
                && u64::from(values_per_block)
                    == logical_length.min(u64::from(REPLAY_BLOCK_VALUES_V2))
                && scratch.is_none() => {}
        _ => return Err(QPcsSpoolErrorV2::InvalidGeometry),
    }
    let blocks = logical_length / u64::from(values_per_block);
    let slots = blocks
        .checked_mul(u64::from(columns))
        .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
    let plaintext = u64::from(values_per_block)
        .checked_mul(FQ2_WIRE_BYTES_V2)
        .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
    let file = slots
        .checked_mul(
            plaintext
                .checked_add(AUTHENTICATION_TAG_BYTES_V2)
                .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?,
        )
        .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
    let mut hash = Keccak256::new();
    hash.update(match role {
        StorageRoleV2::CqColumnStage => CQ_MAPPING_DOMAIN_V2,
        StorageRoleV2::RowScratch => ROW_SCRATCH_MAPPING_DOMAIN_V2,
        StorageRoleV2::FriLayer => FRI_MAPPING_DOMAIN_V2,
    });
    hash.update(&[Q_PCS_SPOOL_VERSION_V2, role as u8, layer]);
    hash.update(&parameter_digest);
    hash.update(&logical_length.to_be_bytes());
    hash.update(&columns.to_be_bytes());
    hash.update(&values_per_block.to_be_bytes());
    hash.update(&blocks.to_be_bytes());
    hash.update(&slots.to_be_bytes());
    hash.update(&plaintext.to_be_bytes());
    hash.update(&file.to_be_bytes());
    match (role, scratch) {
        (StorageRoleV2::RowScratch, Some(axes)) => {
            if axes.limb >= RELEASE_LIMB_COUNT_V2
                || axes.repetition >= OPENING_REPETITIONS_V2
                || axes.pass >= ROW_SCRATCH_MAX_SNAPSHOTS_V2
                || axes.tile >= 512
            {
                return Err(QPcsSpoolErrorV2::InvalidReplayPurpose);
            }
            let role = match axes.role {
                LdeRowRoleV2::Product => 0,
                LdeRowRoleV2::Quotient => 1,
            };
            hash.update(&[
                axes.limb,
                axes.repetition,
                role,
                axes.pass,
                axes.orientation as u8,
            ]);
            hash.update(&axes.tile.to_be_bytes());
            hash.update(ROW_SCRATCH_SLOT_FORMULA_V2);
        }
        (StorageRoleV2::RowScratch, None) | (_, Some(_)) => {
            return Err(QPcsSpoolErrorV2::InvalidReplayPurpose);
        }
        (StorageRoleV2::CqColumnStage, None) => hash.update(CQ_SLOT_FORMULA_V2),
        (StorageRoleV2::FriLayer, None) => hash.update(BLOCK_MAJOR_SLOT_FORMULA_V2),
    }
    hash.update(FQ2_ENCODING_V2);
    hash.update(&slots.to_be_bytes());
    for slot in 0..slots {
        let (block, column) = if role == StorageRoleV2::CqColumnStage {
            (slot % blocks, slot / blocks)
        } else {
            (slot / u64::from(columns), slot % u64::from(columns))
        };
        let first_index = block * u64::from(values_per_block);
        hash.update(&slot.to_be_bytes());
        hash.update(&[layer]);
        hash.update(&logical_length.to_be_bytes());
        hash.update(&block.to_be_bytes());
        hash.update(
            &u16::try_from(column)
                .map_err(|_| QPcsSpoolErrorV2::InvalidGeometry)?
                .to_be_bytes(),
        );
        hash.update(&first_index.to_be_bytes());
        hash.update(&values_per_block.to_be_bytes());
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    Ok(digest)
}

fn derived_context_digest_v2(
    descriptor: StorageLayoutDescriptorV2,
    context: PublicSpoolContextV2,
) -> Result<[u8; 32], QPcsSpoolErrorV2> {
    context.validate_v2()?;
    let mut hash = Keccak256::new();
    hash.update(DERIVED_CONTEXT_DOMAIN_V2);
    hash.update(&[
        Q_PCS_SPOOL_VERSION_V2,
        descriptor.role as u8,
        descriptor.layer,
    ]);
    hash.update(&descriptor.mapping_digest);
    hash.update(&context.sealed_source_transcript_digest);
    hash.update(&context.source_algebra_binding_digest);
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(QPcsSpoolErrorV2::InertPublicContext);
    }
    Ok(digest)
}

struct LiveCoefficientReplayStageV2 {
    coefficient: ConfidentialSpoolSnapshotV1,
    lde: ConfidentialSpoolWriterV1,
    next_lde_block: u64,
    next_coefficient_purpose: u16,
    replay_permit: AuthenticatedReplayPermitV2,
}

pub(super) struct QPcsCoefficientReplayStageV2 {
    live: Option<LiveCoefficientReplayStageV2>,
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
    coefficient_context_digest: [u8; 32],
    lde_context_digest: [u8; 32],
}

impl QPcsSpoolWriterV2 {
    pub(super) fn seal_coefficients_for_replay_v2(
        mut self,
    ) -> Result<QPcsCoefficientReplayStageV2, QPcsSpoolErrorV2> {
        let live = self.live.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if live.next_coefficient_slot != self.geometry.coefficient_slot_count_v2()? {
            return Err(QPcsSpoolErrorV2::MissingCoefficientBlocks);
        }
        let coefficient = live.coefficient.seal_v1()?;
        Ok(QPcsCoefficientReplayStageV2 {
            live: Some(LiveCoefficientReplayStageV2 {
                coefficient,
                lde: live.lde,
                next_lde_block: 0,
                next_coefficient_purpose: 0,
                replay_permit: live.replay_permit,
            }),
            geometry: self.geometry,
            parameter_digest: self.parameter_digest,
            coefficient_context_digest: self.coefficient_context_digest,
            lde_context_digest: self.lde_context_digest,
        })
    }
}

impl QPcsCoefficientReplayStageV2 {
    pub(super) fn begin_next_coefficient_row_v2(
        self,
    ) -> Result<CoefficientReplayReaderV2, QPcsSpoolErrorV2> {
        let mut owner = Some(self);
        let purpose = owner
            .as_ref()
            .and_then(|stage| stage.live.as_ref())
            .ok_or(QPcsSpoolErrorV2::Poisoned)?
            .next_coefficient_purpose;
        let pair_count = u16::from(owner.as_ref().unwrap().geometry.limb_count_v2()?)
            .checked_mul(u16::from(OPENING_REPETITIONS_V2))
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        let pair = purpose / u16::from(COEFFICIENT_COMPONENTS_V2);
        let component = match purpose % u16::from(COEFFICIENT_COMPONENTS_V2) {
            0 => CoefficientComponentV2::ProductLow,
            1 => CoefficientComponentV2::ProductHighWithTopZero,
            2 => CoefficientComponentV2::QuotientWithTopZero,
            _ => return Err(QPcsSpoolErrorV2::InvalidGeometry),
        };
        if pair >= pair_count {
            let _poisoned = owner.take();
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        Ok(CoefficientReplayReaderV2 {
            stage: owner,
            pair,
            component,
            next_block: 0,
        })
    }

    pub(super) fn push_lde_block_v2(
        &mut self,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), QPcsSpoolErrorV2> {
        let mut live = self.live.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let slot = live.next_lde_block;
        if slot >= self.geometry.lde_slot_count_v2()? {
            return Err(QPcsSpoolErrorV2::ExtraLdeBlock);
        }
        let coordinate = lde_coordinate_v2(self.geometry, slot)?;
        validate_lde_chunk_v2(self.geometry, coordinate, &chunk)?;
        live.lde.write_slot_v1(slot, chunk)?;
        live.next_lde_block = slot
            .checked_add(1)
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        self.live = Some(live);
        Ok(())
    }

    pub(super) fn seal_lde_v2(mut self) -> Result<QPcsSpoolSnapshotV2, QPcsSpoolErrorV2> {
        let live = self.live.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let purpose_count = u16::from(self.geometry.limb_count_v2()?)
            .checked_mul(u16::from(OPENING_REPETITIONS_V2))
            .and_then(|value| value.checked_mul(u16::from(COEFFICIENT_COMPONENTS_V2)))
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        if live.next_coefficient_purpose != purpose_count {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        if live.next_lde_block != self.geometry.lde_slot_count_v2()? {
            return Err(QPcsSpoolErrorV2::MissingLdeBlocks);
        }
        let lde = live.lde.seal_v1()?;
        let snapshot_binding_digest = snapshot_binding_digest_v2(
            self.parameter_digest,
            self.coefficient_context_digest,
            self.lde_context_digest,
            *live.coefficient.snapshot_digest_v1(),
            *lde.snapshot_digest_v1(),
        )?;
        Ok(QPcsSpoolSnapshotV2 {
            live: Some(LiveSpoolSnapshotsV2 {
                coefficient: live.coefficient,
                lde,
                replay_permit: live.replay_permit,
            }),
            geometry: self.geometry,
            parameter_digest: self.parameter_digest,
            coefficient_context_digest: self.coefficient_context_digest,
            lde_context_digest: self.lde_context_digest,
            snapshot_binding_digest,
        })
    }
}

pub(super) struct CoefficientReplayReaderV2 {
    stage: Option<QPcsCoefficientReplayStageV2>,
    pair: u16,
    component: CoefficientComponentV2,
    next_block: u64,
}

impl CoefficientReplayReaderV2 {
    pub(super) fn read_next_block_v2(
        &mut self,
    ) -> Result<AuthenticatedReplayChunkV2, QPcsSpoolErrorV2> {
        let mut stage = self.stage.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let mut live = stage.live.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let blocks = stage.geometry.coefficient_blocks_per_component_v2()?;
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
        let coordinate = coefficient_coordinate_v2(stage.geometry, slot)?;
        let chunk = live
            .coefficient
            .read_slot_v1(slot, stage.coefficient_context_digest)?;
        validate_coefficient_chunk_v2(stage.geometry, coordinate, &chunk)?;
        self.next_block += 1;
        stage.live = Some(live);
        self.stage = Some(stage);
        Ok(AuthenticatedReplayChunkV2 { chunk })
    }

    pub(super) fn complete_v2(mut self) -> Result<QPcsCoefficientReplayStageV2, QPcsSpoolErrorV2> {
        let mut stage = self.stage.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if self.next_block != stage.geometry.coefficient_blocks_per_component_v2()? {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        let live = stage.live.as_mut().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        live.next_coefficient_purpose = live
            .next_coefficient_purpose
            .checked_add(1)
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        Ok(stage)
    }

    #[cfg(test)]
    fn panic_after_take_for_test_v2(&mut self) {
        let _stage = self.stage.take().expect("live coefficient replay reader");
        panic!("intentional coefficient replay unwind test");
    }
}

struct LiveSpoolSnapshotsV2 {
    coefficient: ConfidentialSpoolSnapshotV1,
    lde: ConfidentialSpoolSnapshotV1,
    replay_permit: AuthenticatedReplayPermitV2,
}

pub(super) struct QPcsSpoolSnapshotV2 {
    live: Option<LiveSpoolSnapshotsV2>,
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
    coefficient_context_digest: [u8; 32],
    lde_context_digest: [u8; 32],
    snapshot_binding_digest: [u8; 32],
}

impl QPcsSpoolSnapshotV2 {
    pub(super) const fn parameter_digest_v2(&self) -> [u8; 32] {
        self.parameter_digest
    }

    pub(super) const fn snapshot_binding_digest_v2(&self) -> [u8; 32] {
        self.snapshot_binding_digest
    }

    pub(super) fn begin_c0_replay_v2(self) -> Result<C0ReplayReaderV2, QPcsSpoolErrorV2> {
        if self.live.is_none() {
            return Err(QPcsSpoolErrorV2::Poisoned);
        }
        Ok(C0ReplayReaderV2 {
            snapshot: Some(self),
            next_block: 0,
            next_column: 0,
        })
    }
}

pub(super) struct C0ReplayReaderV2 {
    snapshot: Option<QPcsSpoolSnapshotV2>,
    next_block: u64,
    next_column: u64,
}

impl C0ReplayReaderV2 {
    pub(super) fn read_next_block_column_v2(
        &mut self,
    ) -> Result<AuthenticatedReplayChunkV2, QPcsSpoolErrorV2> {
        let mut snapshot = self.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let mut live = snapshot.live.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let columns = snapshot.geometry.lde_column_count_v2()?;
        let blocks = snapshot.geometry.lde_blocks_per_column_v2()?;
        if self.next_block >= blocks || self.next_column >= columns {
            return Err(QPcsSpoolErrorV2::ExtraLdeBlock);
        }
        let slot = self.next_block * columns + self.next_column;
        let chunk = live.lde.read_slot_v1(slot, snapshot.lde_context_digest)?;
        self.next_column += 1;
        if self.next_column == columns {
            self.next_column = 0;
            self.next_block += 1;
        }
        snapshot.live = Some(live);
        self.snapshot = Some(snapshot);
        Ok(AuthenticatedReplayChunkV2 { chunk })
    }

    pub(super) fn complete_v2(mut self) -> Result<QPcsC0CompleteV2, QPcsSpoolErrorV2> {
        let snapshot = self.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if self.next_block != snapshot.geometry.lde_blocks_per_column_v2()? || self.next_column != 0
        {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        Ok(QPcsC0CompleteV2 { snapshot })
    }
}

pub(super) struct QPcsC0CompleteV2 {
    snapshot: QPcsSpoolSnapshotV2,
}
pub(super) struct AuthenticatedReplayChunkV2 {
    chunk: ConfidentialSpoolChunkV1,
}

impl AuthenticatedReplayChunkV2 {
    pub(super) fn bytes_v2(&self) -> &[u8] {
        self.chunk.as_slice_v1()
    }
}

impl Drop for AuthenticatedReplayChunkV2 {
    fn drop(&mut self) {
        self.chunk.as_mut_slice_v1().fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        #[cfg(test)]
        REPLAY_CHUNK_ZEROIZED_DROPS_V2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

pub(super) struct QPcsDerivedReplayV2 {
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    next_unit: u64,
    replay_permit: AuthenticatedReplayPermitV2,
}

fn bind_derived_replay_v2(
    snapshot: ConfidentialSpoolSnapshotV1,
    descriptor: StorageLayoutDescriptorV2,
    context: PublicSpoolContextV2,
    replay_permit: AuthenticatedReplayPermitV2,
) -> Result<QPcsDerivedReplayV2, QPcsSpoolErrorV2> {
    let context_digest = derived_context_digest_v2(descriptor, context)?;
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

impl QPcsDerivedReplayV2 {
    pub(super) fn begin_next_cq_transpose_window_v2(
        mut self,
    ) -> Result<CqTransposeWindowReaderV2, QPcsSpoolErrorV2> {
        let snapshot = self.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if self.descriptor.role != StorageRoleV2::CqColumnStage
            || self.next_unit >= self.descriptor.blocks_per_column
        {
            return Err(QPcsSpoolErrorV2::InvalidStoragePhase);
        }
        self.snapshot = Some(snapshot);
        Ok(CqTransposeWindowReaderV2 {
            owner: Some(self),
            next_column: 0,
        })
    }

    pub(super) fn begin_next_fri_fold_column_v2(
        mut self,
    ) -> Result<FriFoldPairReaderV2, QPcsSpoolErrorV2> {
        let snapshot = self.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if self.descriptor.role != StorageRoleV2::FriLayer
            || self.next_unit >= u64::from(self.descriptor.columns)
        {
            return Err(QPcsSpoolErrorV2::InvalidStoragePhase);
        }
        self.snapshot = Some(snapshot);
        Ok(FriFoldPairReaderV2 {
            owner: Some(self),
            next_pair_block: 0,
        })
    }
}

pub(super) struct CqTransposeWindowReaderV2 {
    owner: Option<QPcsDerivedReplayV2>,
    next_column: u16,
}

impl CqTransposeWindowReaderV2 {
    pub(super) fn read_next_column_v2(
        &mut self,
    ) -> Result<AuthenticatedReplayChunkV2, QPcsSpoolErrorV2> {
        let mut owner = self.owner.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let mut snapshot = owner.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if self.next_column >= owner.descriptor.columns {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        let slot =
            u64::from(self.next_column) * owner.descriptor.blocks_per_column + owner.next_unit;
        let chunk = snapshot.read_slot_v1(slot, owner.context_digest)?;
        self.next_column += 1;
        owner.snapshot = Some(snapshot);
        self.owner = Some(owner);
        Ok(AuthenticatedReplayChunkV2 { chunk })
    }

    pub(super) fn complete_v2(mut self) -> Result<QPcsDerivedReplayV2, QPcsSpoolErrorV2> {
        let mut owner = self.owner.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        if self.next_column != owner.descriptor.columns {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        owner.next_unit += 1;
        Ok(owner)
    }
}

pub(super) struct FriFoldPairChunksV2 {
    pub(super) lower: AuthenticatedReplayChunkV2,
    pub(super) upper: Option<AuthenticatedReplayChunkV2>,
    pub(super) values_per_half: u16,
}

pub(super) struct FriFoldPairReaderV2 {
    owner: Option<QPcsDerivedReplayV2>,
    next_pair_block: u64,
}

impl FriFoldPairReaderV2 {
    pub(super) fn read_next_pair_v2(&mut self) -> Result<FriFoldPairChunksV2, QPcsSpoolErrorV2> {
        let mut owner = self.owner.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let mut snapshot = owner.snapshot.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let layout = owner.descriptor;
        let column = owner.next_unit;
        let pair_blocks = if layout.blocks_per_column >= 2 {
            layout.blocks_per_column / 2
        } else {
            1
        };
        if self.next_pair_block >= pair_blocks {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        let lower_slot = self.next_pair_block * u64::from(layout.columns) + column;
        let lower = AuthenticatedReplayChunkV2 {
            chunk: snapshot.read_slot_v1(lower_slot, owner.context_digest)?,
        };
        let (upper, values_per_half) = if layout.blocks_per_column >= 2 {
            let upper_slot =
                (self.next_pair_block + pair_blocks) * u64::from(layout.columns) + column;
            (
                Some(AuthenticatedReplayChunkV2 {
                    chunk: snapshot.read_slot_v1(upper_slot, owner.context_digest)?,
                }),
                layout.values_per_block,
            )
        } else {
            (None, layout.values_per_block / 2)
        };
        self.next_pair_block += 1;
        owner.snapshot = Some(snapshot);
        self.owner = Some(owner);
        Ok(FriFoldPairChunksV2 {
            lower,
            upper,
            values_per_half,
        })
    }

    pub(super) fn complete_v2(mut self) -> Result<QPcsDerivedReplayV2, QPcsSpoolErrorV2> {
        let mut owner = self.owner.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let expected = if owner.descriptor.blocks_per_column >= 2 {
            owner.descriptor.blocks_per_column / 2
        } else {
            1
        };
        if self.next_pair_block != expected {
            return Err(QPcsSpoolErrorV2::ReplayIncomplete);
        }
        owner.next_unit += 1;
        Ok(owner)
    }
}

#[cfg(test)]
static REPLAY_CHUNK_ZEROIZED_DROPS_V2: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[path = "replay_v2/canonical_proof_replay_v2.rs"]
mod canonical_proof_replay_v2;
use canonical_proof_replay_v2::*;
#[path = "replay_v2/prover_v2.rs"]
mod prover_v2;
#[cfg(test)]
#[path = "replay_v2_tests.rs"]
mod tests;

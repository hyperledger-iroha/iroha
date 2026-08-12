//! Concrete authenticated spooling prerequisite for fixed-width qPCS V2.
//!
//! This private child freezes two sequential stores. The coefficient store
//! contains five `(P, H)` pairs per RNS limb. `P` occupies an `N`-coefficient
//! low component and an `N`-coefficient high component whose last coefficient
//! is authenticated zero padding; `H` occupies an `N`-coefficient component
//! with the same top-zero rule. The LDE store contains the corresponding ten
//! fixed-width rows over `Fq2` on the `2^19` domain in block-major order, so one
//! sequential 380-slot window contains the same 1,024 evaluation indices from
//! every limb/row column. Each mapping digest count-prefixes and streams every
//! actual slot-to-coordinate output after its geometry, formula, and encoding.
//! Coefficients seal before the block-major LDE writer completes, so a nested
//! move-only replay typestate can authenticate exact-purpose coefficient reads
//! while that LDE writer remains open. Both stores use the process-local
//! authenticated confidential-spool leaf and expose no file,
//! key, path, random-access, callback, or caller-selected slot surface.
//!
//! This is not source aggregation, algebra verification, a commitment, or a
//! proof. The sole production constructor consumes a private seal whose two
//! fields are uninhabited. Consequently no production caller can create these
//! spools yet. Test-only construction exercises the storage state machine with
//! tiny geometry. Every readiness and completion axis remains false.

use core::{convert::Infallible, fmt};
use std::path::Path;

use iroha_confidential_spool::{
    CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1, CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1,
    CONFIDENTIAL_SPOOL_MAX_SLOTS_V1, ConfidentialSpoolChunkV1, ConfidentialSpoolErrorV1,
    ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1, ConfidentialSpoolWriterV1,
};

use crate::vega::sponge::{Keccak256, keccak256};

use super::super::super::manifest::{RELEASE_MODULI_V1, ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1};
use super::is_prime_u64;

const Q_PCS_SPOOL_VERSION_V2: u8 = 2;
const OPENING_REPETITIONS_V2: u8 = 5;
const ROWS_PER_REPETITION_V2: u8 = 2;
const FIXED_ROW_COUNT_V2: u8 = OPENING_REPETITIONS_V2 * ROWS_PER_REPETITION_V2;
const COEFFICIENT_COMPONENTS_V2: u8 = 3;
const RELEASE_LIMB_COUNT_V2: u8 = 38;
const RELEASE_DOMAIN_LOG_V2: u8 = 19;
const RELEASE_QUERY_COUNT_V2: u16 = 160;
const EXTENSION_DEGREE_V2: u8 = 2;
const RELEASE_COEFFICIENT_VALUES_PER_BLOCK_V2: u16 = 1_024;
const RELEASE_LDE_VALUES_PER_BLOCK_V2: u16 = 1_024;
const BASE_FIELD_WIRE_BYTES_V2: u64 = 8;
const FQ2_WIRE_BYTES_V2: u64 = 16;
const RELEASE_COEFFICIENT_BLOCK_BYTES_V2: u64 = 8_192;
const RELEASE_COEFFICIENT_BLOCKS_PER_COMPONENT_V2: u64 = 128;
const RELEASE_COEFFICIENT_SLOTS_V2: u64 = 72_960;
const RELEASE_COEFFICIENT_FILE_BYTES_V2: u64 = 598_855_680;
const RELEASE_LDE_BLOCK_BYTES_V2: u64 = 16_384;
const RELEASE_LDE_BLOCKS_PER_COLUMN_V2: u64 = 512;
const RELEASE_LDE_COLUMNS_V2: u64 = 380;
const RELEASE_LDE_SLOTS_V2: u64 = 194_560;
const RELEASE_LDE_FILE_BYTES_V2: u64 = 3_190_784_000;
const RELEASE_TOTAL_FILE_BYTES_V2: u64 =
    RELEASE_COEFFICIENT_FILE_BYTES_V2 + RELEASE_LDE_FILE_BYTES_V2;
const AUTHENTICATION_TAG_BYTES_V2: u64 = 16;

const PARAMETER_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.parameters\0";
const FIXED_WIDTH_TAG_V2: &[u8] = b"P:2N/c[2N-1]=0;H:N/c[N-1]=0";
const ROW_ORDER_TAG_V2: &[u8] = b"column=limb*10+repetition*2+role;P:0;H:1";
const BATCH_FORMULA_TAG_V2: &[u8] = b"Bp=aP+bXUP;Bh=aX^NH+bX^(N+1)UH";
const COEFFICIENT_MAPPING_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.coefficient-spool.exhaustive-mapping\0";
const LDE_MAPPING_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.lde-spool.exhaustive-mapping\0";
const COEFFICIENT_COORDINATE_ENUMERATION_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.coefficient-spool.coordinate-enumeration.count-u64.tuple-slot-u64-limb-u8-repetition-u8-component-u8-block-u64\0";
const LDE_COORDINATE_ENUMERATION_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.lde-spool.coordinate-enumeration.count-u64.tuple-slot-u64-limb-u8-repetition-u8-role-u8-block-u64\0";
const CONTEXT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.confidential-spool.context\0";
const SNAPSHOT_BINDING_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.spool-snapshot.binding\0";
const COEFFICIENT_SLOT_FORMULA_V2: &[u8] = b"pair=limb*5+repetition;slot=((pair*blocks_per_component+block)*3)+component;component=p-low:0,p-high-top-zero:1,h-top-zero:2";
const LDE_SLOT_FORMULA_V2: &[u8] =
    b"column=limb*10+repetition*2+role;slot=block*columns+column;role=p:0,h:1";
const COEFFICIENT_ENCODING_V2: &[u8] =
    b"canonical big-endian u64 residues;descriptor fixes values-per-block";
const LDE_ENCODING_V2: &[u8] =
    b"canonical (c0,c1) big-endian u64 Fq2 values;descriptor fixes values-per-block";

const SOURCE_AGGREGATION_COMPLETE_V2: bool = false;
const SOURCE_ALGEBRA_VERIFIED_V2: bool = false;
const Q_PCS_MASKING_INTEGRATED_V2: bool = false;
const Q_PCS_COMMITMENT_INTEGRATED_V2: bool = false;
const Q_PCS_PROOF_INTEGRATED_V2: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false;
const RELEASE_READY_V2: bool = false;
const RELEASE_COMPLETE_V2: bool = false;

const _: () = {
    assert!(FIXED_ROW_COUNT_V2 == 10);
    assert!(RELEASE_LIMB_COUNT_V2 as usize == RELEASE_MODULI_V1.len());
    assert!(
        ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64
            == RELEASE_COEFFICIENT_BLOCKS_PER_COMPONENT_V2
                * RELEASE_COEFFICIENT_VALUES_PER_BLOCK_V2 as u64
    );
    assert!(
        RELEASE_COEFFICIENT_SLOTS_V2
            == RELEASE_LIMB_COUNT_V2 as u64
                * OPENING_REPETITIONS_V2 as u64
                * RELEASE_COEFFICIENT_BLOCKS_PER_COMPONENT_V2
                * COEFFICIENT_COMPONENTS_V2 as u64
    );
    assert!(
        RELEASE_COEFFICIENT_FILE_BYTES_V2
            == RELEASE_COEFFICIENT_SLOTS_V2
                * (RELEASE_COEFFICIENT_BLOCK_BYTES_V2 + AUTHENTICATION_TAG_BYTES_V2)
    );
    assert!(RELEASE_LDE_COLUMNS_V2 == RELEASE_LIMB_COUNT_V2 as u64 * FIXED_ROW_COUNT_V2 as u64);
    assert!(RELEASE_LDE_SLOTS_V2 == RELEASE_LDE_COLUMNS_V2 * RELEASE_LDE_BLOCKS_PER_COLUMN_V2);
    assert!(
        RELEASE_LDE_FILE_BYTES_V2
            == RELEASE_LDE_SLOTS_V2 * (RELEASE_LDE_BLOCK_BYTES_V2 + AUTHENTICATION_TAG_BYTES_V2)
    );
    assert!(RELEASE_TOTAL_FILE_BYTES_V2 == 3_789_639_680);
    assert!(RELEASE_COEFFICIENT_SLOTS_V2 <= CONFIDENTIAL_SPOOL_MAX_SLOTS_V1);
    assert!(RELEASE_LDE_SLOTS_V2 <= CONFIDENTIAL_SPOOL_MAX_SLOTS_V1);
    assert!(RELEASE_COEFFICIENT_BLOCK_BYTES_V2 <= CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1);
    assert!(RELEASE_LDE_BLOCK_BYTES_V2 <= CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1);
    assert!(RELEASE_COEFFICIENT_FILE_BYTES_V2 <= CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1);
    assert!(RELEASE_LDE_FILE_BYTES_V2 <= CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1);
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
enum QPcsSpoolErrorV2 {
    InvalidGeometry,
    InertPublicContext,
    InvalidChunkLength,
    NonCanonicalResidue,
    NonZeroTopPadding,
    ExtraCoefficientBlock,
    ExtraLdeBlock,
    MissingCoefficientBlocks,
    MissingLdeBlocks,
    InvalidReplayPurpose,
    ReplayIncomplete,
    InvalidStoragePhase,
    InvalidFriLayer,
    Poisoned,
    Leaf(ConfidentialSpoolErrorV1),
}

impl fmt::Display for QPcsSpoolErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl From<ConfidentialSpoolErrorV1> for QPcsSpoolErrorV2 {
    fn from(error: ConfidentialSpoolErrorV1) -> Self {
        Self::Leaf(error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FixedTenRowParameterDescriptorV2 {
    version: u8,
    ring_degree: u32,
    limb_count: u8,
    opening_repetitions: u8,
    fixed_row_count: u8,
    maximum_product_degree: u32,
    product_fixed_width: u32,
    maximum_quotient_degree: u32,
    quotient_fixed_width: u32,
    domain_log: u8,
    query_count: u16,
    fri_rounds: u8,
    extension_degree: u8,
}

impl FixedTenRowParameterDescriptorV2 {
    fn from_geometry_v2(geometry: SpoolGeometryV2) -> Result<Self, QPcsSpoolErrorV2> {
        geometry.validate_v2()?;
        let product_fixed_width = geometry
            .ring_degree
            .checked_mul(2)
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        Ok(Self {
            version: Q_PCS_SPOOL_VERSION_V2,
            ring_degree: geometry.ring_degree,
            limb_count: geometry.limb_count_v2()?,
            opening_repetitions: OPENING_REPETITIONS_V2,
            fixed_row_count: FIXED_ROW_COUNT_V2,
            maximum_product_degree: product_fixed_width
                .checked_sub(2)
                .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?,
            product_fixed_width,
            maximum_quotient_degree: geometry
                .ring_degree
                .checked_sub(2)
                .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?,
            quotient_fixed_width: geometry.ring_degree,
            domain_log: geometry.domain_log,
            query_count: geometry.query_count,
            fri_rounds: u8::try_from(geometry.ring_degree.trailing_zeros() + 1)
                .map_err(|_| QPcsSpoolErrorV2::InvalidGeometry)?,
            extension_degree: EXTENSION_DEGREE_V2,
        })
    }

    fn digest_v2(self, moduli: &[u64]) -> Result<[u8; 32], QPcsSpoolErrorV2> {
        if usize::from(self.limb_count) != moduli.len() || moduli.is_empty() {
            return Err(QPcsSpoolErrorV2::InvalidGeometry);
        }
        let log_n = u8::try_from(self.ring_degree.trailing_zeros())
            .map_err(|_| QPcsSpoolErrorV2::InvalidGeometry)?;
        let domain_size = 1_u32
            .checked_shl(u32::from(self.domain_log))
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        let capacity = PARAMETER_DOMAIN_V2.len()
            + 3
            + 4
            + 4
            + 2
            + 4
            + FIXED_WIDTH_TAG_V2.len()
            + ROW_ORDER_TAG_V2.len()
            + BATCH_FORMULA_TAG_V2.len()
            + 9 * moduli.len();
        let mut frame = Vec::with_capacity(capacity);
        frame.extend_from_slice(PARAMETER_DOMAIN_V2);
        frame.extend_from_slice(&[self.version, log_n, self.domain_log]);
        frame.extend_from_slice(&self.ring_degree.to_be_bytes());
        frame.extend_from_slice(&domain_size.to_be_bytes());
        frame.extend_from_slice(&self.query_count.to_be_bytes());
        frame.extend_from_slice(&[
            self.limb_count,
            self.opening_repetitions,
            self.fixed_row_count,
            self.fri_rounds,
        ]);
        frame.extend_from_slice(FIXED_WIDTH_TAG_V2);
        frame.extend_from_slice(ROW_ORDER_TAG_V2);
        frame.extend_from_slice(BATCH_FORMULA_TAG_V2);
        for (limb, modulus) in moduli.iter().enumerate() {
            frame.push(u8::try_from(limb).map_err(|_| QPcsSpoolErrorV2::InvalidGeometry)?);
            frame.extend_from_slice(&modulus.to_be_bytes());
        }
        if frame.len() != capacity {
            return Err(QPcsSpoolErrorV2::InvalidGeometry);
        }
        let digest = keccak256(&frame);
        if digest == [0; 32] {
            return Err(QPcsSpoolErrorV2::InvalidGeometry);
        }
        Ok(digest)
    }
}

#[derive(Clone, Copy)]
struct SpoolGeometryV2 {
    ring_degree: u32,
    domain_log: u8,
    query_count: u16,
    coefficient_values_per_block: u16,
    lde_values_per_block: u16,
    moduli: &'static [u64],
}

impl SpoolGeometryV2 {
    const fn release_v2() -> Self {
        Self {
            ring_degree: ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u32,
            domain_log: RELEASE_DOMAIN_LOG_V2,
            query_count: RELEASE_QUERY_COUNT_V2,
            coefficient_values_per_block: RELEASE_COEFFICIENT_VALUES_PER_BLOCK_V2,
            lde_values_per_block: RELEASE_LDE_VALUES_PER_BLOCK_V2,
            moduli: &RELEASE_MODULI_V1,
        }
    }

    fn validate_v2(self) -> Result<(), QPcsSpoolErrorV2> {
        let limb_count = self.limb_count_v2()?;
        if limb_count == 0
            || self.ring_degree < 2
            || !self.ring_degree.is_power_of_two()
            || self.query_count == 0
            || self.coefficient_values_per_block == 0
            || self.lde_values_per_block == 0
            || self.ring_degree % u32::from(self.coefficient_values_per_block) != 0
            || self.domain_size_v2()? % u64::from(self.lde_values_per_block) != 0
            || self.domain_size_v2()? != u64::from(self.ring_degree) * 4
            || u64::from(self.query_count) >= self.domain_size_v2()? / 2
        {
            return Err(QPcsSpoolErrorV2::InvalidGeometry);
        }
        for (limb, modulus) in self.moduli.iter().enumerate() {
            if *modulus < 3 || *modulus >= 1_u64 << 62 || (*modulus).is_multiple_of(2) {
                return Err(QPcsSpoolErrorV2::InvalidGeometry);
            }
            let extension_adicity = (*modulus - 1)
                .trailing_zeros()
                .checked_add((*modulus + 1).trailing_zeros())
                .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
            if !is_prime_u64(*modulus)
                || self.moduli[..limb].contains(modulus)
                || extension_adicity < u32::from(self.domain_log)
            {
                return Err(QPcsSpoolErrorV2::InvalidGeometry);
            }
        }
        self.coefficient_slot_count_v2()?;
        self.lde_slot_count_v2()?;
        Ok(())
    }

    fn limb_count_v2(self) -> Result<u8, QPcsSpoolErrorV2> {
        u8::try_from(self.moduli.len()).map_err(|_| QPcsSpoolErrorV2::InvalidGeometry)
    }

    fn domain_size_v2(self) -> Result<u64, QPcsSpoolErrorV2> {
        1_u64
            .checked_shl(u32::from(self.domain_log))
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)
    }

    fn coefficient_blocks_per_component_v2(self) -> Result<u64, QPcsSpoolErrorV2> {
        if self.coefficient_values_per_block == 0 {
            return Err(QPcsSpoolErrorV2::InvalidGeometry);
        }
        Ok(u64::from(self.ring_degree) / u64::from(self.coefficient_values_per_block))
    }

    fn coefficient_block_bytes_v2(self) -> Result<u64, QPcsSpoolErrorV2> {
        u64::from(self.coefficient_values_per_block)
            .checked_mul(BASE_FIELD_WIRE_BYTES_V2)
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)
    }

    fn coefficient_slot_count_v2(self) -> Result<u64, QPcsSpoolErrorV2> {
        u64::try_from(self.moduli.len())
            .ok()
            .and_then(|value| value.checked_mul(u64::from(OPENING_REPETITIONS_V2)))
            .and_then(|value| value.checked_mul(self.coefficient_blocks_per_component_v2().ok()?))
            .and_then(|value| value.checked_mul(u64::from(COEFFICIENT_COMPONENTS_V2)))
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)
    }

    fn lde_blocks_per_column_v2(self) -> Result<u64, QPcsSpoolErrorV2> {
        if self.lde_values_per_block == 0 {
            return Err(QPcsSpoolErrorV2::InvalidGeometry);
        }
        Ok(self.domain_size_v2()? / u64::from(self.lde_values_per_block))
    }

    fn lde_block_bytes_v2(self) -> Result<u64, QPcsSpoolErrorV2> {
        u64::from(self.lde_values_per_block)
            .checked_mul(FQ2_WIRE_BYTES_V2)
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)
    }

    fn lde_column_count_v2(self) -> Result<u64, QPcsSpoolErrorV2> {
        u64::try_from(self.moduli.len())
            .ok()
            .and_then(|value| value.checked_mul(u64::from(FIXED_ROW_COUNT_V2)))
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)
    }

    fn lde_slot_count_v2(self) -> Result<u64, QPcsSpoolErrorV2> {
        let columns = self.lde_column_count_v2()?;
        let blocks = self.lde_blocks_per_column_v2()?;
        columns
            .checked_mul(blocks)
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoefficientComponentV2 {
    ProductLow,
    ProductHighWithTopZero,
    QuotientWithTopZero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CoefficientCoordinateV2 {
    limb: u8,
    repetition: u8,
    block: u64,
    component: CoefficientComponentV2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LdeRowRoleV2 {
    Product,
    Quotient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LdeCoordinateV2 {
    limb: u8,
    repetition: u8,
    role: LdeRowRoleV2,
    block: u64,
}

fn coefficient_coordinate_v2(
    geometry: SpoolGeometryV2,
    slot: u64,
) -> Result<CoefficientCoordinateV2, QPcsSpoolErrorV2> {
    if slot >= geometry.coefficient_slot_count_v2()? {
        return Err(QPcsSpoolErrorV2::ExtraCoefficientBlock);
    }
    let component = match slot % u64::from(COEFFICIENT_COMPONENTS_V2) {
        0 => CoefficientComponentV2::ProductLow,
        1 => CoefficientComponentV2::ProductHighWithTopZero,
        2 => CoefficientComponentV2::QuotientWithTopZero,
        _ => return Err(QPcsSpoolErrorV2::InvalidGeometry),
    };
    let pair_and_block = slot / u64::from(COEFFICIENT_COMPONENTS_V2);
    let blocks = geometry.coefficient_blocks_per_component_v2()?;
    let pair = pair_and_block / blocks;
    Ok(CoefficientCoordinateV2 {
        limb: u8::try_from(pair / u64::from(OPENING_REPETITIONS_V2))
            .map_err(|_| QPcsSpoolErrorV2::InvalidGeometry)?,
        repetition: u8::try_from(pair % u64::from(OPENING_REPETITIONS_V2))
            .map_err(|_| QPcsSpoolErrorV2::InvalidGeometry)?,
        block: pair_and_block % blocks,
        component,
    })
}

fn lde_coordinate_v2(
    geometry: SpoolGeometryV2,
    slot: u64,
) -> Result<LdeCoordinateV2, QPcsSpoolErrorV2> {
    if slot >= geometry.lde_slot_count_v2()? {
        return Err(QPcsSpoolErrorV2::ExtraLdeBlock);
    }
    let columns = geometry.lde_column_count_v2()?;
    let column = slot % columns;
    let row = column % u64::from(FIXED_ROW_COUNT_V2);
    Ok(LdeCoordinateV2 {
        limb: u8::try_from(column / u64::from(FIXED_ROW_COUNT_V2))
            .map_err(|_| QPcsSpoolErrorV2::InvalidGeometry)?,
        repetition: u8::try_from(row / u64::from(ROWS_PER_REPETITION_V2))
            .map_err(|_| QPcsSpoolErrorV2::InvalidGeometry)?,
        role: if row.is_multiple_of(u64::from(ROWS_PER_REPETITION_V2)) {
            LdeRowRoleV2::Product
        } else {
            LdeRowRoleV2::Quotient
        },
        block: slot / columns,
    })
}

fn parameter_digest_v2(geometry: SpoolGeometryV2) -> Result<[u8; 32], QPcsSpoolErrorV2> {
    FixedTenRowParameterDescriptorV2::from_geometry_v2(geometry)?.digest_v2(geometry.moduli)
}

fn mapping_digest_v2(
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
    coefficient: bool,
) -> Result<[u8; 32], QPcsSpoolErrorV2> {
    geometry.validate_v2()?;
    let mut hash = Keccak256::new();
    if coefficient {
        hash.update(COEFFICIENT_MAPPING_DOMAIN_V2);
    } else {
        hash.update(LDE_MAPPING_DOMAIN_V2);
    }
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&geometry.ring_degree.to_be_bytes());
    hash.update(&[geometry.limb_count_v2()?]);
    hash.update(&[OPENING_REPETITIONS_V2]);
    hash.update(&[FIXED_ROW_COUNT_V2]);
    if coefficient {
        hash.update(&[COEFFICIENT_COMPONENTS_V2]);
        hash.update(&geometry.coefficient_values_per_block.to_be_bytes());
        hash.update(
            &geometry
                .coefficient_blocks_per_component_v2()?
                .to_be_bytes(),
        );
        let slot_count = geometry.coefficient_slot_count_v2()?;
        hash.update(&slot_count.to_be_bytes());
        hash.update(&geometry.coefficient_block_bytes_v2()?.to_be_bytes());
        hash.update(COEFFICIENT_SLOT_FORMULA_V2);
        hash.update(COEFFICIENT_ENCODING_V2);
        hash.update(COEFFICIENT_COORDINATE_ENUMERATION_DOMAIN_V2);
        hash.update(&slot_count.to_be_bytes());
        for slot in 0..slot_count {
            let coordinate = coefficient_coordinate_v2(geometry, slot)?;
            let component = match coordinate.component {
                CoefficientComponentV2::ProductLow => 0,
                CoefficientComponentV2::ProductHighWithTopZero => 1,
                CoefficientComponentV2::QuotientWithTopZero => 2,
            };
            hash.update(&slot.to_be_bytes());
            hash.update(&[coordinate.limb]);
            hash.update(&[coordinate.repetition]);
            hash.update(&[component]);
            hash.update(&coordinate.block.to_be_bytes());
        }
    } else {
        hash.update(&[ROWS_PER_REPETITION_V2]);
        hash.update(&geometry.lde_values_per_block.to_be_bytes());
        hash.update(&geometry.lde_blocks_per_column_v2()?.to_be_bytes());
        let slot_count = geometry.lde_slot_count_v2()?;
        hash.update(&slot_count.to_be_bytes());
        hash.update(&geometry.lde_block_bytes_v2()?.to_be_bytes());
        hash.update(LDE_SLOT_FORMULA_V2);
        hash.update(LDE_ENCODING_V2);
        hash.update(LDE_COORDINATE_ENUMERATION_DOMAIN_V2);
        hash.update(&slot_count.to_be_bytes());
        for slot in 0..slot_count {
            let coordinate = lde_coordinate_v2(geometry, slot)?;
            let role = match coordinate.role {
                LdeRowRoleV2::Product => 0,
                LdeRowRoleV2::Quotient => 1,
            };
            hash.update(&slot.to_be_bytes());
            hash.update(&[coordinate.limb]);
            hash.update(&[coordinate.repetition]);
            hash.update(&[role]);
            hash.update(&coordinate.block.to_be_bytes());
        }
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    Ok(digest)
}

#[derive(Clone, Copy)]
struct PublicSpoolContextV2 {
    sealed_source_transcript_digest: [u8; 32],
    source_algebra_binding_digest: [u8; 32],
}

impl PublicSpoolContextV2 {
    fn validate_v2(self) -> Result<(), QPcsSpoolErrorV2> {
        if self.sealed_source_transcript_digest == [0; 32]
            || self.source_algebra_binding_digest == [0; 32]
        {
            return Err(QPcsSpoolErrorV2::InertPublicContext);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum SpoolRoleV2 {
    Coefficients = 1,
    Lde = 2,
}

fn context_digest_v2(
    role: SpoolRoleV2,
    parameter_digest: [u8; 32],
    mapping_digest: [u8; 32],
    context: PublicSpoolContextV2,
) -> Result<[u8; 32], QPcsSpoolErrorV2> {
    context.validate_v2()?;
    let mut frame = Vec::with_capacity(CONTEXT_DOMAIN_V2.len() + 130);
    frame.extend_from_slice(CONTEXT_DOMAIN_V2);
    frame.push(Q_PCS_SPOOL_VERSION_V2);
    frame.push(role as u8);
    frame.extend_from_slice(&parameter_digest);
    frame.extend_from_slice(&mapping_digest);
    frame.extend_from_slice(&context.sealed_source_transcript_digest);
    frame.extend_from_slice(&context.source_algebra_binding_digest);
    let digest = keccak256(&frame);
    if digest == [0; 32] {
        return Err(QPcsSpoolErrorV2::InertPublicContext);
    }
    Ok(digest)
}

enum AuthenticatedReplayPermitV2 {
    Production {
        source_aggregation: Infallible,
        algebra_verification: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

struct LiveSpoolWritersV2 {
    coefficient: ConfidentialSpoolWriterV1,
    lde: ConfidentialSpoolWriterV1,
    next_coefficient_slot: u64,
    replay_permit: AuthenticatedReplayPermitV2,
}

struct QPcsSpoolWriterV2 {
    live: Option<LiveSpoolWritersV2>,
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
    coefficient_context_digest: [u8; 32],
    lde_context_digest: [u8; 32],
}

impl QPcsSpoolWriterV2 {
    fn create_in_v2(
        directory: &Path,
        context: PublicSpoolContextV2,
        replay_permit: AuthenticatedReplayPermitV2,
    ) -> Result<Self, QPcsSpoolErrorV2> {
        Self::create_with_geometry_v2(
            directory,
            SpoolGeometryV2::release_v2(),
            context,
            replay_permit,
        )
    }

    fn create_with_geometry_v2(
        directory: &Path,
        geometry: SpoolGeometryV2,
        context: PublicSpoolContextV2,
        replay_permit: AuthenticatedReplayPermitV2,
    ) -> Result<Self, QPcsSpoolErrorV2> {
        geometry.validate_v2()?;
        let parameter_digest = parameter_digest_v2(geometry)?;
        let coefficient_mapping = mapping_digest_v2(geometry, parameter_digest, true)?;
        let lde_mapping = mapping_digest_v2(geometry, parameter_digest, false)?;
        let coefficient_context_digest = context_digest_v2(
            SpoolRoleV2::Coefficients,
            parameter_digest,
            coefficient_mapping,
            context,
        )?;
        let lde_context_digest =
            context_digest_v2(SpoolRoleV2::Lde, parameter_digest, lde_mapping, context)?;
        if coefficient_context_digest == lde_context_digest {
            return Err(QPcsSpoolErrorV2::InvalidGeometry);
        }

        let coefficient_layout = ConfidentialSpoolLayoutV1::new_v1(
            geometry.coefficient_slot_count_v2()?,
            geometry.coefficient_block_bytes_v2()?,
            coefficient_context_digest,
        )?;
        let lde_layout = ConfidentialSpoolLayoutV1::new_v1(
            geometry.lde_slot_count_v2()?,
            geometry.lde_block_bytes_v2()?,
            lde_context_digest,
        )?;
        if geometry.ring_degree == ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u32
            && geometry.moduli == RELEASE_MODULI_V1.as_slice()
            && (coefficient_layout.slot_count_v1() != RELEASE_COEFFICIENT_SLOTS_V2
                || coefficient_layout.plaintext_len_v1() != RELEASE_COEFFICIENT_BLOCK_BYTES_V2
                || coefficient_layout.file_len_v1() != RELEASE_COEFFICIENT_FILE_BYTES_V2
                || lde_layout.slot_count_v1() != RELEASE_LDE_SLOTS_V2
                || lde_layout.plaintext_len_v1() != RELEASE_LDE_BLOCK_BYTES_V2
                || lde_layout.file_len_v1() != RELEASE_LDE_FILE_BYTES_V2)
        {
            return Err(QPcsSpoolErrorV2::InvalidGeometry);
        }

        let coefficient = ConfidentialSpoolWriterV1::create_in_v1(directory, coefficient_layout)?;
        let lde = ConfidentialSpoolWriterV1::create_in_v1(directory, lde_layout)?;
        Ok(Self {
            live: Some(LiveSpoolWritersV2 {
                coefficient,
                lde,
                next_coefficient_slot: 0,
                replay_permit,
            }),
            geometry,
            parameter_digest,
            coefficient_context_digest,
            lde_context_digest,
        })
    }

    fn push_coefficient_block_v2(
        &mut self,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), QPcsSpoolErrorV2> {
        let mut live = self.live.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;
        let slot = live.next_coefficient_slot;
        if slot >= self.geometry.coefficient_slot_count_v2()? {
            return Err(QPcsSpoolErrorV2::ExtraCoefficientBlock);
        }
        let coordinate = coefficient_coordinate_v2(self.geometry, slot)?;
        validate_coefficient_chunk_v2(self.geometry, coordinate, &chunk)?;
        live.coefficient.write_slot_v1(slot, chunk)?;
        live.next_coefficient_slot = slot
            .checked_add(1)
            .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
        self.live = Some(live);
        Ok(())
    }

    #[cfg(test)]
    fn panic_after_take_for_test_v2(&mut self) {
        let _live = self.live.take().expect("live test writer");
        panic!("intentional qPCS spool unwind test");
    }
}

fn validate_coefficient_chunk_v2(
    geometry: SpoolGeometryV2,
    coordinate: CoefficientCoordinateV2,
    chunk: &ConfidentialSpoolChunkV1,
) -> Result<(), QPcsSpoolErrorV2> {
    if chunk.len_v1() != geometry.coefficient_block_bytes_v2()? {
        return Err(QPcsSpoolErrorV2::InvalidChunkLength);
    }
    let modulus = *geometry
        .moduli
        .get(usize::from(coordinate.limb))
        .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
    let bytes = chunk.as_slice_v1();
    for (index, encoded) in bytes
        .chunks_exact(BASE_FIELD_WIRE_BYTES_V2 as usize)
        .enumerate()
    {
        let value = u64::from_be_bytes(
            encoded
                .try_into()
                .map_err(|_| QPcsSpoolErrorV2::InvalidChunkLength)?,
        );
        if value >= modulus {
            return Err(QPcsSpoolErrorV2::NonCanonicalResidue);
        }
        let padded_component = matches!(
            coordinate.component,
            CoefficientComponentV2::ProductHighWithTopZero
                | CoefficientComponentV2::QuotientWithTopZero
        );
        if padded_component
            && coordinate.block + 1 == geometry.coefficient_blocks_per_component_v2()?
            && index + 1 == usize::from(geometry.coefficient_values_per_block)
            && value != 0
        {
            return Err(QPcsSpoolErrorV2::NonZeroTopPadding);
        }
    }
    Ok(())
}

fn validate_lde_chunk_v2(
    geometry: SpoolGeometryV2,
    coordinate: LdeCoordinateV2,
    chunk: &ConfidentialSpoolChunkV1,
) -> Result<(), QPcsSpoolErrorV2> {
    if chunk.len_v1() != geometry.lde_block_bytes_v2()? {
        return Err(QPcsSpoolErrorV2::InvalidChunkLength);
    }
    let modulus = *geometry
        .moduli
        .get(usize::from(coordinate.limb))
        .ok_or(QPcsSpoolErrorV2::InvalidGeometry)?;
    for encoded in chunk.as_slice_v1().chunks_exact(FQ2_WIRE_BYTES_V2 as usize) {
        let c0 = u64::from_be_bytes(
            encoded[..BASE_FIELD_WIRE_BYTES_V2 as usize]
                .try_into()
                .map_err(|_| QPcsSpoolErrorV2::InvalidChunkLength)?,
        );
        let c1 = u64::from_be_bytes(
            encoded[BASE_FIELD_WIRE_BYTES_V2 as usize..]
                .try_into()
                .map_err(|_| QPcsSpoolErrorV2::InvalidChunkLength)?,
        );
        if c0 >= modulus || c1 >= modulus {
            return Err(QPcsSpoolErrorV2::NonCanonicalResidue);
        }
    }
    Ok(())
}

fn snapshot_binding_digest_v2(
    parameter_digest: [u8; 32],
    coefficient_context_digest: [u8; 32],
    lde_context_digest: [u8; 32],
    coefficient_snapshot_digest: [u8; 32],
    lde_snapshot_digest: [u8; 32],
) -> Result<[u8; 32], QPcsSpoolErrorV2> {
    let mut frame = Vec::with_capacity(SNAPSHOT_BINDING_DOMAIN_V2.len() + 161);
    frame.extend_from_slice(SNAPSHOT_BINDING_DOMAIN_V2);
    frame.push(Q_PCS_SPOOL_VERSION_V2);
    frame.extend_from_slice(&parameter_digest);
    frame.extend_from_slice(&coefficient_context_digest);
    frame.extend_from_slice(&lde_context_digest);
    frame.extend_from_slice(&coefficient_snapshot_digest);
    frame.extend_from_slice(&lde_snapshot_digest);
    let digest = keccak256(&frame);
    if digest == [0; 32] {
        return Err(QPcsSpoolErrorV2::InvalidGeometry);
    }
    Ok(digest)
}

#[path = "phase23_rns_link_q_pcs_spool/replay_v2.rs"]
mod replay_v2;

#[cfg(test)]
#[path = "phase23_rns_link_q_pcs_spool_tests.rs"]
mod tests;

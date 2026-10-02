//! Exact sparse-wire acceptance and original Processed-key polynomial/range oracles.
//!
//! These tests use tiny plaintext frames. They do not authenticate artifacts, qualify proof
//! consumers, measure process memory or claim device or production readiness.

use super::indexed::IndexedStructuredProvingKeyV1;
use super::*;
use crate::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{
        Advice, Column, ConstraintSystem, Error, Expression, Fixed, Instance, Selector, keygen_pk2,
    },
    poly::{Rotation, commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
};
use std::{
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Clone)]
struct ScanCircuit<F: Field, const EMPTY: bool>(PhantomData<F>);
#[derive(Clone)]
struct ScanConfig {
    advice: Vec<Column<Advice>>,
    fixed: Vec<Column<Fixed>>,
    instance: Vec<Column<Instance>>,
    selector: Vec<Selector>,
}
impl<F: PrimeField, const EMPTY: bool> Circuit<F> for ScanCircuit<F, EMPTY> {
    type Config = ScanConfig;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self(PhantomData)
    }
    fn configure(cs: &mut ConstraintSystem<F>) -> ScanConfig {
        if EMPTY {
            return ScanConfig {
                advice: vec![],
                fixed: vec![],
                instance: vec![],
                selector: vec![],
            };
        }
        let advice = vec![cs.advice_column(), cs.advice_column()];
        let fixed = vec![cs.fixed_column(), cs.fixed_column(), cs.fixed_column()];
        let instance = vec![cs.instance_column()];
        let selector = vec![cs.selector(), cs.selector(), cs.complex_selector()];
        for column in &advice {
            cs.enable_equality(*column);
        }
        cs.enable_equality(fixed[0]);
        cs.enable_equality(instance[0]);
        cs.create_gate("indexed scan selector and rotations", |meta| {
            let q = meta.query_selector(selector[0]);
            let x = meta.query_advice(advice[0], Rotation::cur());
            let next = meta.query_advice(advice[1], Rotation::next());
            let complex = meta.query_selector(selector[2]);
            let binary = meta.query_fixed(fixed[1], Rotation::cur());
            vec![
                q * (x - next),
                complex * (binary.clone() * (binary - Expression::Constant(F::ONE))),
            ]
        });
        ScanConfig {
            advice,
            fixed,
            instance,
            selector,
        }
    }
    fn synthesize(&self, config: ScanConfig, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        if EMPTY {
            return Ok(());
        }
        let first = layouter.assign_region(
            || "scan cross-row/cross-column cycles",
            |mut region| {
                let mut cells = Vec::new();
                for row in 0..8 {
                    if row < 6 {
                        config.selector[0].enable(&mut region, row)?;
                    }
                    if row % 2 == 0 {
                        config.selector[2].enable(&mut region, row)?;
                    }
                    let left =
                        region.assign_advice(config.advice[0], row, Value::known(F::from(3)));
                    left.copy_advice(&mut region, config.advice[1], row);
                    cells.push(left.cell());
                    let fixed = region.assign_fixed(config.fixed[0], row, F::from(3));
                    if row == 0 {
                        region.constrain_equal(cells[0], fixed);
                    }
                    region.assign_fixed(config.fixed[1], row, F::from((row % 2) as u64));
                }
                region.constrain_equal(cells[0], cells[3]);
                region.constrain_equal(cells[3], cells[7]);
                Ok(cells[0])
            },
        )?;
        layouter.constrain_instance(first, config.instance[0], 0);
        Ok(())
    }
}
fn fixture<C: SerdeCurveAffine, const EMPTY: bool>(
    k: u32,
    compressed: bool,
    arbitrary_masks: bool,
) -> (ProvingKey<C>, Vec<u8>)
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(k);
    let mut pk = keygen_pk2(
        &params,
        &ScanCircuit::<C::Scalar, EMPTY>(PhantomData),
        compressed,
    )
    .unwrap();
    if arbitrary_masks {
        for (mask, polynomial) in [&mut pk.l0, &mut pk.l_last, &mut pk.l_active_row]
            .into_iter()
            .enumerate()
        {
            for (row, value) in polynomial.values.iter_mut().enumerate() {
                *value = C::Scalar::from((1 + row * 7 + mask * 19) as u64);
            }
        }
    }
    let mut bytes = Vec::new();
    pk.write_structured_v1(&mut bytes).unwrap();
    (pk, bytes)
}
fn index<C: SerdeCurveAffine, const EMPTY: bool, R: Read, W: Write>(
    reader: &mut R,
    k: u32,
    length: u64,
    writer: &mut W,
) -> io::Result<IndexedStructuredProvingKeyV1<C>>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    IndexedStructuredProvingKeyV1::<C>::read_checked::<_, _, ScanCircuit<C::Scalar, EMPTY>>(
        reader,
        k,
        length,
        #[cfg(feature = "circuit-params")]
        (),
        writer,
    )
}
fn processed<C: SerdeCurveAffine, const EMPTY: bool>(pk: &ProvingKey<C>) -> ProvingKey<C>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    // Explicit real Processed-PK test oracle; this is never a sparse-format fallback decoder.
    ProvingKey::<C>::read_checked::<_, ScanCircuit<C::Scalar, EMPTY>>(
        &mut pk.to_bytes(SerdeFormat::Processed).as_slice(),
        SerdeFormat::Processed,
        pk.vk.domain.k(),
        #[cfg(feature = "circuit-params")]
        (),
    )
    .unwrap()
}
fn dense<C: SerdeCurveAffine, const EMPTY: bool>(
    bytes: &[u8],
    k: u32,
    length: u64,
) -> io::Result<ProvingKey<C>>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    ProvingKey::<C>::read_structured_v1_checked::<_, ScanCircuit<C::Scalar, EMPTY>>(
        &mut &bytes[..],
        k,
        length,
        #[cfg(feature = "circuit-params")]
        (),
    )
}
fn scalar<F: PrimeField>(bytes: &[u8]) -> F {
    let mut repr = F::Repr::default();
    assert_eq!(bytes.len(), repr.as_ref().len());
    repr.as_mut().copy_from_slice(bytes);
    Option::<F>::from(F::from_repr(repr)).unwrap()
}
fn scalar_vec<F: PrimeField>(bytes: &[u8]) -> Vec<F> {
    bytes
        .chunks_exact(F::Repr::default().as_ref().len())
        .map(scalar::<F>)
        .collect()
}
fn range_bytes<'a>(bytes: &'a [u8], r: CheckedRange) -> &'a [u8] {
    &bytes[r.offset as usize..(r.offset + r.length) as usize]
}
fn explicit_column_targets(
    bytes: &[u8],
    record: &PermutationRecord,
    column: usize,
    n: usize,
) -> Vec<u32> {
    let mut targets = (column * n..(column + 1) * n)
        .map(|v| v as u32)
        .collect::<Vec<_>>();
    match record.mode {
        Mode::Identity => assert_eq!(record.payload.length, 0),
        Mode::Sparse => {
            let pairs = range_bytes(bytes, record.targets);
            for pair in pairs.chunks_exact(8) {
                let row = u32::from_le_bytes(pair[..4].try_into().unwrap()) as usize;
                targets[row] = u32::from_le_bytes(pair[4..].try_into().unwrap());
            }
        }
        Mode::Bitmap => {
            let mut values = range_bytes(bytes, record.targets).chunks_exact(4);
            let bitmap = range_bytes(bytes, record.bitmap);
            for row in 0..n {
                if bitmap[row / 8] >> (row % 8) & 1 != 0 {
                    targets[row] = u32::from_le_bytes(values.next().unwrap().try_into().unwrap());
                }
            }
            assert!(values.next().is_none());
        }
        Mode::Dense => {
            targets = range_bytes(bytes, record.targets)
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect();
        }
    }
    assert_eq!(targets.len(), n);
    targets
}
fn encoded_target_offsets(m: &StructuredMetadata) -> Vec<usize> {
    m.permutations
        .iter()
        .flat_map(|r| {
            let (width, first) = if r.mode == Mode::Sparse {
                (8, 4)
            } else {
                (4, 0)
            };
            (0..r.targets.length as usize / width)
                .map(move |i| r.targets.offset as usize + i * width + first)
        })
        .collect()
}

fn check_index<C: SerdeCurveAffine>(
    pk: &ProvingKey<C>,
    bytes: &[u8],
    actual: &IndexedStructuredProvingKeyV1<C>,
) where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let n = 1usize << pk.vk.domain.k();
    let width = <C::Scalar as PrimeField>::Repr::default().as_ref().len();
    assert_eq!(actual.rows(), n);
    assert_eq!(actual.frame_bytes(), bytes.len() as u64);
    assert_eq!(
        actual.get_vk().to_bytes(SerdeFormat::Processed),
        pk.vk.to_bytes(SerdeFormat::Processed)
    );
    assert_eq!(actual.get_vk().transcript_repr(), pk.vk.transcript_repr());
    let metadata = actual.metadata();
    assert_eq!(metadata.rows, n);
    assert_eq!(metadata.frame_bytes, bytes.len() as u64);
    let mut cursor = 56 + pk.vk.to_bytes(SerdeFormat::Processed).len();
    for (range, original) in metadata
        .masks
        .iter()
        .zip([&pk.l0, &pk.l_last, &pk.l_active_row])
    {
        assert_eq!(
            u32::from_be_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize,
            n
        );
        cursor += 4;
        assert_eq!(
            (range.offset, range.length),
            (cursor as u64, (n * width) as u64)
        );
        assert_eq!(
            scalar_vec::<C::Scalar>(range_bytes(bytes, *range)),
            original.to_vec()
        );
        cursor += n * width;
    }
    assert_eq!(metadata.fixed.len(), pk.fixed_values.len());
    cursor += 4;
    for ((record, lagrange), coeff) in metadata
        .fixed
        .iter()
        .zip(&pk.fixed_values)
        .zip(&pk.fixed_polys)
    {
        let first = lagrange[0];
        let mode = if lagrange.iter().all(|x| *x == first) {
            0
        } else if lagrange
            .iter()
            .all(|x| *x == C::Scalar::ZERO || *x == C::Scalar::ONE)
        {
            1
        } else {
            2
        };
        assert_eq!(record.mode, mode);
        assert_eq!(bytes[cursor], mode);
        cursor += 1;
        assert_eq!(record.payload.offset, cursor as u64);
        let payload = range_bytes(bytes, record.payload);
        let values = match mode {
            0 => {
                assert_eq!(payload.len(), width);
                vec![scalar::<C::Scalar>(payload); n]
            }
            1 => {
                assert_eq!(payload.len(), n.div_ceil(8));
                (0..n)
                    .map(|row| C::Scalar::from(((payload[row / 8] >> (row % 8)) & 1) as u64))
                    .collect()
            }
            2 => {
                assert_eq!(payload.len(), width * n);
                scalar_vec::<C::Scalar>(payload)
            }
            _ => unreachable!(),
        };
        assert_eq!(values, lagrange.to_vec());
        assert_eq!(
            pk.vk
                .domain
                .lagrange_to_coeff(pk.vk.domain.lagrange_from_vec(values))
                .to_vec(),
            coeff.to_vec()
        );
        cursor += record.payload.length as usize;
    }
    assert_eq!(
        metadata.permutation_columns,
        pk.permutation.permutations.len()
    );
    cursor += 4;
    assert_eq!(metadata.permutation_targets.offset, cursor as u64);
    assert_eq!(
        metadata.permutation_targets.offset + metadata.permutation_targets.length,
        bytes.len() as u64
    );
    let mut nontrivial = false;
    assert_eq!(metadata.permutations.len(), metadata.permutation_columns);
    for (column, record) in metadata.permutations.iter().enumerate() {
        assert_eq!(bytes[cursor], record.mode as u8);
        cursor += 1;
        assert_eq!(record.payload.offset, cursor as u64);
        let targets = explicit_column_targets(bytes, record, column, n);
        let exceptions = targets
            .iter()
            .enumerate()
            .filter(|(row, target)| **target as usize != column * n + *row)
            .count();
        assert_eq!(record.exceptions as usize, exceptions);
        let encoding = canonical_permutation_column_encoding(n as u64, exceptions as u64).unwrap();
        assert_eq!(record.mode, encoding.mode);
        assert_eq!(record.payload.length, encoding.payload_bytes);
        nontrivial |= exceptions != 0;
        let values = targets
            .iter()
            .map(|id| {
                pk.vk
                    .domain
                    .get_omega()
                    .pow_vartime([(*id as usize % n) as u64])
                    * C::Scalar::DELTA.pow_vartime([(*id as usize / n) as u64])
            })
            .collect::<Vec<_>>();
        assert_eq!(values, pk.permutation.permutations[column].to_vec());
        assert_eq!(
            pk.vk
                .domain
                .lagrange_to_coeff(pk.vk.domain.lagrange_from_vec(values))
                .to_vec(),
            pk.permutation.polys[column].to_vec()
        );
        if record.mode == Mode::Bitmap {
            let bitmap = range_bytes(bytes, record.bitmap);
            let ranks = std::iter::once(0)
                .chain(bitmap.chunks(512).scan(0_u32, |rank, chunk| {
                    *rank += chunk.iter().map(|b| b.count_ones()).sum::<u32>();
                    Some(*rank)
                }))
                .collect::<Vec<_>>();
            assert_eq!(record.ranks, ranks);
        } else {
            assert!(record.ranks.is_empty());
        }
        cursor += record.payload.length as usize;
    }
    assert!(metadata.permutation_columns == 0 || nontrivial);
    let charged = std::mem::size_of::<StructuredMetadata>()
        + metadata.fixed.capacity() * std::mem::size_of::<FixedRecord>()
        + metadata.permutations.capacity() * std::mem::size_of::<PermutationRecord>()
        + metadata
            .permutations
            .iter()
            .map(|r| r.ranks.capacity() * 4)
            .sum::<usize>();
    assert_eq!(actual.index_metadata_payload_bytes().unwrap(), charged);

    assert_eq!(cursor, bytes.len());
}
fn roundtrip<C: SerdeCurveAffine, const EMPTY: bool>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    for k in [4, 5] {
        for compressed in [false, true] {
            for arbitrary in [false, true] {
                let (pk, bytes) = fixture::<C, EMPTY>(k, compressed, arbitrary);
                let expected = processed::<C, EMPTY>(&pk);
                assert_eq!(
                    expected.to_bytes(SerdeFormat::Processed),
                    pk.to_bytes(SerdeFormat::Processed)
                );
                let restored = dense::<C, EMPTY>(&bytes, k, bytes.len() as u64).unwrap();
                assert_eq!(
                    restored.to_bytes(SerdeFormat::Processed),
                    pk.to_bytes(SerdeFormat::Processed)
                );
                let mut outer = bytes.clone();
                outer.extend_from_slice(b"outer-tail");
                let mut reader = outer.as_slice();
                let mut canonical = Vec::new();
                let actual =
                    index::<C, EMPTY, _, _>(&mut reader, k, bytes.len() as u64, &mut canonical)
                        .unwrap();
                assert_eq!(reader, b"outer-tail");
                assert_eq!(canonical, bytes);
                check_index(&pk, &bytes, &actual);
                if !EMPTY {
                    for mode in [0, 1, 2] {
                        assert!(
                            actual
                                .metadata()
                                .fixed
                                .iter()
                                .any(|record| record.mode == mode)
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn both_pasta_index_ranges_reconstruct_original_masks_fixed_modes_and_nontrivial_permutations_with_exact_canonical_bytes()
 {
    roundtrip::<EqAffine, false>();
    roundtrip::<EpAffine, false>();
    roundtrip::<EqAffine, true>();
    roundtrip::<EpAffine, true>();
}

fn rejection<C: SerdeCurveAffine>(bytes: &[u8], k: u32, length: u64, label: &str)
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let normal = dense::<C, false>(bytes, k, length)
        .err()
        .unwrap_or_else(|| panic!("dense accepted {label}"));
    let mut canonical = Vec::new();
    let actual = index::<C, false, _, _>(&mut &bytes[..], k, length, &mut canonical)
        .err()
        .unwrap_or_else(|| panic!("index accepted {label}"));
    assert!(matches!(
        normal.kind(),
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof | io::ErrorKind::InvalidInput
    ));
    assert_eq!(actual.kind(), normal.kind(), "index {label}");
    assert!(canonical.len() as u64 <= length);
}
fn malformed<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let (pk, bytes) = fixture::<C, false>(4, true, true);
    let length = bytes.len() as u64;
    let good = index::<C, false, _, _>(&mut bytes.as_slice(), 4, length, &mut Vec::new()).unwrap();
    let m = good.metadata();
    let mut cases = Vec::<(String, Vec<u8>, u32, u64)>::new();
    let mut retired = bytes.clone();
    retired[..16].copy_from_slice(b"Halo2StructPK1\0\0");
    cases.push(("retired structured magic".into(), retired, 4, length));
    for (label, offset) in [("magic", 0), ("curve", 16), ("encoded length", 48)] {
        let mut bad = bytes.clone();
        bad[offset] ^= 1;
        cases.push((label.into(), bad, 4, length));
    }
    for k in [3, 5, 32] {
        cases.push((format!("trusted k {k}"), bytes.clone(), k, length));
    }
    for bound in [0, 55, length - 1, length + 1, u64::MAX] {
        cases.push((
            format!("external frame bound {bound}"),
            bytes.clone(),
            4,
            bound,
        ));
    }
    for (mask, range) in m.masks.iter().enumerate() {
        let offset = range.offset as usize;
        for count in [0, 15, 17, u32::MAX] {
            let mut bad = bytes.clone();
            bad[offset - 4..offset].copy_from_slice(&count.to_be_bytes());
            cases.push((format!("mask {mask} count {count}"), bad, 4, length));
        }
        for scalar in [offset, offset + range.length as usize - 32] {
            let mut bad = bytes.clone();
            bad[scalar..scalar + 32].fill(255);
            cases.push((format!("mask {mask} scalar {scalar}"), bad, 4, length));
        }
    }
    let fixed_count = (m.masks[2].offset + m.masks[2].length) as usize;
    let perm_count = m.permutation_targets.offset as usize - 4;
    for (label, offset) in [
        ("fixed count", fixed_count),
        ("permutation count", perm_count),
    ] {
        for value in [0, u32::MAX] {
            let mut bad = bytes.clone();
            bad[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
            cases.push((format!("{label} {value}"), bad, 4, length));
        }
    }
    for (column, record) in m.fixed.iter().enumerate() {
        let offset = record.payload.offset as usize;
        let mut bad = bytes.clone();
        bad[offset - 1] = 255;
        cases.push((format!("unknown mode {column}"), bad, 4, length));
    }
    let constant = m.fixed.iter().find(|r| r.mode == 0).unwrap();
    let binary = m.fixed.iter().find(|r| r.mode == 1).unwrap();
    let raw = m.fixed.iter().find(|r| r.mode == 2).unwrap();
    for record in [constant, raw] {
        for offset in [
            record.payload.offset as usize,
            (record.payload.offset + record.payload.length - 32) as usize,
        ] {
            let mut bad = bytes.clone();
            bad[offset..offset + 32].fill(255);
            cases.push((
                format!("noncanonical mode {} scalar {offset}", record.mode),
                bad,
                4,
                length,
            ));
        }
    }
    for fill in [0, 255] {
        let mut bad = bytes.clone();
        bad[binary.payload.offset as usize
            ..(binary.payload.offset + binary.payload.length) as usize]
            .fill(fill);
        cases.push((format!("nonminimal binary {fill}"), bad, 4, length));
    }
    for alternating in [false, true] {
        let mut bad = bytes.clone();
        for row in 0..16 {
            let start = raw.payload.offset as usize + row * 32;
            bad[start..start + 32].copy_from_slice(
                C::Scalar::from(if alternating { (row % 2) as u64 } else { 7 })
                    .to_repr()
                    .as_ref(),
            );
        }
        cases.push((format!("nonminimal raw {alternating}"), bad, 4, length));
    }
    let cells = (16 * m.permutation_columns) as u32;
    let target_offsets = encoded_target_offsets(m);
    assert!(target_offsets.len() >= 2);
    for offset in [target_offsets[0], *target_offsets.last().unwrap()] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&cells.to_le_bytes());
        cases.push((format!("out of range target {offset}"), bad, 4, length));
    }
    let mut bad = bytes.clone();
    bad.copy_within(
        target_offsets[0]..target_offsets[0] + 4,
        *target_offsets.last().unwrap(),
    );
    cases.push(("duplicate final target".into(), bad, 4, length));
    let mut longer = bytes.clone();
    longer.push(0);
    longer[48..56].copy_from_slice(&(length + 1).to_le_bytes());
    cases.push(("declared trailing byte".into(), longer, 4, length + 1));
    let mut maximum = bytes.clone();
    maximum[48..56].copy_from_slice(&u64::MAX.to_le_bytes());
    cases.push(("declared excessive frame".into(), maximum, 4, u64::MAX));
    let mut boundaries = vec![
        0,
        1,
        15,
        16,
        47,
        48,
        55,
        56,
        56 + pk.vk.to_bytes(SerdeFormat::Processed).len() - 1,
    ];
    for range in m
        .masks
        .iter()
        .chain(std::iter::once(&m.permutation_targets))
    {
        boundaries.extend([
            range.offset as usize - 1,
            range.offset as usize,
            (range.offset + range.length) as usize - 1,
        ]);
    }
    for record in &m.fixed {
        boundaries.extend([
            record.payload.offset as usize - 1,
            record.payload.offset as usize,
            (record.payload.offset + record.payload.length) as usize - 1,
        ]);
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    for end in boundaries {
        cases.push((
            format!("truncated at {end}"),
            bytes[..end].to_vec(),
            4,
            length,
        ));
    }
    assert_eq!(
        cases.len(),
        92,
        "original91 boundaries plus explicit retired-magic refusal"
    );
    for (label, bad, k, length) in cases {
        rejection::<C>(&bad, k, length, &label);
    }
    assert!(dense::<C, true>(&bytes, 4, length).is_err());
    assert!(index::<C, true, _, _>(&mut bytes.as_slice(), 4, length, &mut Vec::new()).is_err());
    // The wire preserves arbitrary canonical masks/constants and any complete bijection.
    // These inputs are format-valid; no new VK-commitment or regenerated-mask rule is allowed.
    for variation in 0..3 {
        let mut variant = bytes.clone();
        match variation {
            0 => variant[constant.payload.offset as usize..constant.payload.offset as usize + 32]
                .copy_from_slice(C::Scalar::from(91).to_repr().as_ref()),
            1 => {
                // Preserve bijection by an exact directed target swap and let the current
                // canonical writer select any changed mode/length; no fixed mode is assumed.
                let mut key = processed::<C, false>(&pk);
                key.permutation.permutations[0].values.swap(0, 1);
                key.permutation.polys[0] =
                    coefficients(&key.vk.domain, &key.permutation.permutations[0]).unwrap();
                variant.clear();
                key.write_structured_v1(&mut variant).unwrap();
            }
            2 => variant[m.masks[1].offset as usize..m.masks[1].offset as usize + 32]
                .copy_from_slice(C::Scalar::from(919).to_repr().as_ref()),
            _ => unreachable!(),
        }
        let expected = dense::<C, false>(&variant, 4, variant.len() as u64).unwrap();
        let mut canonical = Vec::new();
        let actual = index::<C, false, _, _>(
            &mut variant.as_slice(),
            4,
            variant.len() as u64,
            &mut canonical,
        )
        .unwrap();
        assert_eq!(canonical, variant);
        check_index(&expected, &variant, &actual);
    }
}
#[test]
fn both_pasta_index_and_dense_scanner_preserve_original_checked_reader_wire_rejections_and_canonical_acceptance()
 {
    malformed::<EqAffine>();
    malformed::<EpAffine>();
}

fn fixed_cases<F: PrimeField>() {
    for rows in [0, 1, 2, 7, 8, 9, 17] {
        let mut inputs = Vec::<Vec<u8>>::new();
        if rows > 0 {
            for variant in 0..5 {
                let values = (0..rows)
                    .map(|i| match variant {
                        0 => F::ZERO,
                        1 => F::ONE,
                        2 => F::from(7),
                        3 => F::from((i % 2) as u64),
                        _ => F::from((i * 3 + 2) as u64),
                    })
                    .collect::<Vec<_>>();
                let mut encoded = Vec::new();
                write_fixed(&mut encoded, &values).unwrap();
                let mut padded = encoded.clone();
                padded.extend_from_slice(&[91, 92]);
                let mode = if rows == 1 || variant < 3 {
                    CONSTANT
                } else if variant == 3 {
                    BITSET
                } else {
                    RAW
                };
                assert_eq!(
                    encoded[0], mode,
                    "fixed grammar mode at rows{rows}/variant{variant}"
                );
                assert_eq!(
                    encoded.len() as u64,
                    1 + fixed_payload_bytes::<F>(mode, rows).unwrap()
                );
                let mut source = padded.as_slice();
                assert_eq!(read_fixed::<F, _>(&mut source, rows).unwrap(), values);
                assert_eq!(source, &[91, 92]);
            }
        }
        inputs.push(vec![255]);
        inputs.push(vec![]);
        inputs.push(vec![0]);
        let mut noncanonical = vec![0];
        noncanonical.extend_from_slice(&vec![255; F::Repr::default().as_ref().len()]);
        inputs.push(noncanonical);
        for fill in [0, 255] {
            let mut v = vec![1];
            v.extend_from_slice(&vec![fill; rows.div_ceil(8)]);
            inputs.push(v);
        }
        for alternating in [false, true] {
            let mut v = vec![2];
            for row in 0..rows {
                v.extend_from_slice(
                    F::from(if alternating { (row % 2) as u64 } else { 7 })
                        .to_repr()
                        .as_ref(),
                );
            }
            inputs.push(v);
        }
        if rows > 0 && rows % 8 != 0 {
            let mut v = vec![1];
            v.extend_from_slice(&vec![0x55; rows.div_ceil(8)]);
            *v.last_mut().unwrap() |= 1 << (rows % 8);
            inputs.push(v);
        }
        for bytes in inputs {
            let required = match bytes.first() {
                Some(&CONSTANT) => 1 + scalar_bytes::<F>(),
                Some(&BITSET) => 1 + rows.div_ceil(8),
                Some(&RAW) => 1 + rows * scalar_bytes::<F>(),
                _ => 0,
            };
            let kind = if rows == 0
                || required == 0 && !bytes.is_empty()
                || bytes.len() >= required && required != 0
            {
                io::ErrorKind::InvalidData
            } else {
                io::ErrorKind::UnexpectedEof
            };
            let error = read_fixed::<F, _>(&mut bytes.as_slice(), rows)
                .err()
                .expect("reject nonminimal, padding, noncanonical or truncated fixed encoding");
            assert_eq!(error.kind(), kind);
        }
    }
}
#[test]
fn both_pasta_shared_fixed_scan_matches_original_modes_padding_and_noncanonical_scalars_at_partial_byte_shapes()
 {
    fixed_cases::<Fp>();
    fixed_cases::<Fq>();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IoFault {
    Error,
    Panic,
    Interrupted,
    Zero,
    Overcount,
}
struct Source<'a> {
    bytes: &'a [u8],
    position: usize,
    maximum: usize,
    fault: Option<(usize, IoFault)>,
    hit: bool,
}
impl Read for Source<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        if self.fault.is_some_and(|(at, _)| at == self.position) {
            let (_, fault) = self.fault.take().unwrap();
            self.hit = true;
            match fault {
                IoFault::Error => return Err(io::Error::other("injected source failure")),
                IoFault::Panic => panic!("injected source unwind"),
                IoFault::Interrupted => return Err(io::ErrorKind::Interrupted.into()),
                IoFault::Zero => return Ok(0),
                IoFault::Overcount => unreachable!(),
            }
        }
        let boundary = self.fault.map_or(self.bytes.len(), |(at, _)| at);
        let count = out
            .len()
            .min(self.maximum)
            .min(self.bytes.len() - self.position)
            .min(boundary - self.position);
        out[..count].copy_from_slice(&self.bytes[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}
struct Sink {
    bytes: Vec<u8>,
    maximum: usize,
    fault: Option<(usize, IoFault)>,
    hit: bool,
    flushes: usize,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.fault.is_some_and(|(at, _)| at == self.bytes.len()) {
            let (_, fault) = self.fault.take().unwrap();
            self.hit = true;
            match fault {
                IoFault::Error => return Err(io::Error::other("injected canonical sink failure")),
                IoFault::Panic => panic!("injected canonical sink unwind"),
                IoFault::Interrupted => return Err(io::ErrorKind::Interrupted.into()),
                IoFault::Zero => return Ok(0),
                IoFault::Overcount => return Ok(bytes.len() + 1),
            }
        }
        let boundary = self.fault.map_or(usize::MAX, |(at, _)| at);
        let count = bytes
            .len()
            .min(self.maximum)
            .min(boundary - self.bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}
fn io_boundaries<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let (pk, bytes) = fixture::<C, false>(4, true, false);
    let length = bytes.len() as u64;
    let mut outer = bytes.clone();
    outer.extend_from_slice(b"untouched");
    let info = index::<C, false, _, _>(&mut bytes.as_slice(), 4, length, &mut Vec::new()).unwrap();
    let m = info.metadata();
    let mut boundaries = vec![
        0,
        16,
        48,
        56,
        56 + pk.vk.to_bytes(SerdeFormat::Processed).len() - 1,
    ];
    for range in m
        .masks
        .iter()
        .chain(std::iter::once(&m.permutation_targets))
    {
        boundaries.extend([
            range.offset as usize - 4,
            range.offset as usize,
            (range.offset + range.length) as usize - 1,
        ]);
    }
    for record in &m.fixed {
        boundaries.extend([
            record.payload.offset as usize - 1,
            record.payload.offset as usize,
        ]);
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    assert_eq!(boundaries.len(), 29);
    // Single-byte successful I/O is independent of production write sizes or read_exact splits.
    let mut reader = Source {
        bytes: &outer,
        position: 0,
        maximum: 1,
        fault: None,
        hit: false,
    };
    let mut sink = Sink {
        bytes: vec![],
        maximum: 1,
        fault: None,
        hit: false,
        flushes: 0,
    };
    let actual = index::<C, false, _, _>(&mut reader, 4, length, &mut sink).unwrap();
    assert_eq!(reader.position, bytes.len());
    assert_eq!(&outer[reader.position..], b"untouched");
    assert_eq!(sink.bytes, bytes);
    check_index(&pk, &bytes, &actual);
    assert_eq!(sink.flushes, 0);
    for at in boundaries {
        for source in [false, true] {
            for fault in [
                IoFault::Error,
                IoFault::Panic,
                IoFault::Interrupted,
                IoFault::Zero,
                IoFault::Overcount,
            ] {
                if source && fault == IoFault::Overcount {
                    continue;
                }
                let mut reader = Source {
                    bytes: &outer,
                    position: 0,
                    maximum: 7,
                    fault: source.then_some((at, fault)),
                    hit: false,
                };
                let mut sink = Sink {
                    bytes: vec![],
                    maximum: 3,
                    fault: (!source).then_some((at, fault)),
                    hit: false,
                    flushes: 0,
                };
                let result = catch_unwind(AssertUnwindSafe(|| {
                    index::<C, false, _, _>(&mut reader, 4, length, &mut sink)
                }));
                assert!(
                    if source { reader.hit } else { sink.hit },
                    "unreached {source} {at} {fault:?}"
                );
                if fault == IoFault::Interrupted {
                    let actual = result.expect("Interrupted must be retried").unwrap();
                    assert_eq!(sink.bytes, bytes);
                    assert_eq!(reader.position, bytes.len());
                    check_index(&pk, &bytes, &actual);
                } else if fault == IoFault::Panic {
                    assert!(result.is_err());
                } else {
                    assert!(
                        result.unwrap().is_err(),
                        "usable index escaped failure {source} {at} {fault:?}"
                    );
                }
                assert!(reader.position <= bytes.len());
                assert!(sink.bytes.len() <= bytes.len());
                assert_eq!(sink.bytes, &bytes[..sink.bytes.len()]);
                assert_eq!(sink.flushes, 0);
            }
        }
    }
}
#[test]
fn both_pasta_index_read_and_canonical_sink_boundaries_propagate_errors_unwinds_and_short_io_without_returning_partial_indexes()
 {
    io_boundaries::<EqAffine>();
    io_boundaries::<EpAffine>();
}

fn shape_helpers<F: PrimeField>() {
    for (offset, length, frame, accepted) in [
        (0, 0, 0, true),
        (0, 1, 1, true),
        (1, 0, 1, true),
        (1, 1, 1, false),
        (u64::MAX, 1, u64::MAX, false),
        (u64::MAX, 0, u64::MAX, true),
    ] {
        assert_eq!(CheckedRange::new(offset, length, frame).is_ok(), accepted);
    }
    for (rows, columns, omega) in [
        (0, 1, F::ONE),
        (3, 1, F::ONE),
        (8, 1, F::ONE),
        (8, 1, F::ZERO),
        (1, 1, F::ZERO),
        (1, usize::MAX, F::ONE),
    ] {
        let after = permutation_cells(rows, columns, omega).err().unwrap();
        assert_eq!(after.kind(), io::ErrorKind::InvalidData);
    }
    for cells in [0, 1, 7, 8, 9, 17] {
        let mut seen = Seen::new(cells).unwrap();
        for target in (0..cells).rev() {
            seen.mark(target as u32).unwrap();
        }
        assert!(seen.mark(cells as u32).is_err());
        if cells > 0 {
            assert!(seen.mark(0).is_err());
        }
    }
    let mut duplicates = [
        (F::ONE.to_repr(), 0),
        (F::ZERO.to_repr(), 1),
        (F::ONE.to_repr(), 2),
    ];
    assert!(sorted_unique::<F>(&mut duplicates).is_err());
    let mut distinct = [(F::ONE.to_repr(), 0), (F::ZERO.to_repr(), 1)];
    sorted_unique::<F>(&mut distinct).unwrap();
    assert!(distinct[0].0.as_ref() < distinct[1].0.as_ref());
}
#[test]
fn both_pasta_index_range_and_permutation_helpers_refuse_overflow_wrong_roots_and_duplicate_labels()
{
    shape_helpers::<Fp>();
    shape_helpers::<Fq>();
}

#[path = "indexed_reads_tests.rs"]
mod indexed_reads_tests;

#[path = "indexed_snapshot_tests.rs"]
mod indexed_snapshot_tests;

#[path = "permutation_tests.rs"]
mod permutation_tests;

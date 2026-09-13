//! Independent original-codec acceptance and polynomial/range oracles for bounded key scanning.
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

// Frozen pre-refactor helper algorithms and checked reader. Only the reader's enclosing impl
// is replaced by a free generic function; it returns the original full ProvingKey. Its internal
// acceptance/reconstruction body is preserved independently of the shared production scanner.
#[allow(dead_code)]
mod original {
    use super::*;
    const MAGIC: &[u8; 16] = b"Halo2StructPK1\0\0";
    const HEADER_BYTES: u64 = 16 + 32 + 8;
    const CONSTANT: u8 = 0;
    const BITSET: u8 = 1;
    const RAW: u8 = 2;

    fn invalid(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, message)
    }

    /// Bind the curve equation, generator, both field moduli and field representation byte order.
    /// Every part is length-prefixed, in the fixed order below; the codec version is also explicit.
    fn curve_domain<C: SerdeCurveAffine>() -> [u8; 32] {
        let generator = C::generator().to_bytes();
        let a = C::a().to_repr();
        let b = C::b().to_repr();
        let base_one = C::Base::ONE.to_repr();
        let scalar_one = C::Scalar::ONE.to_repr();
        let mut digest = Blake2bParams::new()
            .hash_length(32)
            .personal(b"Halo2-PK-Struct1")
            .to_state();
        for bytes in [
            MAGIC.as_slice(),
            C::Base::MODULUS.as_bytes(),
            C::Scalar::MODULUS.as_bytes(),
            base_one.as_ref(),
            scalar_one.as_ref(),
            a.as_ref(),
            b.as_ref(),
            generator.as_ref(),
        ] {
            digest.update(&(bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        let mut result = [0; 32];
        result.copy_from_slice(digest.finalize().as_bytes());
        result
    }

    fn checked_rows<C: SerdeCurveAffine>(vk: &VerifyingKey<C>) -> io::Result<usize> {
        let k = vk.domain.k();
        let rows = 1_usize
            .checked_shl(k)
            .filter(|rows| k <= C::Scalar::S && u32::try_from(*rows).is_ok())
            .ok_or_else(|| invalid("structured key domain is unsupported"))?;
        if rows as u64 != vk.domain.get_n()
            || rows < vk.cs.minimum_rows()
            || vk.fixed_commitments.len() != vk.cs.num_fixed_columns
            || vk.permutation.commitments().len() != vk.cs.permutation.columns.len()
            || (vk.compress_selectors && vk.selectors.len() != vk.cs.num_selectors)
            || (!vk.compress_selectors && !vk.selectors.is_empty())
            || vk.selectors.iter().any(|selector| selector.len() != rows)
        {
            return Err(invalid(
                "structured key verification-key shape is inconsistent",
            ));
        }
        u32::try_from(vk.fixed_commitments.len())
            .map_err(|_| invalid("structured fixed-column count exceeds its wire bound"))?;
        u32::try_from(vk.cs.permutation.columns.len())
            .map_err(|_| invalid("structured permutation count exceeds its wire bound"))?;
        Ok(rows)
    }

    #[derive(Default)]
    struct ByteCounter(u64);

    impl Write for ByteCounter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0 = self
                .0
                .checked_add(
                    u64::try_from(bytes.len()).map_err(|_| invalid("byte count overflow"))?,
                )
                .ok_or_else(|| invalid("byte count overflow"))?;
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn coefficients<F: WithSmallOrderMulGroup<3>>(
        domain: &EvaluationDomain<F>,
        polynomial: &Polynomial<F, LagrangeCoeff>,
    ) -> io::Result<Polynomial<F, Coeff>> {
        let mut values = Vec::new();
        values.try_reserve_exact(polynomial.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                "cannot reserve structured coefficient copy",
            )
        })?;
        values.extend_from_slice(polynomial);
        Ok(domain.lagrange_to_coeff(domain.lagrange_from_vec(values)))
    }

    fn reconstruct<F: WithSmallOrderMulGroup<3>>(
        domain: &EvaluationDomain<F>,
        lagrange: &[Polynomial<F, LagrangeCoeff>],
    ) -> io::Result<Vec<Polynomial<F, Coeff>>> {
        let mut result = Vec::new();
        result.try_reserve_exact(lagrange.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                "cannot reserve structured coefficient vector",
            )
        })?;
        for polynomial in lagrange {
            result.push(coefficients(domain, polynomial)?);
        }
        Ok(result)
    }

    fn validate_bases<F: WithSmallOrderMulGroup<3>>(
        domain: &EvaluationDomain<F>,
        lagrange: &[Polynomial<F, LagrangeCoeff>],
        coeff: &[Polynomial<F, Coeff>],
        columns: usize,
        rows: usize,
    ) -> io::Result<()> {
        if lagrange.len() != columns
            || coeff.len() != columns
            || lagrange.iter().any(|polynomial| polynomial.len() != rows)
            || coeff.iter().any(|polynomial| polynomial.len() != rows)
        {
            return Err(invalid("structured key polynomial shape is inconsistent"));
        }
        for (lagrange, coeff) in lagrange.iter().zip(coeff) {
            let expected = coefficients(domain, lagrange)?;
            if expected[..] != coeff[..] {
                return Err(invalid("structured key polynomial bases disagree"));
            }
        }
        Ok(())
    }

    fn reserved<T>(count: usize) -> io::Result<Vec<T>> {
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                "cannot reserve structured key data",
            )
        })?;
        Ok(values)
    }

    fn scalar_bytes<F: PrimeField>() -> usize {
        F::Repr::default().as_ref().len()
    }

    fn read_scalar<F: PrimeField, R: Read>(reader: &mut R) -> io::Result<F> {
        // The legacy SerdePrimeField processed reader unwraps I/O; do not call it here.
        let mut repr = F::Repr::default();
        reader.read_exact(repr.as_mut())?;
        Option::from(F::from_repr(repr)).ok_or_else(|| invalid("noncanonical structured scalar"))
    }

    fn fixed_mode<F: Field>(values: &[F]) -> io::Result<u8> {
        let first = values
            .first()
            .ok_or_else(|| invalid("empty structured fixed column"))?;
        Ok(if values.iter().all(|value| value == first) {
            CONSTANT
        } else if values
            .iter()
            .all(|value| *value == F::ZERO || *value == F::ONE)
        {
            BITSET
        } else {
            RAW
        })
    }

    fn fixed_payload_bytes<F: PrimeField>(mode: u8, rows: usize) -> io::Result<u64> {
        match mode {
            CONSTANT => Ok(scalar_bytes::<F>() as u64),
            BITSET => Ok(rows.div_ceil(8) as u64),
            RAW => (rows as u64)
                .checked_mul(scalar_bytes::<F>() as u64)
                .ok_or_else(|| invalid("structured fixed size overflow")),
            _ => Err(invalid("unknown structured fixed mode")),
        }
    }

    pub(super) fn write_fixed<F: PrimeField, W: Write>(
        writer: &mut W,
        values: &[F],
    ) -> io::Result<()> {
        let mode = fixed_mode(values)?;
        writer.write_all(&[mode])?;
        match mode {
            CONSTANT => writer.write_all(values[0].to_repr().as_ref()),
            BITSET => {
                for chunk in values.chunks(8) {
                    let mut byte = 0;
                    for (bit, value) in chunk.iter().enumerate() {
                        byte |= u8::from(*value == F::ONE) << bit;
                    }
                    writer.write_all(&[byte])?;
                }
                Ok(())
            }
            RAW => {
                for value in values {
                    writer.write_all(value.to_repr().as_ref())?;
                }
                Ok(())
            }
            _ => Err(invalid("unknown structured fixed mode")),
        }
    }

    pub(super) fn read_fixed<F: PrimeField, R: Read>(
        reader: &mut R,
        rows: usize,
    ) -> io::Result<Vec<F>> {
        if rows == 0 {
            return Err(invalid("empty structured fixed column"));
        }
        let mut mode = [0];
        reader.read_exact(&mut mode)?;
        // Validate the tag before allocating the trusted-size column.
        fixed_payload_bytes::<F>(mode[0], rows)?;
        let mut values = reserved(rows)?;
        match mode[0] {
            CONSTANT => values.resize(rows, read_scalar(reader)?),
            BITSET => {
                while values.len() < rows {
                    let mut byte = [0];
                    reader.read_exact(&mut byte)?;
                    let bits = (rows - values.len()).min(8);
                    if bits < 8 && byte[0] >> bits != 0 {
                        return Err(invalid("nonzero structured bitset padding"));
                    }
                    for bit in 0..bits {
                        values.push(if byte[0] >> bit & 1 == 0 {
                            F::ZERO
                        } else {
                            F::ONE
                        });
                    }
                }
            }
            RAW => {
                for _ in 0..rows {
                    values.push(read_scalar(reader)?);
                }
            }
            _ => return Err(invalid("unknown structured fixed mode")),
        }
        if fixed_mode(&values)? != mode[0] {
            return Err(invalid("nonminimal structured fixed mode"));
        }
        Ok(values)
    }

    fn read_count<R: Read>(reader: &mut R, expected: usize) -> io::Result<()> {
        let mut bytes = [0; 4];
        reader.read_exact(&mut bytes)?;
        if u32::from_be_bytes(bytes) as u64 != expected as u64 {
            return Err(invalid(
                "structured column count disagrees with configured shape",
            ));
        }
        Ok(())
    }

    fn row_power<F: Field>(mut value: F, rows: usize) -> F {
        for _ in 0..rows.trailing_zeros() {
            value = value.square();
        }
        value
    }

    pub(super) fn permutation_cells<F: PrimeField>(
        rows: usize,
        columns: usize,
        omega: F,
    ) -> io::Result<usize> {
        let cells = rows
            .checked_mul(columns)
            .filter(|cells| u32::try_from(*cells).is_ok())
            .ok_or_else(|| invalid("structured permutation exceeds u32 cell bound"))?;
        if !rows.is_power_of_two()
            || u32::try_from(rows).is_err()
            || F::DELTA == F::ZERO
            || row_power(omega, rows) != F::ONE
            || (rows > 1 && row_power(omega, rows / 2) == F::ONE)
        {
            return Err(invalid("structured permutation root has wrong order"));
        }
        Ok(cells)
    }

    fn sorted_unique<F: PrimeField>(table: &mut [(F::Repr, u32)]) -> io::Result<()> {
        table.sort_unstable_by(|left, right| left.0.as_ref().cmp(right.0.as_ref()));
        if table
            .windows(2)
            .any(|pair| pair[0].0.as_ref() == pair[1].0.as_ref())
        {
            return Err(invalid("structured permutation labels are not unique"));
        }
        Ok(())
    }

    // Bounded inverse labels: O(n + m) entries, never an n*m map or retained PK metadata.
    struct InverseIndex<F: PrimeField> {
        rows: usize,
        row_labels: Vec<(F::Repr, u32)>,
        column_labels: Vec<(F::Repr, u32)>,
        delta_inverses: Vec<F>,
    }

    impl<F: PrimeField> InverseIndex<F> {
        fn new(rows: usize, columns: usize, omega: F) -> io::Result<Self> {
            permutation_cells(rows, columns, omega)?;
            let mut row_labels = reserved(rows)?;
            let mut value = F::ONE;
            for row in 0..rows {
                row_labels.push((value.to_repr(), row as u32));
                value *= omega;
            }
            sorted_unique::<F>(&mut row_labels)?;
            let mut column_labels = reserved(columns)?;
            let mut delta_inverses = reserved(columns)?;
            let step = row_power(F::DELTA, rows);
            let inverse_step = Option::<F>::from(F::DELTA.invert())
                .ok_or_else(|| invalid("structured permutation delta is zero"))?;
            let (mut label, mut inverse) = (F::ONE, F::ONE);
            for column in 0..columns {
                column_labels.push((label.to_repr(), column as u32));
                delta_inverses.push(inverse);
                label *= step;
                inverse *= inverse_step;
            }
            sorted_unique::<F>(&mut column_labels)?;
            Ok(Self {
                rows,
                row_labels,
                column_labels,
                delta_inverses,
            })
        }

        fn target(&self, value: F) -> io::Result<u32> {
            let class = row_power(value, self.rows).to_repr();
            let at = self
                .column_labels
                .binary_search_by(|entry| entry.0.as_ref().cmp(class.as_ref()))
                .map_err(|_| invalid("permutation scalar is outside configured delta cosets"))?;
            let column = self.column_labels[at].1 as usize;
            let normalized = (value * self.delta_inverses[column]).to_repr();
            let at = self
                .row_labels
                .binary_search_by(|entry| entry.0.as_ref().cmp(normalized.as_ref()))
                .map_err(|_| invalid("permutation scalar is outside configured omega rows"))?;
            Ok((column * self.rows + self.row_labels[at].1 as usize) as u32)
        }
    }

    struct Seen {
        bits: Vec<u8>,
        cells: usize,
    }

    impl Seen {
        fn new(cells: usize) -> io::Result<Self> {
            let mut bits = reserved(cells.div_ceil(8))?;
            bits.resize(cells.div_ceil(8), 0);
            Ok(Self { bits, cells })
        }

        fn mark(&mut self, target: u32) -> io::Result<()> {
            let cell = target as usize;
            if cell >= self.cells {
                return Err(invalid("structured permutation target is out of range"));
            }
            let mask = 1 << (cell % 8);
            if self.bits[cell / 8] & mask != 0 {
                return Err(invalid("structured permutation target is duplicated"));
            }
            self.bits[cell / 8] |= mask;
            Ok(())
        }
    }

    fn shape_bytes<C: SerdeCurveAffine>(vk: &VerifyingKey<C>) -> io::Result<(u64, usize)>
    where
        C::Scalar: SerdePrimeField + FromUniformBytes<64>,
    {
        let rows = checked_rows(vk)?;
        let cells =
            permutation_cells(rows, vk.cs.permutation.columns.len(), vk.domain.get_omega())?;
        let mut vk_bytes = ByteCounter::default();
        vk.write(&mut vk_bytes, SerdeFormat::Processed)?;
        let masks = (rows as u64)
            .checked_mul(scalar_bytes::<C::Scalar>() as u64)
            .and_then(|bytes| bytes.checked_add(4))
            .and_then(|bytes| bytes.checked_mul(3))
            .ok_or_else(|| invalid("structured mask size overflow"))?;
        let bytes = HEADER_BYTES
            .checked_add(vk_bytes.0)
            .and_then(|n| n.checked_add(masks))
            .and_then(|n| n.checked_add(8))
            .and_then(|n| n.checked_add((cells as u64) * 4))
            .ok_or_else(|| invalid("structured key size overflow"))?;
        Ok((bytes, rows))
    }

    pub(super) fn read<C: SerdeCurveAffine, R: Read, ConcreteCircuit: Circuit<C::Scalar>>(
        reader: &mut R,
        expected_k: u32,
        expected_bytes: u64,
        #[cfg(feature = "circuit-params")] params: ConcreteCircuit::Params,
    ) -> io::Result<ProvingKey<C>>
    where
        C::Scalar: SerdePrimeField + FromUniformBytes<64>,
    {
        if expected_bytes < HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "structured frame bound is too small",
            ));
        }
        let mut frame = reader.take(expected_bytes);
        let (mut magic, mut curve, mut length) = ([0; 16], [0; 32], [0; 8]);
        frame.read_exact(&mut magic)?;
        if magic != *MAGIC {
            return Err(invalid("unexpected structured proving-key format"));
        }
        frame.read_exact(&mut curve)?;
        if curve != curve_domain::<C>() {
            return Err(invalid("structured curve domain mismatch"));
        }
        frame.read_exact(&mut length)?;
        if u64::from_le_bytes(length) != expected_bytes {
            return Err(invalid("structured frame length mismatch"));
        }
        let vk = VerifyingKey::<C>::read_checked::<_, ConcreteCircuit>(
            &mut frame,
            SerdeFormat::Processed,
            expected_k,
            #[cfg(feature = "circuit-params")]
            params,
        )?;
        let (base_bytes, rows) = shape_bytes(&vk)?;
        let columns = vk.cs.num_fixed_columns as u64;
        let constant = fixed_payload_bytes::<C::Scalar>(CONSTANT, rows)?;
        let binary = fixed_payload_bytes::<C::Scalar>(BITSET, rows)?;
        let raw = fixed_payload_bytes::<C::Scalar>(RAW, rows)?;
        let minimum = columns
            .checked_mul(1 + constant.min(binary).min(raw))
            .and_then(|n| base_bytes.checked_add(n))
            .ok_or_else(|| invalid("structured minimum size overflow"))?;
        let maximum = columns
            .checked_mul(1 + constant.max(binary).max(raw))
            .and_then(|n| base_bytes.checked_add(n))
            .ok_or_else(|| invalid("structured maximum size overflow"))?;
        if expected_bytes < minimum || expected_bytes > maximum {
            return Err(invalid(
                "structured frame length is outside configured shape bounds",
            ));
        }
        let l0 = Polynomial::read_checked(&mut frame, SerdeFormat::Processed, rows)?;
        let l_last = Polynomial::read_checked(&mut frame, SerdeFormat::Processed, rows)?;
        let l_active_row = Polynomial::read_checked(&mut frame, SerdeFormat::Processed, rows)?;
        read_count(&mut frame, vk.cs.num_fixed_columns)?;
        let mut fixed_values = reserved(vk.cs.num_fixed_columns)?;
        for _ in 0..vk.cs.num_fixed_columns {
            fixed_values.push(vk.domain.lagrange_from_vec(read_fixed(&mut frame, rows)?));
        }
        let columns = vk.cs.permutation.columns.len();
        read_count(&mut frame, columns)?;
        let cells = permutation_cells(rows, columns, vk.domain.get_omega())?;
        let mut seen = Seen::new(cells)?;
        let mut omega_powers = reserved(rows)?;
        let mut value = C::Scalar::ONE;
        for _ in 0..rows {
            omega_powers.push(value);
            value *= vk.domain.get_omega();
        }
        let mut deltas = reserved(columns)?;
        let mut classes = reserved(columns)?;
        let step = row_power(C::Scalar::DELTA, rows);
        let (mut delta, mut class) = (C::Scalar::ONE, C::Scalar::ONE);
        for column in 0..columns {
            deltas.push(delta);
            classes.push((class.to_repr(), column as u32));
            delta *= C::Scalar::DELTA;
            class *= step;
        }
        sorted_unique::<C::Scalar>(&mut classes)?;
        drop(classes);
        let mut permutations = reserved(columns)?;
        for _ in 0..columns {
            let mut values = reserved(rows)?;
            for _ in 0..rows {
                let mut bytes = [0; 4];
                frame.read_exact(&mut bytes)?;
                let target = u32::from_le_bytes(bytes);
                seen.mark(target)?;
                values.push(omega_powers[target as usize % rows] * deltas[target as usize / rows]);
            }
            permutations.push(vk.domain.lagrange_from_vec(values));
        }
        drop(seen);
        drop(omega_powers);
        drop(deltas);
        if frame.limit() != 0 {
            return Err(invalid("structured frame was not fully consumed"));
        }
        let fixed_polys = reconstruct(&vk.domain, &fixed_values)?;
        let polys = reconstruct(&vk.domain, &permutations)?;
        let ev = Evaluator::new(vk.cs());
        Ok(ProvingKey {
            vk,
            l0,
            l_last,
            l_active_row,
            fixed_values,
            fixed_polys,
            permutation: permutation::ProvingKey {
                permutations,
                polys,
            },
            ev,
        })
    }
}

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
fn old<C: SerdeCurveAffine, const EMPTY: bool>(
    bytes: &[u8],
    k: u32,
    length: u64,
) -> io::Result<ProvingKey<C>>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    original::read::<C, _, ScanCircuit<C::Scalar, EMPTY>>(
        &mut &bytes[..],
        k,
        length,
        #[cfg(feature = "circuit-params")]
        (),
    )
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
        metadata.permutation_targets.length,
        (n * metadata.permutation_columns * 4) as u64
    );
    let target_bytes = range_bytes(bytes, metadata.permutation_targets);
    let mut nontrivial = false;
    for (column, chunk) in target_bytes.chunks_exact(n * 4).enumerate() {
        let values = chunk
            .chunks_exact(4)
            .enumerate()
            .map(|(row, encoded)| {
                let id = u32::from_le_bytes(encoded.try_into().unwrap()) as usize;
                nontrivial |= id != column * n + row;
                pk.vk.domain.get_omega().pow_vartime([(id % n) as u64])
                    * C::Scalar::DELTA.pow_vartime([(id / n) as u64])
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
    }
    assert!(metadata.permutation_columns == 0 || nontrivial);
    cursor += target_bytes.len();
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
                let expected = old::<C, EMPTY>(&bytes, k, bytes.len() as u64).unwrap();
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
    let expected = old::<C, false>(bytes, k, length)
        .err()
        .unwrap_or_else(|| panic!("original accepted {label}"));
    let normal = dense::<C, false>(bytes, k, length)
        .err()
        .unwrap_or_else(|| panic!("dense accepted {label}"));
    let mut canonical = Vec::new();
    let actual = index::<C, false, _, _>(&mut &bytes[..], k, length, &mut canonical)
        .err()
        .unwrap_or_else(|| panic!("index accepted {label}"));
    assert_eq!(normal.kind(), expected.kind(), "dense {label}");
    assert_eq!(actual.kind(), expected.kind(), "index {label}");
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
    let targets = m.permutation_targets.offset as usize;
    let end = targets + m.permutation_targets.length as usize;
    let cells = (16 * m.permutation_columns) as u32;
    for offset in [targets, end - 4] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&cells.to_le_bytes());
        cases.push((format!("out of range target {offset}"), bad, 4, length));
    }
    let mut bad = bytes.clone();
    bad.copy_within(targets..targets + 4, end - 4);
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
    assert_eq!(cases.len(), 91);
    for (label, bad, k, length) in cases {
        rejection::<C>(&bad, k, length, &label);
    }
    assert!(old::<C, true>(&bytes, 4, length).is_err());
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
                for i in 0..4 {
                    variant.swap(targets + i, targets + 4 + i);
                }
            }
            2 => variant[m.masks[1].offset as usize..m.masks[1].offset as usize + 32]
                .copy_from_slice(C::Scalar::from(919).to_repr().as_ref()),
            _ => unreachable!(),
        }
        let expected = old::<C, false>(&variant, 4, length).unwrap();
        let mut canonical = Vec::new();
        let actual =
            index::<C, false, _, _>(&mut variant.as_slice(), 4, length, &mut canonical).unwrap();
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
                original::write_fixed(&mut encoded, &values).unwrap();
                let mut padded = encoded.clone();
                padded.extend_from_slice(&[91, 92]);
                let mut a = padded.as_slice();
                let mut b = padded.as_slice();
                assert_eq!(original::read_fixed::<F, _>(&mut a, rows).unwrap(), values);
                assert_eq!(read_fixed::<F, _>(&mut b, rows).unwrap(), values);
                assert_eq!(a, &[91, 92]);
                assert_eq!(b, a);
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
            let a = original::read_fixed::<F, _>(&mut bytes.as_slice(), rows)
                .err()
                .expect("original rejects nonminimal/truncated fixed encoding");
            let b = read_fixed::<F, _>(&mut bytes.as_slice(), rows)
                .err()
                .expect("shared scan rejects same fixed encoding");
            assert_eq!(a.kind(), b.kind());
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
        let before = original::permutation_cells(rows, columns, omega)
            .err()
            .unwrap();
        let after = permutation_cells(rows, columns, omega).err().unwrap();
        assert_eq!(before.kind(), after.kind());
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

//! Explicit structured key storage; the normal two-basis runtime key is preserved.
//!
//! Fixed columns have canonical constant/bitset/raw modes; permutation cells store exact u32
//! target IDs. There is no implicit codec fallback. Authentication, circuit/role binding and
//! outer EOF remain caller duties. This prototype does not reduce resident proving-key memory.

use super::{
    Circuit, Coeff, EvaluationDomain, Evaluator, LagrangeCoeff, Polynomial, ProvingKey,
    SerdeCurveAffine, SerdeFormat, SerdePrimeField, VerifyingKey, permutation,
};
use blake2b_simd::Params as Blake2bParams;
use group::ff::{Field, FromUniformBytes, PrimeField, WithSmallOrderMulGroup};
use std::io::{self, Read, Write};

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
            .checked_add(u64::try_from(bytes.len()).map_err(|_| invalid("byte count overflow"))?)
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

fn write_fixed<F: PrimeField, W: Write>(writer: &mut W, values: &[F]) -> io::Result<()> {
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

fn read_fixed<F: PrimeField, R: Read>(reader: &mut R, rows: usize) -> io::Result<Vec<F>> {
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

fn permutation_cells<F: PrimeField>(rows: usize, columns: usize, omega: F) -> io::Result<usize> {
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
    let cells = permutation_cells(rows, vk.cs.permutation.columns.len(), vk.domain.get_omega())?;
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

impl<C: SerdeCurveAffine> ProvingKey<C>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    /// Return the exact structured-v1 length from configured dimensions and fixed-column values.
    ///
    /// This checks shape and fixed modes; the writer also validates both polynomial bases and
    /// exact permutation membership/bijection before emitting any bytes.
    pub fn structured_v1_bytes_length(&self) -> io::Result<u64> {
        let (mut length, rows) = shape_bytes(&self.vk)?;
        if self.fixed_values.len() != self.vk.cs.num_fixed_columns
            || self.fixed_values.iter().any(|p| p.len() != rows)
        {
            return Err(invalid("structured fixed shape is inconsistent"));
        }
        for polynomial in &self.fixed_values {
            let payload = fixed_payload_bytes::<C::Scalar>(fixed_mode(polynomial)?, rows)?;
            length = length
                .checked_add(1)
                .and_then(|n| n.checked_add(payload))
                .ok_or_else(|| invalid("structured fixed size overflow"))?;
        }
        Ok(length)
    }

    /// Validate both bases, masks and the complete permutation before either writer emits bytes.
    /// The returned inverse labels own no key buffers; the bijection bitmap is already dropped.
    fn validated_structured_v1(&self) -> io::Result<(u64, InverseIndex<C::Scalar>)> {
        let length = self.structured_v1_bytes_length()?;
        let rows = checked_rows(&self.vk)?;
        if [&self.l0, &self.l_last, &self.l_active_row]
            .iter()
            .any(|p| p.len() != rows)
        {
            return Err(invalid("structured mask shape is inconsistent"));
        }
        validate_bases(
            &self.vk.domain,
            &self.fixed_values,
            &self.fixed_polys,
            self.vk.cs.num_fixed_columns,
            rows,
        )?;
        let columns = self.vk.cs.permutation.columns.len();
        validate_bases(
            &self.vk.domain,
            &self.permutation.permutations,
            &self.permutation.polys,
            columns,
            rows,
        )?;
        let index = InverseIndex::new(rows, columns, self.vk.domain.get_omega())?;
        let cells = permutation_cells(rows, columns, self.vk.domain.get_omega())?;
        let mut seen = Seen::new(cells)?;
        for polynomial in &self.permutation.permutations {
            for value in polynomial.iter() {
                seen.mark(index.target(*value)?)?;
            }
        }
        drop(seen);
        Ok((length, index))
    }

    /// Write a distinct structured-v1 frame, preserving the exact Processed PK on reconstruction.
    ///
    /// The frame is magic[16], curve-domain[32], total-u64-LE, Processed VK, three Processed
    /// coefficient masks, fixed-count-u32-BE and each fixed column's tag/data, then
    /// permutation-count-u32-BE and one target-u32-LE per cell in column-major order.
    /// Fixed tags are constant=0 (one scalar), bitset=1 (low-bit-first rows), raw=2 (n scalars),
    /// with constant > bitset > raw priority. Counts and rows derive from the trusted VK shape.
    ///
    /// Before output, this compares all bases by exact inverse FFT and checks permutation
    /// membership/bijection using O(n+m) inverse-label scratch and an n*m-bit validation bitmap.
    /// The bitmap is dropped before serialization. IDs are derived again during output rather
    /// than retained in an n*m map. At k16/m133 this entails about 278 million field squarings
    /// across the two membership passes, plus FFT checks and binary searches; it is an explicit
    /// correctness prototype, not a latency or RSS qualification. I/O failures propagate and may
    /// leave partial output; callers own flushing and atomic publication. VK commitments and raw
    /// masks are preserved, not recomputed or authenticated; complete artifact authentication and
    /// equality with the trusted standalone VK remain caller duties.
    // TODO: qualify and optimize generic inverse-label time before production caller migration.
    pub fn write_structured_v1<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let (length, index) = self.validated_structured_v1()?;
        let columns = self.vk.cs.permutation.columns.len();
        writer.write_all(MAGIC)?;
        writer.write_all(&curve_domain::<C>())?;
        writer.write_all(&length.to_le_bytes())?;
        self.vk.write(writer, SerdeFormat::Processed)?;
        for polynomial in [&self.l0, &self.l_last, &self.l_active_row] {
            polynomial.write_streaming(writer, SerdeFormat::Processed)?;
        }
        writer.write_all(&(self.fixed_values.len() as u32).to_be_bytes())?;
        for polynomial in &self.fixed_values {
            write_fixed(writer, polynomial)?;
        }
        writer.write_all(&(columns as u32).to_be_bytes())?;
        for polynomial in &self.permutation.permutations {
            for value in polynomial.iter() {
                writer.write_all(&index.target(*value)?.to_le_bytes())?;
            }
        }
        Ok(())
    }

    /// Consume a key while writing exactly the frame produced by [`Self::write_structured_v1`].
    ///
    /// All shape, basis and permutation checks finish before output. Both coefficient banks and
    /// the evaluator are then dropped before the header is emitted. The VK is released after its
    /// bytes, each mask after its bytes, and each fixed/permutation Lagrange polynomial after its
    /// payload. The O(n+m) inverse-label index is the only mapping scratch retained during ID
    /// output; the bijection bitmap was dropped during validation. No n*m ID vector or second
    /// artifact buffer is kept.
    ///
    /// Validation still needs the complete input key, a temporary inverse-FFT polynomial and its
    /// workspace. This does not lower the key-generation peak or qualify process RSS or latency.
    /// I/O errors and unwinding drop all remaining owned buffers. The caller owns flushing and
    /// atomic publication, since sink failure may leave a partial frame.
    pub fn write_structured_v1_consuming<W: Write>(self, writer: &mut W) -> io::Result<()> {
        let (length, index) = self.validated_structured_v1()?;
        let Self {
            vk,
            l0,
            l_last,
            l_active_row,
            fixed_values,
            fixed_polys,
            permutation,
            ev,
        } = self;
        let permutation::ProvingKey {
            permutations,
            polys,
        } = permutation;
        drop(fixed_polys);
        drop(polys);
        drop(ev);

        writer.write_all(MAGIC)?;
        writer.write_all(&curve_domain::<C>())?;
        writer.write_all(&length.to_le_bytes())?;
        vk.write(writer, SerdeFormat::Processed)?;
        drop(vk);
        l0.write_consuming(writer, SerdeFormat::Processed)?;
        l_last.write_consuming(writer, SerdeFormat::Processed)?;
        l_active_row.write_consuming(writer, SerdeFormat::Processed)?;
        writer.write_all(&(fixed_values.len() as u32).to_be_bytes())?;
        for polynomial in fixed_values {
            write_fixed(writer, &polynomial)?;
        }
        writer.write_all(&(permutations.len() as u32).to_be_bytes())?;
        for polynomial in permutations {
            for value in polynomial.iter() {
                writer.write_all(&index.target(*value)?.to_le_bytes())?;
            }
        }
        Ok(())
    }

    /// Read one explicitly framed structured-v1 PK using trusted k, length and circuit parameters.
    ///
    /// Counts and allocations are bounded by the configured VK, not untrusted wire lengths.
    /// Rejects noncanonical modes/fields/padding and nonbijective or out-of-range permutation IDs.
    /// Reconstructs both normal polynomial banks; no cell map is retained. Bytes beyond the exact
    /// frame remain for enclosing EOF policy. Complete authentication, role/parity binding, and
    /// embedded-VK equality with an authenticated standalone VK are required before use. Existing
    /// compact-v1 and Core canonical reencoding/authentication paths are unchanged by this API.
    pub fn read_structured_v1_checked<R: Read, ConcreteCircuit: Circuit<C::Scalar>>(
        reader: &mut R,
        expected_k: u32,
        expected_bytes: u64,
        #[cfg(feature = "circuit-params")] params: ConcreteCircuit::Params,
    ) -> io::Result<Self> {
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
        Ok(Self {
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

#[cfg(test)]
mod tests;

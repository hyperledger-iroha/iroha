//! Canonical public verifier-program transport. Private witness arrays and
//! witness-producing closures are never part of this artifact.
use super::*;

const MAX_PROGRAM_BYTES: usize = 128 * 1024 * 1024;
#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.PublicProgram.v1")]
struct PublicProgram {
    version: u16,
    side: u8,
    modulus_minus_one: [u8; 32],
    geometry: [u32; 12],
    challenges: u32,
    degrees: [u32; 2],
    inputs: u32,
    // Norito's canonical Vec<u8> representation is a bounded raw byte blob.
    // Nested arrays would frame every scalar byte separately (over 9x larger).
    // Instruction records are three little-endian u32 words; field records are
    // exact canonical 32-byte representations, with no alternate encoding.
    operations: Vec<u8>,
    constants: Vec<u8>,
    outputs: [u32; 3],
    // Maximal alternating zero/literal runs: tag u8, count u32 LE, then
    // count canonical nonzero fields for literal tag 1 (zero tag 0 has none).
    // Common layout padding otherwise dwarfs the actual public constants.
    fixed: Vec<u8>,
    late: Vec<[u32; 4]>,
    link: Vec<[u32; 2]>,
}
fn encode(value: &PublicProgram) -> Result<Vec<u8>, String> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
    norito::to_bytes(value).map_err(|e| format!("public program encode: {e}"))
}
fn u32s<const N: usize>(v: [usize; N]) -> Result<[u32; N], String> {
    let mut out = [0; N];
    for (i, v) in v.into_iter().enumerate() {
        out[i] = v.try_into().map_err(|_| "program integer bound")?;
    }
    Ok(out)
}
fn pack_fields<F: Scalar>(values: &[F]) -> Result<Vec<u8>, String> {
    let length = values
        .len()
        .checked_mul(32)
        .ok_or("program field byte bound")?;
    if length > MAX_PROGRAM_BYTES {
        return Err("program field byte bound".into());
    }
    let mut bytes = Vec::with_capacity(length);
    for value in values {
        bytes.extend_from_slice(&value.to_repr());
    }
    Ok(bytes)
}
fn unpack_fields<F: Scalar>(bytes: &[u8]) -> Result<Vec<F>, String> {
    if !bytes.len().is_multiple_of(32) {
        return Err("partial program field".into());
    }
    bytes
        .chunks_exact(32)
        .map(|x| {
            Option::<F>::from(F::from_repr(x.try_into().expect("exact field record")))
                .ok_or_else(|| "noncanonical program field".to_string())
        })
        .collect()
}
fn fixed_field_bound(count: usize) -> Result<(), String> {
    if count.checked_mul(32).is_none_or(|n| n > MAX_PROGRAM_BYTES) {
        return Err("expanded fixed program field bound".into());
    }
    Ok(())
}
fn pack_fixed_fields<F: Scalar>(values: &[F]) -> Result<Vec<u8>, String> {
    fixed_field_bound(values.len())?;
    let mut bytes = Vec::new();
    let mut start = 0;
    while start < values.len() {
        let zero = bool::from(values[start].is_zero());
        let mut end = start + 1;
        while end < values.len() && bool::from(values[end].is_zero()) == zero {
            end += 1;
        }
        bytes.push(u8::from(!zero));
        bytes.extend_from_slice(
            &u32::try_from(end - start)
                .map_err(|_| "fixed run bound")?
                .to_le_bytes(),
        );
        if !zero {
            for value in &values[start..end] {
                bytes.extend_from_slice(&value.to_repr());
            }
        }
        start = end;
    }
    Ok(bytes)
}
fn unpack_fixed_fields<F: Scalar>(bytes: &[u8], count: usize) -> Result<Vec<F>, String> {
    fixed_field_bound(count)?;
    let mut fields = Vec::with_capacity(count);
    let mut at = 0;
    let mut previous = None;
    while at < bytes.len() {
        if bytes.len() - at < 5 {
            return Err("partial fixed program run".into());
        }
        let tag = bytes[at];
        let length = u32::from_le_bytes(bytes[at + 1..at + 5].try_into().unwrap()) as usize;
        at += 5;
        if tag > 1 || previous == Some(tag) || length == 0 || length > count - fields.len() {
            return Err("noncanonical fixed program run".into());
        }
        previous = Some(tag);
        if tag == 0 {
            fields.resize(fields.len() + length, F::ZERO);
        } else {
            let byte_count = length.checked_mul(32).ok_or("fixed literal byte bound")?;
            if byte_count > bytes.len() - at {
                return Err("partial fixed program literal".into());
            }
            for value in bytes[at..at + byte_count].chunks_exact(32) {
                let field = Option::<F>::from(F::from_repr(value.try_into().unwrap()))
                    .ok_or("noncanonical fixed program field")?;
                if bool::from(field.is_zero()) {
                    return Err("zero in fixed literal run".into());
                }
                fields.push(field);
            }
            at += byte_count;
        }
    }
    if fields.len() != count {
        return Err("fixed program field count".into());
    }
    Ok(fields)
}
impl<F: Scalar> UniformCircuit<F> {
    /// Serialize only exact public verification instructions and constants.
    pub fn export_program(&self) -> Result<Vec<u8>, String> {
        let side = match self.shape.label {
            "P-uniform" => 1,
            "S-uniform" => 2,
            _ => return Err("only monetary program labels may be installed".into()),
        };
        let tape = self.plan();
        let degrees = self.challenge_degrees();
        let operation_bytes = tape
            .ops
            .len()
            .checked_mul(12)
            .ok_or("program instruction byte bound")?;
        if operation_bytes > MAX_PROGRAM_BYTES {
            return Err("program instruction byte bound".into());
        }
        let mut operations = Vec::with_capacity(operation_bytes);
        for op in &tape.ops {
            let words = match *op {
                Op::Add(a, b) => [0, a, b],
                Op::Sub(a, b) => [1, a, b],
                Op::Mul(a, b) => [2, a, b],
                Op::Scale(a, b) => [3, a, b],
                Op::Const(a) => [4, a, 0],
            };
            for word in words {
                operations.extend_from_slice(&word.to_le_bytes());
            }
        }
        let value = PublicProgram {
            version: 1,
            side,
            modulus_minus_one: (-F::ONE).to_repr(),
            geometry: u32s([
                self.shape.sx,
                self.shape.row_bits,
                self.shape.col_bits,
                self.shape.k,
                self.io_len,
                self.p1_rows,
                self.pl_rows,
                self.p2_rows,
                self.fixed_rows,
                self.wallet_rows,
                self.wallet_io.unwrap_or(u32::MAX as usize),
                self.constraints,
            ])?,
            challenges: self.phase2_challenges() as u32,
            degrees: u32s([degrees.0, degrees.1])?,
            inputs: tape.inputs as u32,
            operations,
            constants: pack_fields(&tape.pool)?,
            outputs: tape.out,
            fixed: pack_fixed_fields(&self.fixed_values)?,
            late: self
                .late_io
                .iter()
                .map(|&(a, b, c, d)| u32s([a, b, c, d]))
                .collect::<Result<_, _>>()?,
            link: self
                .link_io
                .iter()
                .map(|&(a, b)| u32s([a, b]))
                .collect::<Result<_, _>>()?,
        };
        let bytes = encode(&value)?;
        if bytes.len() > MAX_PROGRAM_BYTES {
            return Err(format!(
                "public program exceeds device bound ({} bytes; {} instructions, {} constants, {} fixed fields)",
                bytes.len(),
                tape.ops.len(),
                tape.pool.len(),
                self.fixed_values.len()
            ));
        }
        Ok(bytes)
    }

    /// Decode a bounded verifier-only program. Its digest must subsequently be
    /// authenticated by the independent scheme-root manifest before use.
    pub fn import_program(bytes: &[u8], primary: bool) -> Result<Self, String> {
        if bytes.len() > MAX_PROGRAM_BYTES {
            return Err("public program exceeds device bound".into());
        }
        let view = norito::core::from_bytes_view(bytes)
            .map_err(|e| format!("public program frame: {e}"))?;
        if view.flags() != norito::core::V1_LAYOUT_FLAGS {
            return Err("public program layout".into());
        }
        let p: PublicProgram = norito::decode_from_bytes_with_limits(
            bytes,
            norito::DecodeLimits::new(
                MAX_PROGRAM_BYTES,
                MAX_PROGRAM_BYTES,
                4 * MAX_PROGRAM_BYTES,
                4 * MAX_PROGRAM_BYTES,
                16,
            ),
        )
        .map_err(|e| format!("public program decode: {e}"))?;
        if encode(&p)? != bytes {
            return Err("noncanonical public program".into());
        }
        if p.version != 1
            || p.side != if primary { 1 } else { 2 }
            || p.modulus_minus_one != (-F::ONE).to_repr()
        {
            return Err("public program version/field/side".into());
        }
        let g = p.geometry.map(|v| v as usize);
        let [
            sx,
            row_bits,
            col_bits,
            k,
            io_len,
            p1_rows,
            pl_rows,
            p2_rows,
            fixed_rows,
            wallet_rows,
            wallet_io,
            constraints,
        ] = g;
        if !(1..=30).contains(&sx)
            || !(2..=18).contains(&row_bits)
            || !(9..=18).contains(&col_bits)
            || row_bits + col_bits > 30
            || k != 3
            || io_len != if primary { 64 } else { 62 }
            || !matches!(p.challenges, 0 | 2)
            || p.degrees[0] > 1
            || p.degrees[1] > 128
        {
            return Err("public program geometry".into());
        }
        let shape = Shape {
            label: if primary { "P-uniform" } else { "S-uniform" },
            sx,
            row_bits,
            col_bits,
            k,
        };
        let rows = shape.rows();
        if shape.tail_len() >= shape.cols() - crate::r1cs::MASK_COLS {
            return Err("public program IPA tail exceeds column capacity".into());
        }
        let phase_rows = p1_rows
            .checked_add(pl_rows)
            .and_then(|n| n.checked_add(p2_rows))
            .and_then(|n| n.checked_add(fixed_rows));
        if wallet_rows > fixed_rows
            || wallet_rows > 1
            || fixed_rows >= rows
            || phase_rows.is_none_or(|n| n > rows)
            || constraints > 1usize << sx
            || p.inputs as usize != sx + shape.sz() + p.challenges as usize
            || !p.operations.len().is_multiple_of(12)
            || !p.constants.len().is_multiple_of(32)
        {
            return Err("public program phase layout".into());
        }
        if (wallet_rows == 0 && wallet_io != u32::MAX as usize)
            || (wallet_rows == 1 && wallet_io != crate::full::TABLE_IO)
        {
            return Err("public program wallet row".into());
        }
        let expected_link: Vec<usize> = if primary {
            vec![124, 125, 126, 127]
        } else {
            vec![124, 126]
        };
        if p.link.iter().map(|v| v[1] as usize).collect::<Vec<_>>() != expected_link
            || p.late.len() > 128
            || p.late
                .iter()
                .any(|v| v[0] >= 256 || expected_link.contains(&(v[0] as usize)))
        {
            return Err("public program IO ABI".into());
        }
        let mut distinct = std::collections::HashSet::new();
        if p.late.iter().any(|v| !distinct.insert(v[0])) {
            return Err("duplicate late program IO".into());
        }
        let pool = unpack_fields(&p.constants)?;
        let fixed_count = (fixed_rows - wallet_rows)
            .checked_mul(shape.cols())
            .ok_or("fixed program field count overflow")?;
        let fixed_values = unpack_fixed_fields(&p.fixed, fixed_count)?;
        let mut ops = Vec::with_capacity(p.operations.len() / 12);
        let (mut muls, mut scales) = (0, 0);
        for (i, record) in p.operations.chunks_exact(12).enumerate() {
            let words: [u32; 3] = std::array::from_fn(|j| {
                u32::from_le_bytes(
                    record[j * 4..j * 4 + 4]
                        .try_into()
                        .expect("exact instruction word"),
                )
            });
            let [tag, a, b] = words;
            let before = p.inputs as usize + i;
            let slot = |v: u32| (v as usize) < before;
            let constant = |v: u32| (v as usize) < pool.len();
            let op = match tag {
                0 if slot(a) && slot(b) => Op::Add(a, b),
                1 if slot(a) && slot(b) => Op::Sub(a, b),
                2 if slot(a) && slot(b) => {
                    muls += 1;
                    Op::Mul(a, b)
                }
                3 if slot(a) && constant(b) => {
                    scales += 1;
                    Op::Scale(a, b)
                }
                4 if constant(a) && b == 0 => Op::Const(a),
                _ => return Err("public program forward/invalid instruction".into()),
            };
            ops.push(op);
        }
        if p.outputs
            .iter()
            .any(|v| *v as usize >= p.inputs as usize + ops.len())
        {
            return Err("public program output bound".into());
        }
        let regs = Regs::of(p.inputs as usize, &ops, p.outputs);
        let tape = Tape {
            inputs: p.inputs as usize,
            ops,
            pool,
            out: p.outputs,
            muls,
            scales,
            regs,
        };
        let plan = OnceLock::new();
        let _ = plan.set(tape);
        let link_io_values: LinkIo<F> = Box::new(move |rho, r| {
            if primary {
                rho.chunks_exact(16)
                    .chain(r.chunks_exact(16))
                    .map(|b| F::from_u128(u128::from_le_bytes(b.try_into().unwrap())))
                    .collect()
            } else {
                [rho, r]
                    .into_iter()
                    .map(|b| {
                        let lo = u128::from_le_bytes(b[..16].try_into().unwrap());
                        let hi = u128::from_le_bytes(b[16..].try_into().unwrap());
                        F::from_u128(lo) + F::from(2).pow_vartime([128]) * F::from_u128(hi)
                    })
                    .collect()
            }
        });
        Ok(Self {
            verifier_only: Some((
                p.challenges as usize,
                (p.degrees[0] as usize, p.degrees[1] as usize),
            )),
            label: shape.label,
            arrays: vec![],
            pi: vec![],
            counts: vec![],
            shape,
            io_len,
            constraints,
            product_masks: vec![],
            row_base: vec![],
            blocks: vec![],
            bounds: vec![],
            fixed_rows,
            fixed_values,
            wallet_rows,
            wallet_io: (wallet_rows != 0).then_some(wallet_io),
            phase2: vec![],
            lookup_arrays: vec![],
            tables: vec![],
            ch_rows: vec![],
            ch_fill: None,
            io_arrays: vec![],
            late_io: p
                .late
                .into_iter()
                .map(|v| (v[0] as usize, v[1] as usize, v[2] as usize, v[3] as usize))
                .collect(),
            exports: vec![],
            lookup_mask: HashMap::new(),
            lookup_mask_col: HashMap::new(),
            copy_exports: vec![],
            copy_imports: vec![],
            index_arrays: vec![],
            p1_rows,
            pl_rows,
            p2_rows,
            link_tables: vec![],
            link_fill: None,
            link_io: p
                .link
                .into_iter()
                .map(|v| (v[0] as usize, v[1] as usize))
                .collect(),
            link_io_values: Some(link_io_values),
            link_binding: None,
            rfield: vec![],
            moved: vec![],
            split_of: vec![],
            gather: OnceLock::new(),
            symbolic: OnceLock::new(),
            readers: OnceLock::new(),
            m_plans: OnceLock::new(),
            plan,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_pasta::{Fp, Fq};
    fn sample<F: Scalar>(primary: bool) -> UniformCircuit<F> {
        let mut w = crate::full::Wiring::default();
        let f = w.family(0, 1);
        w.push(Array {
            name: "artifact relation",
            family: f,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![255, 255],
            rows: vec![[own(0, 1), own(0, 1), own(1, 1)]],
            values: vec![vec![F::from(7)], vec![F::from(49)]],
        });
        for slot in if primary {
            vec![124, 125, 126, 127]
        } else {
            vec![124, 126]
        } {
            let a = w.push(Array {
                name: "artifact link slot",
                family: f,
                rho: 0,
                r_count: 1,
                fixed: false,
                lookups: vec![],
                widths: vec![255],
                rows: vec![],
                values: vec![vec![F::ZERO]],
            });
            w.io_arrays.push((a, slot));
            w.link.io.push((a, slot));
        }
        w.link.io_values = Some(Box::new(move |rho, r| {
            if primary {
                rho.chunks_exact(16)
                    .chain(r.chunks_exact(16))
                    .map(|b| F::from_u128(u128::from_le_bytes(b.try_into().unwrap())))
                    .collect()
            } else {
                [rho, r]
                    .into_iter()
                    .map(|b| {
                        F::from_u128(u128::from_le_bytes(b[..16].try_into().unwrap()))
                            + F::from_u128(u128::from_le_bytes(b[16..].try_into().unwrap()))
                                * F::from(2).pow_vartime([128])
                    })
                    .collect()
            }
        }));
        let (a, p, n, t, c) = w.finish();
        UniformCircuit::build_with_copies(
            if primary { "P-uniform" } else { "S-uniform" },
            a,
            p,
            n,
            t,
            c,
            10,
            if primary { 64 } else { 62 },
            b"public artifact fixture",
        )
        .0
    }
    fn check<F: Scalar>(primary: bool) {
        let c = sample::<F>(primary);
        let bytes = c.export_program().unwrap();
        let packed: PublicProgram = norito::decode_from_bytes(&bytes).unwrap();
        assert_eq!(packed.operations.len(), c.plan().ops.len() * 12);
        assert_eq!(packed.constants.len(), c.plan().pool.len() * 32);
        assert_eq!(
            unpack_fixed_fields::<F>(&packed.fixed, c.fixed_values.len()).unwrap(),
            c.fixed_values
        );
        assert!(
            bytes.len()
                < packed.operations.len() + packed.constants.len() + packed.fixed.len() + 8192,
            "Norito raw byte blobs must not acquire per-byte/record framing"
        );
        let loaded = UniformCircuit::<F>::import_program(&bytes, primary).unwrap();
        assert_eq!(c.program_digest(), loaded.program_digest());
        assert_eq!(c.layout(), loaded.layout());
        assert!(loaded.arrays.is_empty());
        let mut rng = SpikeRng::new(b"artifact parity");
        for _ in 0..4 {
            let rx = (0..c.shape.sx).map(|_| rng.scalar()).collect::<Vec<_>>();
            let ry = (0..c.shape.sz()).map(|_| rng.scalar()).collect::<Vec<_>>();
            let ch = [rng.scalar(), rng.scalar()];
            assert_eq!(c.evals(&rx, &ry, &ch), loaded.evals(&rx, &ry, &ch));
        }
        let rho = [37; 32];
        let r = [19; 32];
        assert_eq!(c.link_io(&rho, &r), loaded.link_io(&rho, &r));
        assert!(UniformCircuit::<F>::import_program(&bytes, !primary).is_err());
        assert!(UniformCircuit::<F>::import_program(&bytes[..bytes.len() - 1], primary).is_err());
        let mut p: PublicProgram = norito::decode_from_bytes(&bytes).unwrap();
        p.operations[..4].copy_from_slice(&2u32.to_le_bytes());
        p.operations[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        p.operations[8..12].fill(0);
        assert!(UniformCircuit::<F>::import_program(&encode(&p).unwrap(), primary).is_err());
        let mut p: PublicProgram = norito::decode_from_bytes(&bytes).unwrap();
        p.geometry[5] = u32::MAX;
        p.geometry[6] = u32::MAX;
        assert!(UniformCircuit::<F>::import_program(&encode(&p).unwrap(), primary).is_err());
        for field in 0..3 {
            let mut p: PublicProgram = norito::decode_from_bytes(&bytes).unwrap();
            match field {
                0 => {
                    p.operations.push(0);
                }
                1 => {
                    p.constants.push(0);
                }
                _ => {
                    p.fixed.push(0);
                }
            }
            assert!(
                UniformCircuit::<F>::import_program(&encode(&p).unwrap(), primary).is_err(),
                "partial fixed-width records are not canonical"
            );
        }
        let mut p: PublicProgram = norito::decode_from_bytes(&bytes).unwrap();
        p.constants.extend_from_slice(&[255; 32]);
        assert!(
            UniformCircuit::<F>::import_program(&encode(&p).unwrap(), primary).is_err(),
            "each field must be strictly below its modulus"
        );
    }
    #[test]
    fn public_program_roundtrips_without_private_witnesses() {
        check::<Fp>(true);
        check::<Fq>(false);
    }
    #[test]
    fn fixed_padding_runs_have_one_bounded_canonical_encoding() {
        for values in [
            vec![],
            vec![Fp::ZERO; 4096],
            vec![Fp::ONE; 4096],
            vec![Fp::ZERO, Fp::ONE, Fp::from(2), Fp::ZERO, Fp::ONE],
        ] {
            let encoded = pack_fixed_fields(&values).unwrap();
            assert_eq!(
                unpack_fixed_fields::<Fp>(&encoded, values.len()).unwrap(),
                values
            );
            assert_eq!(
                pack_fixed_fields(&unpack_fixed_fields::<Fp>(&encoded, values.len()).unwrap())
                    .unwrap(),
                encoded
            );
        }
        assert_eq!(pack_fixed_fields(&vec![Fp::ZERO; 4096]).unwrap().len(), 5);
        let zero = [0, 1, 0, 0, 0];
        for bad in [
            vec![0],
            vec![2, 1, 0, 0, 0],
            vec![0, 0, 0, 0, 0],
            vec![0, 3, 0, 0, 0],
            [zero, zero].concat(),
            [vec![1, 1, 0, 0, 0], vec![0; 32]].concat(),
            [vec![1, 1, 0, 0, 0], vec![255; 32]].concat(),
            vec![1, 1, 0, 0, 0, 1],
        ] {
            assert!(
                unpack_fixed_fields::<Fp>(&bad, 2).is_err(),
                "noncanonical runs: {bad:?}"
            );
        }
        assert!(unpack_fixed_fields::<Fp>(&zero, 0).is_err());
        assert!(unpack_fixed_fields::<Fp>(&[], MAX_PROGRAM_BYTES / 32 + 1).is_err());
    }
}

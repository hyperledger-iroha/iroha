//! Both-Pasta structured key reconstruction, real-proof equivalence and malformed-frame checks.

use super::*;
use crate::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{
        Advice, Column, ConstraintSystem, Error, Fixed, Instance, Selector, create_proof,
        keygen_pk2, verify_proof,
    },
    poly::{
        Rotation, VerificationStrategy as _,
        commitment::ParamsProver,
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::{ProverIPA, VerifierIPA},
            strategy::SingleStrategy,
        },
    },
    transcript::{
        Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
    },
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

#[derive(Clone)]
struct KeyCircuit<F: Field> {
    value: Value<F>,
}

#[derive(Clone, Copy, Debug)]
struct KeyConfig {
    left: Column<Advice>,
    right: Column<Advice>,
    fixed: Column<Fixed>,
    binary: Column<Fixed>,
    instance: Column<Instance>,
    selector: Selector,
}

impl<F: PrimeField> Circuit<F> for KeyCircuit<F> {
    type Config = KeyConfig;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            value: Value::unknown(),
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let left = meta.advice_column();
        let right = meta.advice_column();
        let fixed = meta.fixed_column();
        let binary = meta.fixed_column();
        let _constant_zero = meta.fixed_column();
        let instance = meta.instance_column();
        let selector = meta.selector();
        meta.enable_equality(left);
        meta.enable_equality(right);
        meta.enable_equality(instance);
        meta.create_gate("nonzero fixed data and copied public advice", |meta| {
            let q = meta.query_selector(selector);
            let lhs = meta.query_advice(left, Rotation::cur());
            let rhs = meta.query_advice(right, Rotation::next());
            let prev = meta.query_advice(left, Rotation::prev());
            let fixed = meta.query_fixed(fixed, Rotation::cur());
            vec![
                q.clone() * (lhs.clone() - rhs),
                q.clone() * (lhs - prev),
                q * (fixed - crate::plonk::Expression::Constant(F::from(3))),
            ]
        });
        KeyConfig {
            left,
            right,
            fixed,
            binary,
            instance,
            selector,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let public = layouter.assign_region(
            || "structured key varied rotations/cycles",
            |mut region| {
                let mut cells = Vec::new();
                for row in 0..8 {
                    if (1..7).contains(&row) {
                        config.selector.enable(&mut region, row)?;
                    }
                    region.assign_fixed(config.fixed, row, F::from(3));
                    region.assign_fixed(config.binary, row, F::from((row % 2) as u64));
                    let left = region.assign_advice(config.left, row, self.value);
                    left.copy_advice(&mut region, config.right, row);
                    cells.push(left.cell());
                }
                // Form cross-row cycles as well as the row-local copies and instance cycle.
                region.constrain_equal(cells[0], cells[5]);
                region.constrain_equal(cells[2], cells[7]);
                region.constrain_equal(cells[5], cells[2]);
                Ok(cells[0])
            },
        )?;
        layouter.constrain_instance(public, config.instance, 0);
        Ok(())
    }
}

fn read_key<C: SerdeCurveAffine>(bytes: &[u8], k: u32, length: u64) -> io::Result<ProvingKey<C>>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    ProvingKey::read_structured_v1_checked::<_, KeyCircuit<C::Scalar>>(
        &mut &bytes[..],
        k,
        length,
        #[cfg(feature = "circuit-params")]
        (),
    )
}

fn proof<C: SerdeCurveAffine>(
    params: &ParamsIPA<C>,
    pk: &ProvingKey<C>,
    witness: C::Scalar,
) -> Vec<u8>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let circuit = KeyCircuit {
        value: Value::known(witness),
    };
    let public = [witness];
    let columns = [public.as_slice()];
    let instances = [columns.as_slice()];
    let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(Vec::new());
    create_proof::<IPACommitmentScheme<C>, ProverIPA<C>, _, _, _, _>(
        params,
        pk,
        &[circuit],
        &instances,
        ChaCha20Rng::from_seed([79; 32]),
        &mut transcript,
    )
    .expect("genuine tiny IPA proof");
    transcript.finalize()
}

fn verify<C: SerdeCurveAffine>(
    params: &ParamsIPA<C>,
    vk: &VerifyingKey<C>,
    public: C::Scalar,
    proof: &[u8],
) -> bool
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let public = [public];
    let columns = [public.as_slice()];
    let instances = [columns.as_slice()];
    let mut transcript = Blake2bRead::<_, _, Challenge255<_>>::init(proof);
    verify_proof::<IPACommitmentScheme<C>, VerifierIPA<C>, _, _, _>(
        params,
        vk,
        SingleStrategy::new(params),
        &instances,
        &mut transcript,
    )
    .is_ok()
}

fn roundtrip<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    for k in [5, 6] {
        let params = ParamsIPA::<C>::new(k);
        let circuit = KeyCircuit {
            value: Value::unknown(),
        };
        for compressed in [false, true] {
            let pk = keygen_pk2(&params, &circuit, compressed).unwrap();
            let original = pk.to_bytes(SerdeFormat::Processed);
            let original_vk = pk.get_vk().to_bytes(SerdeFormat::Processed);
            let mut bytes = Vec::new();
            pk.write_structured_v1(&mut bytes).unwrap();
            assert_eq!(bytes.len() as u64, pk.structured_v1_bytes_length().unwrap());
            let mut compact = Vec::new();
            pk.write_compact_v1(&mut compact).unwrap();
            assert!(bytes.len() < compact.len());
            assert!(compact.len() < original.len());
            let restored = read_key::<C>(&bytes, k, bytes.len() as u64).unwrap();
            assert_eq!(restored.to_bytes(SerdeFormat::Processed), original);
            assert_eq!(
                restored.get_vk().to_bytes(SerdeFormat::Processed),
                original_vk
            );
            assert_eq!(
                restored.get_vk().transcript_repr(),
                pk.get_vk().transcript_repr()
            );
            let mut repeated = Vec::new();
            restored.write_structured_v1(&mut repeated).unwrap();
            assert_eq!(repeated, bytes);
            // Actual fixture contains raw, configured binary, and constant-zero fixed columns.
            assert_eq!(fixed_mode(&pk.fixed_values[0]).unwrap(), RAW);
            assert_eq!(fixed_mode(&pk.fixed_values[1]).unwrap(), BITSET);
            assert_eq!(fixed_mode(&pk.fixed_values[2]).unwrap(), CONSTANT);
            for witness in [C::Scalar::from(3), C::Scalar::from(19)] {
                let before = proof(&params, &pk, witness);
                let after = proof(&params, &restored, witness);
                assert_eq!(
                    after, before,
                    "same Processed key, witness and RNG proof bytes"
                );
                assert!(verify(&params, pk.get_vk(), witness, &after));
                assert!(verify(&params, restored.get_vk(), witness, &before));
                assert!(!verify(
                    &params,
                    restored.get_vk(),
                    witness + C::Scalar::ONE,
                    &after
                ));
                let mut corrupted = after;
                corrupted[0] ^= 1;
                assert!(!verify(&params, restored.get_vk(), witness, &corrupted));
            }
            assert!(read_key::<C>(&original, k, original.len() as u64).is_err());
            assert!(read_key::<C>(&compact, k, compact.len() as u64).is_err());
            assert!(
                ProvingKey::<C>::read_compact_v1_checked::<_, KeyCircuit<C::Scalar>>(
                    &mut bytes.as_slice(),
                    k,
                    bytes.len() as u64,
                    #[cfg(feature = "circuit-params")]
                    (),
                )
                .is_err()
            );
            assert!(
                ProvingKey::<C>::read_checked::<_, KeyCircuit<C::Scalar>>(
                    &mut bytes.as_slice(),
                    SerdeFormat::Processed,
                    k,
                    #[cfg(feature = "circuit-params")]
                    (),
                )
                .is_err()
            );
            let mut framed = bytes.clone();
            framed.push(0xA7);
            let mut cursor = io::Cursor::new(framed);
            ProvingKey::<C>::read_structured_v1_checked::<_, KeyCircuit<C::Scalar>>(
                &mut cursor,
                k,
                bytes.len() as u64,
                #[cfg(feature = "circuit-params")]
                (),
            )
            .unwrap();
            assert_eq!(cursor.position(), bytes.len() as u64);
            let mut sentinel = [0];
            cursor.read_exact(&mut sentinel).unwrap();
            assert_eq!(sentinel, [0xA7]);
        }
    }
}

#[test]
fn structured_keys_preserve_processed_keys_and_real_eq_ep_proof_bytes() {
    roundtrip::<EqAffine>();
    roundtrip::<EpAffine>();
}

fn modes<F: PrimeField>() {
    for values in [
        vec![F::ZERO; 3],
        vec![F::ONE; 3],
        vec![F::from(7); 3],
        vec![F::ZERO, F::ONE, F::ZERO],
        vec![F::ZERO, F::from(7), F::ONE],
    ] {
        let mut bytes = Vec::new();
        write_fixed(&mut bytes, &values).unwrap();
        assert_eq!(
            bytes.len() as u64,
            1 + fixed_payload_bytes::<F>(fixed_mode(&values).unwrap(), 3).unwrap()
        );
        assert_eq!(
            read_fixed::<F, _>(&mut bytes.as_slice(), 3).unwrap(),
            values
        );
        for cut in 0..bytes.len() {
            assert!(read_fixed::<F, _>(&mut &bytes[..cut], 3).is_err());
        }
    }
    for bad in [
        vec![255],
        vec![BITSET, 0b1000_0010],
        vec![BITSET, 0],
        vec![BITSET, 7],
    ] {
        assert!(read_fixed::<F, _>(&mut bad.as_slice(), 3).is_err());
    }
    for value in [F::ZERO, F::from(9)] {
        let mut redundant = vec![RAW];
        for _ in 0..3 {
            redundant.extend_from_slice(value.to_repr().as_ref());
        }
        assert!(read_fixed::<F, _>(&mut redundant.as_slice(), 3).is_err());
    }
    let mut redundant = vec![RAW];
    for value in [F::ZERO, F::ONE, F::ZERO] {
        redundant.extend_from_slice(value.to_repr().as_ref());
    }
    assert!(read_fixed::<F, _>(&mut redundant.as_slice(), 3).is_err());
    for tag in [CONSTANT, RAW] {
        let mut noncanonical = vec![tag];
        noncanonical.extend(vec![255; scalar_bytes::<F>() * 3]);
        assert!(read_fixed::<F, _>(&mut noncanonical.as_slice(), 3).is_err());
    }
    assert!(fixed_mode::<F>(&[]).is_err());
    assert!(read_fixed::<F, _>(&mut &[CONSTANT][..], 0).is_err());
}

#[test]
fn structured_fixed_modes_are_unique_and_reject_padding_fields_and_truncation() {
    modes::<Fp>();
    modes::<Fq>();
}

fn index_model<F: PrimeField + WithSmallOrderMulGroup<3>>() {
    for k in [0, 3, 6] {
        let rows = 1 << k;
        let domain = EvaluationDomain::<F>::new(2, k);
        for columns in [0, 1, 7] {
            let index = InverseIndex::new(rows, columns, domain.get_omega()).unwrap();
            let mut seen = Seen::new(rows * columns).unwrap();
            let mut delta = F::ONE;
            for column in 0..columns {
                let mut omega = F::ONE;
                for row in 0..rows {
                    let target = index.target(omega * delta).unwrap();
                    assert_eq!(target as usize, column * rows + row);
                    seen.mark(target).unwrap();
                    omega *= domain.get_omega();
                }
                delta *= F::DELTA;
            }
            assert!(index.target(F::ZERO).is_err());
            assert!(
                index
                    .target(F::DELTA.pow_vartime([columns as u64]))
                    .is_err()
            );
            assert!(seen.mark((rows * columns) as u32).is_err());
            if columns > 0 {
                assert!(seen.mark(0).is_err());
            }
        }
    }
    assert!(permutation_cells(0, 1, F::ONE).is_err());
    assert!(permutation_cells(3, 1, F::ONE).is_err());
    assert!(permutation_cells(8, 1, F::ONE).is_err());
    assert!(permutation_cells(8, usize::MAX, F::ONE).is_err());
    let mut duplicate = vec![(F::ONE.to_repr(), 0), (F::ONE.to_repr(), 1)];
    assert!(sorted_unique::<F>(&mut duplicate).is_err());
}

#[test]
fn structured_inverse_labels_match_both_fields_and_enforce_bijection_bounds() {
    index_model::<Fp>();
    index_model::<Fq>();
}

fn corruptions<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(6);
    let pk = keygen_pk2(
        &params,
        &KeyCircuit {
            value: Value::unknown(),
        },
        false,
    )
    .unwrap();
    let mut bytes = Vec::new();
    pk.write_structured_v1(&mut bytes).unwrap();
    let length = bytes.len() as u64;
    let vk_offset = HEADER_BYTES as usize;
    let masks = vk_offset + pk.get_vk().to_bytes(SerdeFormat::Processed).len();
    let poly_bytes = 4 + 64 * scalar_bytes::<C::Scalar>();
    let fixed_count = masks + 3 * poly_bytes;
    let mut offset = fixed_count + 4;
    let mut fixed_offsets = Vec::new();
    for polynomial in &pk.fixed_values {
        let payload =
            fixed_payload_bytes::<C::Scalar>(fixed_mode(polynomial).unwrap(), 64).unwrap() as usize;
        fixed_offsets.push((offset, payload));
        offset += 1 + payload;
    }
    let permutation_count = offset;
    let targets = permutation_count + 4;
    let cells = pk.permutation.permutations.len() * 64;
    assert_eq!(targets + 4 * cells, bytes.len());
    for at in [
        0,
        15,
        16,
        47,
        48,
        55,
        vk_offset,
        vk_offset + 1,
        vk_offset + 5,
    ] {
        let mut bad = bytes.clone();
        bad[at] ^= 255;
        assert!(read_key::<C>(&bad, 6, length).is_err(), "header at {at}");
    }
    for k in [0, 5, 7, u32::MAX] {
        assert!(read_key::<C>(&bytes, k, length).is_err());
    }
    for len in [0, 55, length - 1, length + 1, u64::MAX] {
        assert!(read_key::<C>(&bytes, 6, len).is_err());
    }
    for at in [fixed_count, permutation_count] {
        for value in [0_u32, u32::MAX] {
            let mut bad = bytes.clone();
            bad[at..at + 4].copy_from_slice(&value.to_be_bytes());
            assert!(read_key::<C>(&bad, 6, length).is_err());
        }
    }
    for index in 0..3 {
        let at = masks + index * poly_bytes;
        for value in [0_u32, 63, 65, u32::MAX] {
            let mut bad = bytes.clone();
            bad[at..at + 4].copy_from_slice(&value.to_be_bytes());
            assert!(read_key::<C>(&bad, 6, length).is_err());
        }
        let mut bad = bytes.clone();
        bad[at + 4..at + 4 + scalar_bytes::<C::Scalar>()].fill(255);
        assert!(read_key::<C>(&bad, 6, length).is_err());
    }
    for &(at, _) in &fixed_offsets {
        let mut bad = bytes.clone();
        bad[at] = 255;
        assert!(read_key::<C>(&bad, 6, length).is_err());
    }
    for target in [cells as u32, u32::MAX] {
        let mut bad = bytes.clone();
        bad[targets..targets + 4].copy_from_slice(&target.to_le_bytes());
        assert!(read_key::<C>(&bad, 6, length).is_err());
    }
    let mut duplicate = bytes.clone();
    duplicate.copy_within(targets..targets + 4, targets + 4);
    assert!(read_key::<C>(&duplicate, 6, length).is_err());
    let mut padded = bytes.clone();
    padded.push(0);
    padded[48..56].copy_from_slice(&(length + 1).to_le_bytes());
    assert!(read_key::<C>(&padded, 6, length + 1).is_err());
    let mut point = bytes.clone();
    point[vk_offset + 10..vk_offset + 42].fill(255);
    assert!(read_key::<C>(&point, 6, length).is_err());
    let mut boundaries = vec![
        0,
        1,
        16,
        48,
        56,
        masks,
        fixed_count,
        permutation_count,
        targets,
        bytes.len() - 1,
    ];
    for (at, payload) in fixed_offsets {
        boundaries.extend([at, at + 1, at + payload]);
    }
    for cut in boundaries {
        assert!(
            read_key::<C>(&bytes[..cut], 6, length).is_err(),
            "truncation {cut}"
        );
    }
}

#[test]
fn structured_frames_reject_wrong_codec_curve_domain_shape_ids_and_truncation() {
    corruptions::<EqAffine>();
    corruptions::<EpAffine>();
    assert_ne!(curve_domain::<EqAffine>(), curve_domain::<EpAffine>());
    let params = ParamsIPA::<EqAffine>::new(6);
    let pk = keygen_pk2(
        &params,
        &KeyCircuit {
            value: Value::unknown(),
        },
        false,
    )
    .unwrap();
    let mut bytes = Vec::new();
    pk.write_structured_v1(&mut bytes).unwrap();
    assert!(read_key::<EpAffine>(&bytes, 6, bytes.len() as u64).is_err());
}

fn writer_rejections<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(6);
    for corruption in 0..11 {
        let mut pk = keygen_pk2(
            &params,
            &KeyCircuit {
                value: Value::unknown(),
            },
            false,
        )
        .unwrap();
        match corruption {
            0 => pk.fixed_polys[0][0] += C::Scalar::ONE,
            1 => pk.fixed_values[0][0] += C::Scalar::ONE,
            2 => pk.permutation.polys[0][0] += C::Scalar::ONE,
            3 => pk.permutation.permutations[0][0] += C::Scalar::ONE,
            4 => {
                pk.fixed_polys.pop();
            }
            5 => {
                pk.permutation.permutations.pop();
            }
            6 => {
                pk.fixed_values[0].values.pop();
            }
            7 => {
                pk.l0.values.pop();
            }
            8..=10 => {
                pk.permutation.permutations[0][0] = match corruption {
                    8 => C::Scalar::ZERO,
                    9 => pk.permutation.permutations[0][1],
                    _ => C::Scalar::DELTA.pow_vartime([pk.permutation.permutations.len() as u64]),
                };
                pk.permutation.polys[0] =
                    coefficients(&pk.vk.domain, &pk.permutation.permutations[0]).unwrap();
            }
            _ => unreachable!(),
        }
        let mut bytes = Vec::new();
        assert!(
            pk.write_structured_v1(&mut bytes).is_err(),
            "corruption {corruption}"
        );
        assert!(
            bytes.is_empty(),
            "complete pre-output validation {corruption}"
        );
    }
}

#[test]
fn structured_writer_rejects_bad_bases_shapes_and_nonpermutations_before_output() {
    writer_rejections::<EqAffine>();
    writer_rejections::<EpAffine>();
}

#[test]
fn structured_reader_and_writer_propagate_io_errors_without_legacy_unwraps() {
    struct FailingWriter {
        remaining: usize,
    }
    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::StorageFull,
                    "synthetic write failure",
                ));
            }
            let count = bytes.len().min(self.remaining);
            self.remaining -= count;
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    struct FailingReader;
    impl Read for FailingReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "synthetic read failure",
            ))
        }
    }
    let params = ParamsIPA::<EqAffine>::new(6);
    let pk = keygen_pk2(
        &params,
        &KeyCircuit {
            value: Value::unknown(),
        },
        false,
    )
    .unwrap();
    let length = pk.structured_v1_bytes_length().unwrap();
    for remaining in [0, 15, 48, 56, 100, length as usize - 1] {
        assert_eq!(
            pk.write_structured_v1(&mut FailingWriter { remaining })
                .unwrap_err()
                .kind(),
            io::ErrorKind::StorageFull
        );
    }
    assert_eq!(
        ProvingKey::<EqAffine>::read_structured_v1_checked::<_, KeyCircuit<Fp>>(
            &mut FailingReader,
            6,
            length,
            #[cfg(feature = "circuit-params")]
            (),
        )
        .unwrap_err()
        .kind(),
        io::ErrorKind::PermissionDenied
    );
}

mod consuming;

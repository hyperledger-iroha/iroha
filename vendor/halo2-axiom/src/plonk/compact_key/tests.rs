//! Both-Pasta compact-key reconstruction, real-proof equivalence and malformed-frame checks.

use super::*;
use crate::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::pasta::{EpAffine, EqAffine, Fp},
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
    instance: Column<Instance>,
    selector: Selector,
}

impl<F: Field> Circuit<F> for KeyCircuit<F> {
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
        let instance = meta.instance_column();
        let selector = meta.selector();
        meta.enable_equality(left);
        meta.enable_equality(right);
        meta.enable_equality(instance);
        meta.create_gate("nonzero fixed data and copied public advice", |meta| {
            let q = meta.query_selector(selector);
            let lhs = meta.query_advice(left, Rotation::cur());
            let rhs = meta.query_advice(right, Rotation::cur());
            let fixed = meta.query_fixed(fixed, Rotation::cur());
            vec![
                q.clone() * (lhs - rhs),
                q * (fixed - crate::plonk::Expression::Constant(F::ONE)),
            ]
        });
        KeyConfig {
            left,
            right,
            fixed,
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
            || "compact key fixture",
            |mut region| {
                config.selector.enable(&mut region, 0)?;
                region.assign_fixed(config.fixed, 0, F::ONE);
                let left = region.assign_advice(config.left, 0, self.value);
                left.copy_advice(&mut region, config.right, 0);
                Ok(left.cell())
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
    ProvingKey::read_compact_v1_checked::<_, KeyCircuit<C::Scalar>>(
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
    let params = ParamsIPA::<C>::new(6);
    let circuit = KeyCircuit {
        value: Value::unknown(),
    };
    for compressed in [false, true] {
        let pk = keygen_pk2(&params, &circuit, compressed).unwrap();
        let legacy = pk.to_bytes(SerdeFormat::Processed);
        let vk = pk.get_vk().to_bytes(SerdeFormat::Processed);
        let mut compact = Vec::new();
        pk.write_compact_v1(&mut compact).unwrap();
        assert_eq!(compact.len() as u64, pk.compact_v1_bytes_length().unwrap());
        let removed = (pk.fixed_polys.len() + pk.permutation.polys.len()) * (4 + 64 * 32) + 8;
        assert_eq!(
            compact.len(),
            legacy.len() - removed + HEADER_BYTES as usize
        );
        assert!(compact.len() < legacy.len());
        let restored = read_key::<C>(&compact, 6, compact.len() as u64).unwrap();
        assert_eq!(restored.to_bytes(SerdeFormat::Processed), legacy);
        assert_eq!(restored.get_vk().to_bytes(SerdeFormat::Processed), vk);
        assert_eq!(
            restored.get_vk().transcript_repr(),
            pk.get_vk().transcript_repr()
        );
        let mut repeated = Vec::new();
        restored.write_compact_v1(&mut repeated).unwrap();
        assert_eq!(repeated, compact);
        for witness in [C::Scalar::from(3), C::Scalar::from(19)] {
            let expected = proof(&params, &pk, witness);
            let actual = proof(&params, &restored, witness);
            assert_eq!(
                actual, expected,
                "same key, circuit, instance and RNG transcript"
            );
            assert!(verify(&params, pk.get_vk(), witness, &actual));
            assert!(verify(&params, restored.get_vk(), witness, &expected));
            assert!(!verify(
                &params,
                pk.get_vk(),
                witness + C::Scalar::ONE,
                &actual
            ));
            let mut corrupted = actual;
            corrupted[0] ^= 1;
            assert!(!verify(&params, pk.get_vk(), witness, &corrupted));
        }
        // Legacy readers/writers are unchanged; neither codec silently guesses the other.
        assert!(read_key::<C>(&legacy, 6, legacy.len() as u64).is_err());
        assert!(
            ProvingKey::<C>::read_checked::<_, KeyCircuit<C::Scalar>>(
                &mut compact.as_slice(),
                SerdeFormat::Processed,
                6,
                #[cfg(feature = "circuit-params")]
                (),
            )
            .is_err()
        );
        // The explicit frame is consumed exactly; enclosing authenticated EOF is a caller duty.
        let mut framed = compact.clone();
        framed.push(0xA7);
        let mut cursor = io::Cursor::new(framed);
        ProvingKey::<C>::read_compact_v1_checked::<_, KeyCircuit<C::Scalar>>(
            &mut cursor,
            6,
            compact.len() as u64,
            #[cfg(feature = "circuit-params")]
            (),
        )
        .unwrap();
        assert_eq!(cursor.position(), compact.len() as u64);
        let mut sentinel = [0];
        cursor.read_exact(&mut sentinel).unwrap();
        assert_eq!(sentinel, [0xA7]);
    }
}

#[test]
fn compact_keys_reconstruct_exact_processed_keys_and_real_eq_ep_proofs() {
    roundtrip::<EqAffine>();
    roundtrip::<EpAffine>();
}

fn corruptions<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(6);
    let circuit = KeyCircuit {
        value: Value::unknown(),
    };
    let pk = keygen_pk2(&params, &circuit, true).unwrap();
    let mut bytes = Vec::new();
    pk.write_compact_v1(&mut bytes).unwrap();
    let length = bytes.len() as u64;
    let vk_offset = HEADER_BYTES as usize;
    let body_offset = vk_offset + pk.get_vk().to_bytes(SerdeFormat::Processed).len();
    let polynomial_bytes = 4 + 64 * 32;
    let mut polynomial_offsets = (0..3)
        .map(|index| body_offset + index * polynomial_bytes)
        .collect::<Vec<_>>();
    let mut vector_offsets = Vec::new();
    let mut offset = body_offset + 3 * polynomial_bytes;
    for count in [pk.fixed_values.len(), pk.permutation.permutations.len()] {
        vector_offsets.push(offset);
        offset += 4;
        for _ in 0..count {
            polynomial_offsets.push(offset);
            offset += polynomial_bytes;
        }
    }
    assert_eq!(offset, bytes.len());
    assert!(read_key::<C>(&bytes, 5, length).is_err());
    assert!(read_key::<C>(&bytes, u32::MAX, length).is_err());
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
        let mut malformed = bytes.clone();
        malformed[at] ^= 0xFF;
        assert!(
            read_key::<C>(&malformed, 6, length).is_err(),
            "header byte {at}"
        );
    }
    for expected in [0, HEADER_BYTES - 1, length - 1, length + 1, u64::MAX] {
        assert!(read_key::<C>(&bytes, 6, expected).is_err());
    }
    let mut padded = bytes.clone();
    padded.push(0);
    padded[48..56].copy_from_slice(&(length + 1).to_le_bytes());
    assert!(read_key::<C>(&padded, 6, length + 1).is_err());
    let mut impossible_shape = bytes[..vk_offset + 10].to_vec();
    impossible_shape[vk_offset + 6..vk_offset + 10].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        read_key::<C>(&impossible_shape, 6, length)
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
    for at in &vector_offsets {
        for count in [0_u32, u32::MAX] {
            let mut malformed = bytes[..at + 4].to_vec();
            malformed[*at..at + 4].copy_from_slice(&count.to_be_bytes());
            assert_eq!(
                read_key::<C>(&malformed, 6, length).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
    }
    for at in &polynomial_offsets {
        for count in [0_u32, 63, 65, u32::MAX] {
            let mut malformed = bytes[..at + 4].to_vec();
            malformed[*at..at + 4].copy_from_slice(&count.to_be_bytes());
            assert_eq!(
                read_key::<C>(&malformed, 6, length).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        let mut noncanonical = bytes.clone();
        noncanonical[at + 4..at + 4 + 32].fill(0xFF);
        assert_eq!(
            read_key::<C>(&noncanonical, 6, length).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
    let mut invalid_point = bytes.clone();
    invalid_point[vk_offset + 10..vk_offset + 10 + 32].fill(0xFF);
    assert!(read_key::<C>(&invalid_point, 6, length).is_err());
    let mut boundaries = vec![0, 1, 15, 16, 47, 48, 55, 56, body_offset, bytes.len() - 1];
    for at in polynomial_offsets {
        boundaries.extend([at, at + 2, at + 4, at + 4 + 31, at + polynomial_bytes - 1]);
    }
    for length_cut in boundaries {
        assert!(
            read_key::<C>(&bytes[..length_cut], 6, length).is_err(),
            "truncation {length_cut}"
        );
    }
    // Header-declared domain size never overrides trusted caller policy.
    let mut huge_k = bytes[..vk_offset + 10].to_vec();
    huge_k[vk_offset + 1..vk_offset + 5].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(read_key::<C>(&huge_k, 6, length).is_err());
    assert!(read_key::<C>(&huge_k, u32::MAX, length).is_err());
}

#[test]
fn compact_reader_rejects_bad_framing_shapes_fields_points_and_truncation() {
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
    pk.write_compact_v1(&mut bytes).unwrap();
    assert!(read_key::<EpAffine>(&bytes, 6, bytes.len() as u64).is_err());
}

#[test]
fn compact_writer_rejects_dropped_or_disagreeing_bases_before_output() {
    let params = ParamsIPA::<EqAffine>::new(6);
    let circuit = KeyCircuit {
        value: Value::unknown(),
    };
    for corruption in 0..8 {
        let mut pk = keygen_pk2(&params, &circuit, false).unwrap();
        match corruption {
            0 => pk.fixed_polys[0][0] += Fp::ONE,
            1 => pk.fixed_values[0][0] += Fp::ONE,
            2 => pk.permutation.polys[0][0] += Fp::ONE,
            3 => pk.permutation.permutations[0][0] += Fp::ONE,
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
            _ => unreachable!(),
        }
        let mut output = Vec::new();
        assert!(
            pk.write_compact_v1(&mut output).is_err(),
            "corruption {corruption}"
        );
        assert!(output.is_empty());
    }
}

#[test]
fn compact_writer_and_reader_propagate_io_failures_without_panic() {
    struct FailingWriter {
        remaining: usize,
    }
    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::StorageFull,
                    "synthetic storage error",
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
                "synthetic read error",
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
    let len = pk.compact_v1_bytes_length().unwrap();
    for remaining in [0, 15, 48, 56, 100, len as usize - 1] {
        assert_eq!(
            pk.write_compact_v1(&mut FailingWriter { remaining })
                .unwrap_err()
                .kind(),
            io::ErrorKind::StorageFull
        );
    }
    assert_eq!(
        ProvingKey::<EqAffine>::read_compact_v1_checked::<_, KeyCircuit<Fp>>(
            &mut FailingReader,
            6,
            len,
            #[cfg(feature = "circuit-params")]
            (),
        )
        .unwrap_err()
        .kind(),
        io::ErrorKind::PermissionDenied
    );
}

mod consuming;

//! Consuming structured-key bytes, genuine reload proofs and sink/error contracts.
//!
//! These tests do not instrument allocation lifetimes or measure process RSS.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn roundtrip_consuming<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(6);
    for compressed in [false, true] {
        let key = keygen_pk2(
            &params,
            &KeyCircuit {
                value: Value::unknown(),
            },
            compressed,
        )
        .unwrap();
        let processed = key.to_bytes(SerdeFormat::Processed);
        let verifying = key.get_vk().to_bytes(SerdeFormat::Processed);
        let length = key.structured_v1_bytes_length().unwrap();
        let mut expected = Vec::new();
        key.write_structured_v1(&mut expected).unwrap();
        let witness = C::Scalar::from(23);
        let expected_proof = proof(&params, &key, witness);
        assert!(verify(&params, key.get_vk(), witness, &expected_proof));

        let mut bytes = Vec::new();
        key.write_structured_v1_consuming(&mut bytes).unwrap();
        assert_eq!(bytes, expected, "exact borrowed/consuming structured frame");
        assert_eq!(bytes.len() as u64, length);
        let restored = read_key::<C>(&bytes, 6, length).unwrap();
        assert_eq!(restored.to_bytes(SerdeFormat::Processed), processed);
        assert_eq!(
            restored.get_vk().to_bytes(SerdeFormat::Processed),
            verifying
        );
        let restored_proof = proof(&params, &restored, witness);
        assert_eq!(restored_proof, expected_proof, "same key, witness and RNG");
        assert!(verify(&params, restored.get_vk(), witness, &restored_proof));
        assert!(!verify(
            &params,
            restored.get_vk(),
            witness + C::Scalar::ONE,
            &restored_proof,
        ));
    }
}

#[test]
fn consuming_structured_keys_preserve_eq_ep_bytes_and_real_reload_proofs() {
    roundtrip_consuming::<EqAffine>();
    roundtrip_consuming::<EpAffine>();
}

fn reject_before_output<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(6);
    for compressed in [false, true] {
        let template = keygen_pk2(
            &params,
            &KeyCircuit {
                value: Value::unknown(),
            },
            compressed,
        )
        .unwrap();
        for corruption in 0..14 {
            let mut key = template.clone();
            match corruption {
                0 => key.fixed_polys[0][0] += C::Scalar::ONE,
                1 => key.fixed_values[0][0] += C::Scalar::ONE,
                2 => key.permutation.polys[0][0] += C::Scalar::ONE,
                3 => key.permutation.permutations[0][0] += C::Scalar::ONE,
                4 => {
                    key.fixed_polys.pop();
                }
                5 => {
                    key.permutation.permutations.pop();
                }
                6 => {
                    key.fixed_values[0].values.pop();
                }
                7 => {
                    key.l0.values.pop();
                }
                8..=10 => {
                    key.permutation.permutations[0][0] = match corruption {
                        8 => C::Scalar::ZERO,
                        9 => key.permutation.permutations[0][1],
                        _ => C::Scalar::DELTA
                            .pow_vartime([key.permutation.permutations.len() as u64]),
                    };
                    // Keep the bases consistent so the rejection must reach sigma validation.
                    key.permutation.polys[0] =
                        coefficients(&key.vk.domain, &key.permutation.permutations[0]).unwrap();
                }
                11 => {
                    key.l_last.values.pop();
                }
                12 => {
                    key.l_active_row.values.pop();
                }
                13 => {
                    key.permutation.polys[0].values.pop();
                }
                _ => unreachable!(),
            }
            let mut borrowed = Vec::new();
            let expected_error = key.write_structured_v1(&mut borrowed).unwrap_err().kind();
            assert_eq!(expected_error, io::ErrorKind::InvalidData);
            assert!(borrowed.is_empty());
            let mut consumed = Vec::new();
            assert_eq!(
                key.write_structured_v1_consuming(&mut consumed)
                    .unwrap_err()
                    .kind(),
                expected_error,
            );
            assert!(consumed.is_empty(), "corruption {corruption} emitted bytes");
        }
    }
}

#[test]
fn consuming_structured_writer_rejects_invalid_bases_shapes_and_sigmas_before_output() {
    reject_before_output::<EqAffine>();
    reject_before_output::<EpAffine>();
}

#[derive(Clone, Copy)]
enum End {
    Complete,
    ErrorAt(usize),
    ZeroAt(usize),
    PanicAt(usize),
}

struct ShortSink {
    bytes: Vec<u8>,
    end: End,
    interrupt_once: bool,
}

impl Write for ShortSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.interrupt_once {
            self.interrupt_once = false;
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        let remaining = match self.end {
            End::Complete => usize::MAX,
            End::ErrorAt(at) | End::ZeroAt(at) | End::PanicAt(at) => {
                at.saturating_sub(self.bytes.len())
            }
        };
        if remaining == 0 {
            match self.end {
                End::ErrorAt(_) => return Err(io::Error::from(io::ErrorKind::StorageFull)),
                End::ZeroAt(_) => return Ok(0),
                End::PanicAt(_) => panic!("synthetic structured consuming sink unwind"),
                End::Complete => unreachable!(),
            }
        }
        let count = bytes.len().min(7).min(remaining);
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("the caller owns sink flushing")
    }
}

fn sink_contract<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(6);
    for compressed in [false, true] {
        let template = keygen_pk2(
            &params,
            &KeyCircuit {
                value: Value::unknown(),
            },
            compressed,
        )
        .unwrap();
        let mut expected = Vec::new();
        template.write_structured_v1(&mut expected).unwrap();
        let mut short = ShortSink {
            bytes: Vec::new(),
            end: End::Complete,
            interrupt_once: true,
        };
        template
            .clone()
            .write_structured_v1_consuming(&mut short)
            .unwrap();
        assert_eq!(short.bytes, expected);

        let vk_end = HEADER_BYTES as usize + template.vk.to_bytes(SerdeFormat::Processed).len();
        let mask_bytes = 4 + 64 * scalar_bytes::<C::Scalar>();
        let fixed_start = vk_end + 3 * mask_bytes;
        let mut boundaries = vec![0, 15, 56, vk_end, vk_end + mask_bytes, fixed_start];
        let mut position = fixed_start + 4;
        for polynomial in &template.fixed_values {
            boundaries.extend([position, position + 1]);
            position += 1 + fixed_payload_bytes::<C::Scalar>(fixed_mode(polynomial).unwrap(), 64)
                .unwrap() as usize;
        }
        boundaries.extend([position, position + 4, expected.len() - 1]);
        for offset in boundaries {
            for end in [
                End::ErrorAt(offset),
                End::ZeroAt(offset),
                End::PanicAt(offset),
            ] {
                let key = template.clone();
                let mut sink = ShortSink {
                    bytes: Vec::new(),
                    end,
                    interrupt_once: false,
                };
                let result = catch_unwind(AssertUnwindSafe(|| {
                    key.write_structured_v1_consuming(&mut sink)
                }));
                match end {
                    End::ErrorAt(_) => assert_eq!(
                        result.unwrap().unwrap_err().kind(),
                        io::ErrorKind::StorageFull,
                    ),
                    End::ZeroAt(_) => assert_eq!(
                        result.unwrap().unwrap_err().kind(),
                        io::ErrorKind::WriteZero,
                    ),
                    End::PanicAt(_) => assert_eq!(
                        result.unwrap_err().downcast_ref::<&'static str>(),
                        Some(&"synthetic structured consuming sink unwind"),
                        "the intended sink panic, not a validation assertion",
                    ),
                    End::Complete => unreachable!(),
                }
                assert_eq!(sink.bytes, expected[..offset]);
            }
        }
    }
}

#[test]
fn consuming_structured_writer_handles_short_writes_errors_zero_writes_and_unwind() {
    sink_contract::<EqAffine>();
    sink_contract::<EpAffine>();
}

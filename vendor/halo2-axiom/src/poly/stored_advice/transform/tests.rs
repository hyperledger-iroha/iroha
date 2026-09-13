//! Basis-equivalence and fail-stop tests using a plaintext recording backend as an oracle.
//!
//! This backend asserts a shared single-operation window. It does not replace Core's
//! authenticated-spool, filesystem, or zeroizing-backend qualification.

use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use ff::Field;
use halo2curves::pasta::{Fp, Fq};

use super::super::StoredPastaFieldV1;
use super::*;

#[derive(Default)]
struct Record {
    creates: usize,
    reads: usize,
    writes: usize,
    seals: usize,
    writer_drops: usize,
    snapshot_drops: usize,
    fail_create: bool,
    fail_read: Option<u64>,
    panic_read: Option<u64>,
    fail_write: Option<u64>,
    panic_write: Option<u64>,
    fail_seal: bool,
    short_chunk: bool,
    destination_override: Option<StoredAdviceLayoutV1>,
    returned_override: Option<StoredAdviceLayoutV1>,
    change_writer_after_write: bool,
    change_source_after_read: Option<u64>,
}

#[derive(Default)]
struct State {
    active: Cell<bool>,
    record: RefCell<Record>,
}

struct Window(Rc<State>);

impl Window {
    fn acquire(state: &Rc<State>) -> Result<Self, StoredAdviceErrorV1> {
        if state.active.replace(true) {
            return Err(StoredAdviceErrorV1::Busy);
        }
        Ok(Self(Rc::clone(state)))
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.0.active.set(false);
    }
}

struct Provider {
    state: Rc<State>,
    next_ordinal: u64,
}

struct Writer {
    layout: StoredAdviceLayoutV1,
    state: Rc<State>,
    values: Vec<[u8; 32]>,
    chunks: u64,
}

struct Snapshot {
    layout: StoredAdviceLayoutV1,
    state: Rc<State>,
    values: Vec<[u8; 32]>,
    poisoned: bool,
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.state.record.borrow_mut().writer_drops += 1;
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        self.state.record.borrow_mut().snapshot_drops += 1;
    }
}

impl StoredAdviceProviderV1 for Provider {
    type Writer = Writer;

    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        column: u32,
        phase: u8,
    ) -> Result<Writer, StoredAdviceErrorV1> {
        let _window = Window::acquire(&self.state)?;
        let mut record = self.state.record.borrow_mut();
        record.creates += 1;
        if record.fail_create {
            return Err(StoredAdviceErrorV1::Storage);
        }
        let layout = record
            .destination_override
            .unwrap_or(StoredAdviceLayoutV1::new(
                [5; 32],
                self.next_ordinal,
                field,
                basis,
                k,
                column,
                phase,
            )?);
        self.next_ordinal += 1;
        Ok(Writer {
            layout,
            state: Rc::clone(&self.state),
            values: Vec::new(),
            chunks: 0,
        })
    }
}

impl StoredAdviceWriterV1 for Writer {
    type Snapshot = Snapshot;

    fn layout(&self) -> StoredAdviceLayoutV1 {
        let mut layout = self.layout;
        if self.chunks != 0 && self.state.record.borrow().change_writer_after_write {
            layout.column += 1;
        }
        layout
    }

    fn write_chunk(&mut self, chunk: u64, values: &[[u8; 32]]) -> Result<(), StoredAdviceErrorV1> {
        let _window = Window::acquire(&self.state)?;
        let mut record = self.state.record.borrow_mut();
        record.writes += 1;
        assert_ne!(record.panic_write, Some(chunk), "injected write panic");
        if record.fail_write == Some(chunk) {
            return Err(StoredAdviceErrorV1::Storage);
        }
        assert_eq!(chunk, self.chunks);
        assert_eq!(values.len(), self.layout.chunk_scalar_count(chunk).unwrap());
        assert!(
            values
                .iter()
                .all(|value| self.layout.field().is_canonical(value))
        );
        self.values.extend_from_slice(values);
        self.chunks += 1;
        Ok(())
    }

    fn seal(mut self) -> Result<Snapshot, StoredAdviceErrorV1> {
        let _window = Window::acquire(&self.state)?;
        let mut record = self.state.record.borrow_mut();
        record.seals += 1;
        if record.fail_seal {
            return Err(StoredAdviceErrorV1::Storage);
        }
        assert_eq!(self.chunks as usize, self.layout.chunk_count());
        Ok(Snapshot {
            layout: record.returned_override.unwrap_or(self.layout),
            state: Rc::clone(&self.state),
            values: std::mem::take(&mut self.values),
            poisoned: false,
        })
    }
}

impl StoredAdviceSnapshotV1 for Snapshot {
    fn layout(&self) -> StoredAdviceLayoutV1 {
        self.layout
    }

    fn with_chunk<R>(
        &mut self,
        expected: StoredAdviceLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        if self.poisoned {
            return Err(StoredAdviceErrorV1::Poisoned);
        }
        if expected != self.layout {
            return Err(StoredAdviceErrorV1::Context);
        }
        let _window = Window::acquire(&self.state)?;
        self.poisoned = true;
        let short_chunk = {
            let mut record = self.state.record.borrow_mut();
            record.reads += 1;
            assert_ne!(record.panic_read, Some(chunk), "injected read panic");
            if record.fail_read == Some(chunk) {
                return Err(StoredAdviceErrorV1::Authentication);
            }
            record.short_chunk
        };
        let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
        let count = self.layout.chunk_scalar_count(chunk)? - usize::from(short_chunk);
        let result = consume(&self.values[start..start + count])?;
        if self.state.record.borrow().change_source_after_read == Some(chunk) {
            self.layout.column += 1;
        }
        self.poisoned = false;
        Ok(result)
    }

    fn with_column<R>(
        &mut self,
        _expected: StoredAdviceLayoutV1,
        _consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        panic!("conversion must not allocate a second complete encoded column")
    }
}

fn setup<F: StoredAssignmentFieldV1>(
    basis: StoredPolynomialBasisV1,
    k: u32,
    values: &[F],
) -> (Provider, Snapshot, Rc<State>) {
    let state = Rc::new(State::default());
    let layout = StoredAdviceLayoutV1::new([5; 32], 7, F::STORED_FIELD, basis, k, 3, 1).unwrap();
    assert_eq!(values.len(), layout.scalar_count());
    (
        Provider {
            state: Rc::clone(&state),
            next_ordinal: 8,
        },
        Snapshot {
            layout,
            state: Rc::clone(&state),
            values: values.iter().map(|value| value.to_repr()).collect(),
            poisoned: false,
        },
        state,
    )
}

fn oracle<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>(
    domain: &EvaluationDomain<F>,
    coefficients: &[F],
    basis: StoredPolynomialBasisV1,
) -> Vec<F> {
    // A size-one polynomial is constant in every basis. The existing recursive forward FFT
    // indexes its first stage even though this domain has no stages; use this independent
    // mathematical oracle for k=0 rather than changing existing prover dispatch here.
    if coefficients.len() == 1 {
        return coefficients.to_vec();
    }
    match basis {
        StoredPolynomialBasisV1::Coefficient => coefficients.to_vec(),
        StoredPolynomialBasisV1::Lagrange => {
            // The existing forward domain method's coset scaling cancels exactly here.
            domain
                .coeff_to_extended_part(
                    domain.coeff_from_vec(coefficients.to_vec()),
                    F::ZETA.invert().unwrap(),
                )
                .values
        }
        StoredPolynomialBasisV1::CosetPart { part, .. } => {
            domain
                .coeff_to_extended_part(
                    domain.coeff_from_vec(coefficients.to_vec()),
                    domain.get_extended_omega().pow_vartime([u64::from(part)]),
                )
                .values
        }
    }
}

fn conversion_matrix<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart, Lagrange};
    for k in [0, 1, 3, 9] {
        let domain = EvaluationDomain::<F>::new(5, k);
        let coefficients = (0..1_u64 << k)
            .map(|index| F::from(index * index + 3 * index + 7))
            .collect::<Vec<_>>();
        let bases = [
            Coefficient,
            Lagrange,
            CosetPart {
                extension_log: 2,
                part: 0,
            },
            CosetPart {
                extension_log: 2,
                part: 1,
            },
            CosetPart {
                extension_log: 2,
                part: 3,
            },
        ];
        for from in bases {
            let source_values = oracle(&domain, &coefficients, from);
            // Independently exercise the existing inverse domain methods for this input.
            let recovered = match from {
                Coefficient => source_values.clone(),
                Lagrange => {
                    domain
                        .lagrange_to_coeff(domain.lagrange_from_vec(source_values.clone()))
                        .values
                }
                CosetPart { part, .. } => {
                    domain
                        .extended_part_to_coeff(
                            domain.lagrange_from_vec(source_values.clone()),
                            domain.get_extended_omega().pow_vartime([u64::from(part)]),
                        )
                        .values
                }
            };
            assert_eq!(recovered, coefficients);
            for to in bases {
                let (mut provider, mut source, state) = setup(from, k, &source_values);
                let expected = source.layout;
                let destination =
                    convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, to)
                        .unwrap();
                let expected_values = oracle(&domain, &coefficients, to)
                    .iter()
                    .map(|value| value.to_repr())
                    .collect::<Vec<_>>();
                assert_eq!(
                    destination.values, expected_values,
                    "k={k}, {from:?} -> {to:?}"
                );
                assert_eq!(destination.layout.ordinal(), 8);
                assert_eq!(destination.layout.basis(), to);
                assert_eq!(destination.layout.field(), expected.field());
                assert_eq!(destination.layout.column(), expected.column());
                assert_eq!(destination.layout.phase(), expected.phase());
                assert_eq!(destination.layout.proof_context, expected.proof_context);
                assert_eq!(source.layout, expected);
                assert_eq!(
                    source.values,
                    source_values
                        .iter()
                        .map(|v| v.to_repr())
                        .collect::<Vec<_>>()
                );
                assert!(!source.poisoned);
                assert!(!state.active.get());
                let record = state.record.borrow();
                assert_eq!(record.reads, expected.chunk_count());
                assert_eq!(record.writes, expected.chunk_count());
                assert_eq!(record.creates, 1);
                assert_eq!(record.seals, 1);
            }
        }
    }
}

#[test]
fn fp_all_basis_pairs_match_existing_domain_transforms() {
    conversion_matrix::<Fp>();
}

#[test]
fn fq_all_basis_pairs_match_existing_domain_transforms() {
    conversion_matrix::<Fq>();
}

#[test]
fn wrong_source_identity_field_domain_and_coset_fail_before_backend_creation() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart, Lagrange};
    let domain = EvaluationDomain::<Fp>::new(5, 3);
    let (mut provider, mut source, state) = setup(Lagrange, 3, &[Fp::ONE; 8]);
    let original = source.layout;
    let mut variants = [original; 7];
    variants[0].proof_context[0] ^= 1;
    variants[1].ordinal += 1;
    variants[2].field = StoredPastaFieldV1::Fq;
    variants[3].basis = Coefficient;
    variants[4].k += 1;
    variants[5].column += 1;
    variants[6].phase += 1;
    for expected in variants {
        assert!(matches!(
            convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient),
            Err(StoredAdviceErrorV1::Context)
        ));
    }
    let wrong_field = EvaluationDomain::<Fq>::new(5, 3);
    assert!(matches!(
        convert_stored_advice_v1(
            &wrong_field,
            &mut provider,
            &mut source,
            original,
            Coefficient
        ),
        Err(StoredAdviceErrorV1::Context)
    ));
    let wrong_domain = EvaluationDomain::<Fp>::new(5, 2);
    assert!(matches!(
        convert_stored_advice_v1(
            &wrong_domain,
            &mut provider,
            &mut source,
            original,
            Coefficient
        ),
        Err(StoredAdviceErrorV1::Context)
    ));
    assert!(matches!(
        convert_stored_advice_v1(
            &domain,
            &mut provider,
            &mut source,
            original,
            CosetPart {
                extension_log: 1,
                part: 0
            }
        ),
        Err(StoredAdviceErrorV1::Context)
    ));
    for (extension_log, part) in [(0, 0), (2, 4), (32, 0)] {
        assert!(matches!(
            convert_stored_advice_v1(
                &domain,
                &mut provider,
                &mut source,
                original,
                CosetPart {
                    extension_log,
                    part
                }
            ),
            Err(StoredAdviceErrorV1::Layout)
        ));
    }
    source.layout.basis = CosetPart {
        extension_log: 1,
        part: 0,
    };
    let expected = source.layout;
    assert!(matches!(
        convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient),
        Err(StoredAdviceErrorV1::Context)
    ));
    assert_eq!(state.record.borrow().creates, 0);
    assert_eq!(state.record.borrow().reads, 0);
}

#[test]
fn destination_requires_same_coordinates_and_a_strictly_newer_identity() {
    use StoredPolynomialBasisV1::{Coefficient, Lagrange};
    let domain = EvaluationDomain::<Fp>::new(5, 3);
    let (mut provider, mut source, state) = setup(Lagrange, 3, &[Fp::ONE; 8]);
    let expected = source.layout;
    let mut correct = expected;
    correct.ordinal = 8;
    correct.basis = Coefficient;
    let mut variants = [correct; 8];
    variants[0].proof_context[0] ^= 1;
    variants[1].ordinal = expected.ordinal;
    variants[2].ordinal = expected.ordinal - 1;
    variants[3].field = StoredPastaFieldV1::Fq;
    variants[4].basis = Lagrange;
    variants[5].k += 1;
    variants[6].column += 1;
    variants[7].phase += 1;
    for wrong in variants {
        state.record.borrow_mut().destination_override = Some(wrong);
        assert!(matches!(
            convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient),
            Err(StoredAdviceErrorV1::Context)
        ));
    }
    assert_eq!(state.record.borrow().reads, 0);
    assert_eq!(state.record.borrow().writer_drops, variants.len());
}

#[test]
fn malformed_scalar_and_chunk_length_poison_source_without_output() {
    use StoredPolynomialBasisV1::{Coefficient, Lagrange};
    for malformed_scalar in [true, false] {
        let domain = EvaluationDomain::<Fp>::new(5, 9);
        let (mut provider, mut source, state) = setup(Lagrange, 9, &[Fp::ONE; 512]);
        let expected = source.layout;
        if malformed_scalar {
            source.values[300] = [0xff; 32];
        } else {
            state.record.borrow_mut().short_chunk = true;
        }
        assert!(matches!(
            convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient),
            Err(StoredAdviceErrorV1::Encoding)
        ));
        assert!(source.poisoned);
        assert!(!state.active.get());
        assert_eq!(state.record.borrow().writes, 0);
        assert_eq!(state.record.borrow().writer_drops, 1);
    }
}

#[test]
fn metadata_changed_by_the_last_read_rejects_before_transform_or_output() {
    use StoredPolynomialBasisV1::{Coefficient, Lagrange};
    let domain = EvaluationDomain::<Fp>::new(5, 9);
    let (mut provider, mut source, state) = setup(Lagrange, 9, &[Fp::ONE; 512]);
    let expected = source.layout;
    state.record.borrow_mut().change_source_after_read = Some(1);
    assert!(matches!(
        convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient),
        Err(StoredAdviceErrorV1::Context)
    ));
    let record = state.record.borrow();
    assert_eq!(record.reads, expected.chunk_count());
    assert_eq!(record.writes, 0);
    assert_eq!(record.writer_drops, 1);
    assert!(!state.active.get());
}

#[test]
fn backend_failures_never_return_a_partial_destination() {
    use StoredPolynomialBasisV1::{Coefficient, Lagrange};
    for stage in 0..4 {
        let domain = EvaluationDomain::<Fp>::new(5, 9);
        let (mut provider, mut source, state) = setup(Lagrange, 9, &[Fp::ONE; 512]);
        let expected = source.layout;
        {
            let mut record = state.record.borrow_mut();
            match stage {
                0 => record.fail_create = true,
                1 => record.fail_read = Some(1),
                2 => record.fail_write = Some(1),
                _ => record.fail_seal = true,
            }
        }
        let error =
            convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient);
        assert!(matches!(
            error,
            Err(StoredAdviceErrorV1::Storage | StoredAdviceErrorV1::Authentication)
        ));
        assert_eq!(source.poisoned, stage == 1);
        assert!(!state.active.get());
        let record = state.record.borrow();
        assert_eq!(record.writer_drops, usize::from(stage != 0));
        assert_eq!(record.snapshot_drops, 0);
        if stage <= 1 {
            assert_eq!(record.writes, 0);
        }
    }
}

#[test]
fn read_and_write_unwind_release_window_and_incomplete_writer() {
    use StoredPolynomialBasisV1::{Coefficient, Lagrange};
    for reading in [true, false] {
        let domain = EvaluationDomain::<Fq>::new(5, 9);
        let (mut provider, mut source, state) = setup(Lagrange, 9, &[Fq::ONE; 512]);
        let expected = source.layout;
        if reading {
            state.record.borrow_mut().panic_read = Some(1);
        } else {
            state.record.borrow_mut().panic_write = Some(1);
        }
        assert!(
            catch_unwind(AssertUnwindSafe(|| convert_stored_advice_v1(
                &domain,
                &mut provider,
                &mut source,
                expected,
                Coefficient
            )))
            .is_err()
        );
        assert!(!state.active.get());
        assert_eq!(source.poisoned, reading);
        assert_eq!(state.record.borrow().writer_drops, 1);
        assert_eq!(state.record.borrow().snapshot_drops, 0);
    }
}

#[test]
fn changing_writer_or_returned_snapshot_metadata_rejects_output() {
    use StoredPolynomialBasisV1::{Coefficient, Lagrange};
    for changed_writer in [true, false] {
        let domain = EvaluationDomain::<Fp>::new(5, 3);
        let (mut provider, mut source, state) = setup(Lagrange, 3, &[Fp::ONE; 8]);
        let expected = source.layout;
        if changed_writer {
            state.record.borrow_mut().change_writer_after_write = true;
        } else {
            state.record.borrow_mut().returned_override = Some(expected);
        }
        assert!(matches!(
            convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient),
            Err(StoredAdviceErrorV1::Context)
        ));
        assert_eq!(state.record.borrow().seals, usize::from(!changed_writer));
        assert_eq!(
            state.record.borrow().snapshot_drops,
            usize::from(!changed_writer)
        );
        assert_eq!(state.record.borrow().writer_drops, 1);
    }
}

#[test]
fn active_backend_window_is_not_bypassed() {
    use StoredPolynomialBasisV1::{Coefficient, Lagrange};
    let domain = EvaluationDomain::<Fp>::new(5, 3);
    let (mut provider, mut source, state) = setup(Lagrange, 3, &[Fp::ONE; 8]);
    let expected = source.layout;
    let window = Window::acquire(&state).unwrap();
    assert!(matches!(
        convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient),
        Err(StoredAdviceErrorV1::Busy)
    ));
    assert!(state.active.get());
    assert_eq!(state.record.borrow().reads, 0);
    drop(window);
    assert!(
        convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient)
            .is_ok()
    );
}

#[test]
fn owned_field_and_encoding_buffers_clear_all_initialized_slots() {
    fn check<F: StoredAssignmentFieldV1>() {
        let mut column = FieldColumn::<F>::zeroed(513).unwrap();
        let address = column.0.as_ptr();
        column.0.fill(F::ONE);
        column.clear();
        assert!(column.0.iter().all(|value| *value == F::ZERO));
        assert_eq!(column.0.as_ptr(), address);
    }
    check::<Fp>();
    check::<Fq>();
    let mut encoded = EncodedChunk([[255; 32]; STORED_SCALARS_PER_CHUNK_V1]);
    encoded.clear();
    assert!(encoded.0.iter().all(|value| *value == [0; 32]));
}

#[test]
fn borrowed_domain_helper_rejects_bad_geometry_before_mutation() {
    let domain = EvaluationDomain::<Fp>::new(5, 2);
    let mut wrong_length = [Fp::ONE; 3];
    assert!(
        catch_unwind(AssertUnwindSafe(
            || domain.stored_column_transform_in_place(&mut wrong_length, true, None)
        ))
        .is_err()
    );
    assert_eq!(wrong_length, [Fp::ONE; 3]);
    for inverse in [true, false] {
        let mut values = [Fp::ONE; 4];
        assert!(
            catch_unwind(AssertUnwindSafe(|| domain
                .stored_column_transform_in_place(
                    &mut values,
                    inverse,
                    Some(Fp::ZERO)
                )))
            .is_err()
        );
        assert_eq!(values, [Fp::ONE; 4]);
    }
}

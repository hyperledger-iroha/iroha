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

use super::super::{StoredLookupSideV1, StoredPastaFieldV1, StoredPolynomialRoleV1};
use super::*;

fn advance_role_index(layout: &mut StoredPolynomialLayoutV1) {
    match &mut layout.role {
        StoredPolynomialRoleV1::Advice { column, .. } => *column += 1,
        StoredPolynomialRoleV1::LookupCompressed { lookup, .. }
        | StoredPolynomialRoleV1::LookupSorted { lookup, .. }
        | StoredPolynomialRoleV1::LookupLeftoverTable { lookup }
        | StoredPolynomialRoleV1::LookupPermuted { lookup, .. }
        | StoredPolynomialRoleV1::LookupProduct { lookup } => *lookup += 1,
        StoredPolynomialRoleV1::CopyPermutationProduct { set } => *set += 1,
        StoredPolynomialRoleV1::Instance { column }
        | StoredPolynomialRoleV1::KeyFixed { column }
        | StoredPolynomialRoleV1::KeyPermutation { column } => *column += 1,
        StoredPolynomialRoleV1::KeyMask { kind } => {
            use super::super::StoredKeyMaskV1;
            *kind = match kind {
                StoredKeyMaskV1::L0 => StoredKeyMaskV1::LLast,
                StoredKeyMaskV1::LLast => StoredKeyMaskV1::LActiveRow,
                StoredKeyMaskV1::LActiveRow => StoredKeyMaskV1::L0,
            };
        }
        StoredPolynomialRoleV1::QuotientAliasedPart { part, .. } => *part += 1,
        StoredPolynomialRoleV1::QuotientPiece { piece } => *piece += 1,
        StoredPolynomialRoleV1::VanishingRandom | StoredPolynomialRoleV1::QuotientNumerator => {
            layout.role = StoredPolynomialRoleV1::Instance { column: 0 };
        }
    }
}

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
    destination_override: Option<StoredPolynomialLayoutV1>,
    returned_override: Option<StoredPolynomialLayoutV1>,
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
    fn acquire(state: &Rc<State>) -> Result<Self, StoredPolynomialErrorV1> {
        if state.active.replace(true) {
            return Err(StoredPolynomialErrorV1::Busy);
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
    layout: StoredPolynomialLayoutV1,
    state: Rc<State>,
    values: Vec<[u8; 32]>,
    chunks: u64,
}

struct Snapshot {
    layout: StoredPolynomialLayoutV1,
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

impl StoredPolynomialProviderV1 for Provider {
    type Writer = Writer;

    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Writer, StoredPolynomialErrorV1> {
        let _window = Window::acquire(&self.state)?;
        let mut record = self.state.record.borrow_mut();
        record.creates += 1;
        if record.fail_create {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        let layout = record
            .destination_override
            .unwrap_or(StoredPolynomialLayoutV1::new(
                [5; 32],
                self.next_ordinal,
                field,
                basis,
                k,
                role,
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

impl StoredPolynomialWriterV1 for Writer {
    type Snapshot = Snapshot;

    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let mut layout = self.layout;
        if self.chunks != 0 && self.state.record.borrow().change_writer_after_write {
            advance_role_index(&mut layout);
        }
        layout
    }

    fn write_chunk(
        &mut self,
        chunk: u64,
        values: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1> {
        let _window = Window::acquire(&self.state)?;
        let mut record = self.state.record.borrow_mut();
        record.writes += 1;
        assert_ne!(record.panic_write, Some(chunk), "injected write panic");
        if record.fail_write == Some(chunk) {
            return Err(StoredPolynomialErrorV1::Storage);
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

    fn seal(mut self) -> Result<Snapshot, StoredPolynomialErrorV1> {
        let _window = Window::acquire(&self.state)?;
        let mut record = self.state.record.borrow_mut();
        record.seals += 1;
        if record.fail_seal {
            return Err(StoredPolynomialErrorV1::Storage);
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

impl StoredPolynomialSnapshotV1 for Snapshot {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        self.layout
    }

    fn with_chunk<R>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredPolynomialErrorV1>,
    ) -> Result<R, StoredPolynomialErrorV1> {
        if self.poisoned {
            return Err(StoredPolynomialErrorV1::Poisoned);
        }
        if expected != self.layout {
            return Err(StoredPolynomialErrorV1::Context);
        }
        let _window = Window::acquire(&self.state)?;
        self.poisoned = true;
        let short_chunk = {
            let mut record = self.state.record.borrow_mut();
            record.reads += 1;
            assert_ne!(record.panic_read, Some(chunk), "injected read panic");
            if record.fail_read == Some(chunk) {
                return Err(StoredPolynomialErrorV1::Authentication);
            }
            record.short_chunk
        };
        let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
        let count = self.layout.chunk_scalar_count(chunk)? - usize::from(short_chunk);
        let result = consume(&self.values[start..start + count])?;
        if self.state.record.borrow().change_source_after_read == Some(chunk) {
            advance_role_index(&mut self.layout);
        }
        self.poisoned = false;
        Ok(result)
    }

    fn with_column<R>(
        &mut self,
        _expected: StoredPolynomialLayoutV1,
        _consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredPolynomialErrorV1>,
    ) -> Result<R, StoredPolynomialErrorV1> {
        panic!("conversion must not allocate a second complete encoded column")
    }
}

fn setup<F: StoredAssignmentFieldV1>(
    basis: StoredPolynomialBasisV1,
    k: u32,
    values: &[F],
) -> (Provider, Snapshot, Rc<State>) {
    let state = Rc::new(State::default());
    let layout = StoredPolynomialLayoutV1::new(
        [5; 32],
        7,
        F::STORED_FIELD,
        basis,
        k,
        StoredPolynomialRoleV1::Advice {
            column: 3,
            phase: 1,
        },
    )
    .unwrap();
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

fn conversion_matrix<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>(
    role: StoredPolynomialRoleV1,
) {
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
                source.layout.role = role;
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
                assert_eq!(destination.layout.role(), expected.role());
                assert_eq!(
                    destination.layout.advice_coordinates(),
                    expected.advice_coordinates()
                );
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
    conversion_matrix::<Fp>(StoredPolynomialRoleV1::Advice {
        column: 3,
        phase: 1,
    });
}

#[test]
fn fq_all_basis_pairs_match_existing_domain_transforms() {
    conversion_matrix::<Fq>(StoredPolynomialRoleV1::Advice {
        column: 3,
        phase: 1,
    });
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
    variants[5].role = StoredPolynomialRoleV1::Advice {
        column: 4,
        phase: 1,
    };
    variants[6].role = StoredPolynomialRoleV1::Advice {
        column: 3,
        phase: 2,
    };
    for expected in variants {
        assert!(matches!(
            convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient),
            Err(StoredPolynomialErrorV1::Context)
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
        Err(StoredPolynomialErrorV1::Context)
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
        Err(StoredPolynomialErrorV1::Context)
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
        Err(StoredPolynomialErrorV1::Context)
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
            Err(StoredPolynomialErrorV1::Layout)
        ));
    }
    source.layout.basis = CosetPart {
        extension_log: 1,
        part: 0,
    };
    let expected = source.layout;
    assert!(matches!(
        convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient),
        Err(StoredPolynomialErrorV1::Context)
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
    variants[6].role = StoredPolynomialRoleV1::Advice {
        column: 4,
        phase: 1,
    };
    variants[7].role = StoredPolynomialRoleV1::Advice {
        column: 3,
        phase: 2,
    };
    for wrong in variants {
        state.record.borrow_mut().destination_override = Some(wrong);
        assert!(matches!(
            convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, Coefficient),
            Err(StoredPolynomialErrorV1::Context)
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
            Err(StoredPolynomialErrorV1::Encoding)
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
        Err(StoredPolynomialErrorV1::Context)
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
            Err(StoredPolynomialErrorV1::Storage | StoredPolynomialErrorV1::Authentication)
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
            Err(StoredPolynomialErrorV1::Context)
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
        Err(StoredPolynomialErrorV1::Busy)
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

#[test]
fn fp_lookup_roles_preserve_every_basis_conversion() {
    for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
        conversion_matrix::<Fp>(StoredPolynomialRoleV1::LookupCompressed {
            lookup: u32::MAX,
            side,
        });
    }
}

#[test]
fn fq_lookup_roles_preserve_every_basis_conversion() {
    for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
        conversion_matrix::<Fq>(StoredPolynomialRoleV1::LookupCompressed {
            lookup: u32::MAX,
            side,
        });
    }
}

fn rejects_role_substitution<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>() {
    use StoredPolynomialBasisV1::{Coefficient, Lagrange};
    let roles = [
        StoredPolynomialRoleV1::Advice {
            column: 3,
            phase: 0,
        },
        StoredPolynomialRoleV1::Advice {
            column: 3,
            phase: 1,
        },
        StoredPolynomialRoleV1::LookupCompressed {
            lookup: 3,
            side: StoredLookupSideV1::Input,
        },
        StoredPolynomialRoleV1::LookupCompressed {
            lookup: 3,
            side: StoredLookupSideV1::Table,
        },
        StoredPolynomialRoleV1::LookupCompressed {
            lookup: 4,
            side: StoredLookupSideV1::Input,
        },
    ];
    for role in roles {
        for wrong_role in roles.into_iter().filter(|wrong| *wrong != role) {
            for stage in 0..3 {
                let domain = EvaluationDomain::<F>::new(5, 9);
                let (mut provider, mut source, state) = setup(Lagrange, 9, &[F::ONE; 512]);
                source.layout.role = role;
                let mut expected = source.layout;
                let mut wrong = expected;
                wrong.role = wrong_role;
                if stage == 0 {
                    expected = wrong;
                } else {
                    wrong.ordinal = 8;
                    wrong.basis = Coefficient;
                    if stage == 1 {
                        state.record.borrow_mut().destination_override = Some(wrong);
                    } else {
                        state.record.borrow_mut().returned_override = Some(wrong);
                    }
                }
                assert!(
                    matches!(
                        convert_stored_advice_v1(
                            &domain,
                            &mut provider,
                            &mut source,
                            expected,
                            Coefficient
                        ),
                        Err(StoredPolynomialErrorV1::Context)
                    ),
                    "role={role:?}, wrong={wrong_role:?}, stage={stage}"
                );
                assert!(!state.active.get());
                assert!(!source.poisoned);
                assert_eq!(source.layout.role(), role);
                let record = state.record.borrow();
                assert_eq!(record.creates, usize::from(stage != 0));
                assert_eq!(
                    record.reads,
                    if stage == 2 {
                        expected.chunk_count()
                    } else {
                        0
                    }
                );
                assert_eq!(
                    record.writes,
                    if stage == 2 {
                        expected.chunk_count()
                    } else {
                        0
                    }
                );
                assert_eq!(record.seals, usize::from(stage == 2));
                assert_eq!(record.writer_drops, usize::from(stage != 0));
                assert_eq!(record.snapshot_drops, usize::from(stage == 2));
            }
        }
    }
}

#[test]
fn fp_role_substitutions_reject_before_reads_or_discard_sealed_output() {
    rejects_role_substitution::<Fp>();
}

#[test]
fn fq_role_substitutions_reject_before_reads_or_discard_sealed_output() {
    rejects_role_substitution::<Fq>();
}

/// Undivided numerator parts never enter an ordinary basis conversion, even an equal-basis copy.
fn numerator_scratch_refuses_generic_conversion<
    F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
>() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart, Lagrange};
    for k in [7, 8, 9] {
        let domain = EvaluationDomain::<F>::new(5, k);
        let extension_log = domain.extended_k() - k;
        assert_eq!(extension_log, 2);
        for part in 0..(1_u32 << extension_log) {
            let basis = CosetPart {
                extension_log,
                part,
            };
            let different = CosetPart {
                extension_log,
                part: (part + 1) % 4,
            };
            for destination in [Coefficient, Lagrange, basis, different] {
                let (mut provider, mut source, state) = setup(basis, k, &vec![F::ONE; 1 << k]);
                source.layout.role = StoredPolynomialRoleV1::QuotientNumerator;
                let expected = source.layout;
                assert!(matches!(
                    convert_stored_advice_v1(
                        &domain,
                        &mut provider,
                        &mut source,
                        expected,
                        destination
                    ),
                    Err(StoredPolynomialErrorV1::Context)
                ));
                assert_eq!(provider.next_ordinal, 8);
                assert_eq!(source.layout, expected);
                assert!(!source.poisoned);
                assert!(!state.active.get());
                let record = state.record.borrow();
                assert_eq!(
                    (record.creates, record.reads, record.writes, record.seals),
                    (0, 0, 0, 0)
                );
                assert_eq!((record.writer_drops, record.snapshot_drops), (0, 0));
            }
            // An already-created, otherwise valid numerator writer cannot bypass refusal.
            for destination in [basis, different] {
                let (mut provider, mut source, state) = setup(basis, k, &vec![F::ONE; 1 << k]);
                source.layout.role = StoredPolynomialRoleV1::QuotientNumerator;
                let expected = source.layout;
                let writer = provider
                    .create(
                        F::STORED_FIELD,
                        destination,
                        k,
                        StoredPolynomialRoleV1::QuotientNumerator,
                    )
                    .unwrap();
                let destination_layout = writer.layout();
                assert!(matches!(
                    convert_stored_advice_with_writer_v1(
                        &domain,
                        &mut source,
                        expected,
                        destination,
                        writer,
                        destination_layout
                    ),
                    Err(StoredPolynomialErrorV1::Context)
                ));
                assert_eq!(source.layout, expected);
                assert!(!source.poisoned);
                assert!(!state.active.get());
                let record = state.record.borrow();
                assert_eq!(
                    (record.creates, record.reads, record.writes, record.seals),
                    (1, 0, 0, 0)
                );
                assert_eq!((record.writer_drops, record.snapshot_drops), (1, 0));
            }
        }
    }
}

#[test]
fn fp_numerator_scratch_refuses_generic_conversion_before_source_io() {
    numerator_scratch_refuses_generic_conversion::<Fp>();
}

#[test]
fn fq_numerator_scratch_refuses_generic_conversion_before_source_io() {
    numerator_scratch_refuses_generic_conversion::<Fq>();
}

/// Inverse aliases must not be mistaken for ordinary n-coefficient polynomials.
fn inverse_roles_preserve_closed_conversion_bounds<
    F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
>() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart, Lagrange};
    for k in [7, 8, 9] {
        let domain = EvaluationDomain::<F>::new(5, k);
        let extension_log = domain.extended_k() - k;
        for part in 0..4 {
            let role = StoredPolynomialRoleV1::QuotientAliasedPart {
                part,
                extension_log,
            };
            for destination in [
                Coefficient,
                Lagrange,
                CosetPart {
                    extension_log,
                    part,
                },
            ] {
                let (mut provider, mut source, state) =
                    setup(Coefficient, k, &vec![F::ONE; 1 << k]);
                source.layout.role = role;
                let expected = source.layout;
                assert!(matches!(
                    convert_stored_advice_v1(
                        &domain,
                        &mut provider,
                        &mut source,
                        expected,
                        destination
                    ),
                    Err(StoredPolynomialErrorV1::Context)
                ));
                assert_eq!(provider.next_ordinal, 8);
                assert!(!source.poisoned);
                assert!(!state.active.get());
                let record = state.record.borrow();
                assert_eq!(
                    (
                        record.creates,
                        record.reads,
                        record.writes,
                        record.seals,
                        record.writer_drops,
                        record.snapshot_drops
                    ),
                    (0, 0, 0, 0, 0, 0)
                );
            }
            let (mut provider, mut source, state) = setup(Coefficient, k, &vec![F::ONE; 1 << k]);
            source.layout.role = role;
            let expected = source.layout;
            let writer = provider
                .create(F::STORED_FIELD, Coefficient, k, role)
                .unwrap();
            let destination = writer.layout();
            assert!(matches!(
                convert_stored_advice_with_writer_v1(
                    &domain,
                    &mut source,
                    expected,
                    Coefficient,
                    writer,
                    destination
                ),
                Err(StoredPolynomialErrorV1::Context)
            ));
            assert!(!source.poisoned);
            assert!(!state.active.get());
            let record = state.record.borrow();
            assert_eq!(
                (
                    record.creates,
                    record.reads,
                    record.writes,
                    record.seals,
                    record.writer_drops,
                    record.snapshot_drops
                ),
                (1, 0, 0, 0, 1, 0)
            );
        }
        for piece in [0, (1_u32 << (super::super::STORED_MAX_K_V1 - k)) - 1] {
            let role = StoredPolynomialRoleV1::QuotientPiece { piece };
            let values: Vec<F> = (0..(1 << k)).map(|v| F::from(v as u64 + 1)).collect();
            let (mut provider, mut source, state) = setup(Coefficient, k, &values);
            source.layout.role = role;
            let expected = source.layout;
            let mut result = convert_stored_advice_v1(
                &domain,
                &mut provider,
                &mut source,
                expected,
                Coefficient,
            )
            .unwrap();
            assert_eq!(result.layout.role(), role);
            assert_eq!(result.layout.basis(), Coefficient);
            assert_eq!(result.values, source.values);
            assert!(!source.poisoned);
            assert!(!state.active.get());
            assert_eq!(result.layout.ordinal(), 8);
            let record = state.record.borrow();
            assert_eq!(
                (record.creates, record.reads, record.writes, record.seals),
                (1, expected.chunk_count(), expected.chunk_count(), 1)
            );
            let before = (record.creates, record.reads, record.writes, record.seals);
            // Changing the basis would no longer represent an ordinary coefficient piece.
            drop(record);
            for basis in [
                Lagrange,
                CosetPart {
                    extension_log,
                    part: 0,
                },
            ] {
                let layout = result.layout;
                assert!(matches!(
                    convert_stored_advice_v1(&domain, &mut provider, &mut result, layout, basis),
                    Err(StoredPolynomialErrorV1::Layout)
                ));
            }
            let record = state.record.borrow();
            assert_eq!(
                (record.creates, record.reads, record.writes, record.seals),
                before
            );
        }
    }
}

#[test]
fn fp_inverse_roles_preserve_closed_conversion_bounds() {
    inverse_roles_preserve_closed_conversion_bounds::<Fp>();
}
#[test]
fn fq_inverse_roles_preserve_closed_conversion_bounds() {
    inverse_roles_preserve_closed_conversion_bounds::<Fq>();
}

#[path = "key_role_tests.rs"]
mod key_role_tests;

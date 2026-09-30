//! Quotient interpretation tests against the actual encrypted Core provider.
//!
//! Metadata substitutions test the spool's authenticated-context refusal, not ciphertext
//! corruption. Counter-only leases exercise the original 512 limit without opening 512 files.
//! These tests do not construct proofs or qualify process memory, latency or devices.

use super::*;
use StoredPolynomialBasisV1 as Basis;
use StoredPolynomialRoleV1 as Role;

type Interpretation = (Basis, Role);

fn representatives() -> [Interpretation; 3] {
    [
        (
            Basis::CosetPart {
                extension_log: 2,
                part: 1,
            },
            Role::QuotientNumerator,
        ),
        (
            Basis::Coefficient,
            Role::QuotientAliasedPart {
                extension_log: 2,
                part: 1,
            },
        ),
        (Basis::Coefficient, Role::QuotientPiece { piece: 1 }),
    ]
}

fn coordinates(k: u32) -> Vec<Interpretation> {
    let mut result = Vec::new();
    for extension_log in [1, 2, 19 - k] {
        for part in [0, (1_u32 << extension_log) - 1] {
            result.push((
                Basis::CosetPart {
                    extension_log,
                    part,
                },
                Role::QuotientNumerator,
            ));
            result.push((
                Basis::Coefficient,
                Role::QuotientAliasedPart {
                    extension_log,
                    part,
                },
            ));
        }
    }
    for piece in [0, (1_u32 << (19 - k)) - 1] {
        result.push((Basis::Coefficient, Role::QuotientPiece { piece }));
    }
    assert_eq!(result.len(), 14);
    result
}

fn quotient_value(row: usize) -> [u8; 32] {
    if row % 4 == 0 {
        [0; 32]
    } else {
        scalar(row as u64 * 17 + 1)
    }
}

fn filled_quotient(
    provider: &mut CoreStoredPolynomialProviderV1,
    field: StoredPastaFieldV1,
    k: u32,
    interpretation: Interpretation,
) -> CoreStoredPolynomialSnapshotV1 {
    let mut writer = provider
        .create(field, interpretation.0, k, interpretation.1)
        .unwrap();
    let layout = writer.layout();
    for chunk in 0..layout.chunk_count() as u64 {
        let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
        let values = (0..layout.chunk_scalar_count(chunk).unwrap())
            .map(|offset| quotient_value(start + offset))
            .collect::<Vec<_>>();
        writer.write_chunk(chunk, &values).unwrap();
    }
    writer.seal().unwrap()
}

fn assert_sentinel(snapshot: &mut CoreStoredPolynomialSnapshotV1) {
    snapshot
        .with_column(snapshot.layout(), |values| {
            assert_eq!(values, &[scalar(1), scalar(2)]);
            Ok(())
        })
        .unwrap();
}

fn assert_poisoned(snapshot: &mut CoreStoredPolynomialSnapshotV1) {
    assert!(snapshot.raw.is_none());
    assert!(!snapshot.window.get());
    let layout = snapshot.layout();
    assert_eq!(
        snapshot.with_chunk(layout, 0, |_| Ok(())),
        Err(StoredPolynomialErrorV1::Poisoned)
    );
    assert_eq!(
        snapshot.with_column(layout, |_| Ok(())),
        Err(StoredPolynomialErrorV1::Poisoned)
    );
}

#[test]
fn quotient_roles_roundtrip_both_fields_coordinates_and_chunk_geometry() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut previous = None;
    let mut setups = 0;
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for k in [0, 7, 8, 9] {
            for interpretation in coordinates(k) {
                let mut snapshot = filled_quotient(&mut provider, field, k, interpretation);
                setups += 1;
                let layout = snapshot.layout();
                assert_eq!((layout.basis(), layout.role()), interpretation);
                assert_eq!(layout.field(), field);
                assert_eq!(layout.k(), k);
                assert_eq!(layout.scalar_count(), 1 << k);
                assert_eq!(
                    layout.chunk_count(),
                    (1_usize << k).div_ceil(STORED_SCALARS_PER_CHUNK_V1)
                );
                if let Some(last) = previous {
                    assert!(layout.ordinal() > last);
                }
                previous = Some(layout.ordinal());
                assert_eq!(provider.handles.live.get(), 1);
                assert!(snapshot.raw.is_some());
                for chunk in (0..layout.chunk_count() as u64).rev() {
                    snapshot
                        .with_chunk(layout, chunk, |values| {
                            assert_eq!(values.len(), layout.chunk_scalar_count(chunk).unwrap());
                            for (offset, value) in values.iter().enumerate() {
                                assert_eq!(
                                    *value,
                                    quotient_value(
                                        chunk as usize * STORED_SCALARS_PER_CHUNK_V1 + offset
                                    )
                                );
                            }
                            Ok(())
                        })
                        .unwrap();
                }
                snapshot
                    .with_column(layout, |values| {
                        assert_eq!(values.len(), 1 << k);
                        for (row, value) in values.iter().enumerate() {
                            assert_eq!(*value, quotient_value(row));
                        }
                        Ok(())
                    })
                    .unwrap();
                assert!(!provider.window.get());
                drop(snapshot);
                assert_eq!(provider.handles.live.get(), 0);
                assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
            }
        }
    }
    assert_eq!(setups, 112);
}

#[test]
fn quotient_roles_reject_invalid_basis_extension_part_piece_and_domain_before_backend_effects() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut invalid = vec![
        (1, Basis::Lagrange, Role::QuotientNumerator),
        (1, Basis::Coefficient, Role::QuotientNumerator),
        (
            1,
            Basis::CosetPart {
                extension_log: 0,
                part: 0,
            },
            Role::QuotientNumerator,
        ),
        (
            1,
            Basis::CosetPart {
                extension_log: 19,
                part: 0,
            },
            Role::QuotientNumerator,
        ),
        (
            1,
            Basis::CosetPart {
                extension_log: 2,
                part: 4,
            },
            Role::QuotientNumerator,
        ),
        (
            1,
            Basis::Lagrange,
            Role::QuotientAliasedPart {
                extension_log: 2,
                part: 1,
            },
        ),
        (
            1,
            Basis::CosetPart {
                extension_log: 2,
                part: 1,
            },
            Role::QuotientAliasedPart {
                extension_log: 2,
                part: 1,
            },
        ),
        (
            1,
            Basis::Coefficient,
            Role::QuotientAliasedPart {
                extension_log: 0,
                part: 0,
            },
        ),
        (
            1,
            Basis::Coefficient,
            Role::QuotientAliasedPart {
                extension_log: 19,
                part: 0,
            },
        ),
        (
            1,
            Basis::Coefficient,
            Role::QuotientAliasedPart {
                extension_log: 2,
                part: 4,
            },
        ),
        (1, Basis::Lagrange, Role::QuotientPiece { piece: 1 }),
        (
            1,
            Basis::CosetPart {
                extension_log: 2,
                part: 1,
            },
            Role::QuotientPiece { piece: 1 },
        ),
        (
            1,
            Basis::Coefficient,
            Role::QuotientPiece { piece: 1 << 18 },
        ),
        (
            1,
            Basis::Coefficient,
            Role::QuotientPiece { piece: u32::MAX },
        ),
        (
            19,
            Basis::CosetPart {
                extension_log: 1,
                part: 0,
            },
            Role::QuotientNumerator,
        ),
        (
            19,
            Basis::Coefficient,
            Role::QuotientAliasedPart {
                extension_log: 1,
                part: 0,
            },
        ),
        (19, Basis::Coefficient, Role::QuotientPiece { piece: 1 }),
    ];
    invalid.extend(
        representatives()
            .into_iter()
            .map(|(basis, role)| (20, basis, role)),
    );
    assert_eq!(invalid.len(), 20);
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut unrelated = filled(&mut provider, field, 1);
        for &(k, basis, role) in &invalid {
            let next = provider.next_ordinal;
            let files = std::fs::read_dir(directory.path()).unwrap().count();
            assert_eq!(
                StoredPolynomialLayoutV1::new(provider.proof_context, next, field, basis, k, role),
                Err(StoredPolynomialErrorV1::Layout)
            );
            assert!(matches!(
                provider.create(field, basis, k, role),
                Err(StoredPolynomialErrorV1::Layout)
            ));
            assert_eq!(provider.next_ordinal, next);
            assert_eq!(provider.handles.live.get(), 1);
            assert!(!provider.window.get());
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), files);
            assert_sentinel(&mut unrelated);
        }
        // Metadata-only endpoint admission avoids materializing a 2^19-value column.
        let endpoint = StoredPolynomialLayoutV1::new(
            provider.proof_context,
            provider.next_ordinal,
            field,
            Basis::Coefficient,
            19,
            Role::QuotientPiece { piece: 0 },
        )
        .unwrap();
        assert_eq!(endpoint.scalar_count(), 1 << 19);
        for interpretation in representatives() {
            let mut writer = provider
                .create(field, interpretation.0, 7, interpretation.1)
                .unwrap();
            let layout = writer.layout();
            let next = provider.next_ordinal;
            let values = (0..128).map(quotient_value).collect::<Vec<_>>();
            assert_eq!(
                writer.write_chunk(1, &values),
                Err(StoredPolynomialErrorV1::WriteOrder)
            );
            assert_eq!(
                writer.write_chunk(0, &values[..127]),
                Err(StoredPolynomialErrorV1::WriteOrder)
            );
            let mut long = values.clone();
            long.push([0; 32]);
            assert_eq!(
                writer.write_chunk(0, &long),
                Err(StoredPolynomialErrorV1::WriteOrder)
            );
            let mut noncanonical = values.clone();
            noncanonical[127] = [255; 32];
            assert_eq!(
                writer.write_chunk(0, &noncanonical),
                Err(StoredPolynomialErrorV1::Encoding)
            );
            assert!(writer.raw.is_some());
            assert_eq!(writer.next_chunk, 0);
            assert_eq!(provider.next_ordinal, next);
            assert_eq!(provider.handles.live.get(), 2);
            writer.write_chunk(0, &values).unwrap();
            assert_eq!(
                writer.write_chunk(0, &values),
                Err(StoredPolynomialErrorV1::WriteOrder)
            );
            let mut snapshot = writer.seal().unwrap();
            assert_eq!(snapshot.layout(), layout);
            snapshot
                .with_column(layout, |actual| {
                    assert_eq!(actual, values);
                    Ok(())
                })
                .unwrap();
            drop(snapshot);
            let incomplete = provider
                .create(field, interpretation.0, 7, interpretation.1)
                .unwrap();
            assert!(matches!(
                incomplete.seal(),
                Err(StoredPolynomialErrorV1::Incomplete)
            ));
            assert_eq!(provider.handles.live.get(), 1);
            assert_sentinel(&mut unrelated);
        }
        drop(unrelated);
    }
    assert_eq!(provider.handles.live.get(), 0);
    assert!(!provider.window.get());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

fn forged_layouts(
    original: StoredPolynomialLayoutV1,
    proof_context: [u8; 32],
) -> Vec<StoredPolynomialLayoutV1> {
    let mut interpretations = representatives().to_vec();
    match original.role() {
        Role::QuotientNumerator => {
            interpretations.push((
                Basis::CosetPart {
                    extension_log: 2,
                    part: 0,
                },
                original.role(),
            ));
            interpretations.push((
                Basis::CosetPart {
                    extension_log: 3,
                    part: 1,
                },
                original.role(),
            ));
        }
        Role::QuotientAliasedPart { .. } => {
            interpretations.push((
                Basis::Coefficient,
                Role::QuotientAliasedPart {
                    extension_log: 2,
                    part: 0,
                },
            ));
            interpretations.push((
                Basis::Coefficient,
                Role::QuotientAliasedPart {
                    extension_log: 3,
                    part: 1,
                },
            ));
        }
        Role::QuotientPiece { .. } => {
            interpretations.push((Basis::Coefficient, Role::QuotientPiece { piece: 0 }));
            interpretations.push((Basis::Coefficient, Role::QuotientPiece { piece: 2 }));
        }
        _ => unreachable!(),
    }
    for role in [
        Role::Advice {
            column: 0,
            phase: 0,
        },
        Role::LookupCompressed {
            lookup: 0,
            side: StoredLookupSideV1::Input,
        },
        Role::LookupPermuted {
            lookup: 0,
            side: StoredLookupSideV1::Table,
        },
        Role::CopyPermutationProduct { set: 0 },
        Role::LookupProduct { lookup: 0 },
        Role::Instance { column: 0 },
        Role::VanishingRandom,
    ] {
        interpretations.push((original.basis(), role));
    }
    let mut result = Vec::new();
    for (basis, role) in interpretations {
        let layout = StoredPolynomialLayoutV1::new(
            proof_context,
            original.ordinal(),
            original.field(),
            basis,
            original.k(),
            role,
        )
        .unwrap();
        if layout != original && !result.contains(&layout) {
            result.push(layout);
        }
    }
    let other_field = match original.field() {
        StoredPastaFieldV1::Fp => StoredPastaFieldV1::Fq,
        StoredPastaFieldV1::Fq => StoredPastaFieldV1::Fp,
    };
    let mut other_context = proof_context;
    other_context[0] ^= 1;
    if other_context == [0; 32] {
        other_context[1] = 1;
    }
    for (context, ordinal, field, k) in [
        (proof_context, original.ordinal(), other_field, original.k()),
        (
            proof_context,
            original.ordinal(),
            original.field(),
            original.k() + 1,
        ),
        (
            other_context,
            original.ordinal(),
            original.field(),
            original.k(),
        ),
        (
            proof_context,
            original.ordinal() + 1,
            original.field(),
            original.k(),
        ),
    ] {
        result.push(
            StoredPolynomialLayoutV1::new(
                context,
                ordinal,
                field,
                original.basis(),
                k,
                original.role(),
            )
            .unwrap(),
        );
    }
    assert_eq!(result.len(), 15);
    for layout in &result {
        assert_ne!(layout.context_digest(), original.context_digest());
    }
    result
}

#[test]
fn quotient_interpretation_forgeries_are_retryable_before_read_and_poison_after_admission() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut attempts = 0;
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut unrelated = filled(&mut provider, field, 1);
        for interpretation in representatives() {
            for alternative in 0..15 {
                for full_column in [false, true] {
                    let mut snapshot = filled_quotient(&mut provider, field, 1, interpretation);
                    attempts += 1;
                    let original = snapshot.layout();
                    let forged = forged_layouts(original, provider.proof_context)[alternative];
                    let next = provider.next_ordinal;
                    assert_eq!(
                        snapshot.with_chunk(forged, 0, |_| panic!(
                            "wrong expected quotient layout exposed plaintext"
                        )),
                        Err::<(), _>(StoredPolynomialErrorV1::Context)
                    );
                    assert_eq!(
                        snapshot.with_column(forged, |_| panic!(
                            "wrong expected quotient layout exposed a column"
                        )),
                        Err::<(), _>(StoredPolynomialErrorV1::Context)
                    );
                    assert!(snapshot.raw.is_some());
                    snapshot
                        .with_column(original, |values| {
                            assert_eq!(values, &[quotient_value(0), quotient_value(1)]);
                            Ok(())
                        })
                        .unwrap();
                    // This test changes only Core metadata. The same encrypted spool retains
                    // the original context and must refuse the substituted interpretation.
                    snapshot.layout = forged;
                    let result = if full_column {
                        snapshot.with_column(forged, |_| panic!("forged quotient column exposed"))
                    } else {
                        snapshot.with_chunk(forged, 0, |_| panic!("forged quotient chunk exposed"))
                    };
                    assert_eq!(result, Err::<(), _>(StoredPolynomialErrorV1::Context));
                    assert_poisoned(&mut snapshot);
                    assert_eq!(provider.next_ordinal, next);
                    assert_eq!(provider.handles.live.get(), 2);
                    drop(snapshot);
                    assert_eq!(provider.handles.live.get(), 1);
                    assert_sentinel(&mut unrelated);
                }
            }
        }
        drop(unrelated);
    }
    assert_eq!(attempts, 180);
    assert_eq!(provider.handles.live.get(), 0);
    assert!(!provider.window.get());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

fn malformed_quotient(
    provider: &mut CoreStoredPolynomialProviderV1,
    field: StoredPastaFieldV1,
    interpretation: Interpretation,
    malformed: usize,
) -> CoreStoredPolynomialSnapshotV1 {
    let k = if malformed == 2 { 9 } else { 1 };
    let writer = provider
        .create(field, interpretation.0, k, interpretation.1)
        .unwrap();
    let layout = writer.layout();
    // Test-only construction keeps the actual provider lease and encrypted raw writer.
    // It bypasses canonical-input checks solely to supply authenticated malformed plaintext.
    let mut raw = writer.raw.unwrap();
    for chunk in 0..layout.chunk_count() as u64 {
        let mut plaintext = ConfidentialSpoolChunkV1::new_zeroed_v1(CHUNK_BYTES as u64).unwrap();
        match malformed {
            0 => plaintext.as_mut_slice_v1()[..32].fill(0xff),
            1 => plaintext.as_mut_slice_v1()[64] = 1,
            2 if chunk == 1 => plaintext.as_mut_slice_v1()[..32].fill(0xff),
            _ => (),
        }
        raw.write_slot_v1(chunk, plaintext).unwrap();
    }
    CoreStoredPolynomialSnapshotV1 {
        layout,
        raw: Some(raw.seal_v1().unwrap()),
        _lease: writer.lease,
        window: Rc::clone(&provider.window),
        injected_read_error: None,
        panic_on_read: false,
    }
}

#[test]
fn quotient_authenticated_payload_and_consumer_failures_destroy_only_the_admitted_owner() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut attempts = 0;
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut unrelated = filled(&mut provider, field, 1);
        for interpretation in representatives() {
            for malformed in 0..3 {
                for full_column in [false, true] {
                    let mut snapshot =
                        malformed_quotient(&mut provider, field, interpretation, malformed);
                    attempts += 1;
                    let layout = snapshot.layout();
                    if malformed == 2 {
                        snapshot
                            .with_chunk(layout, 0, |values| {
                                assert_eq!(values, &[[0; 32]; 256]);
                                Ok(())
                            })
                            .unwrap();
                    }
                    let result = if full_column {
                        snapshot.with_column(layout, |_| {
                            panic!("malformed or partially decoded quotient column exposed")
                        })
                    } else {
                        snapshot.with_chunk(layout, u64::from(malformed == 2), |_| {
                            panic!("malformed quotient chunk exposed")
                        })
                    };
                    assert_eq!(result, Err::<(), _>(StoredPolynomialErrorV1::Encoding));
                    assert_poisoned(&mut snapshot);
                    drop(snapshot);
                    assert_eq!(provider.handles.live.get(), 1);
                    assert_sentinel(&mut unrelated);
                }
            }
            for full_column in [false, true] {
                for unwind in [false, true] {
                    let mut snapshot = filled_quotient(&mut provider, field, 1, interpretation);
                    attempts += 1;
                    let layout = snapshot.layout();
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        let consumer =
                            |values: &[[u8; 32]]| -> Result<(), StoredPolynomialErrorV1> {
                                assert_eq!(values, &[quotient_value(0), quotient_value(1)]);
                                assert!(
                                    !unwind,
                                    "consumer unwind after actual quotient decryption"
                                );
                                Err(StoredPolynomialErrorV1::Consumer)
                            };
                        if full_column {
                            snapshot.with_column(layout, consumer)
                        } else {
                            snapshot.with_chunk(layout, 0, consumer)
                        }
                    }));
                    if unwind {
                        assert!(result.is_err());
                    } else {
                        assert_eq!(result.unwrap(), Err(StoredPolynomialErrorV1::Consumer));
                    }
                    assert_poisoned(&mut snapshot);
                    drop(snapshot);
                    assert_eq!(provider.handles.live.get(), 1);
                    assert_sentinel(&mut unrelated);
                }
                // These are explicitly injected adapter errors, not real ciphertext faults.
                for error in [
                    Some(StoredPolynomialErrorV1::Authentication),
                    Some(StoredPolynomialErrorV1::Storage),
                    Some(StoredPolynomialErrorV1::Allocation),
                    None,
                ] {
                    let mut snapshot = filled_quotient(&mut provider, field, 1, interpretation);
                    attempts += 1;
                    let layout = snapshot.layout();
                    snapshot.injected_read_error = error;
                    snapshot.panic_on_read = error.is_none();
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        if full_column {
                            snapshot.with_column(layout, |_| {
                                panic!("injected quotient error exposed plaintext")
                            })
                        } else {
                            snapshot.with_chunk(layout, 0, |_| {
                                panic!("injected quotient error exposed plaintext")
                            })
                        }
                    }));
                    if let Some(error) = error {
                        assert_eq!(result.unwrap(), Err::<(), _>(error));
                    } else {
                        assert!(result.is_err());
                    }
                    assert_poisoned(&mut snapshot);
                    drop(snapshot);
                    assert_eq!(provider.handles.live.get(), 1);
                    assert_sentinel(&mut unrelated);
                }
            }
        }
        drop(unrelated);
    }
    assert_eq!(attempts, 108);
    assert_eq!(provider.handles.live.get(), 0);
    assert!(!provider.window.get());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn quotient_roles_share_original_512_counter_window_and_monotonic_cursor() {
    let directory = tempfile::tempdir().unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
        assert_eq!(provider.handles.limit, 512);
        let [raw_kind, alias_kind, piece_kind] = representatives();
        let mut source = filled_quotient(&mut provider, field, 1, raw_kind);
        let mut alias = filled_quotient(&mut provider, field, 1, alias_kind);
        let mut piece = filled_quotient(&mut provider, field, 1, piece_kind);
        let mut unrelated = filled(&mut provider, field, 1);
        let mut writer = provider
            .create(field, piece_kind.0, 1, piece_kind.1)
            .unwrap();
        assert_eq!(provider.handles.live.get(), 5);
        // These are real leases against the original counter, with no files or plaintext.
        // At this boundary, exactly five actual encrypted writers/snapshots exist.
        let mut counter_only = (0..507)
            .map(|_| LiveSnapshotLease::acquire(&provider.handles).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(provider.handles.live.get(), 512);
        let next = provider.next_ordinal;
        source
            .with_chunk(source.layout(), 0, |values| {
                assert_eq!(values, &[quotient_value(0), quotient_value(1)]);
                assert_eq!(
                    alias.with_column(alias.layout(), |_| panic!("overlapping alias read")),
                    Err::<(), _>(StoredPolynomialErrorV1::Busy)
                );
                assert_eq!(
                    piece.with_chunk(piece.layout(), 0, |_| panic!("overlapping piece read")),
                    Err::<(), _>(StoredPolynomialErrorV1::Busy)
                );
                assert_eq!(
                    writer.write_chunk(0, values),
                    Err(StoredPolynomialErrorV1::Busy)
                );
                assert!(matches!(
                    provider.create(field, alias_kind.0, 1, alias_kind.1),
                    Err(StoredPolynomialErrorV1::Busy)
                ));
                assert_eq!(provider.next_ordinal, next);
                assert_eq!(provider.handles.live.get(), 512);
                Ok(())
            })
            .unwrap();
        assert!(!provider.window.get());
        assert!(matches!(
            provider.create(field, raw_kind.0, 1, raw_kind.1),
            Err(StoredPolynomialErrorV1::Capacity)
        ));
        assert_eq!(provider.next_ordinal, next);
        assert_eq!(provider.handles.live.get(), 512);
        assert!(LiveSnapshotLease::acquire(&provider.handles).is_err());
        drop(counter_only.pop().unwrap());
        assert_eq!(provider.handles.live.get(), 511);
        writer
            .write_chunk(0, &[quotient_value(0), quotient_value(1)])
            .unwrap();
        let mut destination = writer.seal().unwrap();
        assert_eq!(
            provider.handles.live.get(),
            511,
            "seal transfers its original lease"
        );
        let fresh = provider
            .create(field, alias_kind.0, 1, alias_kind.1)
            .unwrap();
        assert_eq!(fresh.layout().ordinal(), next);
        assert_eq!(provider.handles.live.get(), 512);
        drop(fresh);
        drop(counter_only);
        assert_eq!(provider.handles.live.get(), 5);
        for snapshot in [&mut alias, &mut piece, &mut destination] {
            snapshot
                .with_column(snapshot.layout(), |values| {
                    assert_eq!(values, &[quotient_value(0), quotient_value(1)]);
                    Ok(())
                })
                .unwrap();
        }
        for interpretation in representatives() {
            let mut busy_seal = provider
                .create(field, interpretation.0, 1, interpretation.1)
                .unwrap();
            busy_seal
                .write_chunk(0, &[quotient_value(0), quotient_value(1)])
                .unwrap();
            assert_eq!(provider.handles.live.get(), 6);
            piece
                .with_chunk(piece.layout(), 0, |values| {
                    assert_eq!(values, &[quotient_value(0), quotient_value(1)]);
                    assert!(matches!(
                        busy_seal.seal(),
                        Err(StoredPolynomialErrorV1::Busy)
                    ));
                    // A consuming seal refusal destroys only that writer and its lease.
                    assert_eq!(provider.handles.live.get(), 5);
                    Ok(())
                })
                .unwrap();
            assert!(!provider.window.get());
            assert_sentinel(&mut unrelated);
        }
        let after_fresh = provider.next_ordinal;
        provider.directory = directory.path().join("absent-quotient-test-directory");
        assert!(matches!(
            provider.create(field, raw_kind.0, 1, raw_kind.1),
            Err(StoredPolynomialErrorV1::Storage)
        ));
        assert_eq!(
            provider.next_ordinal,
            after_fresh + 1,
            "external failure burns an identity"
        );
        assert_eq!(provider.handles.live.get(), 5);
        assert!(!provider.window.get());
        provider.directory = directory.path().to_owned();
        drop(source);
        assert_eq!(provider.handles.live.get(), 4);
        let replacement = provider.create(field, raw_kind.0, 1, raw_kind.1).unwrap();
        assert_eq!(replacement.layout().ordinal(), after_fresh + 1);
        drop(replacement);
        assert_sentinel(&mut unrelated);
        drop((alias, piece, destination, unrelated));
        assert_eq!(provider.handles.live.get(), 0);
        assert!(!provider.window.get());
        for interpretation in representatives() {
            let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
            provider.next_ordinal = u64::MAX - 1;
            let mut snapshot = filled_quotient(&mut provider, field, 1, interpretation);
            assert_eq!(snapshot.layout().ordinal(), u64::MAX - 1);
            assert_eq!(provider.next_ordinal, u64::MAX);
            assert!(matches!(
                provider.create(field, interpretation.0, 1, interpretation.1),
                Err(StoredPolynomialErrorV1::Capacity)
            ));
            assert_eq!(provider.next_ordinal, u64::MAX);
            assert_eq!(provider.handles.live.get(), 1);
            snapshot
                .with_column(snapshot.layout(), |values| {
                    assert_eq!(values, &[quotient_value(0), quotient_value(1)]);
                    Ok(())
                })
                .unwrap();
            drop(snapshot);
            assert_eq!(provider.handles.live.get(), 0);
            assert!(!provider.window.get());
        }
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

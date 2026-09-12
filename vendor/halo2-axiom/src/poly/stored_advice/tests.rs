//! Metadata, canonical encoding and geometry tests for confidential polynomial storage.

use super::*;

fn layout() -> StoredAdviceLayoutV1 {
    StoredAdviceLayoutV1::new(
        [7; 32],
        4,
        StoredPastaFieldV1::Fp,
        StoredPolynomialBasisV1::Lagrange,
        16,
        9,
        0,
    )
    .unwrap()
}

#[test]
fn metadata_binding_separates_every_polynomial_coordinate() {
    let original = layout();
    let mut variants = vec![original];
    let mut changed = original;
    changed.proof_context[0] ^= 1;
    variants.push(changed);
    let mut changed = original;
    changed.ordinal += 1;
    variants.push(changed);
    let mut changed = original;
    changed.field = StoredPastaFieldV1::Fq;
    variants.push(changed);
    let mut changed = original;
    changed.k -= 1;
    variants.push(changed);
    let mut changed = original;
    changed.column += 1;
    variants.push(changed);
    let mut changed = original;
    changed.phase += 1;
    variants.push(changed);
    let mut changed = original;
    changed.basis = StoredPolynomialBasisV1::Coefficient;
    variants.push(changed);
    for (extension_log, part) in [(1, 0), (2, 0), (2, 1)] {
        let mut changed = original;
        changed.basis = StoredPolynomialBasisV1::CosetPart {
            extension_log,
            part,
        };
        variants.push(changed);
    }
    let bindings = variants
        .iter()
        .map(|value| value.context_digest())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(bindings.len(), variants.len());
    assert!(!bindings.contains(&[0; 32]));
    assert_eq!(original.context_digest(), layout().context_digest());
    assert_eq!(
        (
            original.field(),
            original.basis(),
            original.ordinal(),
            original.k(),
            original.column(),
            original.phase()
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            4,
            16,
            9,
            0
        )
    );
}

#[test]
fn layout_rejects_invalid_domains_phases_and_coset_parts() {
    let make = |proof, k, phase, basis| {
        StoredAdviceLayoutV1::new(proof, 0, StoredPastaFieldV1::Fp, basis, k, 0, phase)
    };
    let lagrange = StoredPolynomialBasisV1::Lagrange;
    assert_eq!(
        make([0; 32], 16, 0, lagrange),
        Err(StoredAdviceErrorV1::Layout)
    );
    assert_eq!(
        make([1; 32], 20, 0, lagrange),
        Err(StoredAdviceErrorV1::Layout)
    );
    assert_eq!(
        make([1; 32], 16, 3, lagrange),
        Err(StoredAdviceErrorV1::Layout)
    );
    for (extension_log, part) in [(0, 0), (4, 0), (3, 8), (u32::MAX, 0)] {
        assert_eq!(
            make(
                [1; 32],
                16,
                0,
                StoredPolynomialBasisV1::CosetPart {
                    extension_log,
                    part
                }
            ),
            Err(StoredAdviceErrorV1::Layout)
        );
    }
    assert!(
        make(
            [1; 32],
            16,
            2,
            StoredPolynomialBasisV1::CosetPart {
                extension_log: 3,
                part: 7
            }
        )
        .is_ok()
    );
}

#[test]
fn chunk_geometry_is_exact_bounded_and_has_one_canonical_tail() {
    for k in 0..=STORED_MAX_K_V1 {
        let layout = StoredAdviceLayoutV1::new(
            [1; 32],
            0,
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Coefficient,
            k,
            0,
            0,
        )
        .unwrap();
        let counts = (0..layout.chunk_count())
            .map(|index| layout.chunk_scalar_count(index as u64).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(counts.iter().sum::<usize>(), 1 << k);
        assert!(counts.iter().all(|count| (1..=256).contains(count)));
        assert_eq!(
            layout.chunk_scalar_count(layout.chunk_count() as u64),
            Err(StoredAdviceErrorV1::ChunkIndex)
        );
        assert_eq!(
            layout.chunk_scalar_count(u64::MAX),
            Err(StoredAdviceErrorV1::ChunkIndex)
        );
    }
    assert_eq!(STORED_SCALAR_BYTES_V1 * STORED_SCALARS_PER_CHUNK_V1, 8192);
}

#[test]
fn field_encodings_are_canonical_without_modular_reduction() {
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        assert!(field.is_canonical(&[0; 32]));
        let mut one = [0; 32];
        one[0] = 1;
        assert!(field.is_canonical(&one));
        assert!(!field.is_canonical(&[0xff; 32]));
        let mut high = [0; 32];
        high[31] = 0x80;
        assert!(!field.is_canonical(&high));
    }
}

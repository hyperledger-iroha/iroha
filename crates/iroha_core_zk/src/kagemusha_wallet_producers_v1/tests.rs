//! Complete catalog syntax/resource tests. Dummy DATA here is never imported as
//! a key, accepted as a monetary proof, or used to construct an installed owner.

use super::*;
use iroha_pasta::msm::MemoryBudget;

fn config() -> CatalogReadConfigV1 {
    CatalogReadConfigV1 {
        key: ReadConfig {
            maximum_bytes: 16,
            maximum_rows: 1 << 16,
            coset_cache: iroha_plonk::keys::CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::DEFAULT,
        },
        maximum_package_bytes: 1 << 16,
    }
}
fn original() -> ProducerOriginalV1 {
    ProducerOriginalV1 {
        descriptor: vec![1],
        verifying_key: vec![2],
        proving_key: vec![3],
    }
}
fn syntax_pack() -> ProducerPackV1 {
    ProducerPackV1 {
        version: 1,
        sigma_proving_keys: vec![vec![1]; 16],
        operations: PRODUCER_CLASSES_V1
            .into_iter()
            .map(|class| OperationOriginalsV1 {
                class,
                sigma_q: original(),
                signatures: vec![original(); class.signature_count()],
                a: vec![original(); class.stage_count()],
                w: vec![original(); class.stage_count() - 1],
            })
            .collect(),
        omega_proving_key: vec![1],
    }
}

#[test]
fn complete_source_covers_all_masks_history_renewal_and_refresh_classes() {
    for (index, class) in PRODUCER_CLASSES_V1.iter().enumerate() {
        assert!(!PRODUCER_CLASSES_V1[..index].contains(class));
    }
    let masks = PRODUCER_CLASSES_V1
        .iter()
        .filter_map(|class| match class {
            ProducerClassV1::Send { controls } => Some(*controls),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(masks, (0..8).collect::<Vec<_>>());
    for renewed in [false, true] {
        for blacklist in [false, true] {
            let class = ProducerClassV1::Receive { blacklist, renewed };
            assert!(PRODUCER_CLASSES_V1.contains(&class));
            assert_eq!(class.sigma_index(), 10 + usize::from(blacklist));
            assert_eq!(class.stage_count(), 11);
        }
    }
    for blacklist in [false, true] {
        assert!(PRODUCER_CLASSES_V1.contains(&ProducerClassV1::ArchiveReceive { blacklist }));
    }
    for class in [
        ProducerClassV1::RefreshCredential,
        ProducerClassV1::RefreshSchemePolicy,
        ProducerClassV1::RefreshBlacklist,
        ProducerClassV1::RefreshQuotaShare,
        ProducerClassV1::RefreshTimeAnchor,
    ] {
        assert!(PRODUCER_CLASSES_V1.contains(&class));
        assert_eq!(class.sigma_index(), 14);
    }
    assert_eq!(
        PRODUCER_CLASSES_V1
            .iter()
            .map(|c| c.stage_count())
            .sum::<usize>(),
        130
    );
}

#[test]
fn partial_extra_reordered_and_wrong_stage_source_trees_are_rejected() {
    let pack = syntax_pack();
    assert_eq!(inventory_bounds(&pack, config()), Ok(()));
    for change in 0..8 {
        let mut bad = pack.clone();
        match change {
            0 => {
                bad.operations.pop();
            }
            1 => bad.operations.push(bad.operations[0].clone()),
            2 => bad.operations.swap(0, 1),
            3 => bad.operations[1].class = bad.operations[0].class,
            4 => {
                bad.sigma_proving_keys.pop();
            }
            5 => {
                bad.operations[10].a.pop();
            }
            6 => bad.operations[0].w.push(original()),
            _ => {
                bad.operations[10].signatures.pop();
            }
        }
        assert_eq!(
            inventory_bounds(&bad, config()),
            Err(Error::Inventory),
            "change{change}"
        );
    }
}

#[test]
fn original_key_and_aggregate_bounds_precede_any_source_import() {
    let pack = syntax_pack();
    for change in 0..6 {
        let mut bad = pack.clone();
        match change {
            0 => bad.sigma_proving_keys[4].clear(),
            1 => bad.omega_proving_key.clear(),
            2 => bad.operations[11].sigma_q.proving_key = vec![0; 17],
            3 => bad.operations[11].a[0].descriptor.clear(),
            4 => bad.operations[11].w[0].verifying_key.clear(),
            _ => bad.version = 2,
        }
        assert_eq!(inventory_bounds(&bad, config()), Err(Error::Inventory));
    }
    let mut tight = config();
    tight.maximum_package_bytes = 64;
    assert_eq!(inventory_bounds(&pack, tight), Err(Error::Inventory));
}

#[test]
fn actual_signed_root_sec1_and_scope_limbs_keep_integer_byte_order() {
    let generator = Affine::GENERATOR;
    let mut original = vec![4];
    for value in [generator.x, generator.y] {
        for word in value.iter().rev() {
            original.extend(word.to_be_bytes());
        }
    }
    let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(&original).unwrap();
    assert_eq!(root_point(key), Ok(generator));
    let digest = core::array::from_fn(|i| i as u8);
    let limbs = digest_limbs(digest);
    let mut bytes = Vec::new();
    for limb in limbs {
        bytes.extend(limb.to_le_bytes());
    }
    assert_eq!(bytes, digest);
}

#[test]
fn source_geometry_has_no_domain_or_shape_fallback() {
    let geometry = SigmaGeometryV1 {
        prefix: PrefixMode::Folded,
        lanes: 1,
        limb_bits: 10,
    };
    for k in [12, 14] {
        let shape = geometry_shape(geometry, SigmaRelation::send(7), k).unwrap();
        assert_eq!(shape.k, k);
        assert_eq!(shape.params.relation().relation, SigmaRelation::send(7));
    }
    for k in [0, 11, 13, 15, 16, 32] {
        assert_eq!(
            geometry_shape(geometry, SigmaRelation::send(7), k),
            Err(Error::Profile)
        );
    }
    assert_eq!(
        geometry_shape(
            SigmaGeometryV1 {
                lanes: 0,
                ..geometry
            },
            SigmaRelation::send(7),
            14
        ),
        Err(Error::Profile)
    );
}

#[test]
fn complete_canonical_carrier_keeps_every_semantic_original_and_has_no_alias_decoder() {
    let pack = syntax_pack();
    let bytes = norito::encode_canonical(&pack).unwrap();
    let decoded: ProducerPackV1 =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .unwrap();
    assert_eq!(inventory_bounds(&decoded, config()), Ok(()));
    assert_eq!(
        decoded
            .operations
            .iter()
            .map(|o| o.class)
            .collect::<Vec<_>>(),
        PRODUCER_CLASSES_V1
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert!(
        norito::decode_canonical_with_limits::<ProducerPackV1>(
            &trailing,
            norito::canonical_decode_limits(trailing.len())
        )
        .is_err()
    );
}

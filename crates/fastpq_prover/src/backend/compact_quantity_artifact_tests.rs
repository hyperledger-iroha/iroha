//! Full-domain artifact identity, context and inherited decode-budget regressions.

use super::*;
use crate::backend::compact_quantity_tests::{QuantityCase, QuantityFixture};
use iroha_data_model::fastpq::{FastpqAxtPreProofMirrorsV1, FastpqAxtPublicMetadataV1};

fn policy() -> ArtifactLimits {
    ArtifactLimits {
        transport: FastpqCompactArtifactDecodeLimits {
            max_wire_bytes: 20 * 1024 * 1024,
            max_bundle_frame_bytes: 16 * 1024 * 1024,
            norito: DecodeLimits::new(
                20 * 1024 * 1024,
                20 * 1024 * 1024,
                25 * 1024 * 1024,
                96 * 1024 * 1024,
                32,
            ),
        },
        public_statement: PublicTransferLimits::default(),
        bundle: BundleLimits {
            max_segments: 2,
            max_wire_bytes: 16 * 1024 * 1024,
            max_total_segment_bytes: 16 * 1024 * 1024,
            max_total_statement_bytes: 512 * 1024,
            max_total_queries: 128,
            max_total_decode_allocation_charges: 128 * 1024 * 1024,
            segment: crate::VerifyLimits {
                max_proof_bytes: 5 * 1024 * 1024,
                max_queries: 64,
                ..crate::VerifyLimits::default()
            },
        },
        max_segment_decode_allocation_charges: 64 * 1024 * 1024,
        total_decode: DecodeLimits::new(
            20 * 1024 * 1024,
            20 * 1024 * 1024,
            30 * 1024 * 1024,
            192 * 1024 * 1024,
            32,
        ),
    }
}

fn fixture() -> QuantityFixture {
    let (fixture, private) = QuantityFixture::new(QuantityCase::MixedScale, 2);
    drop(private);
    fixture
}

fn ordinary(fixture: &QuantityFixture, frame: Vec<u8>) -> FastpqOrdinaryCompactArtifactV1 {
    FastpqOrdinaryCompactArtifactV1 {
        profile_id: quantity_diagnostic_profile_id(),
        statement: fixture.model(),
        bundle_frame: frame,
    }
}

fn axt(fixture: &QuantityFixture, frame: Vec<u8>) -> FastpqAxtCompactArtifactV1 {
    let context = fixture.context();
    let metadata = context.metadata;
    let mirrors = context.mirrors;
    FastpqAxtCompactArtifactV1 {
        profile_id: quantity_diagnostic_profile_id(),
        statement: fixture.model(),
        binding: context.binding.clone(),
        metadata: FastpqAxtPublicMetadataV1 {
            parameter: metadata.parameter.to_owned(),
            entry_hash: metadata.entry_hash.try_into().unwrap(),
            committed_amount: metadata.committed_amount.map(|b| b.try_into().unwrap()),
            expiry_slot: metadata.expiry_slot.try_into().unwrap(),
            manifest_root: metadata.manifest_root.try_into().unwrap(),
            da_commitment: metadata.da_commitment.try_into().unwrap(),
        },
        mirrors: FastpqAxtPreProofMirrorsV1 {
            dsid: mirrors.dsid,
            manifest_root: mirrors.manifest_root,
            da_commitment: mirrors.da_commitment,
            committed_amount: mirrors.committed_amount,
            expiry_slot: mirrors.expiry_slot,
        },
        remote_spend_claims: context.remote_spend_claims.map(<[_]>::to_vec),
        bundle_frame: frame,
    }
}

#[test]
fn fixed_quantity_profile_is_nominal_distinct_and_codec_independent() {
    let expected = quantity_diagnostic_profile_id();
    assert_ne!(expected, diagnostic_profile_id());
    assert_eq!(
        hex::encode(diagnostic_profile_id().0),
        "0f1fcc226630bbf6f89e84dc6a4841868e4835b8d4a5d6a9261f057194a70676"
    );
    for flags in [0, 1, 2, 3, norito::core::default_encode_flags()] {
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(quantity_diagnostic_profile_id(), expected);
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
    }
    eprintln!("quantity_artifact_profile={}", hex::encode(expected.0));
}

// Exact descriptor retained solely as a rejection fixture from the previous
// quantity profile. Field order and both declared Norito identities are the
// predecessor's original values; no production constructor or decoder uses it.
#[derive(NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::compact_model_statement::candidate_artifact::QuantityArtifactProfile",
    frame = "fastpq_prover::compact_v1::QuantityArtifactProfileV1"
)]
struct PredecessorQuantityArtifactProfile {
    version: u16,
    catalog: &'static str,
    protocol: &'static str,
    compact_geometry_identity: Vec<u8>,
    lane_parameter_sha3_256: [u8; 32],
    tape_bytes: [u32; 22],
    quantity_value_schema: &'static str,
    quantity_context_schema: &'static str,
    value_hash_domain: Vec<u8>,
    relation_identities: [&'static str; 4],
}

fn predecessor_quantity_profile() -> FastpqCompactProfileIdV1 {
    let description = PredecessorQuantityArtifactProfile {
        version: 1,
        catalog: fastpq_isi::FASTPQ_CATALOG_V1,
        protocol: fastpq_isi::FASTPQ_FINAL_V1.name,
        compact_geometry_identity: compact_v1::IDENTITY.to_vec(),
        lane_parameter_sha3_256: fastpq_isi::GOLDILOCKS_DIGEST384_PARAMETER_SHA3_256_V1,
        tape_bytes: core::array::from_fn(|round| {
            compact_v1::Round::new(round as u8 + 1)
                .expect("predecessor round")
                .tape_bytes() as u32
        }),
        quantity_value_schema: "fastpq_prover::public_transfer::QuantityValueV1",
        quantity_context_schema: "fastpq_prover::compact_v1::QuantityTransferContextV1",
        value_hash_domain: b"fastpq:quantity:v1:smt:value|".to_vec(),
        relation_identities: [
            FastpqQuantityUnits::TRANSFER_IDENTITY,
            FastpqQuantityUnits::AXT_IDENTITY,
            FastpqQuantityUnits::BATCH_IDENTITY,
            FastpqQuantityUnits::AXT_BATCH_IDENTITY,
        ],
    };
    FastpqCompactProfileIdV1(Sha256::digest(norito::encode_canonical(&description).unwrap()).into())
}

fn assert_profile_rejected_before_child_decode(
    fixture: &QuantityFixture,
    profile_id: FastpqCompactProfileIdV1,
) {
    let mut ordinary = ordinary(fixture, vec![0]);
    let mut axt = axt(fixture, vec![0]);
    ordinary.profile_id = profile_id;
    axt.profile_id = profile_id;
    let expected = fixture.expected();
    let statement_digest = statement_digest(&fixture.model(), usize::MAX).unwrap();
    assert!(matches!(
        verify_bound_quantity_ordinary_artifact(
            &norito::encode_canonical(&ordinary).unwrap(),
            &expected,
            statement_digest,
            policy(),
        ),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::ProfileMismatch
        ))
    ));
    assert!(matches!(
        verify_bound_quantity_axt_artifact(
            &norito::encode_canonical(&axt).unwrap(),
            &expected,
            statement_digest,
            fixture.context(),
            policy(),
        ),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::ProfileMismatch
        ))
    ));
}

#[test]
fn deep_quantity_descriptor_binds_actual_protocol_and_preserves_value_relations() {
    use norito::schema::identity::NoritoSchema as _;

    let descriptor = DeepQuantityArtifactProfile::fixed();
    assert_eq!(
        DeepQuantityArtifactProfile::frame_name(),
        "fastpq_prover::deep_compact::QuantityArtifactProfileV1"
    );
    assert_eq!(descriptor.version, 1);
    assert_eq!(descriptor.catalog, fastpq_isi::FASTPQ_CATALOG_V1);
    assert_eq!(descriptor.protocol_identity, deep_binding::IDENTITY);
    assert_ne!(descriptor.protocol_identity, compact_v1::IDENTITY);
    assert_eq!(descriptor.trace_rows, 65_536);
    assert_eq!(descriptor.trace_root, 0xbe5b_4f4b_47ee_4647);
    assert_eq!(descriptor.lde_rows, 8_388_608);
    assert_eq!(descriptor.lde_root, 0x35c4_528b_4aa6_2eb8);
    assert_eq!(descriptor.coset_offset, COSET_OFFSET);
    assert_eq!(descriptor.committed_columns, 301);
    assert_eq!(descriptor.public_column_layout, LAYOUT_ID);
    assert_eq!(descriptor.constraints, 923);
    assert_eq!(descriptor.modulus, 0xffff_ffff_0000_0001);
    assert_eq!(descriptor.extension_degree, 4);
    assert_eq!(descriptor.extension_nonresidue, 7);
    assert_eq!(
        descriptor.extension_schema,
        crate::GoldilocksFp4V1::frame_name()
    );
    assert_eq!(descriptor.extension_bytes, 32);
    assert_eq!(descriptor.hash_digest_lanes, 6);
    assert_eq!(
        descriptor.lane_parameter_sha3_256,
        fastpq_isi::GOLDILOCKS_DIGEST384_PARAMETER_SHA3_256_V1
    );
    assert_eq!(descriptor.fri_arities, [16, 16, 8, 8, 4]);
    assert_eq!(
        descriptor.fri_lengths,
        [8_388_608, 524_288, 32_768, 4_096, 512, 128]
    );
    assert_eq!(descriptor.fri_degrees, [65_536, 4_096, 256, 32, 4, 1]);
    assert_eq!(descriptor.query_count, 64);
    assert_eq!(descriptor.query_candidates, 74);
    assert_eq!(
        descriptor.tape_bytes,
        [48, 29_568, 48, 48, 48, 48, 48, 48, 48, 624]
    );
    assert_eq!(
        descriptor.proof_frame_schema,
        "fastpq_prover::deep_compact::ProofV1"
    );
    assert_eq!(
        descriptor.proof_frame_hash,
        norito::schema::identity::frame_hash::<DeepProof>()
    );
    assert_eq!(
        descriptor.quantity_value_schema,
        "fastpq_prover::public_transfer::QuantityValueV1"
    );
    assert_eq!(
        descriptor.quantity_context_schema,
        "fastpq_prover::compact_v1::QuantityTransferContextV1"
    );
    assert_eq!(
        descriptor.value_hash_domain,
        b"fastpq:quantity:v1:smt:value|"
    );
    assert_eq!(
        descriptor.relation_identities,
        [
            FastpqQuantityUnits::BATCH_IDENTITY,
            FastpqQuantityUnits::AXT_BATCH_IDENTITY
        ]
    );
    assert_eq!(descriptor.profile_id(), quantity_diagnostic_profile_id());
}

#[test]
fn every_deep_descriptor_field_changes_the_fixed_artifact_profile() {
    let fixture = fixture();
    let fixed = DeepQuantityArtifactProfile::fixed();
    let expected = fixed.profile_id();
    let reject = |changed: DeepQuantityArtifactProfile| {
        let profile = changed.profile_id();
        assert_ne!(profile, expected);
        assert_profile_rejected_before_child_decode(&fixture, profile);
    };
    macro_rules! change {
        ($field:ident, $value:expr) => {{
            let mut changed = fixed.clone();
            changed.$field = $value;
            reject(changed);
        }};
    }
    change!(version, 2);
    change!(catalog, "different catalog");
    change!(protocol_identity, compact_v1::IDENTITY.to_vec());
    change!(trace_rows, 32_768);
    change!(trace_root, fixed.trace_root ^ 1);
    change!(lde_rows, 524_288);
    change!(lde_root, fastpq_isi::FASTPQ_FINAL_V1.lde_root);
    change!(coset_offset, fixed.coset_offset ^ 1);
    change!(committed_columns, 342);
    change!(public_column_layout, "different column layout");
    change!(constraints, 922);
    change!(modulus, fixed.modulus - 1);
    change!(extension_degree, 2);
    change!(extension_nonresidue, 11);
    change!(extension_schema, "different extension".to_owned());
    change!(extension_bytes, 16);
    change!(hash_digest_lanes, 4);
    change!(lane_parameter_sha3_256, [0; 32]);
    change!(query_count, 375);
    change!(query_candidates, 75);
    change!(proof_frame_schema, "different proof frame".to_owned());
    change!(proof_frame_hash, [0; 16]);
    change!(quantity_value_schema, "different value frame");
    change!(quantity_context_schema, "different context frame");
    change!(value_hash_domain, b"different value domain".to_vec());
    for index in 0..5 {
        let mut changed = fixed.clone();
        changed.fri_arities[index] *= 2;
        reject(changed);
    }
    for index in 0..6 {
        let mut changed = fixed.clone();
        changed.fri_lengths[index] *= 2;
        reject(changed);
        let mut changed = fixed.clone();
        changed.fri_degrees[index] *= 2;
        reject(changed);
    }
    for index in 0..10 {
        let mut changed = fixed.clone();
        changed.tape_bytes[index] += 48;
        reject(changed);
    }
    for index in 0..2 {
        let mut changed = fixed.clone();
        changed.relation_identities[index] = "different outer relation";
        reject(changed);
    }
    let mut changed = fixed.clone();
    changed.relation_identities.swap(0, 1);
    reject(changed);
}

#[test]
fn predecessor_quantity_profile_is_rejected_on_both_bound_routes() {
    let predecessor = predecessor_quantity_profile();
    assert_ne!(predecessor, quantity_diagnostic_profile_id());
    assert_ne!(predecessor, diagnostic_profile_id());
    assert_profile_rejected_before_child_decode(&fixture(), predecessor);
}

#[test]
fn advertised_profiles_and_structural_route_cannot_select_quantity_verifiers() {
    let f = fixture();
    let ordinary_bytes = norito::encode_canonical(&ordinary(&f, vec![])).unwrap();
    let axt_bytes = norito::encode_canonical(&axt(&f, vec![])).unwrap();
    assert!(matches!(
        verify_ordinary_artifact(&ordinary_bytes, &f.expected(), policy()),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::ProfileMismatch
        ))
    ));
    assert!(matches!(
        verify_axt_artifact(&axt_bytes, &f.expected(), f.context(), policy()),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::ProfileMismatch
        ))
    ));
    let mut changed = ordinary(&f, vec![]);
    changed.profile_id = diagnostic_profile_id();
    let bytes = norito::encode_canonical(&changed).unwrap();
    assert!(matches!(
        verify_quantity_ordinary_artifact(&bytes, &f.expected(), policy()),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::ProfileMismatch
        ))
    ));
    assert!(matches!(
        verify_ordinary_artifact(&bytes, &f.expected(), policy()),
        Err(ArtifactError::Verify(_))
    ));
    assert!(matches!(
        verify_quantity_ordinary_artifact(&axt_bytes, &f.expected(), policy()),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::Norito(_)
        ))
    ));
    assert!(matches!(
        verify_quantity_axt_artifact(&ordinary_bytes, &f.expected(), f.context(), policy()),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::Norito(_)
        ))
    ));
}

#[test]
fn quantity_artifact_compares_every_expected_input_before_child_decode() {
    let f = fixture();
    let ordinary = norito::encode_canonical(&ordinary(&f, vec![0])).unwrap();
    let axt = norito::encode_canonical(&axt(&f, vec![0])).unwrap();
    for index in 0..7 {
        let mut expected = f.expected();
        match index {
            0 => expected.dsid[0] ^= 1,
            1 => expected.slot ^= 1,
            2 => expected.old_root[0] ^= 1,
            3 => expected.new_root[0] ^= 1,
            4 => expected.perm_root[0] ^= 1,
            5 => expected.tx_set_hash[0] ^= 1,
            _ => expected.ordering_hash[0] ^= 1,
        }
        assert!(matches!(
            verify_quantity_ordinary_artifact(&ordinary, &expected, policy()),
            Err(ArtifactError::Verify(Error::PublicIoMismatch { .. }))
        ));
        assert!(matches!(
            verify_quantity_axt_artifact(&axt, &expected, f.context(), policy()),
            Err(ArtifactError::Verify(Error::PublicIoMismatch { .. }))
        ));
    }
}

#[test]
fn quantity_artifact_requires_original_axt_advertisements_and_remote_preimages() {
    let f = fixture();
    let original = axt(&f, vec![0]);
    for i in 0..4 {
        let mut changed = original.clone();
        match i {
            0 => changed.binding.source_receipt_id.push('x'),
            1 => changed.metadata.parameter.push('x'),
            2 => changed.mirrors.manifest_root[0] ^= 1,
            _ => changed
                .remote_spend_claims
                .as_mut()
                .unwrap()
                .pop()
                .map(|_| ())
                .unwrap(),
        }
        let bytes = norito::encode_canonical(&changed).unwrap();
        assert!(matches!(
            verify_quantity_axt_artifact(&bytes, &f.expected(), f.context(), policy()),
            Err(ArtifactError::Verify(Error::PublicIoMismatch { .. }))
        ));
    }
}

#[test]
fn quantity_artifact_raw_and_inherited_caps_precede_untrusted_allocations() {
    let f = fixture();
    let expected = f.expected();
    let mut limits = policy();
    limits.transport.max_wire_bytes = 0;
    assert!(matches!(
        verify_quantity_ordinary_artifact(&[0], &expected, limits),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::WireBytes { .. }
        ))
    ));
    assert!(matches!(
        verify_quantity_axt_artifact(&[0], &expected, f.context(), limits),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::WireBytes { .. }
        ))
    ));
    let bytes = norito::encode_canonical(&ordinary(&f, vec![0])).unwrap();
    assert!(
        norito::core::with_decode_limits_scope(
            DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32),
            || verify_quantity_ordinary_artifact(&bytes, &expected, policy())
        )
        .is_err()
    );
    let mut limits = policy();
    limits.transport.max_bundle_frame_bytes = 0;
    assert!(matches!(
        verify_quantity_ordinary_artifact(&bytes, &expected, limits),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::BundleBytes { .. }
        ))
    ));
}

#[test]
fn quantity_artifact_false_rows_do_not_reach_the_proof_decoder() {
    let f = fixture();
    let mut artifact = ordinary(&f, vec![0]);
    artifact.statement.transitions[0].pre_value = 1_u64.to_le_bytes().to_vec();
    let bytes = norito::encode_canonical(&artifact).unwrap();
    assert!(matches!(
        verify_quantity_ordinary_artifact(&bytes, &f.expected(), policy()),
        Err(ArtifactError::Verify(_))
    ));
    let limits = PublicTransferLimits {
        max_public_bytes: 0,
        ..PublicTransferLimits::default()
    };
    assert!(matches!(
        verify_quantity_ordinary_artifact(
            &norito::encode_canonical(&ordinary(&f, vec![0])).unwrap(),
            &f.expected(),
            ArtifactLimits {
                public_statement: limits,
                ..policy()
            }
        ),
        Err(ArtifactError::Verify(Error::VerifierLimitExceeded { .. }))
    ));
}

#[test]
fn public_quantity_construction_preserves_enclosing_decode_allocation_budget() {
    use crate::gadgets::public_transfer_statement::{
        TransferSmtBuildLimits, materialize_quantity_public_transfers,
        prepare_quantity_public_transfers,
    };
    let f = fixture();
    // Balance keys use canonical identity encoding, independently of account display.
    // The actual bounded decode here is each QuantityValueV1 transition value.
    let budget = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32);
    let prepared = norito::core::with_decode_limits_scope(budget, || {
        prepare_quantity_public_transfers(
            &f.rows,
            &f.claims,
            f.inputs,
            ProofSemantics::StateTransition,
            PublicTransferLimits::default(),
        )
    });
    assert!(matches!(
        prepared,
        Err(Error::Encode(norito::Error::TotalAllocationExceeded {
            attempted,
            limit: 0,
        })) if attempted > 0
    ));
    let materialized = norito::core::with_decode_limits_scope(budget, || {
        materialize_quantity_public_transfers(
            &f.claims,
            f.inputs,
            ProofSemantics::StateTransition,
            PublicTransferLimits::default(),
            TransferSmtBuildLimits::for_update_limit(4).unwrap(),
        )
    });
    assert!(matches!(
        materialized,
        Err(Error::Encode(norito::Error::TotalAllocationExceeded {
            attempted,
            limit: 0,
        })) if attempted > 0
    ));

    // A rejected nested decode must release its scope. The same immutable public
    // inputs still prepare and materialize to the original rows, roots and order.
    let restored = f.prepare(ProofSemantics::StateTransition);
    assert_eq!(restored.transitions(), f.rows.as_slice());
    assert_eq!(*restored.public_inputs(), f.inputs);
    let materialized = materialize_quantity_public_transfers(
        &f.claims,
        f.inputs,
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
        TransferSmtBuildLimits::for_update_limit(4).unwrap(),
    )
    .unwrap();
    assert_eq!(materialized.transitions(), f.rows.as_slice());
    assert_eq!(materialized.public_inputs(), f.inputs);
    assert_eq!(materialized.ordering_hash(), restored.ordering_hash());
}

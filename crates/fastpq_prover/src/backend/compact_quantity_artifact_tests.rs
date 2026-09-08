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
            max_total_queries: 750,
            max_total_decode_allocation_charges: 128 * 1024 * 1024,
            segment: crate::VerifyLimits {
                max_proof_bytes: 5 * 1024 * 1024,
                max_queries: 375,
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
        "19093354f57a228cf17a92d94212a4419167225d04e4e2ab46d0a2a6c6860ba4"
    );
    for flags in [0, 1, 2, 3, norito::core::default_encode_flags()] {
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(quantity_diagnostic_profile_id(), expected);
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
    }
    eprintln!("quantity_artifact_profile={}", hex::encode(expected.0));
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

fn complete_retained_quantity_artifact(is_axt: bool) {
    let f = fixture();
    let expected = f.expected();
    let label = if is_axt { "axt" } else { "ordinary" };
    let bundle_hash = if is_axt {
        "3ceff34c2ca74ef1554f23bc7e8ec1f4c493a286dd9e532614b511cdfbd5bb20"
    } else {
        "2d655210af9e7f550f8cd9706899851fe804ff0d06ac50dc2fc08e8b0ddfa014"
    };
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation");
    let frame =
        std::fs::read(directory.join(format!("quantity-compact-{label}-bundle-{bundle_hash}.bin")))
            .expect("run the corresponding complete full-domain bundle diagnostic first");
    assert_eq!(hex::encode(Sha256::digest(&frame)), bundle_hash);
    let inner_digest: [u8; 32] = Hash::new(&frame).into();
    let statement_digest: [u8; 32] =
        Hash::new(norito::encode_canonical(&f.model()).unwrap()).into();
    let bytes = if is_axt {
        norito::encode_canonical(&axt(&f, frame)).unwrap()
    } else {
        norito::encode_canonical(&ordinary(&f, frame)).unwrap()
    };
    let verify = |bytes: &[u8], limits| {
        if is_axt {
            verify_quantity_axt_artifact(bytes, &expected, f.context(), limits)
        } else {
            verify_quantity_ordinary_artifact(bytes, &expected, limits)
        }
    };
    let started = std::time::Instant::now();
    let (result, usage) = norito::core::with_decode_limits_measured(policy().total_decode, || {
        verify(&bytes, policy())
    });
    let result = result.unwrap();
    let elapsed = started.elapsed();
    let identity = result.identity();
    assert_eq!(identity.profile_id, quantity_diagnostic_profile_id());
    assert_eq!(
        identity.proof_kind,
        if is_axt {
            FastpqProofKindV1::AxtCompact
        } else {
            FastpqProofKindV1::OrdinaryCompact
        }
    );
    assert_eq!(identity.public_statement_digest, statement_digest);
    assert_eq!(
        identity.artifact_digest,
        <[u8; 32]>::from(Hash::new(&bytes))
    );
    assert_eq!(identity.inner_bundle_digest, inner_digest);
    assert_eq!(identity.artifact_bytes, bytes.len() as u64);
    assert_eq!(result.bundle().public_io(), f.expected());
    assert_eq!(result.bundle().work().air_evaluations, 750);
    assert_eq!(result.bundle().work().terminal_degree_checks, 2);
    let FastpqCommitmentDescriptionV1::OrderedCompactAir(roots) = &identity.commitments else {
        panic!("unexpected commitment description")
    };
    assert_eq!(roots.segment_count, 2);
    assert_eq!(
        roots.segment_air_row_roots.as_slice(),
        result.bundle().row_roots()
    );
    assert_eq!(
        norito::decode_canonical::<FastpqArtifactIdentityDescriptionV1>(
            &norito::encode_canonical(identity).unwrap()
        )
        .unwrap(),
        *identity
    );
    // Exercise the public library boundary on this same complete retained proof.
    // Expectations are rebuilt from the independent fixture, never decoded bytes.
    let p = policy();
    let public_limits = crate::offline_compact::VerificationLimits {
        transport: p.transport,
        public_statement: p.public_statement,
        bundle: crate::offline_compact::BundleVerificationLimits {
            max_segments: p.bundle.max_segments,
            max_wire_bytes: p.bundle.max_wire_bytes,
            max_total_segment_bytes: p.bundle.max_total_segment_bytes,
            max_total_statement_bytes: p.bundle.max_total_statement_bytes,
            max_total_queries: p.bundle.max_total_queries,
            max_total_decode_allocation_charges: p.bundle.max_total_decode_allocation_charges,
            segment: p.bundle.segment,
        },
        max_segment_decode_allocation_charges: p.max_segment_decode_allocation_charges,
        total_decode: p.total_decode,
    };
    let public_expected = crate::offline_compact::ExpectedStatement {
        inputs: f.model().public_inputs,
        ordering_hash: expected.ordering_hash,
        public_statement_digest: statement_digest,
    };
    let offline = if is_axt {
        let independent = axt(&f, Vec::new());
        crate::offline_compact::verify_quantity_axt_artifact(
            &bytes,
            public_expected,
            crate::offline_compact::ExpectedAxtContext {
                binding: &independent.binding,
                metadata: &independent.metadata,
                mirrors: independent.mirrors,
                remote_spend_claims: independent.remote_spend_claims.as_deref(),
            },
            public_limits,
        )
    } else {
        crate::offline_compact::verify_quantity_ordinary_artifact(
            &bytes,
            public_expected,
            public_limits,
        )
    }
    .unwrap();
    assert_eq!(offline.expected_statement(), public_expected);
    assert_eq!(offline.identity(), result.identity());
    assert_eq!(offline.air_row_roots(), result.bundle().row_roots());
    assert_eq!(offline.segments(), result.bundle().segments());
    assert_eq!(offline.bundle_frame_bytes(), result.bundle().wire_bytes());
    assert_eq!(offline.statement_bytes(), result.bundle().statement_bytes());
    let old_work = result.bundle().work();
    assert_eq!(
        offline.work(),
        crate::offline_compact::VerificationWork {
            proof_bytes: old_work.proof_bytes,
            transcripts: old_work.transcripts,
            row_leaves: old_work.row_leaves,
            oracle_leaves: old_work.oracle_leaves,
            fri_leaves: old_work.fri_leaves,
            parent_hashes: old_work.parent_hashes,
            air_evaluations: old_work.air_evaluations,
            terminal_degree_checks: old_work.terminal_degree_checks,
        }
    );
    let charges = usage.total_allocated_bytes();
    let elements = usage.total_elements();
    let mut exact = policy();
    exact.total_decode =
        DecodeLimits::new(20 * 1024 * 1024, 20 * 1024 * 1024, elements, charges, 32);
    assert_eq!(verify(&bytes, exact).unwrap(), result);
    let mut low = exact;
    low.total_decode = DecodeLimits::new(
        20 * 1024 * 1024,
        20 * 1024 * 1024,
        elements,
        charges - 1,
        32,
    );
    assert!(verify(&bytes, low).is_err());
    low.total_decode = DecodeLimits::new(
        20 * 1024 * 1024,
        20 * 1024 * 1024,
        elements - 1,
        charges,
        32,
    );
    assert!(verify(&bytes, low).is_err());
    assert!(
        norito::core::with_decode_limits_scope(low.total_decode, || verify(&bytes, policy()))
            .is_err()
    );
    let mut changed = bytes.clone();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    assert!(verify(&changed, policy()).is_err());
    let sha = hex::encode(Sha256::digest(&bytes));
    let path = directory.join(format!("quantity-artifact-{label}-{sha}.bin"));
    std::fs::write(&path, &bytes).unwrap();
    eprintln!(
        "quantity_artifact={label}; profile={}; bytes={}; verify={elapsed:?}; decode_allocation_charges={charges}; decode_elements={elements}; work={:?}; retained={}; sha256={sha}",
        hex::encode(quantity_diagnostic_profile_id().0),
        bytes.len(),
        result.bundle().work(),
        path.display()
    );
}

#[test]
#[ignore = "requires the pinned complete full-domain ordinary bundle; verifies exact cumulative artifact caps"]
fn complete_ordinary_quantity_artifact_verifies_retained_bundle_and_cumulative_limits() {
    complete_retained_quantity_artifact(false);
}

#[test]
#[ignore = "requires the pinned complete full-domain AXT bundle; verifies exact cumulative artifact caps"]
fn complete_axt_quantity_artifact_verifies_retained_bundle_and_cumulative_limits() {
    complete_retained_quantity_artifact(true);
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

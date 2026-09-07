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
        "c1f0ca64798a78dc186b99fefd72645d584701dd3980886dcc50b5317011558e"
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
        "106eedbef45e0c652289e014643004be8c4d02d76dee9341b5dc6bef68f50280"
    } else {
        "72446454083030585da910cf121a0b76ce61d1ba04db8ea911a412b6fda4f14e"
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
fn public_quantity_construction_returns_an_error_when_account_formatting_is_over_budget() {
    use crate::gadgets::public_transfer_statement::{
        TransferSmtBuildLimits, materialize_quantity_public_transfers,
        prepare_quantity_public_transfers,
    };
    let f = fixture();
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
    assert!(matches!(prepared, Err(Error::TransferInvariant { .. })));
    let materialized = norito::core::with_decode_limits_scope(budget, || {
        materialize_quantity_public_transfers(
            &f.claims,
            f.inputs,
            ProofSemantics::StateTransition,
            PublicTransferLimits::default(),
            TransferSmtBuildLimits::for_update_limit(4).unwrap(),
        )
    });
    assert!(matches!(materialized, Err(Error::TransferInvariant { .. })));
}

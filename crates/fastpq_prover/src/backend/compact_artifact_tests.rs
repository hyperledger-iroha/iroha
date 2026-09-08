//! Candidate model transport, independent-context and cumulative-budget regressions.

use super::*;
use crate::backend::compact_axt_context::tests::Fixture;
use iroha_data_model::fastpq::{FastpqAxtPreProofMirrorsV1, FastpqAxtPublicMetadataV1};

fn context(fixture: &Fixture) -> AxtVerificationContext<'_> {
    AxtVerificationContext {
        binding: &fixture.binding,
        metadata: fixture.metadata(),
        mirrors: fixture.outer,
        remote_spend_claims: fixture.remote.as_deref(),
    }
}

fn limits() -> ArtifactLimits {
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
                max_proof_bytes: 4_326_227,
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

fn ordinary(
    fixture: &Fixture,
    bundle_frame: Vec<u8>,
) -> (FastpqOrdinaryCompactArtifactV1, PublicIO) {
    let prepared = fixture.prepare(ProofSemantics::StateTransition);
    let expected = fixture.expected(&prepared);
    (
        FastpqOrdinaryCompactArtifactV1 {
            profile_id: diagnostic_profile_id(),
            statement: super::super::tests::model(&prepared),
            bundle_frame,
        },
        expected,
    )
}

fn axt(fixture: &Fixture, bundle_frame: Vec<u8>) -> (FastpqAxtCompactArtifactV1, PublicIO) {
    let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
    let expected = fixture.expected(&prepared);
    let metadata = fixture.metadata();
    (
        FastpqAxtCompactArtifactV1 {
            profile_id: diagnostic_profile_id(),
            statement: super::super::tests::model(&prepared),
            binding: fixture.binding.clone(),
            metadata: FastpqAxtPublicMetadataV1 {
                parameter: metadata.parameter.to_owned(),
                entry_hash: metadata.entry_hash.try_into().unwrap(),
                committed_amount: metadata.committed_amount.map(|b| b.try_into().unwrap()),
                expiry_slot: metadata.expiry_slot.try_into().unwrap(),
                manifest_root: metadata.manifest_root.try_into().unwrap(),
                da_commitment: metadata.da_commitment.try_into().unwrap(),
            },
            mirrors: FastpqAxtPreProofMirrorsV1 {
                dsid: fixture.outer.dsid,
                manifest_root: fixture.outer.manifest_root,
                da_commitment: fixture.outer.da_commitment,
                committed_amount: fixture.outer.committed_amount,
                expiry_slot: fixture.outer.expiry_slot,
            },
            remote_spend_claims: fixture.remote.clone(),
            bundle_frame,
        },
        expected,
    )
}

#[test]
fn fixed_profile_and_nominal_route_cannot_be_chosen_by_artifact() {
    assert_eq!(
        hex::encode(diagnostic_profile_id().0),
        "0f1fcc226630bbf6f89e84dc6a4841868e4835b8d4a5d6a9261f057194a70676"
    );
    let fixture = Fixture::new(false);
    let (ordinary, expected) = ordinary(&fixture, Vec::new());
    let (axt, _) = axt(&fixture, Vec::new());
    let mut wrong = ordinary.clone();
    wrong.profile_id.0[0] ^= 1;
    assert!(matches!(
        verify_ordinary_artifact(
            &norito::encode_canonical(&wrong).unwrap(),
            &expected,
            limits()
        ),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::ProfileMismatch
        ))
    ));
    assert!(matches!(
        verify_ordinary_artifact(
            &norito::encode_canonical(&axt).unwrap(),
            &expected,
            limits()
        ),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::Norito(_)
        ))
    ));
    assert!(matches!(
        verify_axt_artifact(
            &norito::encode_canonical(&ordinary).unwrap(),
            &expected,
            context(&fixture),
            limits()
        ),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::Norito(_)
        ))
    ));
}

#[test]
fn raw_cap_precedes_decode_and_outer_budget_cannot_be_relaxed() {
    let fixture = Fixture::new(false);
    let (artifact, expected) = ordinary(&fixture, Vec::new());
    let bytes = norito::encode_canonical(&artifact).unwrap();
    let mut policy = limits();
    policy.transport.max_wire_bytes = bytes.len() - 1;
    let zero = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32);
    let (result, usage) = norito::core::with_decode_limits_measured(zero, || {
        verify_ordinary_artifact(&bytes, &expected, policy)
    });
    assert!(matches!(
        result,
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::WireBytes { .. }
        ))
    ));
    assert_eq!(usage.total_allocated_bytes(), 0);
    assert!(matches!(
        norito::core::with_decode_limits_scope(zero, || verify_ordinary_artifact(
            &bytes,
            &expected,
            limits()
        )),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::Norito(norito::Error::TotalAllocationExceeded { .. })
        ))
    ));
    let mut policy = limits();
    policy.total_decode = zero;
    assert!(matches!(
        verify_ordinary_artifact(&bytes, &expected, policy),
        Err(ArtifactError::Transport(
            FastpqCompactArtifactDecodeError::Norito(norito::Error::TotalAllocationExceeded { .. })
        ))
    ));
}

#[test]
fn all_public_expectations_are_checked_before_child_decoding() {
    let fixture = Fixture::new(false);
    let (artifact, expected) = ordinary(&fixture, Vec::new());
    for index in 0..7 {
        let mut altered = artifact.clone();
        let public = &mut altered.statement.public_inputs;
        match index {
            0 => public.dsid[0] ^= 1,
            1 => public.slot ^= 1,
            2 => public.old_root[0] ^= 1,
            3 => public.new_root[0] ^= 1,
            4 => public.perm_root[0] ^= 1,
            5 => public.tx_set_hash[0] ^= 1,
            _ => altered.statement.ordering_hash[0] ^= 1,
        }
        assert!(matches!(
            verify_ordinary_artifact(
                &norito::encode_canonical(&altered).unwrap(),
                &expected,
                limits()
            ),
            Err(ArtifactError::Verify(Error::PublicIoMismatch {
                field: "compact_model_public_io"
            }))
        ));
    }
    let mut policy = limits();
    policy.public_statement.max_rows = 0;
    assert!(matches!(
        verify_ordinary_artifact(
            &norito::encode_canonical(&artifact).unwrap(),
            &expected,
            policy
        ),
        Err(ArtifactError::Verify(Error::VerifierLimitExceeded {
            limit: "max_public_transfer_rows",
            ..
        }))
    ));
}

#[test]
fn all_axt_advertisements_must_match_independent_caller_context() {
    let fixture = Fixture::new(true);
    let (artifact, expected) = axt(&fixture, Vec::new());
    validate_axt_advertisement(&artifact, context(&fixture)).unwrap();
    for index in 0..15 {
        let mut altered = artifact.clone();
        let field = match index {
            0 => {
                altered.binding.parameter.push('x');
                "compact_artifact_binding"
            }
            1 => {
                altered.metadata.parameter.push('x');
                "compact_artifact_parameter"
            }
            2 => {
                altered.metadata.entry_hash[0] ^= 1;
                "compact_artifact_entry_hash"
            }
            3 => {
                altered.metadata.committed_amount = Some([0; 16]);
                "compact_artifact_amount_bytes"
            }
            4 => {
                altered.metadata.expiry_slot[0] ^= 1;
                "compact_artifact_expiry_bytes"
            }
            5 => {
                altered.metadata.manifest_root[0] ^= 1;
                "compact_artifact_manifest_bytes"
            }
            6 => {
                altered.metadata.da_commitment[0] ^= 1;
                "compact_artifact_da_bytes"
            }
            7 => {
                altered.mirrors.dsid = iroha_data_model::nexus::DataSpaceId::new(u64::MAX);
                "compact_artifact_mirror_dsid"
            }
            8 => {
                altered.mirrors.manifest_root[0] ^= 1;
                "compact_artifact_mirror_manifest"
            }
            9 => {
                altered.mirrors.da_commitment = Some([0; 32]);
                "compact_artifact_mirror_da"
            }
            10 => {
                altered.mirrors.committed_amount = Some(0);
                "compact_artifact_mirror_amount"
            }
            11 => {
                altered.mirrors.expiry_slot = Some(0);
                "compact_artifact_mirror_expiry"
            }
            12 => {
                altered.remote_spend_claims = None;
                "compact_artifact_remote_claims"
            }
            13 => {
                altered.remote_spend_claims = Some(Vec::new());
                "compact_artifact_remote_claims"
            }
            _ => {
                altered.metadata.committed_amount = None;
                "compact_artifact_amount_bytes"
            }
        };
        let result = verify_axt_artifact(
            &norito::encode_canonical(&altered).unwrap(),
            &expected,
            context(&fixture),
            limits(),
        );
        assert!(
            matches!(result, Err(ArtifactError::Verify(Error::PublicIoMismatch { field: actual })) if actual == field),
            "case {index}: {result:?}"
        );
    }
    let no_remote = Fixture::new(false);
    let (mut altered, _) = axt(&no_remote, Vec::new());
    altered.remote_spend_claims = Some(Vec::new());
    assert!(validate_axt_advertisement(&altered, context(&no_remote)).is_err());
}

#[test]
fn canonical_model_transport_is_independent_of_ambient_flags() {
    let fixture = Fixture::new(false);
    let (artifact, expected) = ordinary(&fixture, Vec::new());
    let bytes = norito::encode_canonical(&artifact).unwrap();
    for flags in (u8::MIN..=u8::MAX).filter(|&f| norito::core::validate_header_flags(f).is_ok()) {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::encode_canonical(&artifact).unwrap(), bytes);
        let result = verify_ordinary_artifact(&bytes, &expected, limits());
        // Valid model facts reach the intentionally absent child carrier.
        assert!(
            matches!(result, Err(ArtifactError::Verify(Error::Encode(_)))),
            "{result:?}"
        );
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
    }
}

fn fixture_with_actual_public_roots(remote: bool) -> Fixture {
    use iroha_data_model::fastpq::{
        TransferDeltaTranscript, TransferSmtWitness, TransferTranscript,
    };
    let mut fixture = Fixture::multiple(2, remote);
    let mut transcripts: Vec<_> = {
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        prepared
            .claims()
            .iter()
            .map(|claim| TransferTranscript {
                batch_hash: claim.batch_hash,
                authority_digest: claim.authority_digest,
                poseidon_preimage_digest: claim.poseidon_preimage_digest,
                deltas: claim
                    .deltas
                    .iter()
                    .map(|delta| TransferDeltaTranscript {
                        from_account: delta.from_account.clone(),
                        to_account: delta.to_account.clone(),
                        asset_definition: delta.asset_definition.clone(),
                        amount: delta.amount.clone(),
                        from_balance_before: delta.from_balance_before.clone(),
                        from_balance_after: delta.from_balance_after.clone(),
                        to_balance_before: delta.to_balance_before.clone(),
                        to_balance_after: delta.to_balance_after.clone(),
                        from_smt_witness: TransferSmtWitness::default(),
                        to_smt_witness: TransferSmtWitness::default(),
                    })
                    .collect(),
            })
            .collect()
    };
    let (old_root, new_root) =
        crate::gadgets::transfer::attach_transfer_smt_witnesses(&mut transcripts).unwrap();
    fixture.set_touched_roots(old_root, new_root);
    // Only the public fixture leaves this helper. No private trace is generated.
    fixture
}

fn complete_retained_artifact(is_axt: bool) {
    let fixture = fixture_with_actual_public_roots(is_axt);
    let label = if is_axt { "axt" } else { "ordinary" };
    // TODO: Pin new six-lane bundle hashes and roots from actual final-geometry
    // proof captures before executing this retained-artifact diagnostic.
    let bundle_hash = if is_axt {
        "228fa5abfc0fc6cd0c2d2b9d0451933a9123c8287714441c8b46f38a29d8a599"
    } else {
        "c2f832c5ba254b6ee529007ca5801ba86d6a3065394230be3a1f376eb05733c4"
    };
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation");
    let frame =
        std::fs::read(directory.join(format!("compact-v1-{label}-two-delta-{bundle_hash}.bin")))
            .expect("run the corresponding complete candidate bundle diagnostic first");
    assert_eq!(hex::encode(Sha256::digest(&frame)), bundle_hash);
    let expected_inner_digest: [u8; 32] = Hash::new(&frame).into();
    let expected_statement = if is_axt {
        axt(&fixture, Vec::new()).0.statement
    } else {
        ordinary(&fixture, Vec::new()).0.statement
    };
    let expected_statement_digest: [u8; 32] =
        Hash::new(norito::encode_canonical(&expected_statement).unwrap()).into();
    drop(expected_statement);
    let (bytes, expected) = if is_axt {
        let (artifact, expected) = axt(&fixture, frame);
        (norito::encode_canonical(&artifact).unwrap(), expected)
    } else {
        let (artifact, expected) = ordinary(&fixture, frame);
        (norito::encode_canonical(&artifact).unwrap(), expected)
    };
    let verify = |raw: &[u8], policy| {
        if is_axt {
            verify_axt_artifact(raw, &expected, context(&fixture), policy)
        } else {
            verify_ordinary_artifact(raw, &expected, policy)
        }
    };
    let start = std::time::Instant::now();
    let (result, usage) = norito::core::with_decode_limits_measured(limits().total_decode, || {
        verify(&bytes, limits())
    });
    let result = result.unwrap();
    let elapsed = start.elapsed();
    let identity = result.identity();
    assert_eq!(identity.profile_id, diagnostic_profile_id());
    assert_eq!(
        identity.proof_kind,
        if is_axt {
            FastpqProofKindV1::AxtCompact
        } else {
            FastpqProofKindV1::OrdinaryCompact
        }
    );
    assert_eq!(identity.artifact_bytes, bytes.len() as u64);
    assert_eq!(identity.public_statement_digest, expected_statement_digest);
    assert_eq!(identity.inner_bundle_digest, expected_inner_digest);
    assert_eq!(
        identity.artifact_digest,
        <[u8; 32]>::from(Hash::new(&bytes))
    );
    assert_ne!(identity.artifact_digest, identity.inner_bundle_digest);
    let FastpqCommitmentDescriptionV1::OrderedCompactAir(commitments) = &identity.commitments
    else {
        panic!("compact result cannot claim preprocessing commitment")
    };
    assert_eq!(commitments.segment_count, 2);
    assert_eq!(
        commitments.segment_air_row_roots.as_slice(),
        result.bundle().row_roots()
    );
    let expected_roots = if is_axt {
        [
            "e48cd4d28087297e9c9be8ee2c423e266a3cf7770d8dd267271ea48ca5784e8a8ce3b73a95d601cbeec28fe9855f4adc",
            "56e667a70e2827cee948ae0a19e0d9452388b6df0b98096d96aa4604b7026b1e73774a07c18b63b19162852ba3ce2167",
        ]
    } else {
        [
            "fa1fa6d7f39702a0390b972b999719470650538e8d4959463fa9527e2a56de1a034ece9f8d97f4949fb2fb52262aafc7",
            "a309fb1f726e5574484cf642f4a73fc6379116597c6420006e6ce4b0665115712aed3dbe3eb902bdab5a27d5ff5b42e2",
        ]
    };
    // Independently parsed from the pinned complete proof frames, including lane six.
    for (root, expected) in commitments.segment_air_row_roots.iter().zip(expected_roots) {
        assert_eq!(hex::encode(root.to_le_bytes()), expected);
    }
    let described = norito::encode_canonical(identity).unwrap();
    assert_eq!(
        norito::decode_canonical::<FastpqArtifactIdentityDescriptionV1>(&described).unwrap(),
        *identity
    );
    assert_eq!(result.bundle().public_io(), expected);
    assert_eq!(result.bundle().segments(), 2);
    assert_eq!(result.bundle().work().air_evaluations, 750);
    assert_eq!(result.bundle().work().terminal_degree_checks, 2);
    let charged = usage.total_allocated_bytes();
    let total_elements = usage.total_elements();
    assert!(charged > result.bundle().wire_bytes());
    let mut exact = limits();
    exact.total_decode = DecodeLimits::new(
        20 * 1024 * 1024,
        20 * 1024 * 1024,
        total_elements,
        charged,
        32,
    );
    assert_eq!(verify(&bytes, exact).unwrap(), result);
    let mut low = exact;
    low.total_decode = DecodeLimits::new(
        20 * 1024 * 1024,
        20 * 1024 * 1024,
        total_elements,
        charged - 1,
        32,
    );
    assert!(verify(&bytes, low).is_err());
    low.total_decode = DecodeLimits::new(
        20 * 1024 * 1024,
        20 * 1024 * 1024,
        total_elements - 1,
        charged,
        32,
    );
    assert!(verify(&bytes, low).is_err());
    // A surrounding caller cannot have its stricter cumulative budget relaxed.
    assert!(
        norito::core::with_decode_limits_scope(low.total_decode, || verify(&bytes, limits()))
            .is_err()
    );
    let mut changed = bytes.clone();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    assert!(verify(&changed, limits()).is_err());
    let hash = hex::encode(Sha256::digest(&bytes));
    let path = directory.join(format!("compact-v1-{label}-model-artifact-{hash}.bin"));
    std::fs::write(&path, &bytes).unwrap();
    eprintln!(
        "candidate_model_artifact={label}; bytes={}; raw_verification={elapsed:?}; decode_allocation_charges={charged}; decode_elements={total_elements}; work={:?}; public_fixture_artifact={}; production_security_qualified=false",
        bytes.len(),
        result.bundle().work(),
        path.display()
    );
}

#[test]
#[ignore = "requires the complete ordinary candidate bundle fixture; verifies several full artifacts"]
fn complete_retained_ordinary_model_artifact_has_cumulative_limits() {
    complete_retained_artifact(false);
}

#[test]
#[ignore = "requires the complete AXT candidate bundle fixture; verifies several full artifacts"]
fn complete_retained_axt_model_artifact_has_cumulative_limits() {
    complete_retained_artifact(true);
}

#[test]
fn identity_statement_digest_is_canonical_and_bounded_before_output_allocation() {
    let fixture = Fixture::new(false);
    let (artifact, _) = ordinary(&fixture, Vec::new());
    let canonical = norito::encode_canonical(&artifact.statement).unwrap();
    let expected: [u8; 32] = Hash::new(&canonical).into();
    for flags in (u8::MIN..=u8::MAX).filter(|&f| norito::core::validate_header_flags(f).is_ok()) {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            statement_digest(&artifact.statement, canonical.len()).unwrap(),
            expected
        );
        assert!(matches!(
            statement_digest(&artifact.statement, canonical.len() - 1),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_identity_statement_bytes",
                ..
            })
        ));
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
    }
}

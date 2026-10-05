//! Internal typed public-input dispatch for canonical masked compact verification.
//!
//! Ordinary transfers and AXT transfers have separate entry points. The caller
//! supplies validated public transfer facts and independently expected `PublicIO`;
//! neither a proof nor an arbitrary AIR/schema/semantic selector can replace
//! these inputs. The AXT route additionally requires the complete typed context.
//!
//! Successful verification establishes the selected mathematical relation to
//! the caller's expected inputs. It does not authenticate execution authority,
//! source finality, permission membership, handle use or production qualification.
//! The caller authenticates execution context before using these typed routes.
//!
//! TODO: Complete independent protocol and resource qualification for the masked
//! producer and bounded verifier; mathematical acceptance is not caller authority.

use iroha_data_model::nexus::{AxtFastpqBinding, AxtRemoteSpendClaimV1};

#[cfg(test)]
use super::compact_value_domain::CompactTransferValue;
#[cfg(test)]
use super::{
    compact_protocol::FixedAir, deep_engine::VerificationWork, deep_relation::DeepRelation,
};
use crate::{
    Result, VerifyLimits,
    axt_binding::{AxtProofContextMirrors, AxtPublicMetadataBytes},
};

#[cfg(test)]
use super::{compact_axt_air::AxtTransferAir, compact_public_transfer::PublicTransferAir};
#[cfg(test)]
use crate::{
    Error, ProofSemantics, gadgets::public_transfer_statement::PreparedPublicTransfers,
    proof::PublicIO,
};

/// Child decoding policy for the sole normal compact proof dispatcher.
#[derive(Clone, Copy)]
pub(super) struct DeepVerifier {
    /// Maximum cumulative allocation charges for this complete child frame.
    pub(super) max_decode_allocation_charges: usize,
}

impl DeepVerifier {
    /// Test-only public facade over the same complete canonical verifier.
    #[cfg(test)]
    pub(super) fn verify_frame(
        self,
        relation: &impl DeepRelation,
        bytes: &[u8],
        limits: VerifyLimits,
    ) -> Result<VerificationWork> {
        Ok(self.verify_frame_committed(relation, bytes, limits)?.work())
    }

    /// Fixed query count; neither the carrier nor caller can choose a profile.
    #[allow(
        clippy::unused_self,
        reason = "bundle preflight asks the verifier it holds; policy never changes it"
    )]
    pub(super) const fn queries(self) -> usize {
        super::deep_geometry::QUERY_COUNT
    }

    /// Return the row commitment only after the same decoded proof passes all checks.
    pub(super) fn verify_frame_committed(
        self,
        relation: &impl super::deep_relation::DeepRelation,
        bytes: &[u8],
        limits: VerifyLimits,
    ) -> Result<super::deep_engine::VerifiedDeepProof> {
        super::deep_engine::verify_committed(
            relation,
            bytes,
            super::air::q77::VerifierLimits::for_segment(
                limits,
                self.max_decode_allocation_charges,
            ),
        )
    }
}

/// Complete borrowed AXT inputs supplied by the surrounding authenticated caller.
///
/// Possessing these bytes does not itself establish their authority. Canonical
/// encodings, exact mirrors and remote transfer linkage are checked by the
/// existing AXT relation constructor before decoding any proof frame.
#[derive(Clone, Copy)]
pub(super) struct AxtVerificationContext<'a> {
    /// Exact canonical outer AXT binding.
    pub(super) binding: &'a AxtFastpqBinding,
    /// Exact public metadata encodings from the surrounding AXT statement.
    pub(super) metadata: AxtPublicMetadataBytes<'a>,
    /// Outer pre-proof values that must match the public metadata.
    pub(super) mirrors: AxtProofContextMirrors,
    /// Complete remote-spend preimages when required by the binding.
    pub(super) remote_spend_claims: Option<&'a [AxtRemoteSpendClaimV1]>,
}

/// Checked relation inputs and verification work, without any authority grant.
///
/// Only successful typed facade paths construct this result. Its inputs were
/// supplied by the caller and exactly checked by the selected relation before
/// the bounded raw-byte verifier authenticated the complete canonical proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
pub(super) struct VerifiedPublicTransfer {
    public_io: PublicIO,
    work: VerificationWork,
}

#[cfg(test)]
impl VerifiedPublicTransfer {
    /// Return the caller-expected `PublicIO` checked by the selected relation.
    pub(super) const fn public_io(&self) -> PublicIO {
        self.public_io
    }

    /// Return measured bounded verification work, with no private trace replay.
    pub(super) const fn work(&self) -> VerificationWork {
        self.work
    }
}

/// Verify ordinary transfer bytes against exact caller-expected public inputs.
///
/// A prepared AXT or opaque profile is rejected even when its transfer rows
/// otherwise have the same shape. AXT context cannot be omitted via this route.
/// `limits` is an explicit test policy and never production qualification.
#[cfg(test)]
pub(super) fn verify_transfer<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    proof_bytes: &[u8],
    limits: VerifyLimits,
) -> Result<VerifiedPublicTransfer> {
    verify_transfer_with(
        prepared,
        expected,
        proof_bytes,
        limits,
        DeepVerifier {
            max_decode_allocation_charges: 32 * 1024 * 1024,
        },
    )
}

#[cfg(test)]
fn verify_transfer_with<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    proof_bytes: &[u8],
    limits: VerifyLimits,
    verifier: DeepVerifier,
) -> Result<VerifiedPublicTransfer> {
    preflight_inputs(prepared, proof_bytes, limits)?;
    require_profile(prepared.semantics(), ProofSemantics::StateTransition)?;
    let batch = super::compact_public_batch::PublicTransferBatch::new(
        prepared,
        expected,
        &[],
        super::compact_public_batch::BatchContextLimits::default(),
    )?;
    let relation = batch.segment(0)?;
    let work = verifier.verify_frame(&relation, proof_bytes, limits)?;
    Ok(VerifiedPublicTransfer {
        public_io: *expected,
        work,
    })
}

/// Verify AXT transfer bytes with the complete explicit caller-supplied context.
///
/// The shared AXT constructor performs all canonical binding, public-fact and
/// mirror checks. The returned result cannot authorize handles or source roots;
/// the surrounding caller retains those obligations and post-proof ABI checks.
#[cfg(test)]
#[allow(
    clippy::large_types_passed_by_value,
    reason = "keeps the by-value `Copy` context contract of its sibling-module callers"
)]
pub(super) fn verify_axt_transfer<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    context: AxtVerificationContext<'_>,
    proof_bytes: &[u8],
    limits: VerifyLimits,
) -> Result<VerifiedPublicTransfer> {
    verify_axt_transfer_with(
        prepared,
        expected,
        &context,
        proof_bytes,
        limits,
        DeepVerifier {
            max_decode_allocation_charges: 32 * 1024 * 1024,
        },
    )
}

#[cfg(test)]
fn verify_axt_transfer_with<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    context: &AxtVerificationContext<'_>,
    proof_bytes: &[u8],
    limits: VerifyLimits,
    verifier: DeepVerifier,
) -> Result<VerifiedPublicTransfer> {
    preflight_inputs(prepared, proof_bytes, limits)?;
    require_profile(prepared.semantics(), ProofSemantics::AxtTransferClaim)?;
    let batch = super::compact_axt_batch::AxtTransferBatch::new(
        prepared,
        expected,
        &[],
        *context,
        super::compact_public_batch::BatchContextLimits::default(),
    )?;
    let relation = batch.segment(0)?;
    let work = verifier.verify_frame(&relation, proof_bytes, limits)?;
    Ok(VerifiedPublicTransfer {
        public_io: *expected,
        work,
    })
}

/// Verify candidate ordinary bytes under exact expected public inputs and caller limits.
/// The prepared semantics cannot select AXT or omit its context through this path.
#[cfg(test)]
pub(super) fn verify_transfer_with_allocation<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    proof_bytes: &[u8],
    limits: VerifyLimits,
    max_decode_allocation_charges: usize,
) -> Result<VerifiedPublicTransfer> {
    verify_transfer_with(
        prepared,
        expected,
        proof_bytes,
        limits,
        DeepVerifier {
            max_decode_allocation_charges,
        },
    )
}

/// Verify candidate AXT bytes with every original binding, mirror and remote preimage.
/// Successful mathematical verification grants no source-state authority or finality.
#[cfg(test)]
#[allow(
    clippy::large_types_passed_by_value,
    reason = "keeps the by-value `Copy` context contract of its sibling-module callers"
)]
pub(super) fn verify_axt_transfer_with_allocation<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    context: AxtVerificationContext<'_>,
    proof_bytes: &[u8],
    limits: VerifyLimits,
    max_decode_allocation_charges: usize,
) -> Result<VerifiedPublicTransfer> {
    verify_axt_transfer_with(
        prepared,
        expected,
        &context,
        proof_bytes,
        limits,
        DeepVerifier {
            max_decode_allocation_charges,
        },
    )
}

#[cfg(test)]
fn preflight_inputs<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    bytes: &[u8],
    limits: VerifyLimits,
) -> Result<()> {
    // Only O(1) lengths and the immutable constructor's recorded byte count
    // are inspected here. Proof bytes take precedence over every other input.
    for (limit, actual, max) in [
        ("max_proof_bytes", bytes.len(), limits.max_proof_bytes),
        (
            "max_transitions",
            prepared.transitions().len(),
            limits.max_transitions,
        ),
        (
            "max_batch_bytes",
            prepared.work().public_bytes,
            limits.max_batch_bytes,
        ),
    ] {
        if actual > max {
            return Err(Error::VerifierLimitExceeded { limit, actual, max });
        }
    }
    Ok(())
}

#[cfg(test)]
fn require_profile(actual: ProofSemantics, required: ProofSemantics) -> Result<()> {
    if actual != required {
        return Err(Error::InvalidProofSemantics {
            profile: actual.name(),
            details: format!(
                "compact public facade requires the {} profile",
                required.name()
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    fn final_test_limits() -> crate::VerifyLimits {
        crate::VerifyLimits {
            max_queries: super::super::deep_geometry::QUERY_COUNT,
            ..crate::VerifyLimits::default()
        }
    }

    use super::*;
    use crate::backend::compact_axt_context::tests::Fixture;

    // Invalid bytes distinguish early public/resource errors from the eventual
    // raw Norito decode error without constructing another expensive full proof.
    const BAD_PROOF: &[u8] = &[0xff];

    fn context(fixture: &Fixture) -> AxtVerificationContext<'_> {
        AxtVerificationContext {
            binding: &fixture.binding,
            metadata: fixture.metadata(),
            mirrors: fixture.outer,
            remote_spend_claims: fixture.remote.as_deref(),
        }
    }

    #[test]
    fn proof_size_preflight_precedes_profile_context_and_raw_decoding() {
        let fixture = Fixture::new(false);
        let ordinary = fixture.prepare(ProofSemantics::StateTransition);
        let axt = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let limits = VerifyLimits {
            max_proof_bytes: 0,
            ..final_test_limits()
        };
        // Both deliberately use the wrong prepared profile; the byte ceiling
        // still rejects first without inspecting or constructing an AIR.
        for result in [
            verify_transfer(&axt, &fixture.expected(&axt), BAD_PROOF, limits),
            verify_axt_transfer(
                &ordinary,
                &fixture.expected(&ordinary),
                context(&fixture),
                BAD_PROOF,
                limits,
            ),
        ] {
            assert!(matches!(
                result,
                Err(Error::VerifierLimitExceeded {
                    limit: "max_proof_bytes",
                    actual: 1,
                    max: 0
                })
            ));
        }
        preflight_inputs(&ordinary, &[], limits).unwrap();
        preflight_inputs(
            &ordinary,
            BAD_PROOF,
            VerifyLimits {
                max_proof_bytes: 1,
                ..limits
            },
        )
        .unwrap();
        preflight_inputs(
            &ordinary,
            BAD_PROOF,
            VerifyLimits {
                max_proof_bytes: BAD_PROOF.len(),
                max_transitions: ordinary.transitions().len(),
                max_batch_bytes: ordinary.work().public_bytes,
                ..final_test_limits()
            },
        )
        .unwrap();
    }

    #[test]
    fn entry_points_enforce_exact_profiles_before_decoding() {
        let fixture = Fixture::new(false);
        let ordinary = fixture.prepare(ProofSemantics::StateTransition);
        let axt = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let limits = final_test_limits();
        assert!(matches!(
            verify_transfer(&axt, &fixture.expected(&axt), BAD_PROOF, limits),
            Err(Error::InvalidProofSemantics { .. })
        ));
        assert!(matches!(
            verify_axt_transfer(
                &ordinary,
                &fixture.expected(&ordinary),
                context(&fixture),
                BAD_PROOF,
                limits
            ),
            Err(Error::InvalidProofSemantics { .. })
        ));
        // The public preparation constructor already rejects opaque tables.
        // This gate remains explicit if that constructor gains other profiles.
        for required in [
            ProofSemantics::StateTransition,
            ProofSemantics::AxtTransferClaim,
        ] {
            for actual in [
                ProofSemantics::StateTransition,
                ProofSemantics::AxtTransferClaim,
                ProofSemantics::AxtOpaqueEffect,
            ] {
                assert_eq!(
                    require_profile(actual, required).is_ok(),
                    actual == required
                );
            }
        }
        assert!(matches!(
            verify_transfer(&ordinary, &fixture.expected(&ordinary), BAD_PROOF, limits),
            Err(Error::Encode(_))
        ));
        assert!(matches!(
            verify_axt_transfer(
                &axt,
                &fixture.expected(&axt),
                context(&fixture),
                BAD_PROOF,
                limits
            ),
            Err(Error::Encode(_))
        ));
    }

    #[test]
    fn both_routes_check_every_expected_public_io_field_before_decoding() {
        let fixture = Fixture::new(false);
        for semantics in [
            ProofSemantics::StateTransition,
            ProofSemantics::AxtTransferClaim,
        ] {
            let prepared = fixture.prepare(semantics);
            for field in 0..7 {
                let mut expected = fixture.expected(&prepared);
                match field {
                    0 => expected.dsid[15] ^= 1,
                    1 => expected.slot ^= 1,
                    2 => expected.old_root[0] ^= 1,
                    3 => expected.new_root[0] ^= 1,
                    4 => expected.perm_root[0] ^= 1,
                    5 => expected.tx_set_hash[0] ^= 1,
                    6 => expected.ordering_hash[0] ^= 1,
                    _ => unreachable!(),
                }
                let result = if semantics == ProofSemantics::StateTransition {
                    verify_transfer(&prepared, &expected, BAD_PROOF, final_test_limits())
                } else {
                    verify_axt_transfer(
                        &prepared,
                        &expected,
                        context(&fixture),
                        BAD_PROOF,
                        final_test_limits(),
                    )
                };
                assert!(
                    matches!(
                        result,
                        Err(Error::PublicIoMismatch {
                            field: "compact_public_io"
                        })
                    ),
                    "{semantics:?}/{field}"
                );
            }
        }
    }

    #[test]
    fn exact_axt_binding_metadata_and_remote_preimages_precede_decoding() {
        let fixture = Fixture::new(true);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&prepared);
        let limits = final_test_limits();
        let mut binding = fixture.binding.clone();
        binding.source_dataspace.push(' ');
        let mut changed = context(&fixture);
        changed.binding = &binding;
        assert!(matches!(
            verify_axt_transfer(&prepared, &expected, changed, BAD_PROOF, limits),
            Err(Error::InvalidAxtBinding { .. })
        ));
        for field in 0..5 {
            let mut changed = context(&fixture);
            match field {
                0 => changed.mirrors.dsid = iroha_model_base::topology::DataSpaceId::new(8),
                1 => changed.mirrors.manifest_root[0] ^= 1,
                2 => changed.mirrors.da_commitment = None,
                3 => changed.mirrors.committed_amount = None,
                4 => changed.mirrors.expiry_slot = None,
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    verify_axt_transfer(&prepared, &expected, changed, BAD_PROOF, limits),
                    Err(Error::InvalidAxtBinding { .. })
                ),
                "mirror {field}"
            );
        }
        let mut changed = context(&fixture);
        changed.metadata.entry_hash = &[0; 32];
        assert!(matches!(
            verify_axt_transfer(&prepared, &expected, changed, BAD_PROOF, limits),
            Err(Error::InvalidAxtBinding { .. })
        ));
        let mut changed = context(&fixture);
        changed.remote_spend_claims = None;
        assert!(matches!(
            verify_axt_transfer(&prepared, &expected, changed, BAD_PROOF, limits),
            Err(Error::MissingMetadata { .. })
        ));
        let mut changed = context(&fixture);
        changed.metadata.da_commitment = &[0; 32];
        assert!(matches!(
            verify_axt_transfer(&prepared, &expected, changed, BAD_PROOF, limits),
            Err(Error::MetadataLength { .. })
        ));
        let mut claims = fixture.remote.as_ref().unwrap().clone();
        claims[0].effective_amount = iroha_primitives::numeric::Quantity::from(34_u64);
        let mut changed = context(&fixture);
        changed.remote_spend_claims = Some(&claims);
        assert!(matches!(
            verify_axt_transfer(&prepared, &expected, changed, BAD_PROOF, limits),
            Err(Error::InvalidAxtBinding { .. })
        ));
    }

    #[test]
    fn decoder_resource_limits_are_retained_and_result_accessors_are_read_only() {
        let fixture = Fixture::new(false);
        let prepared = fixture.prepare(ProofSemantics::StateTransition);
        let expected = fixture.expected(&prepared);
        for (limits, name) in [
            (
                VerifyLimits {
                    max_transitions: 1,
                    ..final_test_limits()
                },
                "max_transitions",
            ),
            (
                VerifyLimits {
                    max_batch_bytes: 0,
                    ..final_test_limits()
                },
                "max_batch_bytes",
            ),
            (
                VerifyLimits {
                    max_air_row_values: super::super::compact_public_columns::COMMITTED_COLUMN_COUNT
                        - 1,
                    ..final_test_limits()
                },
                "max_air_row_values",
            ),
            (
                VerifyLimits {
                    max_queries: super::super::deep_geometry::QUERY_COUNT - 1,
                    ..final_test_limits()
                },
                "max_queries",
            ),
        ] {
            assert!(
                matches!(verify_transfer(&prepared, &expected, BAD_PROOF, limits), Err(Error::VerifierLimitExceeded { limit, .. }) if limit == name)
            );
        }
        // A private test value exercises only the Copy accessors; it is not a
        // successful facade verification and cannot be constructed by callers.
        let work = VerificationWork {
            proof_bytes: 123,
            air_evaluations: 1,
            leaf_hashes: 0,
            parent_hashes: 0,
            h_calls: 0,
            verifier_messages: 10,
            g_tape_bytes: 0,
            fold_checks: 0,
            terminal_values: 128,
        };
        let result = VerifiedPublicTransfer {
            public_io: expected,
            work,
        };
        assert_eq!(result.public_io(), expected);
        assert_eq!(result.work(), work);
        let mut copied = result.public_io();
        copied.slot ^= 1;
        assert_ne!(copied, result.public_io());
    }

    #[test]
    #[ignore = "explicit full AXT 65536x342 proving and raw facade verification diagnostic"]
    fn complete_axt_transfer_verifies_after_private_witnesses_are_dropped() {
        use sha2::{Digest as _, Sha256};

        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let mut fixture = Fixture::new(true);
        assert_eq!(fixture.remote.as_ref().unwrap().len(), 1);
        assert_eq!(fixture.binding.remote_spend_intent_commitments.len(), 1);
        let construction_started = std::time::Instant::now();
        let columns = private_axt_prover_columns(&mut fixture);
        let construction = construction_started.elapsed();
        let limits = super::super::deep_fixture::verification_limits();
        let (encoded, proving) = prove_canonical_axt_frame(&fixture, columns);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&prepared);
        assert!(encoded.len() <= final_test_limits().max_proof_bytes);
        assert!(encoded.len() <= super::super::deep_proof::PROOF_BYTE_TARGET);
        assert!(encoded.len() <= limits.max_proof_bytes);
        assert!(matches!(
            verify_axt_transfer(
                &prepared,
                &expected,
                context(&fixture),
                &encoded,
                VerifyLimits {
                    max_proof_bytes: encoded.len() - 1,
                    ..limits
                },
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                ..
            })
        ));
        let verifying_started = std::time::Instant::now();
        let verified = {
            let flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let verified =
                verify_axt_transfer(&prepared, &expected, context(&fixture), &encoded, limits)
                    .unwrap();
            assert_eq!(norito::core::get_decode_flags(), flags);
            verified
        };
        let verifying = verifying_started.elapsed();
        let work = assert_verified_axt_work(&verified, expected, encoded.len());

        // All negative controls reuse this public frame and run after private
        // proving material has gone out of scope.
        assert_axt_negative_controls(&fixture, &prepared, expected, &encoded, limits);

        // Retain only this deterministic public-fixture proof, keyed by exact
        // bytes. No private-input files, signing material or live state are retained.
        let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/fastpq-production-validation");
        std::fs::create_dir_all(&artifact_dir).unwrap();
        let artifact = artifact_dir.join(format!(
            "compact-q77-axt-transfer-{}.bin",
            hex::encode(Sha256::digest(&encoded))
        ));
        std::fs::write(&artifact, &encoded).unwrap();
        eprintln!(
            "compact_axt_construction={construction:?}; proving={proving:?}; raw_facade_verifying={verifying:?}; work={work:?}; diagnostic_limit={}; production_security_qualified=false; public_fixture_artifact={}",
            limits.max_proof_bytes,
            artifact.display()
        );
    }

    /// Attach genuine SMT paths to the remote AXT fixture and build its prover columns.
    ///
    /// Every private path, witness and witness-bearing transcript is scoped
    /// here. Only exact prover columns leave this function.
    fn private_axt_prover_columns(
        fixture: &mut Fixture,
    ) -> super::super::deep_trace_source::OwnedTraceSource {
        use crate::gadgets::{
            compact_smt_air::{PATH_LEVELS, SmtWitness},
            public_transfer_statement::{PublicTransferLimits, public_claims_from_transcripts},
            transfer::attach_transfer_smt_witnesses,
        };
        use iroha_data_model::fastpq::{
            TransferDeltaTranscript, TransferSmtWitness, TransferTranscript,
        };

        let mut transcripts = {
            let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
            assert_eq!(prepared.claims().len(), 1);
            let claim = &prepared.claims()[0];
            assert_eq!(claim.deltas.len(), 1);
            vec![TransferTranscript {
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
            }]
        };
        let (old_root, new_root) = attach_transfer_smt_witnesses(&mut transcripts).unwrap();
        assert_ne!(old_root, new_root);
        fixture.set_touched_roots(old_root, new_root);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        // Attaching paths must preserve all original public quantities,
        // authority/batch identities and the Poseidon preimage digest.
        assert_eq!(
            public_claims_from_transcripts(&transcripts, PublicTransferLimits::default())
                .unwrap()
                .as_slice(),
            prepared.claims()
        );
        let expected = fixture.expected(&prepared);
        assert_eq!(expected.old_root, old_root);
        assert_eq!(expected.new_root, new_root);
        let statements = prepared.compact_statements(&[]).unwrap();
        assert_eq!(statements.len(), 1);
        let delta = &transcripts[0].deltas[0];
        let paths = [&delta.from_smt_witness, &delta.to_smt_witness];
        for (path, update) in paths.iter().zip(statements[0].updates) {
            assert_eq!(path.path_bits.as_slice(), update.path.to_le_bytes());
            assert_eq!(path.siblings.len(), PATH_LEVELS);
        }
        let siblings = core::array::from_fn(|update| {
            core::array::from_fn(|level| {
                let bytes = paths[update].siblings[level];
                core::array::from_fn(|limb| {
                    u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
                })
            })
        });
        let witness = SmtWitness::from_inputs(&statements[0], &siblings)
            .unwrap()
            .into_physical();
        super::super::deep_trace_source::OwnedTraceSource::from_rows(witness.rows()).unwrap()
    }

    /// Construct the exact AXT frame; consumed private source drops before return.
    fn prove_canonical_axt_frame(
        fixture: &Fixture,
        source: super::super::deep_trace_source::OwnedTraceSource,
    ) -> (Vec<u8>, std::time::Duration) {
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&prepared);
        let air = AxtTransferAir::new(
            &prepared,
            &expected,
            &fixture.binding,
            fixture.metadata(),
            fixture.outer,
            fixture.remote.as_deref(),
        )
        .unwrap();
        assert_eq!(
            air.schema().identity,
            "fastpq:compact:v1:axt-public-transfer:v1:342cols:923slots:65536rows"
        );
        assert_ne!(
            air.schema().identity,
            PublicTransferAir::new(&prepared, &expected)
                .unwrap()
                .schema()
                .identity
        );
        assert_eq!(
            air.statement_bytes(),
            crate::backend::compact_axt_context::encode_context(
                &prepared,
                &expected,
                &fixture.binding,
                fixture.metadata(),
                fixture.outer,
                fixture.remote.as_deref(),
            )
            .unwrap()
        );
        let batch = super::super::compact_axt_batch::AxtTransferBatch::new(
            &prepared,
            &expected,
            &[],
            context(fixture),
            super::super::compact_public_batch::BatchContextLimits::default(),
        )
        .unwrap();
        let relation = batch.segment(0).unwrap();
        let started = std::time::Instant::now();
        let encoded = super::super::deep_fixture::prove(&relation, source, 0x077_a17).unwrap();
        let proof =
            super::super::deep_proof::decode(&encoded, super::super::deep_proof::PROOF_BYTE_TARGET)
                .unwrap();
        assert_eq!(
            encoded.len(),
            norito::core::encoded_frame_len(&proof).unwrap()
        );
        assert_eq!(norito::encode_canonical(&proof).unwrap(), encoded);
        (encoded, started.elapsed())
    }

    /// Check the verified public result and its measured bounded opening work.
    fn assert_verified_axt_work(
        verified: &VerifiedPublicTransfer,
        expected: PublicIO,
        proof_bytes: usize,
    ) -> VerificationWork {
        use super::super::deep_geometry::{FRI_ARITIES, FRI_LENGTHS, LDE_ROWS, QUERY_COUNT};
        let work = verified.work();
        assert_eq!(
            DeepVerifier {
                max_decode_allocation_charges: 32 * 1024 * 1024
            }
            .queries(),
            QUERY_COUNT
        );
        assert_eq!(verified.public_io(), expected);
        assert_eq!(work.proof_bytes, proof_bytes);
        assert_eq!(work.air_evaluations, 1);
        assert_eq!(work.verifier_messages, 10);
        assert_eq!(work.terminal_values, 128);
        let mut leaf_bound = 2 * QUERY_COUNT + 1;
        let mut parent_bound = 2 * QUERY_COUNT * LDE_ROWS.ilog2() as usize + 1;
        for (&length, &arity) in FRI_LENGTHS.iter().zip(FRI_ARITIES.iter()) {
            let leaves = length / arity;
            let groups = QUERY_COUNT.min(leaves);
            leaf_bound += groups;
            parent_bound += groups * (leaves.ilog2() as usize).max(1);
        }
        assert!((2 * QUERY_COUNT + 1..=leaf_bound).contains(&work.leaf_hashes));
        assert!((1..=parent_bound).contains(&work.parent_hashes));
        assert!(work.fold_checks >= QUERY_COUNT);

        work
    }

    /// Reject changed public inputs, context, profile and framing for one valid frame.
    fn assert_axt_negative_controls(
        fixture: &Fixture,
        prepared: &PreparedPublicTransfers<'_>,
        expected: PublicIO,
        encoded: &[u8],
        limits: VerifyLimits,
    ) {
        let mut changed_expected = expected;
        changed_expected.slot ^= 1;
        assert!(matches!(
            verify_axt_transfer(
                prepared,
                &changed_expected,
                context(fixture),
                encoded,
                limits
            ),
            Err(Error::PublicIoMismatch {
                field: "compact_public_io"
            })
        ));
        let mut changed_context = context(fixture);
        changed_context.mirrors.manifest_root[0] ^= 1;
        assert!(matches!(
            verify_axt_transfer(prepared, &expected, changed_context, encoded, limits),
            Err(Error::InvalidAxtBinding { .. })
        ));
        let mut changed_context = context(fixture);
        changed_context.remote_spend_claims = None;
        assert!(matches!(
            verify_axt_transfer(prepared, &expected, changed_context, encoded, limits),
            Err(Error::MissingMetadata { .. })
        ));
        let mut binding = fixture.binding.clone();
        binding.source_receipt_id.push_str("-different");
        let mut changed_context = context(fixture);
        changed_context.binding = &binding;
        // This context is independently well formed, so failure must come from
        // binding the complete proof transcript to the original public context.
        AxtTransferAir::new(
            prepared,
            &expected,
            &binding,
            fixture.metadata(),
            fixture.outer,
            fixture.remote.as_deref(),
        )
        .unwrap();
        assert!(
            verify_axt_transfer(prepared, &expected, changed_context, encoded, limits).is_err()
        );
        let mut changed_fixture = fixture.clone();
        let mut changed_root = expected.new_root;
        changed_root[0] ^= 1;
        changed_fixture.set_touched_roots(expected.old_root, changed_root);
        let changed = changed_fixture.prepare(ProofSemantics::AxtTransferClaim);
        let changed_io = changed_fixture.expected(&changed);
        AxtTransferAir::new(
            &changed,
            &changed_io,
            &changed_fixture.binding,
            changed_fixture.metadata(),
            changed_fixture.outer,
            changed_fixture.remote.as_deref(),
        )
        .unwrap();
        assert!(
            verify_axt_transfer(
                &changed,
                &changed_io,
                context(&changed_fixture),
                encoded,
                limits,
            )
            .is_err()
        );
        assert!(matches!(
            verify_transfer(prepared, &expected, encoded, limits),
            Err(Error::InvalidProofSemantics { .. })
        ));
        let ordinary = fixture.prepare(ProofSemantics::StateTransition);
        let ordinary_io = fixture.expected(&ordinary);
        assert!(matches!(
            verify_axt_transfer(&ordinary, &ordinary_io, context(fixture), encoded, limits),
            Err(Error::InvalidProofSemantics { .. })
        ));
        assert!(verify_transfer(&ordinary, &ordinary_io, encoded, limits).is_err());
        let mut trailing = encoded.to_vec();
        trailing.push(0);
        assert!(matches!(
            verify_axt_transfer(prepared, &expected, context(fixture), &trailing, limits),
            Err(Error::Encode(_))
        ));
        drop(trailing);
    }

    fn final_limits() -> VerifyLimits {
        VerifyLimits {
            max_proof_bytes: super::super::deep_proof::MAX_FRAME_BYTES,
            max_queries: super::super::deep_geometry::QUERY_COUNT,
            ..final_test_limits()
        }
    }

    /// Final limits with exactly one public-resource ceiling exceeded, and its name.
    fn exceeded_final_limits() -> [(VerifyLimits, &'static str); 4] {
        [
            (
                VerifyLimits {
                    max_transitions: 1,
                    ..final_limits()
                },
                "max_transitions",
            ),
            (
                VerifyLimits {
                    max_batch_bytes: 0,
                    ..final_limits()
                },
                "max_batch_bytes",
            ),
            (
                VerifyLimits {
                    max_air_row_values: super::super::compact_public_columns::COMMITTED_COLUMN_COUNT
                        - 1,
                    ..final_limits()
                },
                "max_air_row_values",
            ),
            (
                VerifyLimits {
                    max_queries: super::super::deep_geometry::QUERY_COUNT - 1,
                    ..final_limits()
                },
                "max_queries",
            ),
        ]
    }

    #[test]
    fn candidate_facades_preserve_resource_and_semantic_preflights() {
        let fixture = Fixture::new(false);
        let ordinary = fixture.prepare(ProofSemantics::StateTransition);
        let axt = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let zero = VerifyLimits {
            max_proof_bytes: 0,
            ..final_limits()
        };
        for result in [
            verify_transfer_with_allocation(&axt, &fixture.expected(&axt), BAD_PROOF, zero, 0),
            verify_axt_transfer_with_allocation(
                &ordinary,
                &fixture.expected(&ordinary),
                context(&fixture),
                BAD_PROOF,
                zero,
                0,
            ),
        ] {
            assert!(matches!(
                result,
                Err(Error::VerifierLimitExceeded {
                    limit: "max_proof_bytes",
                    actual: 1,
                    max: 0
                })
            ));
        }
        assert!(matches!(
            verify_transfer_with_allocation(
                &axt,
                &fixture.expected(&axt),
                BAD_PROOF,
                final_limits(),
                0
            ),
            Err(Error::InvalidProofSemantics { .. })
        ));
        assert!(matches!(
            verify_axt_transfer_with_allocation(
                &ordinary,
                &fixture.expected(&ordinary),
                context(&fixture),
                BAD_PROOF,
                final_limits(),
                0
            ),
            Err(Error::InvalidProofSemantics { .. })
        ));
        for semantics in [
            ProofSemantics::StateTransition,
            ProofSemantics::AxtTransferClaim,
        ] {
            let prepared = fixture.prepare(semantics);
            for (limits, required_error) in exceeded_final_limits() {
                let result = if semantics == ProofSemantics::StateTransition {
                    verify_transfer_with_allocation(
                        &prepared,
                        &fixture.expected(&prepared),
                        BAD_PROOF,
                        limits,
                        usize::MAX,
                    )
                } else {
                    verify_axt_transfer_with_allocation(
                        &prepared,
                        &fixture.expected(&prepared),
                        context(&fixture),
                        BAD_PROOF,
                        limits,
                        usize::MAX,
                    )
                };
                assert!(
                    matches!(result, Err(Error::VerifierLimitExceeded { limit, .. }) if limit == required_error),
                    "{semantics:?}/{required_error}"
                );
            }
        }
        for max_decode_allocation_charges in [0, 1, 32 * 1024 * 1024, usize::MAX] {
            assert_eq!(
                DeepVerifier {
                    max_decode_allocation_charges
                }
                .queries(),
                super::super::deep_geometry::QUERY_COUNT
            );
        }
    }

    #[test]
    fn candidate_facades_check_all_expected_fields_before_raw_decoding() {
        let fixture = Fixture::new(false);
        for semantics in [
            ProofSemantics::StateTransition,
            ProofSemantics::AxtTransferClaim,
        ] {
            let prepared = fixture.prepare(semantics);
            for field in 0..7 {
                let mut expected = fixture.expected(&prepared);
                match field {
                    0 => expected.dsid[15] ^= 1,
                    1 => expected.slot ^= 1,
                    2 => expected.old_root[0] ^= 1,
                    3 => expected.new_root[0] ^= 1,
                    4 => expected.perm_root[0] ^= 1,
                    5 => expected.tx_set_hash[0] ^= 1,
                    6 => expected.ordering_hash[0] ^= 1,
                    _ => unreachable!(),
                }
                let result = if semantics == ProofSemantics::StateTransition {
                    verify_transfer_with_allocation(
                        &prepared,
                        &expected,
                        BAD_PROOF,
                        final_limits(),
                        64 * 1024 * 1024,
                    )
                } else {
                    verify_axt_transfer_with_allocation(
                        &prepared,
                        &expected,
                        context(&fixture),
                        BAD_PROOF,
                        final_limits(),
                        64 * 1024 * 1024,
                    )
                };
                assert!(
                    matches!(
                        result,
                        Err(Error::PublicIoMismatch {
                            field: "compact_public_io"
                        })
                    ),
                    "{semantics:?}/{field}"
                );
            }
            let result = if semantics == ProofSemantics::StateTransition {
                verify_transfer_with_allocation(
                    &prepared,
                    &fixture.expected(&prepared),
                    BAD_PROOF,
                    final_limits(),
                    64 * 1024 * 1024,
                )
            } else {
                verify_axt_transfer_with_allocation(
                    &prepared,
                    &fixture.expected(&prepared),
                    context(&fixture),
                    BAD_PROOF,
                    final_limits(),
                    64 * 1024 * 1024,
                )
            };
            assert!(matches!(result, Err(Error::Encode(_))));
        }
    }

    #[test]
    fn candidate_axt_facade_requires_exact_binding_mirrors_and_remote_preimages() {
        let fixture = Fixture::new(true);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&prepared);
        let mut binding = fixture.binding.clone();
        binding.source_dataspace.push(' ');
        let mut bad = context(&fixture);
        bad.binding = &binding;
        assert!(matches!(
            verify_axt_transfer_with_allocation(
                &prepared,
                &expected,
                bad,
                BAD_PROOF,
                final_limits(),
                usize::MAX
            ),
            Err(Error::InvalidAxtBinding { .. })
        ));
        for field in 0..5 {
            let mut bad = context(&fixture);
            match field {
                0 => bad.mirrors.dsid = iroha_model_base::topology::DataSpaceId::new(8),
                1 => bad.mirrors.manifest_root[0] ^= 1,
                2 => bad.mirrors.da_commitment = None,
                3 => bad.mirrors.committed_amount = None,
                4 => bad.mirrors.expiry_slot = None,
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    verify_axt_transfer_with_allocation(
                        &prepared,
                        &expected,
                        bad,
                        BAD_PROOF,
                        final_limits(),
                        usize::MAX
                    ),
                    Err(Error::InvalidAxtBinding { .. })
                ),
                "mirror {field}"
            );
        }
        let mut bad = context(&fixture);
        bad.remote_spend_claims = None;
        assert!(matches!(
            verify_axt_transfer_with_allocation(
                &prepared,
                &expected,
                bad,
                BAD_PROOF,
                final_limits(),
                usize::MAX
            ),
            Err(Error::MissingMetadata { .. })
        ));
        let mut claims = fixture.remote.as_ref().unwrap().clone();
        claims[0].effective_amount = iroha_primitives::numeric::Quantity::from(34_u64);
        let mut bad = context(&fixture);
        bad.remote_spend_claims = Some(&claims);
        assert!(matches!(
            verify_axt_transfer_with_allocation(
                &prepared,
                &expected,
                bad,
                BAD_PROOF,
                final_limits(),
                usize::MAX
            ),
            Err(Error::InvalidAxtBinding { .. })
        ));
    }
}

#[cfg(test)]
#[path = "compact_public_diagnostic.rs"]
mod compact_diagnostic;

//! Typed public-input facade for test-only shared compact proof verification.
//!
//! Ordinary transfers and AXT transfers have separate entry points. The caller
//! supplies validated public transfer facts and independently expected PublicIO;
//! neither a proof nor an arbitrary AIR/schema/semantic selector can replace
//! these inputs. The AXT route additionally requires the complete typed context.
//!
//! Successful verification establishes the selected mathematical relation to
//! the caller's expected inputs. It does not authenticate execution authority,
//! source finality, permission membership, handle use or production qualification.
//! Existing production APIs continue to require replay and their default limits.
//!
//! TODO: Complete independent protocol qualification and an authenticated core
//! context before exposing this facade outside tests. Proving integration must
//! separate public claims from private witness material and retain distinct wire
//! identity; it must not silently replace legacy `Proof` or AXT payload bytes.

use iroha_data_model::nexus::{AxtFastpqBinding, AxtRemoteSpendClaimV1};

use super::{
    compact_axt_air::AxtTransferAir,
    compact_protocol::{
        FixedAir,
        shared_openings::{
            SharedVerificationWork,
            codec::{
                VerifiedSharedProof, decode_and_verify_committed, decode_and_verify_shake_committed,
            },
        },
    },
    compact_public_transfer::PublicTransferAir,
};
use crate::{
    Error, ProofSemantics, Result, VerifyLimits,
    axt_binding::{AxtProofContextMirrors, AxtPublicMetadataBytes},
    gadgets::public_transfer_statement::PreparedPublicTransfers,
    proof::PublicIO,
};

/// Fixed verifier selected by a typed entry point, never read from proof bytes.
/// The explicit allocation charge is caller diagnostic policy, not qualification.
#[derive(Clone, Copy)]
pub(super) enum SharedVerifier {
    /// Original diagnostic transcript and bounded codec.
    Prototype,
    /// Fixed 375-query whole-tape candidate with an explicit child decode cap.
    ShakeCandidate {
        max_decode_allocation_charges: usize,
    },
}

impl SharedVerifier {
    /// Fixed initial query count for one complete segment.
    pub(super) fn queries(self) -> usize {
        match self {
            Self::Prototype => fastpq_isi::FASTPQ_FINAL_V1.fri.queries as usize,
            Self::ShakeCandidate { .. } => 375,
        }
    }

    /// Whether the selected entry point requires the distinct candidate frame.
    pub(super) const fn is_shake(self) -> bool {
        matches!(self, Self::ShakeCandidate { .. })
    }

    /// Authenticate a frame using the selected transcript and unchanged relation.
    pub(super) fn verify_frame(
        self,
        relation: &impl FixedAir,
        bytes: &[u8],
        limits: VerifyLimits,
    ) -> Result<SharedVerificationWork> {
        Ok(self.verify_frame_committed(relation, bytes, limits)?.work())
    }

    /// Return a full row commitment only with complete successful verification.
    /// No callback can observe successful prefixes of a rejected bundle.
    pub(super) fn verify_frame_committed(
        self,
        relation: &impl FixedAir,
        bytes: &[u8],
        limits: VerifyLimits,
    ) -> Result<VerifiedSharedProof> {
        match self {
            Self::Prototype => decode_and_verify_committed(relation, bytes, limits),
            Self::ShakeCandidate {
                max_decode_allocation_charges,
            } => decode_and_verify_shake_committed(
                relation,
                bytes,
                limits,
                max_decode_allocation_charges,
            ),
        }
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
/// the bounded raw-byte verifier authenticated the complete shared proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VerifiedPublicTransfer {
    public_io: PublicIO,
    work: SharedVerificationWork,
}

impl VerifiedPublicTransfer {
    /// Return the caller-expected PublicIO checked by the selected relation.
    pub(super) const fn public_io(&self) -> PublicIO {
        self.public_io
    }

    /// Return measured bounded verification work, with no private trace replay.
    pub(super) const fn work(&self) -> SharedVerificationWork {
        self.work
    }
}

/// Verify ordinary transfer bytes against exact caller-expected public inputs.
///
/// A prepared AXT or opaque profile is rejected even when its transfer rows
/// otherwise have the same shape. AXT context cannot be omitted via this route.
/// `limits` is an explicit test policy and never production qualification.
pub(super) fn verify_transfer(
    prepared: &PreparedPublicTransfers<'_>,
    expected: &PublicIO,
    proof_bytes: &[u8],
    limits: VerifyLimits,
) -> Result<VerifiedPublicTransfer> {
    verify_transfer_with(
        prepared,
        expected,
        proof_bytes,
        limits,
        SharedVerifier::Prototype,
    )
}

fn verify_transfer_with(
    prepared: &PreparedPublicTransfers<'_>,
    expected: &PublicIO,
    proof_bytes: &[u8],
    limits: VerifyLimits,
    verifier: SharedVerifier,
) -> Result<VerifiedPublicTransfer> {
    preflight_inputs(prepared, proof_bytes, limits)?;
    require_profile(
        prepared.semantics(),
        ProofSemantics::TransferStateTransition,
    )?;
    let relation = PublicTransferAir::new(prepared, expected)?;
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
pub(super) fn verify_axt_transfer(
    prepared: &PreparedPublicTransfers<'_>,
    expected: &PublicIO,
    context: AxtVerificationContext<'_>,
    proof_bytes: &[u8],
    limits: VerifyLimits,
) -> Result<VerifiedPublicTransfer> {
    verify_axt_transfer_with(
        prepared,
        expected,
        context,
        proof_bytes,
        limits,
        SharedVerifier::Prototype,
    )
}

fn verify_axt_transfer_with(
    prepared: &PreparedPublicTransfers<'_>,
    expected: &PublicIO,
    context: AxtVerificationContext<'_>,
    proof_bytes: &[u8],
    limits: VerifyLimits,
    verifier: SharedVerifier,
) -> Result<VerifiedPublicTransfer> {
    preflight_inputs(prepared, proof_bytes, limits)?;
    require_profile(prepared.semantics(), ProofSemantics::AxtTransferClaim)?;
    let relation = AxtTransferAir::new(
        prepared,
        expected,
        context.binding,
        context.metadata,
        context.mirrors,
        context.remote_spend_claims,
    )?;
    let work = verifier.verify_frame(&relation, proof_bytes, limits)?;
    Ok(VerifiedPublicTransfer {
        public_io: *expected,
        work,
    })
}

/// Verify candidate ordinary bytes under exact expected public inputs and caller limits.
/// The prepared semantics cannot select AXT or omit its context through this path.
pub(super) fn verify_shake_transfer(
    prepared: &PreparedPublicTransfers<'_>,
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
        SharedVerifier::ShakeCandidate {
            max_decode_allocation_charges,
        },
    )
}

/// Verify candidate AXT bytes with every original binding, mirror and remote preimage.
/// Successful mathematical verification grants no source-state authority or finality.
pub(super) fn verify_shake_axt_transfer(
    prepared: &PreparedPublicTransfers<'_>,
    expected: &PublicIO,
    context: AxtVerificationContext<'_>,
    proof_bytes: &[u8],
    limits: VerifyLimits,
    max_decode_allocation_charges: usize,
) -> Result<VerifiedPublicTransfer> {
    verify_axt_transfer_with(
        prepared,
        expected,
        context,
        proof_bytes,
        limits,
        SharedVerifier::ShakeCandidate {
            max_decode_allocation_charges,
        },
    )
}

fn preflight_inputs(
    prepared: &PreparedPublicTransfers<'_>,
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
        let ordinary = fixture.prepare(ProofSemantics::TransferStateTransition);
        let axt = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let limits = VerifyLimits {
            max_proof_bytes: 0,
            ..VerifyLimits::default()
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
                ..VerifyLimits::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn entry_points_enforce_exact_profiles_before_decoding() {
        let fixture = Fixture::new(false);
        let ordinary = fixture.prepare(ProofSemantics::TransferStateTransition);
        let axt = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let limits = VerifyLimits::default();
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
            ProofSemantics::TransferStateTransition,
            ProofSemantics::AxtTransferClaim,
        ] {
            for actual in [
                ProofSemantics::TransferStateTransition,
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
            ProofSemantics::TransferStateTransition,
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
                let result = if semantics == ProofSemantics::TransferStateTransition {
                    verify_transfer(&prepared, &expected, BAD_PROOF, VerifyLimits::default())
                } else {
                    verify_axt_transfer(
                        &prepared,
                        &expected,
                        context(&fixture),
                        BAD_PROOF,
                        VerifyLimits::default(),
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
        let limits = VerifyLimits::default();
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
                0 => changed.mirrors.dsid = iroha_data_model::DataSpaceId::new(8),
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
        let prepared = fixture.prepare(ProofSemantics::TransferStateTransition);
        let expected = fixture.expected(&prepared);
        for (limits, name) in [
            (
                VerifyLimits {
                    max_transitions: 1,
                    ..VerifyLimits::default()
                },
                "max_transitions",
            ),
            (
                VerifyLimits {
                    max_batch_bytes: 0,
                    ..VerifyLimits::default()
                },
                "max_batch_bytes",
            ),
            (
                VerifyLimits {
                    max_air_row_values: 341,
                    ..VerifyLimits::default()
                },
                "max_air_row_values",
            ),
            (
                VerifyLimits {
                    max_queries: 135,
                    ..VerifyLimits::default()
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
        let work = SharedVerificationWork {
            proof_bytes: 123,
            air_evaluations: 136,
            ..SharedVerificationWork::default()
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
        use crate::{
            backend::{
                compact_axt_context::encode_context,
                compact_protocol::{self, FixedAir, shared_openings},
            },
            gadgets::{
                compact_smt_air::{COLUMN_COUNT, PATH_LEVELS, PHYSICAL_ROW_COUNT, SmtWitness},
                compact_trace_columns::smt_row_cells,
                public_transfer_statement::{PublicTransferLimits, public_claims_from_transcripts},
                transfer::attach_transfer_smt_witnesses,
            },
        };
        use iroha_data_model::fastpq::{
            TransferDeltaTranscript, TransferSmtWitness, TransferTranscript,
        };
        use sha2::{Digest as _, Sha256};

        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let mut fixture = Fixture::new(true);
        assert_eq!(fixture.remote.as_ref().unwrap().len(), 1);
        assert_eq!(fixture.binding.remote_spend_intent_commitments.len(), 1);
        let construction_started = std::time::Instant::now();
        let columns = {
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
            let mut columns = (0..COLUMN_COUNT)
                .map(|_| Vec::with_capacity(PHYSICAL_ROW_COUNT))
                .collect::<Vec<_>>();
            for row in witness.rows() {
                for (column, value) in columns.iter_mut().zip(smt_row_cells(row)) {
                    column.push(value);
                }
            }
            // Every private path, witness and witness-bearing transcript is
            // scoped here. Only exact prover columns leave this block.
            columns
        };
        let construction = construction_started.elapsed();
        let limits = VerifyLimits {
            max_proof_bytes: 4 * 1024 * 1024,
            ..VerifyLimits::default()
        };
        let (encoded, typed_bytes, proving, conversion) = {
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
                "fastpq:prototype:axt-public-transfer:v1:342cols:923slots:65536rows"
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
                encode_context(
                    &prepared,
                    &expected,
                    &fixture.binding,
                    fixture.metadata(),
                    fixture.outer,
                    fixture.remote.as_deref(),
                )
                .unwrap()
            );
            let proving_started = std::time::Instant::now();
            let proof = compact_protocol::prove(&air, &columns).unwrap();
            let proving = proving_started.elapsed();
            drop(columns);
            let typed_bytes = norito::core::encoded_frame_len(&proof).unwrap();
            let conversion_started = std::time::Instant::now();
            let shared = shared_openings::from_compact(&air, &proof, limits).unwrap();
            let encoded = norito::core::to_bytes(&shared).unwrap();
            assert_eq!(
                encoded.len(),
                norito::core::encoded_frame_len(&shared).unwrap()
            );
            let conversion = conversion_started.elapsed();
            // Proving AIR, typed proof, shared DTO and public preparation also
            // leave scope. The facade receives only original public facts and
            // one canonical raw frame, never a retained prover/verifier object.
            (encoded, typed_bytes, proving, conversion)
        };
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&prepared);
        assert!(encoded.len() > VerifyLimits::default().max_proof_bytes);
        assert!(encoded.len() < typed_bytes);
        assert!(encoded.len() <= limits.max_proof_bytes);
        assert!(matches!(
            verify_axt_transfer(
                &prepared,
                &expected,
                context(&fixture),
                &encoded,
                VerifyLimits::default(),
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
        let work = verified.work();
        let queries = fastpq_isi::FASTPQ_FINAL_V1.fri.queries as usize;
        assert_eq!(queries, 136);
        assert_eq!(verified.public_io(), expected);
        assert_eq!(work.proof_bytes, encoded.len());
        assert_eq!(work.transcripts, 1);
        assert_eq!(work.air_evaluations, queries);
        assert!((queries..=2 * queries).contains(&work.row_leaves));
        assert_eq!(work.oracle_leaves, 2 * queries);
        assert_eq!(work.terminal_degree_checks, 1);
        let lde_rows = PHYSICAL_ROW_COUNT * fastpq_isi::FASTPQ_FINAL_V1.fri.blowup_factor as usize;
        let mut length = lde_rows;
        let mut fri_leaf_bound = 1; // One complete terminal leaf.
        let mut parent_bound =
            (work.row_leaves + work.oracle_leaves) * lde_rows.ilog2() as usize + 1;
        while length > fastpq_isi::FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1 as usize {
            let leaves = length / 2;
            let groups = queries.min(leaves);
            fri_leaf_bound += groups;
            parent_bound += groups * (leaves.ilog2() as usize).max(1);
            length /= 2;
        }
        assert!((1..=fri_leaf_bound).contains(&work.fri_leaves));
        assert!((1..=parent_bound).contains(&work.parent_hashes));

        // All negative controls reuse this public frame and run after private
        // proving material has gone out of scope.
        let mut changed_expected = expected;
        changed_expected.slot ^= 1;
        assert!(matches!(
            verify_axt_transfer(
                &prepared,
                &changed_expected,
                context(&fixture),
                &encoded,
                limits
            ),
            Err(Error::PublicIoMismatch {
                field: "compact_public_io"
            })
        ));
        let mut changed_context = context(&fixture);
        changed_context.mirrors.manifest_root[0] ^= 1;
        assert!(matches!(
            verify_axt_transfer(&prepared, &expected, changed_context, &encoded, limits),
            Err(Error::InvalidAxtBinding { .. })
        ));
        let mut changed_context = context(&fixture);
        changed_context.remote_spend_claims = None;
        assert!(matches!(
            verify_axt_transfer(&prepared, &expected, changed_context, &encoded, limits),
            Err(Error::MissingMetadata { .. })
        ));
        let mut binding = fixture.binding.clone();
        binding.source_receipt_id.push_str("-different");
        let mut changed_context = context(&fixture);
        changed_context.binding = &binding;
        // This context is independently well formed, so failure must come from
        // binding the complete proof transcript to the original public context.
        AxtTransferAir::new(
            &prepared,
            &expected,
            &binding,
            fixture.metadata(),
            fixture.outer,
            fixture.remote.as_deref(),
        )
        .unwrap();
        assert!(
            verify_axt_transfer(&prepared, &expected, changed_context, &encoded, limits).is_err()
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
                &encoded,
                limits,
            )
            .is_err()
        );
        assert!(matches!(
            verify_transfer(&prepared, &expected, &encoded, limits),
            Err(Error::InvalidProofSemantics { .. })
        ));
        let ordinary = fixture.prepare(ProofSemantics::TransferStateTransition);
        let ordinary_io = fixture.expected(&ordinary);
        assert!(matches!(
            verify_axt_transfer(&ordinary, &ordinary_io, context(&fixture), &encoded, limits),
            Err(Error::InvalidProofSemantics { .. })
        ));
        assert!(verify_transfer(&ordinary, &ordinary_io, &encoded, limits).is_err());
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(matches!(
            verify_axt_transfer(&prepared, &expected, context(&fixture), &trailing, limits),
            Err(Error::Encode(_))
        ));
        drop(trailing);

        // Retain only this deterministic public-fixture proof, keyed by exact
        // bytes. No private-input files, signing material or live state are retained.
        let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/fastpq-production-validation");
        std::fs::create_dir_all(&artifact_dir).unwrap();
        let artifact = artifact_dir.join(format!(
            "compact-shared-axt-transfer-{}.bin",
            hex::encode(Sha256::digest(&encoded))
        ));
        std::fs::write(&artifact, &encoded).unwrap();
        eprintln!(
            "compact_axt_construction={construction:?}; proving={proving:?}; shared_conversion={conversion:?}; raw_facade_verifying={verifying:?}; typed_bytes={typed_bytes}; work={work:?}; default_admitted=false; diagnostic_limit={}; production_security_qualified=false; public_fixture_artifact={}",
            limits.max_proof_bytes,
            artifact.display()
        );
    }

    fn shake_limits() -> VerifyLimits {
        VerifyLimits {
            max_proof_bytes: 4_326_227,
            max_queries: 375,
            ..VerifyLimits::default()
        }
    }

    #[test]
    fn candidate_facades_preserve_resource_and_semantic_preflights() {
        let fixture = Fixture::new(false);
        let ordinary = fixture.prepare(ProofSemantics::TransferStateTransition);
        let axt = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let zero = VerifyLimits {
            max_proof_bytes: 0,
            ..shake_limits()
        };
        for result in [
            verify_shake_transfer(&axt, &fixture.expected(&axt), BAD_PROOF, zero, 0),
            verify_shake_axt_transfer(
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
            verify_shake_transfer(&axt, &fixture.expected(&axt), BAD_PROOF, shake_limits(), 0),
            Err(Error::InvalidProofSemantics { .. })
        ));
        assert!(matches!(
            verify_shake_axt_transfer(
                &ordinary,
                &fixture.expected(&ordinary),
                context(&fixture),
                BAD_PROOF,
                shake_limits(),
                0
            ),
            Err(Error::InvalidProofSemantics { .. })
        ));
        for semantics in [
            ProofSemantics::TransferStateTransition,
            ProofSemantics::AxtTransferClaim,
        ] {
            let prepared = fixture.prepare(semantics);
            for (limits, required_error) in [
                (
                    VerifyLimits {
                        max_transitions: 1,
                        ..shake_limits()
                    },
                    "max_transitions",
                ),
                (
                    VerifyLimits {
                        max_batch_bytes: 0,
                        ..shake_limits()
                    },
                    "max_batch_bytes",
                ),
                (
                    VerifyLimits {
                        max_air_row_values: 341,
                        ..shake_limits()
                    },
                    "max_air_row_values",
                ),
                (
                    VerifyLimits {
                        max_queries: 374,
                        ..shake_limits()
                    },
                    "max_queries",
                ),
            ] {
                let result = if semantics == ProofSemantics::TransferStateTransition {
                    verify_shake_transfer(
                        &prepared,
                        &fixture.expected(&prepared),
                        BAD_PROOF,
                        limits,
                        usize::MAX,
                    )
                } else {
                    verify_shake_axt_transfer(
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
        assert_eq!(SharedVerifier::Prototype.queries(), 136);
        assert!(!SharedVerifier::Prototype.is_shake());
        assert_eq!(
            SharedVerifier::ShakeCandidate {
                max_decode_allocation_charges: 0
            }
            .queries(),
            375
        );
        assert!(
            SharedVerifier::ShakeCandidate {
                max_decode_allocation_charges: 0
            }
            .is_shake()
        );
    }

    #[test]
    fn candidate_facades_check_all_expected_fields_before_raw_decoding() {
        let fixture = Fixture::new(false);
        for semantics in [
            ProofSemantics::TransferStateTransition,
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
                let result = if semantics == ProofSemantics::TransferStateTransition {
                    verify_shake_transfer(
                        &prepared,
                        &expected,
                        BAD_PROOF,
                        shake_limits(),
                        64 * 1024 * 1024,
                    )
                } else {
                    verify_shake_axt_transfer(
                        &prepared,
                        &expected,
                        context(&fixture),
                        BAD_PROOF,
                        shake_limits(),
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
            let result = if semantics == ProofSemantics::TransferStateTransition {
                verify_shake_transfer(
                    &prepared,
                    &fixture.expected(&prepared),
                    BAD_PROOF,
                    shake_limits(),
                    64 * 1024 * 1024,
                )
            } else {
                verify_shake_axt_transfer(
                    &prepared,
                    &fixture.expected(&prepared),
                    context(&fixture),
                    BAD_PROOF,
                    shake_limits(),
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
            verify_shake_axt_transfer(
                &prepared,
                &expected,
                bad,
                BAD_PROOF,
                shake_limits(),
                usize::MAX
            ),
            Err(Error::InvalidAxtBinding { .. })
        ));
        for field in 0..5 {
            let mut bad = context(&fixture);
            match field {
                0 => bad.mirrors.dsid = iroha_data_model::DataSpaceId::new(8),
                1 => bad.mirrors.manifest_root[0] ^= 1,
                2 => bad.mirrors.da_commitment = None,
                3 => bad.mirrors.committed_amount = None,
                4 => bad.mirrors.expiry_slot = None,
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    verify_shake_axt_transfer(
                        &prepared,
                        &expected,
                        bad,
                        BAD_PROOF,
                        shake_limits(),
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
            verify_shake_axt_transfer(
                &prepared,
                &expected,
                bad,
                BAD_PROOF,
                shake_limits(),
                usize::MAX
            ),
            Err(Error::MissingMetadata { .. })
        ));
        let mut claims = fixture.remote.as_ref().unwrap().clone();
        claims[0].effective_amount = iroha_primitives::numeric::Quantity::from(34_u64);
        let mut bad = context(&fixture);
        bad.remote_spend_claims = Some(&claims);
        assert!(matches!(
            verify_shake_axt_transfer(
                &prepared,
                &expected,
                bad,
                BAD_PROOF,
                shake_limits(),
                usize::MAX
            ),
            Err(Error::InvalidAxtBinding { .. })
        ));
    }
}

#[cfg(test)]
#[path = "compact_shake_public_diagnostic.rs"]
mod shake_diagnostic;

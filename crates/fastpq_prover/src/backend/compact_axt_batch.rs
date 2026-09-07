//! Complete typed AXT context for ordered compact transfer bundle segments.
//!
//! Every segment binds the full immutable public transfer batch, expected seven
//! public inputs, ordered root chain and all canonical AXT binding, pre-proof
//! mirrors and remote-spend preimages before the first challenge. The ordinary
//! batch constructor remains a distinct route that rejects AXT. These statements
//! authenticate neither source finality nor caller authority. The outer ABI must
//! still check handle signatures/replay, expiry at use, amount resolution and its
//! post-proof amount commitment; including that commitment here is circular.
//!
//! TODO: Qualify aggregate security, resources and authenticated caller migration
//! before production admission. This module leaves the existing profile intact.

use iroha_data_model::nexus::{AxtFastpqBinding, AxtRemoteSpendClaimV1};
use norito::{NoritoSerialize, codec::Encode};

use super::{
    compact_axt_context::preflight_context,
    compact_protocol::{FixedAir, FixedAirSchema, PreparedAir},
    compact_public_api::AxtVerificationContext,
    compact_public_batch::{BatchContextLimits, preflight_prepared},
    compact_public_transfer::encode_context,
    compact_transfer_air::CompactTransferAir,
};
use crate::{
    Error, ProofSemantics, Result, VerifyLimits,
    axt_binding::{
        AxtProofContextMirrors, validate_axt_public_metadata, validate_axt_public_transfer_facts,
    },
    gadgets::{
        compact_smt_air::PublicStatement, public_transfer_statement::PreparedPublicTransfers,
    },
    proof::PublicIO,
};

const IDENTITY: &str = "fastpq:prototype:axt-transfer-bundle-segment:v1:342cols:923slots:65536rows";

#[derive(NoritoSerialize)]
#[norito(schema_name = "fastpq_prover::compact_prototype::AxtTransferBatchContextV1")]
struct BoundAxtBatchContext {
    version: u16,
    segment_count: u32,
    public_transfer_context: Vec<u8>,
    intermediate_roots: Vec<[u8; 32]>,
    binding: AxtFastpqBinding,
    metadata: AxtProofContextMirrors,
    remote_spend_claims: Option<Vec<AxtRemoteSpendClaimV1>>,
}

#[derive(NoritoSerialize)]
#[norito(schema_name = "fastpq_prover::compact_prototype::AxtTransferSegmentContextV1")]
struct BoundAxtSegmentContext {
    version: u16,
    segment_count: u32,
    ordinal: u32,
    batch_context: Vec<u8>,
}

/// Whole validated public AXT context with chronological claimed SMT statements.
///
/// Private fields and a typed constructor prevent supplying an arbitrary AIR or
/// dropping the AXT facts through the ordinary path. No successful proof or
/// source authority is implied by constructing this public statement object.
pub(super) struct AxtTransferBatch {
    public_io: PublicIO,
    context: Vec<u8>,
    statements: Vec<PublicStatement>,
    total_statement_bytes: usize,
    max_statement_bytes: usize,
}

impl AxtTransferBatch {
    /// Bind complete AXT public facts once, then count every full segment statement.
    ///
    /// Limits cover the original whole preparation and all supplied AXT bytes
    /// before cloning/commitment hashing. All remote occurrences are validated
    /// against the whole batch; no individual delta is re-prepared or validated
    /// as if it were the complete AXT source transaction.
    pub(super) fn new(
        prepared: &PreparedPublicTransfers<'_>,
        expected: &PublicIO,
        intermediate_roots: &[[u8; 32]],
        axt: AxtVerificationContext<'_>,
        limits: BatchContextLimits,
    ) -> Result<Self> {
        let count = prepared.pairs().len();
        let root_bytes = preflight_prepared(prepared, intermediate_roots.len(), limits)?;
        if prepared.semantics() != ProofSemantics::AxtTransferClaim {
            return Err(Error::InvalidProofSemantics {
                profile: prepared.semantics().name(),
                details: "AXT compact bundle requires AXT transfer-claim semantics".to_owned(),
            });
        }
        let axt_bytes =
            preflight_context(prepared, axt.binding, axt.metadata, axt.remote_spend_claims)?;
        let statements = prepared.compact_statements(intermediate_roots)?;
        let transfer_context = encode_context(prepared, expected)?;
        check_statement_bytes(checked_sum(
            checked_sum(axt_bytes, transfer_context.len())?,
            root_bytes,
        )?)?;
        // Validate the original complete table, not an isolated segment. These
        // predicates check exact entry hashes, commitment order and occurrence
        // multiplicity and are shared with the existing one-delta AXT boundary.
        validate_axt_public_transfer_facts(
            axt.binding,
            axt.metadata,
            prepared,
            axt.remote_spend_claims,
        )?;
        validate_axt_public_metadata(axt.binding, axt.metadata, axt.mirrors)?;
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let bound = BoundAxtBatchContext {
            version: 1,
            segment_count: checked_u32(count)?,
            public_transfer_context: transfer_context,
            intermediate_roots: intermediate_roots.to_vec(),
            binding: axt.binding.clone(),
            metadata: axt.mirrors,
            remote_spend_claims: axt
                .remote_spend_claims
                .map(<[AxtRemoteSpendClaimV1]>::to_vec),
        };
        let bytes = norito::core::encoded_payload_len(&bound)?;
        check_statement_bytes(bytes)?;
        check_total(checked_product(count, bytes)?, limits)?;
        let mut batch = Self {
            public_io: *expected,
            context: bound.encode(),
            statements,
            total_statement_bytes: 0,
            max_statement_bytes: 0,
        };
        for ordinal in 0..count {
            let context = batch.segment_context(ordinal)?;
            let bytes = CompactTransferAir::encoded_statement_len(
                &batch.statements[ordinal],
                Some(&context),
            )?;
            batch.total_statement_bytes = checked_sum(batch.total_statement_bytes, bytes)?;
            batch.max_statement_bytes = batch.max_statement_bytes.max(bytes);
            check_total(batch.total_statement_bytes, limits)?;
        }
        Ok(batch)
    }

    /// Number of actual chronological public delta occurrences.
    pub(super) fn segment_count(&self) -> usize {
        self.statements.len()
    }
    /// Exact independently expected overall inputs checked by this constructor.
    pub(super) const fn public_io(&self) -> PublicIO {
        self.public_io
    }
    /// Complete common public transfer and AXT context bound by every segment.
    pub(super) fn context_bytes(&self) -> &[u8] {
        &self.context
    }
    /// Public ports derived using the whole batch's scale and key allocation.
    pub(super) fn statements(&self) -> &[PublicStatement] {
        &self.statements
    }
    /// Exact sum of all canonical bare full segment statement lengths.
    pub(super) const fn total_statement_bytes(&self) -> usize {
        self.total_statement_bytes
    }
    /// Largest exact full statement for a stricter per-child byte ceiling.
    pub(super) const fn max_statement_bytes(&self) -> usize {
        self.max_statement_bytes
    }

    /// Build the fixed relation for one verifier-derived chronological ordinal.
    ///
    /// The outer verifier must verify every ordinal before returning success.
    pub(super) fn segment(&self, ordinal: usize) -> Result<AxtTransferSegmentAir> {
        let statement = self.statement(ordinal)?;
        let context = self.segment_context(ordinal)?;
        Ok(AxtTransferSegmentAir {
            inner: CompactTransferAir::new(statement, Some(&context))?,
        })
    }

    fn statement(&self, ordinal: usize) -> Result<&PublicStatement> {
        self.statements
            .get(ordinal)
            .ok_or(Error::QueryIndexOutOfRange {
                index: ordinal,
                len: self.statements.len(),
            })
    }

    fn segment_context(&self, ordinal: usize) -> Result<Vec<u8>> {
        self.statement(ordinal)?;
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let bound = BoundAxtSegmentContext {
            version: 1,
            segment_count: checked_u32(self.statements.len())?,
            ordinal: checked_u32(ordinal)?,
            batch_context: self.context.clone(),
        };
        check_statement_bytes(norito::core::encoded_payload_len(&bound)?)?;
        Ok(bound.encode())
    }
}

/// Fixed AXT bundle relation with a distinct identity and complete caller context.
pub(super) struct AxtTransferSegmentAir {
    inner: CompactTransferAir,
}

impl FixedAir for AxtTransferSegmentAir {
    fn schema(&self) -> FixedAirSchema {
        FixedAirSchema {
            identity: IDENTITY,
            ..self.inner.schema()
        }
    }
    fn statement_bytes(&self) -> &[u8] {
        self.inner.statement_bytes()
    }
    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        self.inner.evaluate(point, current, next)
    }
    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
        self.inner.prepare_prover()
    }
}

fn checked_u32(value: usize) -> Result<u32> {
    u32::try_from(value)
        .map_err(|_| invariant("AXT compact bundle count is not representable as u32"))
}
fn checked_sum(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| invariant("AXT compact bundle byte count overflows"))
}
fn checked_product(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| invariant("AXT compact bundle byte count overflows"))
}
fn check_total(actual: usize, limits: BatchContextLimits) -> Result<()> {
    check_limit(
        "max_compact_bundle_statement_bytes",
        actual,
        limits.max_total_statement_bytes,
    )
}
fn check_statement_bytes(actual: usize) -> Result<()> {
    check_limit(
        "max_compact_statement_bytes",
        actual,
        VerifyLimits::default().max_batch_bytes,
    )
}
fn check_limit(limit: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        Err(Error::VerifierLimitExceeded { limit, actual, max })
    } else {
        Ok(())
    }
}
fn invariant(details: &str) -> Error {
    Error::TransferInvariant {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        backend::{
            compact_axt_air::AxtTransferAir, compact_axt_context::tests::Fixture,
            compact_public_batch::PublicTransferBatch,
        },
        gadgets::{compact_smt_air::COLUMN_COUNT, public_transfer_statement::PublicTransferLimits},
    };
    use iroha_crypto::Hash;
    use iroha_data_model::{DataSpaceId, nexus::compute_remote_spend_claim_commitment_v1};
    use iroha_primitives::numeric::Quantity;

    fn context(fixture: &Fixture) -> AxtVerificationContext<'_> {
        AxtVerificationContext {
            binding: &fixture.binding,
            metadata: fixture.metadata(),
            mirrors: fixture.outer,
            remote_spend_claims: fixture.remote.as_deref(),
        }
    }
    fn roots(count: usize) -> Vec<[u8; 32]> {
        (1..count)
            .map(|index| Hash::new(index.to_le_bytes()).into())
            .collect()
    }
    fn batch(fixture: &Fixture) -> Result<AxtTransferBatch> {
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        AxtTransferBatch::new(
            &prepared,
            &fixture.expected(&prepared),
            &roots(prepared.pairs().len()),
            context(fixture),
            BatchContextLimits::default(),
        )
    }
    fn recommit(binding: &mut AxtFastpqBinding, claims: &mut [AxtRemoteSpendClaimV1]) {
        claims.sort_by_key(compute_remote_spend_claim_commitment_v1);
        binding.remote_spend_intent_commitments = claims
            .iter()
            .map(compute_remote_spend_claim_commitment_v1)
            .collect();
    }

    #[test]
    fn whole_axt_batch_has_exact_ports_complete_context_and_counted_segment_work() {
        for count in [1, 2, 3] {
            for remote in [false, true] {
                let fixture = Fixture::multiple(count, remote);
                let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
                let batch = batch(&fixture).unwrap();
                assert_eq!(batch.segment_count(), count);
                assert_eq!(batch.public_io(), fixture.expected(&prepared));
                assert_eq!(
                    batch.statements(),
                    prepared.compact_statements(&roots(count)).unwrap()
                );
                assert!(!batch.context_bytes().is_empty());
                assert_eq!(prepared.claims().len(), count);
                let mut total = 0;
                let mut maximum = 0;
                for ordinal in 0..count {
                    assert_eq!(
                        batch.statements()[ordinal].updates,
                        prepared.pairs()[ordinal].updates
                    );
                    let segment = batch.segment(ordinal).unwrap();
                    assert_eq!(segment.schema().identity, IDENTITY);
                    assert_eq!(
                        (
                            segment.schema().trace_rows,
                            segment.schema().width,
                            segment.schema().constraints
                        ),
                        (65_536, 342, 923)
                    );
                    total += segment.statement_bytes().len();
                    maximum = maximum.max(segment.statement_bytes().len());
                }
                assert_eq!(total, batch.total_statement_bytes());
                assert_eq!(maximum, batch.max_statement_bytes());
                for ordinal in [count, usize::MAX] {
                    assert!(
                        matches!(batch.segment(ordinal), Err(Error::QueryIndexOutOfRange { index, len }) if index == ordinal && len == count)
                    );
                }
                if count == 1 {
                    let legacy = AxtTransferAir::new(
                        &prepared,
                        &fixture.expected(&prepared),
                        &fixture.binding,
                        fixture.metadata(),
                        fixture.outer,
                        fixture.remote.as_deref(),
                    )
                    .unwrap();
                    assert_ne!(
                        legacy.schema().identity,
                        batch.segment(0).unwrap().schema().identity
                    );
                    assert_ne!(
                        legacy.statement_bytes(),
                        batch.segment(0).unwrap().statement_bytes()
                    );
                }
            }
        }
    }

    #[test]
    fn wrapper_keeps_exact_smt_equations_and_profile_routes_separate() {
        let fixture = Fixture::multiple(2, true);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let batch = batch(&fixture).unwrap();
        let raw = CompactTransferAir::new(&batch.statements()[0], None).unwrap();
        let axt = batch.segment(0).unwrap();
        let current = core::array::from_fn::<_, COLUMN_COUNT, _>(|index| index as u64 + 13);
        let next = core::array::from_fn::<_, COLUMN_COUNT, _>(|index| index as u64 + 71);
        assert_eq!(
            axt.evaluate(7, &current, &next).unwrap(),
            raw.evaluate(7, &current, &next).unwrap()
        );
        assert!(
            axt.evaluate(7, &current[..COLUMN_COUNT - 1], &next)
                .is_err()
        );
        assert!(
            PublicTransferBatch::new(
                &prepared,
                &fixture.expected(&prepared),
                &roots(2),
                BatchContextLimits::default()
            )
            .is_err()
        );
        let ordinary = fixture.prepare(ProofSemantics::TransferStateTransition);
        assert!(matches!(
            AxtTransferBatch::new(
                &ordinary,
                &fixture.expected(&ordinary),
                &roots(2),
                context(&fixture),
                BatchContextLimits::default()
            ),
            Err(Error::InvalidProofSemantics { .. })
        ));
        let ordinary = PublicTransferBatch::new(
            &ordinary,
            &fixture.expected(&ordinary),
            &roots(2),
            BatchContextLimits::default(),
        )
        .unwrap();
        assert_ne!(
            ordinary.segment(0).unwrap().schema().identity,
            axt.schema().identity
        );
        assert_ne!(
            ordinary.segment(0).unwrap().statement_bytes(),
            axt.statement_bytes()
        );
    }

    #[test]
    fn remote_occurrences_are_checked_against_the_complete_batch() {
        let fixture = Fixture::multiple(2, true);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&prepared);
        let original = batch(&fixture).unwrap();
        let all = fixture.remote.as_ref().unwrap();
        assert_eq!(all.len(), 2);
        // Both public occurrences have the same remote transfer fact. One claim
        // with a coherent commitment list must still fail cardinality matching.
        for length in [1, 3] {
            let mut claims = all.clone();
            if length == 1 {
                claims.pop();
            } else {
                let mut additional = claims[0].clone();
                additional.handle_replay_key.handle_era = 17;
                claims.push(additional);
            }
            let mut binding = fixture.binding.clone();
            recommit(&mut binding, &mut claims);
            let changed = AxtVerificationContext {
                binding: &binding,
                remote_spend_claims: Some(&claims),
                ..context(&fixture)
            };
            assert!(matches!(
                AxtTransferBatch::new(
                    &prepared,
                    &expected,
                    &roots(2),
                    changed,
                    BatchContextLimits::default()
                ),
                Err(Error::InvalidAxtBinding { details }) if details.contains("one-for-one")
            ));
        }
        let missing = AxtVerificationContext {
            remote_spend_claims: None,
            ..context(&fixture)
        };
        assert!(matches!(
            AxtTransferBatch::new(
                &prepared,
                &expected,
                &roots(2),
                missing,
                BatchContextLimits::default()
            ),
            Err(Error::MissingMetadata { .. })
        ));
        let mut changed_claims = all.clone();
        changed_claims[0].effective_amount = Quantity::from(4_u64);
        let mut changed_binding = fixture.binding.clone();
        recommit(&mut changed_binding, &mut changed_claims);
        let changed = AxtVerificationContext {
            binding: &changed_binding,
            remote_spend_claims: Some(&changed_claims),
            ..context(&fixture)
        };
        assert!(matches!(
            AxtTransferBatch::new(
                &prepared,
                &expected,
                &roots(2),
                changed,
                BatchContextLimits::default()
            ),
            Err(Error::InvalidAxtBinding { .. })
        ));
        // A different valid receipt changes every segment even though all local
        // SMT ports and transfer facts are unchanged.
        let mut changed_fixture = fixture.clone();
        changed_fixture.binding.source_receipt_id = "another-canonical-receipt".into();
        let changed = batch(&changed_fixture).unwrap();
        assert_eq!(original.statements(), changed.statements());
        for ordinal in 0..2 {
            assert_ne!(
                original.segment_context(ordinal).unwrap(),
                changed.segment_context(ordinal).unwrap()
            );
        }
    }

    #[test]
    fn every_expected_input_mirror_and_root_coordinate_is_bound_or_rejected() {
        let fixture = Fixture::multiple(3, true);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&prepared);
        let original = batch(&fixture).unwrap();
        for field in 0..7 {
            let mut changed = expected;
            match field {
                0 => changed.dsid[15] ^= 1,
                1 => changed.slot ^= 1,
                2 => changed.old_root[0] ^= 1,
                3 => changed.new_root[0] ^= 1,
                4 => changed.perm_root[31] ^= 0x80,
                5 => changed.tx_set_hash[31] ^= 0x80,
                6 => changed.ordering_hash[31] ^= 0x80,
                _ => unreachable!(),
            }
            assert!(matches!(
                AxtTransferBatch::new(
                    &prepared,
                    &changed,
                    &roots(3),
                    context(&fixture),
                    BatchContextLimits::default()
                ),
                Err(Error::PublicIoMismatch { .. })
            ));
        }
        for field in 0..5 {
            let mut changed = context(&fixture);
            match field {
                0 => changed.mirrors.dsid = DataSpaceId::new(8),
                1 => changed.mirrors.manifest_root[0] ^= 1,
                2 => changed.mirrors.da_commitment = None,
                3 => changed.mirrors.committed_amount = None,
                4 => changed.mirrors.expiry_slot = None,
                _ => unreachable!(),
            }
            assert!(matches!(
                AxtTransferBatch::new(
                    &prepared,
                    &expected,
                    &roots(3),
                    changed,
                    BatchContextLimits::default()
                ),
                Err(Error::InvalidAxtBinding { .. })
            ));
        }
        let mut metadata = context(&fixture);
        metadata.metadata.entry_hash = &[0; 32];
        assert!(matches!(
            AxtTransferBatch::new(
                &prepared,
                &expected,
                &roots(3),
                metadata,
                BatchContextLimits::default()
            ),
            Err(Error::InvalidAxtBinding { .. })
        ));
        for byte in 0..32 {
            let mut changed_roots = roots(3);
            changed_roots[1][byte] ^= if byte == 31 { 0x80 } else { 1 };
            let changed = AxtTransferBatch::new(
                &prepared,
                &expected,
                &changed_roots,
                context(&fixture),
                BatchContextLimits::default(),
            )
            .unwrap();
            assert_eq!(original.statements()[0], changed.statements()[0]);
            assert_ne!(
                original.segment_context(0).unwrap(),
                changed.segment_context(0).unwrap()
            );
        }
        for invalid_roots in [
            &[][..],
            &[[0; 32]][..],
            &[[0; 32]; 2][..],
            &[[0; 32]; 3][..],
        ] {
            assert!(
                AxtTransferBatch::new(
                    &prepared,
                    &expected,
                    invalid_roots,
                    context(&fixture),
                    BatchContextLimits::default()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn exact_cumulative_limits_and_axt_preflight_bound_all_public_inputs() {
        let fixture = Fixture::multiple(2, true);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&prepared);
        let original = batch(&fixture).unwrap();
        let exact = BatchContextLimits {
            max_segments: 2,
            max_total_statement_bytes: original.total_statement_bytes(),
        };
        assert_eq!(
            AxtTransferBatch::new(&prepared, &expected, &roots(2), context(&fixture), exact)
                .unwrap()
                .total_statement_bytes(),
            exact.max_total_statement_bytes
        );
        assert!(matches!(
            AxtTransferBatch::new(
                &prepared,
                &expected,
                &roots(2),
                context(&fixture),
                BatchContextLimits {
                    max_total_statement_bytes: exact.max_total_statement_bytes - 1,
                    ..exact
                }
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_bundle_statement_bytes",
                ..
            })
        ));
        assert!(
            AxtTransferBatch::new(
                &prepared,
                &expected,
                &roots(2),
                context(&fixture),
                BatchContextLimits {
                    max_segments: 1,
                    ..exact
                }
            )
            .is_err()
        );
        let too_many = vec![
            fixture.remote.as_ref().unwrap()[0].clone();
            PublicTransferLimits::default().max_deltas + 1
        ];
        let changed = AxtVerificationContext {
            remote_spend_claims: Some(&too_many),
            ..context(&fixture)
        };
        assert!(matches!(
            AxtTransferBatch::new(&prepared, &expected, &roots(2), changed, exact),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_axt_remote_claims",
                ..
            })
        ));
        let mut huge = fixture.binding.clone();
        huge.source_receipt_id = "x".repeat(VerifyLimits::default().max_batch_bytes + 1);
        let changed = AxtVerificationContext {
            binding: &huge,
            ..context(&fixture)
        };
        assert!(matches!(
            AxtTransferBatch::new(&prepared, &expected, &roots(2), changed, exact),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_axt_context_bytes",
                ..
            })
        ));
        assert!(checked_sum(usize::MAX, 1).is_err());
        assert!(checked_product(usize::MAX, 2).is_err());
        assert_eq!(checked_sum(0, 7).unwrap(), 7);
        assert_eq!(checked_product(7, 0).unwrap(), 0);
        assert_eq!(checked_u32(u32::MAX as usize).unwrap(), u32::MAX);
        if usize::BITS > 32 {
            assert!(checked_u32(u32::MAX as usize + 1).is_err());
        }
        assert!(check_statement_bytes(VerifyLimits::default().max_batch_bytes).is_ok());
        assert!(check_statement_bytes(VerifyLimits::default().max_batch_bytes + 1).is_err());
    }

    #[test]
    fn canonical_axt_contexts_ignore_and_restore_all_ambient_layout_flags() {
        let fixture = Fixture::multiple(2, true);
        let original = batch(&fixture).unwrap();
        let original_segment = original.segment(1).unwrap();
        for flags in
            (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let actual = batch(&fixture).unwrap();
            assert_eq!(actual.context_bytes(), original.context_bytes());
            assert_eq!(
                actual.total_statement_bytes(),
                original.total_statement_bytes()
            );
            assert_eq!(
                actual.segment(1).unwrap().statement_bytes(),
                original_segment.statement_bytes()
            );
            assert_eq!(norito::core::get_decode_flags(), flags);
        }
    }
}

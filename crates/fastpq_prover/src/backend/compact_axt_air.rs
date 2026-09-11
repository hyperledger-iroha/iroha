//! Proof-ready AXT public context for the test-only compact transfer relation.
//!
//! The statement is exactly the canonical context produced by the shared AXT
//! validator. Its complete binding, public transfer claims and pre-proof outer
//! mirrors therefore precede every Fiat–Shamir challenge in the compact engine.
//! The private relation remains the complete fixed public-transfer SMT AIR.
//!
//! The caller must authenticate the expected PublicIO, execution authority and
//! outer AXT inputs. This wrapper grants no handle authority or source finality;
//! its touched-balance roots are not authenticated consensus state roots. The
//! surrounding ABI must still check expiry at use, handle signatures/replay,
//! amount resolution and the post-proof amount commitment.
//!
//! TODO: Qualify the compact protocol and its complete resource profile, and
//! integrate an authenticated public statement API before production admission.
//! This test-only wrapper neither removes mandatory replay nor claims zero knowledge.

use iroha_data_model::nexus::{AxtFastpqBinding, AxtRemoteSpendClaimV1};

use super::compact_value_domain::CompactTransferValue;
use super::{
    compact_axt_context::encode_context,
    compact_protocol::{FixedAir, FixedAirSchema, PreparedAir},
    compact_public_transfer::PublicTransferAir,
};
use crate::{
    Result,
    axt_binding::{AxtProofContextMirrors, AxtPublicMetadataBytes},
    gadgets::public_transfer_statement::PreparedPublicTransfers,
    proof::PublicIO,
};

const IDENTITY: &str = <u64 as CompactTransferValue>::AXT_IDENTITY;

/// Fixed one-delta transfer AIR with the complete validated public AXT context.
pub(super) struct AxtTransferAir {
    identity: &'static str,
    inner: PublicTransferAir,
    statement_bytes: Vec<u8>,
}

impl AxtTransferAir {
    /// Bind caller-authenticated public inputs using the shared AXT predicates.
    ///
    /// The complete context is validated before constructing the stored AIR.
    /// No legacy batch, private witness or proof can supply these public inputs.
    /// `expected` and the outer binding/mirrors must come from the surrounding
    /// authenticated caller; copying their values from a proof is insufficient.
    pub(super) fn new<V: CompactTransferValue>(
        prepared: &PreparedPublicTransfers<'_, V>,
        expected: &PublicIO,
        binding: &AxtFastpqBinding,
        metadata: AxtPublicMetadataBytes<'_>,
        outer: AxtProofContextMirrors,
        remote_spend_claims: Option<&[AxtRemoteSpendClaimV1]>,
    ) -> Result<Self> {
        let statement_bytes = encode_context(
            prepared,
            expected,
            binding,
            metadata,
            outer,
            remote_spend_claims,
        )?;
        Ok(Self {
            identity: V::AXT_IDENTITY,
            inner: PublicTransferAir::new(prepared, expected)?,
            statement_bytes,
        })
    }
}

impl FixedAir for AxtTransferAir {
    fn schema(&self) -> FixedAirSchema {
        FixedAirSchema {
            identity: self.identity,
            ..self.inner.schema()
        }
    }

    fn statement_bytes(&self) -> &[u8] {
        &self.statement_bytes
    }

    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        self.inner.evaluate(point, current, next)
    }

    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
        self.inner.prepare_prover()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Error, ProofSemantics, VerifyLimits,
        backend::{GOLDILOCKS_MODULUS, compact_axt_context::tests::Fixture},
        gadgets::compact_smt_air::COLUMN_COUNT,
    };
    use iroha_model_base::topology::DataSpaceId;

    fn relation(fixture: &Fixture) -> Result<AxtTransferAir> {
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        AxtTransferAir::new(
            &prepared,
            &fixture.expected(&prepared),
            &fixture.binding,
            fixture.metadata(),
            fixture.outer,
            fixture.remote.as_deref(),
        )
    }

    #[test]
    fn statement_is_exact_canonical_axt_context_with_distinct_fixed_identity() {
        for remote in [false, true] {
            let fixture = Fixture::new(remote);
            let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
            let expected = fixture.expected(&prepared);
            let air = relation(&fixture).unwrap();
            let public = PublicTransferAir::new(&prepared, &expected).unwrap();
            let context = encode_context(
                &prepared,
                &expected,
                &fixture.binding,
                fixture.metadata(),
                fixture.outer,
                fixture.remote.as_deref(),
            )
            .unwrap();
            assert_eq!(air.statement_bytes(), context.as_slice());
            assert_ne!(air.statement_bytes(), public.statement_bytes());
            assert!(air.statement_bytes().len() <= VerifyLimits::default().max_batch_bytes);
            assert_eq!(
                air.schema(),
                FixedAirSchema {
                    trace_rows: 65_536,
                    width: 342,
                    constraints: 923,
                    identity: IDENTITY,
                }
            );
            assert_ne!(air.schema().identity, public.schema().identity);
            assert_eq!(
                air.schema(),
                FixedAirSchema {
                    identity: IDENTITY,
                    ..public.schema()
                }
            );
            let flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(relation(&fixture).unwrap().statement_bytes(), context);
        }
    }

    #[test]
    fn evaluations_delegate_every_fixed_slot_and_row_validation() {
        let fixture = Fixture::new(true);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let public = PublicTransferAir::new(&prepared, &fixture.expected(&prepared)).unwrap();
        let air = relation(&fixture).unwrap();
        // Complete canonical field cells, including values outside the u32
        // witness range, are legal LDE inputs to the identical fixed relation.
        let current: [u64; COLUMN_COUNT] =
            core::array::from_fn(|index| GOLDILOCKS_MODULUS - 1 - index as u64);
        let next: [u64; COLUMN_COUNT] = core::array::from_fn(|index| 31 + index as u64);
        for point in [7, 13] {
            let actual = air.evaluate(point, &current, &next).unwrap();
            assert_eq!(actual.len(), 923);
            assert_eq!(actual, public.evaluate(point, &current, &next).unwrap());
        }
        for malformed_current in [false, true] {
            let (mut current, mut next) = (current.to_vec(), next.to_vec());
            if malformed_current {
                current.pop();
            } else {
                next[COLUMN_COUNT - 1] = GOLDILOCKS_MODULUS;
            }
            assert_eq!(
                air.evaluate(7, &current, &next).unwrap_err().to_string(),
                public.evaluate(7, &current, &next).unwrap_err().to_string()
            );
        }
    }

    #[test]
    fn constructor_rejects_non_axt_and_opaque_claim_semantics() {
        let fixture = Fixture::new(false);
        let prepared = fixture.prepare(ProofSemantics::StateTransition);
        assert!(matches!(
            AxtTransferAir::new(
                &prepared,
                &fixture.expected(&prepared),
                &fixture.binding,
                fixture.metadata(),
                fixture.outer,
                None,
            ),
            Err(Error::InvalidProofSemantics { .. })
        ));
        let mut opaque = fixture;
        opaque.binding.claim_type = "authorization".into();
        assert!(matches!(
            relation(&opaque),
            Err(Error::InvalidProofSemantics { .. })
        ));
    }

    #[test]
    fn binding_changes_reach_the_statement_and_bad_outer_mirrors_are_rejected() {
        let fixture = Fixture::new(false);
        let original = relation(&fixture).unwrap();
        let mut changed = fixture.clone();
        changed.binding.source_receipt_id.push_str("-different");
        assert_ne!(
            relation(&changed).unwrap().statement_bytes(),
            original.statement_bytes()
        );
        let mut changed = fixture.clone();
        changed.binding.corridor.push_str("-different");
        assert_ne!(
            relation(&changed).unwrap().statement_bytes(),
            original.statement_bytes()
        );
        assert_ne!(
            relation(&Fixture::new(true)).unwrap().statement_bytes(),
            original.statement_bytes()
        );
        for field in 0..5 {
            let mut changed = fixture.clone();
            match field {
                0 => changed.outer.dsid = DataSpaceId::new(8),
                1 => changed.outer.manifest_root[0] ^= 1,
                2 => changed.outer.da_commitment = None,
                3 => changed.outer.committed_amount = None,
                4 => changed.outer.expiry_slot = None,
                _ => unreachable!(),
            }
            assert!(
                matches!(relation(&changed), Err(Error::InvalidAxtBinding { .. })),
                "outer field {field}"
            );
        }
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let mut metadata = fixture.metadata();
        metadata.entry_hash = &[0; 32];
        assert!(
            AxtTransferAir::new(
                &prepared,
                &fixture.expected(&prepared),
                &fixture.binding,
                metadata,
                fixture.outer,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn every_caller_expected_public_io_field_is_required() {
        let fixture = Fixture::new(false);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
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
            assert!(
                matches!(
                    AxtTransferAir::new(
                        &prepared,
                        &expected,
                        &fixture.binding,
                        fixture.metadata(),
                        fixture.outer,
                        None,
                    ),
                    Err(Error::PublicIoMismatch {
                        field: "compact_public_io"
                    })
                ),
                "public field {field}"
            );
        }
    }
}

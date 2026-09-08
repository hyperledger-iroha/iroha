//! Full-domain public adapter, route, resource and model-bridge regressions.

use super::{
    compact_axt_air::AxtTransferAir,
    compact_axt_batch::AxtTransferBatch,
    compact_axt_context::tests::Fixture,
    compact_bundle::{self, BundleLimits},
    compact_model_statement::{with_prepared_quantity_statement, with_prepared_statement},
    compact_protocol::FixedAir,
    compact_public_api::{self, AxtVerificationContext},
    compact_public_batch::{BatchContextLimits, PublicTransferBatch},
    compact_public_transfer::{PublicTransferAir, encode_context},
    compact_value_domain::CompactTransferValue,
};
use crate::{
    Error, ProofSemantics, PublicInputs, StateTransition, VerifyLimits,
    gadgets::public_transfer_statement::{
        DerivedTransferSmtWitnesses, PreparedPublicTransfers, PublicTransferLimits,
        PublicTransferTranscript, TransferSmtBuildLimits, decode_quantity_units_v1,
        encode_quantity_units_v1, materialize_quantity_public_transfers,
        prepare_quantity_public_transfers,
    },
    proof::PublicIO,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    fastpq::{
        FastpqOperationKind, FastpqPublicInputs, FastpqPublicTransferStatementV1,
        FastpqQuantityUnits, FastpqStateTransition,
    },
    nexus::compute_remote_spend_claim_commitment_v1,
};
use iroha_primitives::{
    bigint::BigInt,
    numeric::{Numeric, Quantity},
};
use iroha_zkp_halo2::poseidon::PoseidonByteHasher;
use norito::codec::Encode;

/// Four representative domains, including the maximum 605-bit common-scale value.
#[derive(Clone, Copy)]
pub(super) enum QuantityCase {
    U128,
    Maximum,
    MixedScale,
    Tiny,
}

fn maximum() -> Quantity {
    let mut bytes = [0xff; 64];
    bytes[63] = 0x7f;
    Quantity::from_canonical_numeric(
        Numeric::try_new(BigInt::from_twos_bytes(&bytes).unwrap(), 0).unwrap(),
    )
    .unwrap()
}

fn tiny() -> Quantity {
    Quantity::from_canonical_numeric(Numeric::try_new(1_u32, 28).unwrap()).unwrap()
}

/// Public-only fixture returned separately from private construction witnesses.
pub(super) struct QuantityFixture {
    pub(super) axt: Fixture,
    pub(super) rows: Vec<StateTransition>,
    pub(super) claims: Vec<PublicTransferTranscript>,
    pub(super) inputs: PublicInputs,
}

impl QuantityFixture {
    pub(super) fn new(case: QuantityCase, count: usize) -> (Self, DerivedTransferSmtWitnesses) {
        let mut axt = Fixture::multiple(count, true);
        let (mut claims, inputs) = {
            let prepared = axt.prepare(ProofSemantics::AxtTransferClaim);
            (prepared.claims().to_vec(), *prepared.public_inputs())
        };
        let (mut sender, mut receiver, amount) = match case {
            QuantityCase::U128 => (Quantity::from(u128::MAX), Quantity::zero(), Quantity::one()),
            QuantityCase::Maximum => (maximum(), Quantity::zero(), maximum()),
            QuantityCase::MixedScale => (
                maximum().try_sub(&Quantity::one()).unwrap(),
                tiny(),
                Quantity::one(),
            ),
            QuantityCase::Tiny => (Quantity::from(2_u32), Quantity::zero(), tiny()),
        };
        assert!(count == 1 || !matches!(case, QuantityCase::Maximum));
        for claim in &mut claims {
            assert_eq!(claim.deltas.len(), 1);
            let delta = &mut claim.deltas[0];
            delta.from_balance_before = sender.clone();
            delta.to_balance_before = receiver.clone();
            delta.amount = amount.clone();
            sender = sender.try_sub(&amount).unwrap();
            receiver = receiver.try_add(&amount).unwrap();
            delta.from_balance_after = sender.clone();
            delta.to_balance_after = receiver.clone();
            let mut digest = PoseidonByteHasher::new();
            delta.from_account.encode_to(&mut digest);
            delta.to_account.encode_to(&mut digest);
            delta.asset_definition.encode_to(&mut digest);
            delta.amount.encode_to(&mut digest);
            digest.update(claim.batch_hash.as_ref());
            claim.poseidon_preimage_digest = Some(Hash::prehashed(digest.finalize()));
        }
        let remote = axt.remote.as_mut().unwrap();
        for claim in remote.iter_mut() {
            claim.effective_amount = amount.clone();
        }
        remote.sort_by_key(compute_remote_spend_claim_commitment_v1);
        axt.binding.remote_spend_intent_commitments = remote
            .iter()
            .map(compute_remote_spend_claim_commitment_v1)
            .collect();
        // The scalar metadata is an exact outer mirror, not the transfer quantity.
        let built = materialize_quantity_public_transfers(
            &claims,
            inputs,
            ProofSemantics::AxtTransferClaim,
            PublicTransferLimits::default(),
            TransferSmtBuildLimits::for_update_limit(count * 2).unwrap(),
        )
        .unwrap();
        let (rows, inputs, _, private) = built.into_parts();
        (
            Self {
                axt,
                rows,
                claims,
                inputs,
            },
            private,
        )
    }

    pub(super) fn prepare(
        &self,
        semantics: ProofSemantics,
    ) -> PreparedPublicTransfers<'_, FastpqQuantityUnits> {
        prepare_quantity_public_transfers(
            &self.rows,
            &self.claims,
            self.inputs,
            semantics,
            PublicTransferLimits::default(),
        )
        .unwrap()
    }

    pub(super) fn expected(&self) -> PublicIO {
        expected(&self.prepare(ProofSemantics::StateTransition))
    }

    pub(super) fn context(&self) -> AxtVerificationContext<'_> {
        AxtVerificationContext {
            binding: &self.axt.binding,
            metadata: self.axt.metadata(),
            mirrors: self.axt.outer,
            remote_spend_claims: self.axt.remote.as_deref(),
        }
    }

    pub(super) fn model(&self) -> FastpqPublicTransferStatementV1 {
        let input = self.inputs;
        FastpqPublicTransferStatementV1 {
            public_inputs: FastpqPublicInputs {
                dsid: input.dsid,
                slot: input.slot,
                old_root: input.old_root,
                new_root: input.new_root,
                perm_root: input.perm_root,
                tx_set_hash: input.tx_set_hash,
            },
            ordering_hash: self.expected().ordering_hash,
            transitions: self
                .rows
                .iter()
                .map(|row| FastpqStateTransition {
                    key: row.key.clone(),
                    pre_value: row.pre_value.clone(),
                    post_value: row.post_value.clone(),
                    operation: FastpqOperationKind::Transfer,
                })
                .collect(),
            transcripts: self.claims.clone(),
        }
    }
}

fn expected<V>(prepared: &PreparedPublicTransfers<'_, V>) -> PublicIO {
    let p = prepared.public_inputs();
    PublicIO {
        dsid: p.dsid,
        slot: p.slot,
        old_root: p.old_root,
        new_root: p.new_root,
        perm_root: p.perm_root,
        tx_set_hash: p.tx_set_hash,
        ordering_hash: prepared.ordering_hash().into(),
    }
}

fn axt_air(f: &QuantityFixture) -> AxtTransferAir {
    AxtTransferAir::new(
        &f.prepare(ProofSemantics::AxtTransferClaim),
        &f.expected(),
        &f.axt.binding,
        f.axt.metadata(),
        f.axt.outer,
        f.axt.remote.as_deref(),
    )
    .unwrap()
}

#[test]
fn value_type_selects_eight_distinct_fixed_route_identities() {
    let ids = [
        u64::TRANSFER_IDENTITY,
        u64::AXT_IDENTITY,
        u64::BATCH_IDENTITY,
        u64::AXT_BATCH_IDENTITY,
        FastpqQuantityUnits::TRANSFER_IDENTITY,
        FastpqQuantityUnits::AXT_IDENTITY,
        FastpqQuantityUnits::BATCH_IDENTITY,
        FastpqQuantityUnits::AXT_BATCH_IDENTITY,
    ];
    assert_eq!(
        ids.into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        8
    );
    for case in [
        QuantityCase::U128,
        QuantityCase::Maximum,
        QuantityCase::MixedScale,
        QuantityCase::Tiny,
    ] {
        let (f, private) = QuantityFixture::new(case, 1);
        drop(private);
        let ordinary = f.prepare(ProofSemantics::StateTransition);
        let axt = f.prepare(ProofSemantics::AxtTransferClaim);
        let air = PublicTransferAir::new(&ordinary, &f.expected()).unwrap();
        assert_eq!(
            air.schema().identity,
            FastpqQuantityUnits::TRANSFER_IDENTITY
        );
        assert_eq!(
            axt_air(&f).schema().identity,
            FastpqQuantityUnits::AXT_IDENTITY
        );
        assert_eq!(
            PublicTransferBatch::new(&ordinary, &f.expected(), &[], BatchContextLimits::default())
                .unwrap()
                .segment(0)
                .unwrap()
                .schema()
                .identity,
            FastpqQuantityUnits::BATCH_IDENTITY
        );
        assert_eq!(
            AxtTransferBatch::new(
                &axt,
                &f.expected(),
                &[],
                f.context(),
                BatchContextLimits::default()
            )
            .unwrap()
            .segment(0)
            .unwrap()
            .schema()
            .identity,
            FastpqQuantityUnits::AXT_BATCH_IDENTITY
        );
        assert_eq!(air.schema().width, 342);
        assert_eq!(air.schema().trace_rows, 65536);
    }
}

#[test]
fn nominal_quantity_context_is_canonical_and_changes_with_complete_claims() {
    let (mut f, private) = QuantityFixture::new(QuantityCase::MixedScale, 1);
    drop(private);
    let bytes = encode_context(&f.prepare(ProofSemantics::StateTransition), &f.expected()).unwrap();
    for flags in [0, 1, 2, 3, norito::core::default_encode_flags()] {
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            encode_context(&f.prepare(ProofSemantics::StateTransition), &f.expected()).unwrap(),
            bytes
        );
    }
    f.claims[0].authority_digest = Hash::new(b"different exact public execution authority");
    assert_ne!(
        encode_context(&f.prepare(ProofSemantics::StateTransition), &f.expected()).unwrap(),
        bytes
    );
    assert_ne!(
        encode_context(&f.prepare(ProofSemantics::AxtTransferClaim), &f.expected()).unwrap(),
        bytes
    );
}

#[test]
fn all_seven_expected_inputs_are_checked_before_single_proof_decode() {
    let (f, private) = QuantityFixture::new(QuantityCase::Maximum, 1);
    drop(private);
    let expected = f.expected();
    for index in 0..7 {
        let mut wrong = expected;
        match index {
            0 => wrong.dsid[0] ^= 1,
            1 => wrong.slot ^= 1,
            2 => wrong.old_root[0] ^= 1,
            3 => wrong.new_root[0] ^= 1,
            4 => wrong.perm_root[0] ^= 1,
            5 => wrong.tx_set_hash[0] ^= 1,
            _ => wrong.ordering_hash[0] ^= 1,
        }
        let ordinary = f.prepare(ProofSemantics::StateTransition);
        let axt = f.prepare(ProofSemantics::AxtTransferClaim);
        assert!(matches!(
            compact_public_api::verify_shake_transfer(
                &ordinary,
                &wrong,
                &[],
                VerifyLimits::default(),
                1
            ),
            Err(Error::PublicIoMismatch { .. })
        ));
        assert!(matches!(
            compact_public_api::verify_shake_axt_transfer(
                &axt,
                &wrong,
                f.context(),
                &[],
                VerifyLimits::default(),
                1
            ),
            Err(Error::PublicIoMismatch { .. })
        ));
    }
}

#[test]
fn quantity_route_and_remote_claim_mismatches_reject_before_proof_decode() {
    let (mut f, private) = QuantityFixture::new(QuantityCase::Maximum, 1);
    drop(private);
    let ordinary = f.prepare(ProofSemantics::StateTransition);
    let axt = f.prepare(ProofSemantics::AxtTransferClaim);
    assert!(matches!(
        compact_public_api::verify_transfer(&axt, &f.expected(), &[], VerifyLimits::default()),
        Err(Error::InvalidProofSemantics { .. })
    ));
    assert!(matches!(
        compact_public_api::verify_axt_transfer(
            &ordinary,
            &f.expected(),
            f.context(),
            &[],
            VerifyLimits::default()
        ),
        Err(Error::InvalidProofSemantics { .. })
    ));
    assert!(
        PublicTransferBatch::new(&axt, &f.expected(), &[], BatchContextLimits::default()).is_err()
    );
    assert!(
        AxtTransferBatch::new(
            &ordinary,
            &f.expected(),
            &[],
            f.context(),
            BatchContextLimits::default()
        )
        .is_err()
    );
    f.axt.remote.as_mut().unwrap()[0].effective_amount = Quantity::one();
    assert!(
        AxtTransferAir::new(
            &f.prepare(ProofSemantics::AxtTransferClaim),
            &f.expected(),
            &f.axt.binding,
            f.axt.metadata(),
            f.axt.outer,
            f.axt.remote.as_deref()
        )
        .is_err()
    );
}

#[test]
fn complete_quantity_bundle_preserves_intermediate_order_and_exact_context_caps() {
    let (f, private) = QuantityFixture::new(QuantityCase::MixedScale, 3);
    let roots: Vec<_> = private.pairs()[..2]
        .iter()
        .map(|pair| pair[1].root_after)
        .collect();
    drop(private);
    let ordinary = f.prepare(ProofSemantics::StateTransition);
    let axt = f.prepare(ProofSemantics::AxtTransferClaim);
    let batch = PublicTransferBatch::new(
        &ordinary,
        &f.expected(),
        &roots,
        BatchContextLimits::default(),
    )
    .unwrap();
    let abatch = AxtTransferBatch::new(
        &axt,
        &f.expected(),
        &roots,
        f.context(),
        BatchContextLimits::default(),
    )
    .unwrap();
    for i in 0..3 {
        assert_ne!(
            batch.segment(i).unwrap().statement_bytes(),
            abatch.segment(i).unwrap().statement_bytes()
        );
    }
    let exact = BatchContextLimits {
        max_segments: 3,
        max_total_statement_bytes: batch.total_statement_bytes(),
    };
    assert!(PublicTransferBatch::new(&ordinary, &f.expected(), &roots, exact).is_ok());
    assert!(
        PublicTransferBatch::new(
            &ordinary,
            &f.expected(),
            &roots,
            BatchContextLimits {
                max_total_statement_bytes: exact.max_total_statement_bytes - 1,
                ..exact
            }
        )
        .is_err()
    );
    let exact_axt = BatchContextLimits {
        max_total_statement_bytes: abatch.total_statement_bytes(),
        ..exact
    };
    assert!(AxtTransferBatch::new(&axt, &f.expected(), &roots, f.context(), exact_axt).is_ok());
    assert!(
        AxtTransferBatch::new(
            &axt,
            &f.expected(),
            &roots,
            f.context(),
            BatchContextLimits {
                max_total_statement_bytes: exact_axt.max_total_statement_bytes - 1,
                ..exact_axt
            }
        )
        .is_err()
    );
    assert!(PublicTransferBatch::new(&ordinary, &f.expected(), &roots[..1], exact).is_err());
    assert_ne!(
        batch.context_bytes(),
        PublicTransferBatch::new(&ordinary, &f.expected(), &[roots[1], roots[0]], exact)
            .unwrap()
            .context_bytes()
    );
    assert!(batch.segment(3).is_err());
    assert!(abatch.segment(3).is_err());
}

#[test]
fn model_bridge_selects_quantity_decoder_and_preserves_complete_public_context() {
    let (f, private) = QuantityFixture::new(QuantityCase::MixedScale, 1);
    drop(private);
    let model = f.model();
    let original =
        encode_context(&f.prepare(ProofSemantics::StateTransition), &f.expected()).unwrap();
    let converted = with_prepared_quantity_statement(
        &model,
        &f.expected(),
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
        |prepared| encode_context(prepared, &f.expected()),
    )
    .unwrap();
    assert_eq!(converted, original);
    assert!(
        with_prepared_statement::<()>(
            &model,
            &f.expected(),
            ProofSemantics::StateTransition,
            PublicTransferLimits::default(),
            |_| panic!("wide bytes reached narrow callback")
        )
        .is_err()
    );
    let mut wrong = f.expected();
    wrong.ordering_hash[0] ^= 1;
    assert!(
        with_prepared_quantity_statement::<()>(
            &model,
            &wrong,
            ProofSemantics::StateTransition,
            PublicTransferLimits::default(),
            |_| panic!("wrong expected inputs reached callback")
        )
        .is_err()
    );
    let narrow_fixture = Fixture::new(false);
    let narrow = narrow_fixture.prepare(ProofSemantics::StateTransition);
    assert!(
        FastpqQuantityUnits::prepare(
            narrow.transitions(),
            narrow.claims(),
            *narrow.public_inputs(),
            narrow.semantics(),
            PublicTransferLimits::default()
        )
        .is_err()
    );
    assert!(
        u64::prepare(
            narrow.transitions(),
            narrow.claims(),
            *narrow.public_inputs(),
            narrow.semantics(),
            PublicTransferLimits::default()
        )
        .is_ok()
    );
}

#[test]
fn high_limb_and_zero_or_nonzero_scale_changes_cannot_reach_quantity_relations() {
    for case in [QuantityCase::Maximum, QuantityCase::MixedScale] {
        let (f, private) = QuantityFixture::new(case, 1);
        drop(private);
        for which in 0..2 {
            let mut rows = f.rows.clone();
            let mut units = decode_quantity_units_v1(&rows[which].pre_value).unwrap();
            let value = units.to_quantity().unwrap();
            let scale = units.scale();
            // Zero must retain its scale domain as strictly as a nonzero value.
            let other_scale = if scale == 0 { 1 } else { 0 };
            if let Some(changed) = FastpqQuantityUnits::from_quantity(&value, other_scale) {
                rows[which].pre_value = encode_quantity_units_v1(&changed).unwrap();
                assert!(
                    prepare_quantity_public_transfers(
                        &rows,
                        &f.claims,
                        f.inputs,
                        ProofSemantics::StateTransition,
                        PublicTransferLimits::default()
                    )
                    .is_err()
                );
            }
            let mut limbs = *units.limbs();
            limbs[15] ^= 1;
            if let Some(changed) = FastpqQuantityUnits::from_limbs(limbs, scale) {
                units = changed;
            } else {
                continue;
            }
            rows = f.rows.clone();
            rows[which].pre_value = encode_quantity_units_v1(&units).unwrap();
            assert!(
                prepare_quantity_public_transfers(
                    &rows,
                    &f.claims,
                    f.inputs,
                    ProofSemantics::StateTransition,
                    PublicTransferLimits::default()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn quantity_facades_keep_proof_and_bundle_preflight_limits() {
    let (f, private) = QuantityFixture::new(QuantityCase::Tiny, 1);
    drop(private);
    let ordinary = f.prepare(ProofSemantics::StateTransition);
    let axt = f.prepare(ProofSemantics::AxtTransferClaim);
    let limits = VerifyLimits {
        max_proof_bytes: 0,
        ..VerifyLimits::default()
    };
    assert!(matches!(
        compact_public_api::verify_shake_transfer(&ordinary, &f.expected(), &[0], limits, 1),
        Err(Error::VerifierLimitExceeded {
            limit: "max_proof_bytes",
            ..
        })
    ));
    assert!(matches!(
        compact_public_api::verify_shake_axt_transfer(
            &axt,
            &f.expected(),
            f.context(),
            &[0],
            limits,
            1
        ),
        Err(Error::VerifierLimitExceeded {
            limit: "max_proof_bytes",
            ..
        })
    ));
    let limits = BundleLimits {
        max_wire_bytes: 0,
        ..BundleLimits::default()
    };
    assert!(matches!(
        compact_bundle::verify_transfer_bundle(&ordinary, &f.expected(), &[0], limits),
        Err(Error::VerifierLimitExceeded {
            limit: "max_bundle_wire_bytes",
            ..
        })
    ));
    assert!(matches!(
        compact_bundle::verify_axt_transfer_bundle(&axt, &f.expected(), f.context(), &[0], limits),
        Err(Error::VerifierLimitExceeded {
            limit: "max_bundle_wire_bytes",
            ..
        })
    ));
}

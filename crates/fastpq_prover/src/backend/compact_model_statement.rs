//! Bounded model-to-prepared transfer bridge for the compact prototype.
//!
//! Only public rows are copied; transcript occurrences are borrowed unchanged.
//! All seven advertised inputs are compared to independently supplied expectations,
//! and ordering is recomputed before the callback can construct or verify a relation.
//! This creates no proof result and grants no ledger-state or spend authority.
//! The candidate artifact adapter remains test-only and requires independent caller inputs.
//! TODO: Connect this bridge to a qualified artifact route and authenticated caller.

use iroha_data_model::fastpq::{
    FastpqOperationKind, FastpqPublicTransferStatementV1, FastpqQuantityUnits,
};

use super::compact_value_domain::CompactTransferValue;
use crate::{
    Error, OperationKind, ProofSemantics, PublicInputs, Result, StateTransition,
    gadgets::public_transfer_statement::{PreparedPublicTransfers, PublicTransferLimits},
    proof::PublicIO,
};

/// Prepare complete model facts within a scope owning the bounded converted rows.
///
/// The surrounding route selects `semantics`; no model or proof field selects it.
/// The callback cannot retain references to the temporary row table. It must still
/// validate route-specific AXT context and every child proof before returning a
/// verified result. This bridge never decodes a private legacy batch or metadata.
pub(super) fn with_prepared_statement<T>(
    statement: &FastpqPublicTransferStatementV1,
    expected: &PublicIO,
    semantics: ProofSemantics,
    limits: PublicTransferLimits,
    use_prepared: impl FnOnce(&PreparedPublicTransfers<'_>) -> Result<T>,
) -> Result<T> {
    with_prepared_statement_as::<u64, T>(statement, expected, semantics, limits, use_prepared)
}

/// Prepare QuantityValueV1 rows through the fixed full-domain public constructor.
///
/// This typed route never infers a value format from statement bytes or metadata.
/// It grants no authority and leaves the legacy narrow bridge's decoder unchanged.
pub(super) fn with_prepared_quantity_statement<T>(
    statement: &FastpqPublicTransferStatementV1,
    expected: &PublicIO,
    semantics: ProofSemantics,
    limits: PublicTransferLimits,
    use_prepared: impl FnOnce(&PreparedPublicTransfers<'_, FastpqQuantityUnits>) -> Result<T>,
) -> Result<T> {
    with_prepared_statement_as::<FastpqQuantityUnits, T>(
        statement,
        expected,
        semantics,
        limits,
        use_prepared,
    )
}

fn with_prepared_statement_as<V: CompactTransferValue, T>(
    statement: &FastpqPublicTransferStatementV1,
    expected: &PublicIO,
    semantics: ProofSemantics,
    limits: PublicTransferLimits,
    use_prepared: impl FnOnce(&PreparedPublicTransfers<'_, V>) -> Result<T>,
) -> Result<T> {
    let inputs = &statement.public_inputs;
    let advertised = PublicIO {
        dsid: inputs.dsid,
        slot: inputs.slot,
        old_root: inputs.old_root,
        new_root: inputs.new_root,
        perm_root: inputs.perm_root,
        tx_set_hash: inputs.tx_set_hash,
        ordering_hash: statement.ordering_hash,
    };
    if advertised != *expected {
        return Err(Error::PublicIoMismatch {
            field: "compact_model_public_io",
        });
    }
    for (limit, actual, max) in [
        (
            "max_public_transfer_rows",
            statement.transitions.len(),
            limits.max_rows,
        ),
        (
            "max_public_transfer_transcripts",
            statement.transcripts.len(),
            limits.max_transcripts,
        ),
    ] {
        if actual > max {
            return Err(Error::VerifierLimitExceeded { limit, actual, max });
        }
    }
    // Bound borrowed variable-length rows before making the model-to-prover copy.
    // The existing preparation then measures complete canonical public encodings,
    // validates counts/semantics and derives every key and chronological occurrence.
    let mut bytes = 0_usize;
    for row in &statement.transitions {
        for value in [&row.key, &row.pre_value, &row.post_value] {
            bytes = bytes
                .checked_add(value.len())
                .ok_or(Error::VerifierLimitExceeded {
                    limit: "max_public_transfer_bytes",
                    actual: usize::MAX,
                    max: limits.max_public_bytes,
                })?;
            if bytes > limits.max_public_bytes {
                return Err(Error::VerifierLimitExceeded {
                    limit: "max_public_transfer_bytes",
                    actual: bytes,
                    max: limits.max_public_bytes,
                });
            }
        }
    }
    let rows: Vec<_> = statement
        .transitions
        .iter()
        .map(|row| StateTransition {
            key: row.key.clone(),
            pre_value: row.pre_value.clone(),
            post_value: row.post_value.clone(),
            operation: match &row.operation {
                FastpqOperationKind::Transfer => OperationKind::Transfer,
                FastpqOperationKind::MetaSet => OperationKind::MetaSet,
                FastpqOperationKind::Mint => OperationKind::Mint,
                FastpqOperationKind::Burn => OperationKind::Burn,
                FastpqOperationKind::RoleGrant(delta) => OperationKind::RoleGrant {
                    role_id: delta.role_id,
                    permission_id: delta.permission_id,
                    epoch: delta.epoch,
                },
                FastpqOperationKind::RoleRevoke(delta) => OperationKind::RoleRevoke {
                    role_id: delta.role_id,
                    permission_id: delta.permission_id,
                    epoch: delta.epoch,
                },
            },
        })
        .collect();
    let inputs = PublicInputs {
        dsid: inputs.dsid,
        slot: inputs.slot,
        old_root: inputs.old_root,
        new_root: inputs.new_root,
        perm_root: inputs.perm_root,
        tx_set_hash: inputs.tx_set_hash,
    };
    let prepared = V::prepare(&rows, &statement.transcripts, inputs, semantics, limits)?;
    let ordering: [u8; 32] = prepared.ordering_hash().into();
    if ordering != statement.ordering_hash {
        return Err(Error::OrderingHashMismatch);
    }
    use_prepared(&prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{
        compact_axt_context::tests::Fixture,
        compact_public_batch::{BatchContextLimits, PublicTransferBatch},
    };
    use iroha_data_model::fastpq::{FastpqPublicInputs, FastpqStateTransition};
    use std::cell::Cell;

    pub(in crate::backend) fn model(
        prepared: &PreparedPublicTransfers<'_>,
    ) -> FastpqPublicTransferStatementV1 {
        let p = prepared.public_inputs();
        FastpqPublicTransferStatementV1 {
            public_inputs: FastpqPublicInputs {
                dsid: p.dsid,
                slot: p.slot,
                old_root: p.old_root,
                new_root: p.new_root,
                perm_root: p.perm_root,
                tx_set_hash: p.tx_set_hash,
            },
            ordering_hash: prepared.ordering_hash().into(),
            transitions: prepared
                .transitions()
                .iter()
                .map(|row| FastpqStateTransition {
                    key: row.key.clone(),
                    pre_value: row.pre_value.clone(),
                    post_value: row.post_value.clone(),
                    operation: match &row.operation {
                        OperationKind::Transfer => FastpqOperationKind::Transfer,
                        OperationKind::MetaSet => FastpqOperationKind::MetaSet,
                        OperationKind::Mint => FastpqOperationKind::Mint,
                        OperationKind::Burn => FastpqOperationKind::Burn,
                        OperationKind::RoleGrant {
                            role_id,
                            permission_id,
                            epoch,
                        } => FastpqOperationKind::RoleGrant(
                            iroha_data_model::fastpq::FastpqRolePermissionDelta {
                                role_id: *role_id,
                                permission_id: *permission_id,
                                epoch: *epoch,
                            },
                        ),
                        OperationKind::RoleRevoke {
                            role_id,
                            permission_id,
                            epoch,
                        } => FastpqOperationKind::RoleRevoke(
                            iroha_data_model::fastpq::FastpqRolePermissionDelta {
                                role_id: *role_id,
                                permission_id: *permission_id,
                                epoch: *epoch,
                            },
                        ),
                    },
                })
                .collect(),
            transcripts: prepared.claims().to_vec(),
        }
    }

    #[test]
    fn complete_model_roundtrip_preserves_preparation_and_owned_batch_context() {
        for count in [1, 2, 3] {
            let fixture = Fixture::multiple(count, false);
            let original = fixture.prepare(ProofSemantics::StateTransition);
            let expected = fixture.expected(&original);
            let statement = model(&original);
            let encoded = norito::encode_canonical(&statement).unwrap();
            let statement =
                norito::decode_canonical::<FastpqPublicTransferStatementV1>(&encoded).unwrap();
            let intermediates =
                vec![iroha_crypto::Hash::new(b"claimed midpoint").into(); count - 1];
            let original_context = PublicTransferBatch::new(
                &original,
                &expected,
                &intermediates,
                BatchContextLimits::default(),
            )
            .unwrap();
            let converted = with_prepared_statement(
                &statement,
                &expected,
                ProofSemantics::StateTransition,
                PublicTransferLimits::default(),
                |prepared| {
                    assert_eq!(prepared.rows(), original.rows());
                    assert_eq!(prepared.pairs(), original.pairs());
                    assert_eq!(prepared.keys(), original.keys());
                    assert_eq!(prepared.work(), original.work());
                    PublicTransferBatch::new(
                        prepared,
                        &expected,
                        &intermediates,
                        BatchContextLimits::default(),
                    )
                },
            )
            .unwrap();
            drop(statement);
            assert_eq!(converted.context_bytes(), original_context.context_bytes());
            assert_eq!(converted.segment_count(), count);
            assert_eq!(
                converted.total_statement_bytes(),
                original_context.total_statement_bytes()
            );
        }
    }

    #[test]
    fn every_model_input_is_compared_before_the_callback() {
        let fixture = Fixture::new(false);
        let original = fixture.prepare(ProofSemantics::StateTransition);
        let expected = fixture.expected(&original);
        for index in 0..7 {
            let mut statement = model(&original);
            match index {
                0 => statement.public_inputs.dsid[0] ^= 1,
                1 => statement.public_inputs.slot ^= 1,
                2 => statement.public_inputs.old_root[0] ^= 1,
                3 => statement.public_inputs.new_root[0] ^= 1,
                4 => statement.public_inputs.perm_root[0] ^= 1,
                5 => statement.public_inputs.tx_set_hash[0] ^= 1,
                6 => statement.ordering_hash[0] ^= 1,
                _ => unreachable!(),
            }
            let result = with_prepared_statement(
                &statement,
                &expected,
                ProofSemantics::StateTransition,
                PublicTransferLimits::default(),
                |_| panic!("mismatched inputs reached callback"),
            );
            assert!(matches!(
                result,
                Err::<(), _>(Error::PublicIoMismatch {
                    field: "compact_model_public_io"
                })
            ));
        }
    }

    #[test]
    fn equal_advertised_ordering_is_recomputed_and_false_facts_never_reach_callback() {
        let fixture = Fixture::multiple(2, false);
        let original = fixture.prepare(ProofSemantics::StateTransition);
        for mutation in 0..5 {
            let mut statement = model(&original);
            let mut expected = fixture.expected(&original);
            match mutation {
                0 => {
                    statement.ordering_hash[0] ^= 1;
                    expected.ordering_hash = statement.ordering_hash;
                }
                1 => statement.transitions.reverse(),
                2 => statement.transitions[0].operation = FastpqOperationKind::MetaSet,
                3 => {
                    statement.transcripts.pop();
                }
                4 => statement.transcripts.reverse(),
                _ => unreachable!(),
            }
            let called = Cell::new(false);
            let result = with_prepared_statement(
                &statement,
                &expected,
                ProofSemantics::StateTransition,
                PublicTransferLimits::default(),
                |_| {
                    called.set(true);
                    Ok(())
                },
            );
            assert!(result.is_err(), "mutation {mutation}");
            assert!(!called.get());
            if mutation == 0 {
                assert!(matches!(result, Err(Error::OrderingHashMismatch)));
            }
        }
    }

    #[test]
    fn model_row_and_byte_caps_precede_conversion_and_preserve_exact_preparation_limit() {
        let fixture = Fixture::new(false);
        let original = fixture.prepare(ProofSemantics::StateTransition);
        let statement = model(&original);
        let expected = fixture.expected(&original);
        for limit in [
            PublicTransferLimits {
                max_rows: 1,
                ..PublicTransferLimits::default()
            },
            PublicTransferLimits {
                max_transcripts: 0,
                ..PublicTransferLimits::default()
            },
            PublicTransferLimits {
                max_public_bytes: 1,
                ..PublicTransferLimits::default()
            },
            PublicTransferLimits {
                max_public_bytes: original.work().public_bytes - 1,
                ..PublicTransferLimits::default()
            },
        ] {
            let result = with_prepared_statement(
                &statement,
                &expected,
                ProofSemantics::StateTransition,
                limit,
                |_| panic!("over-budget statement reached callback"),
            );
            assert!(matches!(
                result,
                Err::<(), _>(Error::VerifierLimitExceeded { .. })
            ));
        }
        let limits = PublicTransferLimits {
            max_public_bytes: original.work().public_bytes,
            ..PublicTransferLimits::default()
        };
        with_prepared_statement(
            &statement,
            &expected,
            ProofSemantics::StateTransition,
            limits,
            |_| Ok(()),
        )
        .unwrap();
    }

    #[test]
    fn route_selects_semantics_and_ambient_layout_cannot_change_model_preparation() {
        let fixture = Fixture::new(false);
        let original = fixture.prepare(ProofSemantics::StateTransition);
        let statement = model(&original);
        let expected = fixture.expected(&original);
        for flags in (u8::MIN..=u8::MAX).filter(|&f| norito::core::validate_header_flags(f).is_ok())
        {
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            for semantics in [
                ProofSemantics::StateTransition,
                ProofSemantics::AxtTransferClaim,
            ] {
                with_prepared_statement(
                    &statement,
                    &expected,
                    semantics,
                    PublicTransferLimits::default(),
                    |prepared| {
                        assert_eq!(prepared.semantics(), semantics);
                        assert_eq!(prepared.ordering_hash(), original.ordering_hash());
                        assert_eq!(prepared.claims(), original.claims());
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(norito::core::effective_decode_flags(), Some(flags));
            }
        }
        assert!(
            with_prepared_statement(
                &statement,
                &expected,
                ProofSemantics::AxtOpaqueEffect,
                PublicTransferLimits::default(),
                |_| Ok(())
            )
            .is_err()
        );
    }
}

#[path = "compact_shake_artifact.rs"]
pub(super) mod candidate_artifact;

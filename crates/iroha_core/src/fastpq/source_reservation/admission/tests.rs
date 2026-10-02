//! Pool feasibility, retained invocation identity and fail-closed fragment controls.

use super::*;
use crate::fastpq::source_reservation::{ReservationInvariant, SourceDimension};

fn prepared(network: u32) -> PreparedSourceQuota {
    PreparedSourceQuota::new(
        FastpqSourcePolicyV1::bootstrap(),
        ExecutionOutputPolicyV1::bootstrap(),
        3,
        Hash::new(b"frozen proposal"),
        network,
    )
    .unwrap()
}

#[test]
fn bootstrap_pool_split_reserves_all_internal_calls_and_retained_mandatory_work() {
    let quota = prepared(1);
    let (ordinary, mandatory) = quota.ceilings();
    assert_eq!(ordinary.executed_entries, 1025);
    assert_eq!(mandatory.executed_entries, 64);
    assert_eq!(
        ordinary.input_transcript_bytes + mandatory.input_transcript_bytes,
        148_710_448
    );
    assert_eq!(
        ordinary.total_statement_bytes + mandatory.total_statement_bytes,
        295_146_863
    );
    assert_eq!(
        ordinary
            .max_statement_bytes
            .max(mandatory.max_statement_bytes),
        272_175
    );
    assert_eq!(quota.ordinary_usage(), SourceUsage::ZERO);
    assert_eq!(quota.mandatory_usage(), SourceUsage::ZERO);
    assert!(
        PreparedSourceQuota::new(
            FastpqSourcePolicyV1::bootstrap(),
            ExecutionOutputPolicyV1::bootstrap(),
            3,
            Hash::new(b"proposal"),
            FastpqSourcePolicyV1::bootstrap()
                .maximum_network_inputs(ExecutionOutputPolicyV1::bootstrap())
                .unwrap()
                + 1
        )
        .is_err()
    );
}

#[test]
fn exhausted_ordinary_pool_cannot_spend_mandatory_capacity() {
    let mut quota = prepared(1);
    for index in 0..1025_u32 {
        quota
            .retain_ordinary_entry(Hash::new(index.to_le_bytes()))
            .unwrap();
    }
    let before = quota.ordinary_usage();
    let mut ordinary = quota.ordinary_fragment().unwrap();
    assert!(matches!(
        ordinary.open_entry(Hash::new(b"overflow")),
        Err(ReservationError::RemainingBlock {
            dimension: SourceDimension::ExecutedEntries,
            ..
        })
    ));
    assert!(
        ordinary
            .commit_after(|| panic!("failed preparation cannot apply State"))
            .is_err()
    );
    assert_eq!(quota.ordinary_usage(), before);
    let mut mandatory = quota.mandatory_fragment().unwrap();
    mandatory
        .open_entry(Hash::new(b"mandatory release"))
        .unwrap();
    mandatory.commit_after(|| Ok(())).unwrap();
    assert_eq!(quota.mandatory_usage().executed_entries, 1);
    assert_eq!(quota.ordinary_usage(), before);
}

#[test]
fn producer_e_survives_discard_and_reopen_does_not_count_fee_fragments_twice() {
    let mut quota = prepared(1);
    let hash = Hash::new(b"authenticated Network call");
    let owner = quota.retain_ordinary_entry(hash).unwrap();
    let before = quota.ordinary_usage();
    {
        let mut business = quota.ordinary_fragment().unwrap();
        business.replace_bundle(&owner, []).unwrap();
        business
            .open_entry(Hash::new(b"uncommitted purpose"))
            .unwrap();
    }
    assert_eq!(quota.ordinary_usage(), before);
    quota.retain_ordinary_entry(hash).unwrap();
    assert_eq!(quota.ordinary_usage(), before);
    let mut fee = quota.ordinary_fragment().unwrap();
    fee.replace_bundle(&owner, []).unwrap();
    fee.commit_after(|| Ok(())).unwrap();
    assert_eq!(quota.ordinary_usage(), before);
}

#[test]
fn pool_and_ledger_identity_reject_foreign_owners_even_with_identical_context() {
    let mut first = prepared(1);
    let owner = first.retain_ordinary_entry(Hash::new(b"call")).unwrap();
    let mut second = prepared(1);
    for fragment in [
        first.mandatory_fragment().unwrap(),
        second.ordinary_fragment().unwrap(),
    ] {
        let mut fragment = fragment;
        assert!(matches!(
            fragment.replace_bundle(&owner, []),
            Err(EntryBundleReservationError::Reservation(
                ReservationError::Invariant(ReservationInvariant::ForeignLedger)
            ))
        ));
        assert!(
            fragment
                .commit_after(|| panic!("foreign owner cannot commit"))
                .is_err()
        );
    }
    assert_eq!(second.ordinary_usage(), SourceUsage::ZERO);
    assert_eq!(first.mandatory_usage(), SourceUsage::ZERO);
}

#[test]
fn empty_carrier_still_reserves_internal_and_mandatory_capacity() {
    let mut quota = prepared(0);
    assert_eq!(quota.ceilings().0.executed_entries, 768);
    assert_eq!(quota.ceilings().1.executed_entries, 64);
    assert_eq!(quota.ordinary_usage(), SourceUsage::ZERO);
    let fragment = quota.ordinary_fragment().unwrap();
    assert_eq!(fragment.commit_after(|| Ok(())).unwrap(), SourceUsage::ZERO);
}

fn transcript(hash: Hash) -> TransferTranscript {
    use iroha_data_model::{
        account::AccountId,
        asset::AssetDefinitionId,
        fastpq::{TransferDeltaTranscript, TransferSmtWitness},
    };
    use iroha_model_base::domain::DomainId;
    use iroha_primitives::numeric::Quantity;
    let account = |seed| {
        AccountId::new(
            iroha_crypto::KeyPair::from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    };
    TransferTranscript {
        batch_hash: hash,
        authority_digest: Hash::new(b"authority"),
        poseidon_preimage_digest: Some(Hash::new(b"fixed width digest; shape measurement only")),
        deltas: vec![TransferDeltaTranscript {
            from_account: account(1),
            to_account: account(2),
            asset_definition: AssetDefinitionId::derive_from_components(
                DomainId::try_new("quota", "universal").unwrap(),
                "coin".parse().unwrap(),
            ),
            amount: Quantity::from(1_u32),
            from_balance_before: Quantity::from(2_u32),
            from_balance_after: Quantity::from(1_u32),
            to_balance_before: Quantity::zero(),
            to_balance_after: Quantity::from(1_u32),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        }],
    }
}

#[test]
fn physical_transaction_cannot_mint_an_ordinary_invocation_or_mandatory_authority() {
    let mut quota = prepared(1);
    let hash = Hash::new(b"unadmitted");
    let bundle = [transcript(hash)];
    for protocol in [false, true] {
        let mut attempt = quota.transaction().unwrap();
        assert!(attempt.replace_entry(hash, protocol, &bundle).is_err());
        assert!(attempt.intrinsic_rejected().is_err());
        assert!(!attempt.allows_apply());
    }
    assert_eq!(quota.ordinary_usage(), SourceUsage::ZERO);
    assert_eq!(quota.mandatory_usage(), SourceUsage::ZERO);
}

#[test]
fn dual_journal_drop_restores_both_pools_while_producer_e_survives() {
    let mut quota = prepared(1);
    let ordinary = Hash::new(b"ordinary");
    let mandatory = Hash::new(b"mandatory");
    quota.retain_ordinary_entry(ordinary).unwrap();
    let before = quota.ordinary_usage();
    {
        let mut attempt = quota.transaction().unwrap();
        attempt.authorize_governance_purposes();
        attempt
            .replace_entry(ordinary, false, &[transcript(ordinary)])
            .unwrap();
        attempt
            .replace_entry(mandatory, true, &[transcript(mandatory)])
            .unwrap();
        attempt.poison();
        assert!(!attempt.allows_apply());
    }
    assert_eq!(quota.ordinary_usage(), before);
    assert_eq!(quota.mandatory_usage(), SourceUsage::ZERO);
    let mut attempt = quota.transaction().unwrap();
    attempt.authorize_governance_purposes();
    attempt
        .replace_entry(ordinary, false, &[transcript(ordinary)])
        .unwrap();
    attempt
        .replace_entry(mandatory, true, &[transcript(mandatory)])
        .unwrap();
    attempt.commit();
    for usage in [quota.ordinary_usage(), quota.mandatory_usage()] {
        assert_eq!(
            (usage.executed_entries, usage.transcripts, usage.deltas),
            (1, 1, 1)
        );
        assert_eq!(usage.max_statement_bytes, usage.total_statement_bytes);
    }
}

#[test]
fn exact_intrinsic_boundary_is_a_rejection_only_for_ordinary_work() {
    let mut quota = prepared(1);
    let hash = Hash::new(b"intrinsic boundary");
    quota.retain_ordinary_entry(hash).unwrap();
    let mut bundle = vec![transcript(hash); 16];
    {
        let mut attempt = quota.transaction().unwrap();
        attempt.replace_entry(hash, false, &bundle).unwrap();
        assert_eq!(attempt.intrinsic_rejected(), Ok(false));
        bundle.push(transcript(hash));
        assert_eq!(
            attempt.replace_entry(hash, false, &bundle).unwrap_err(),
            SOURCE_INTRINSIC_REJECTION
        );
        assert_eq!(attempt.intrinsic_rejected(), Ok(true));
        assert!(!attempt.allows_apply());
    }
    assert_eq!(quota.ordinary_usage().executed_entries, 1);
    assert_eq!(quota.ordinary_usage().transcripts, 0);
    let mut attempt = quota.transaction().unwrap();
    attempt.authorize_governance_purposes();
    assert!(attempt.replace_entry(hash, true, &bundle).is_err());
    assert!(attempt.intrinsic_rejected().is_err());
}

#[test]
fn final_archive_reconciles_empty_calls_and_complete_original_paths() {
    use iroha_data_model::fastpq::{
        FastpqSourceExecutionEntryV1, FastpqSourceExecutionKindV1, FastpqSourceRouteV1,
    };
    let mut quota = prepared(1);
    let empty = Hash::new(b"empty invoked call");
    let hash = Hash::new(b"applied call");
    for identity in [empty, hash] {
        quota.retain_ordinary_entry(identity).unwrap();
    }
    let mut transcripts = BTreeMap::from([(hash, vec![transcript(hash)])]);
    let mut attempt = quota.transaction().unwrap();
    attempt
        .replace_entry(hash, false, &transcripts[&hash])
        .unwrap();
    attempt.commit();
    let entries = [empty, hash].map(|entry_hash| FastpqSourceExecutionEntryV1 {
        entry_hash,
        execution_kind: FastpqSourceExecutionKindV1::ExecutionCall,
        route: FastpqSourceRouteV1::Unrouted,
        dataspace_id: iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    });
    quota.reconcile(&entries, &transcripts).unwrap();
    assert!(quota.reconcile(&entries[1..], &transcripts).is_err());
    let original = transcripts[&hash][0].clone();
    transcripts.get_mut(&hash).unwrap()[0].deltas[0]
        .from_smt_witness
        .siblings
        .push([0; 32]);
    assert!(quota.reconcile(&entries, &transcripts).is_err());
    transcripts.insert(hash, vec![original.clone(), original]);
    assert!(quota.reconcile(&entries, &transcripts).is_err());
    assert!(quota.verify_ordinary_entries([hash]).is_err());
    quota.verify_ordinary_entries([hash, empty]).unwrap();
}

#[test]
fn unavailable_context_never_has_a_publishable_transaction() {
    let mut attempt = SourceQuotaTransaction::unavailable("malformed frozen profile".into());
    assert_eq!(
        attempt.intrinsic_rejected(),
        Err("malformed frozen profile".into())
    );
    assert!(!attempt.allows_apply());
    attempt.authorize_governance_purposes();
    assert!(
        attempt
            .replace_entry(Hash::new(b"no fallback"), true, [])
            .is_err()
    );
    assert!(!attempt.allows_apply());
}

#[test]
fn fee_admission_requires_a_fresh_retained_ordinary_entry_without_mutation() {
    let mut quota = prepared(1);
    let hash = Hash::new(b"fee entry");
    {
        let attempt = quota.transaction().unwrap();
        assert!(attempt.require_empty_ordinary_entry(hash).is_err());
        assert!(attempt.allows_apply());
    }
    quota.retain_ordinary_entry(hash).unwrap();
    let before = quota.ordinary_usage();
    {
        let mut attempt = quota.transaction().unwrap();
        attempt.require_empty_ordinary_entry(hash).unwrap();
        attempt
            .replace_entry(hash, false, &[transcript(hash)])
            .unwrap();
        assert!(attempt.require_empty_ordinary_entry(hash).is_err());
        attempt.poison();
        assert!(attempt.require_empty_ordinary_entry(hash).is_err());
    }
    assert_eq!(quota.ordinary_usage(), before);
    let attempt = SourceQuotaTransaction::unavailable("no source journal".into());
    assert!(attempt.require_empty_ordinary_entry(hash).is_err());
}

#[test]
fn candidate_quantity_capture_can_only_inspect_retained_invocations() {
    let mut quota = prepared(1);
    let ordinary = Hash::new(b"retained ordinary producer");
    let mandatory = Hash::new(b"retained mandatory purpose");
    let absent = Hash::new(b"hash does not grant an invocation");
    quota.retain_ordinary_entry(ordinary).unwrap();
    let mut retained = quota.mandatory_fragment().unwrap();
    retained.open_entry(mandatory).unwrap();
    retained.commit_after(|| Ok(())).unwrap();
    let before = (quota.ordinary_usage(), quota.mandatory_usage());
    {
        let mut transaction = quota.transaction().unwrap();
        transaction
            .require_existing_quantity_capture_entry(ordinary, false)
            .unwrap();
        assert!(
            transaction
                .require_existing_quantity_capture_entry(absent, false)
                .is_err()
        );
        assert!(
            transaction
                .require_existing_quantity_capture_entry(mandatory, true)
                .is_err()
        );
        transaction.authorize_governance_purposes();
        transaction
            .require_existing_quantity_capture_entry(mandatory, true)
            .unwrap();
        assert!(
            transaction
                .require_existing_quantity_capture_entry(absent, true)
                .is_err()
        );
        assert!(
            transaction
                .require_existing_quantity_capture_entry(ordinary, true)
                .is_err()
        );
    }
    assert_eq!((quota.ordinary_usage(), quota.mandatory_usage()), before);
    let failed = SourceQuotaTransaction::unavailable("retained construction failure".into());
    assert!(
        failed
            .require_existing_quantity_capture_entry(ordinary, false)
            .is_err()
    );
}

fn reconciled_dual_pool() -> (
    PreparedSourceQuota,
    [iroha_data_model::fastpq::FastpqSourceExecutionEntryV1; 3],
    BTreeMap<Hash, Vec<TransferTranscript>>,
) {
    use iroha_data_model::fastpq::{
        FastpqSourceExecutionEntryV1, FastpqSourceExecutionKindV1, FastpqSourceRouteV1,
    };
    let mut quota = prepared(1);
    let empty = Hash::new(b"retained rejected invocation");
    let ordinary = Hash::new(b"retained successful invocation");
    let mandatory = Hash::new(b"applied mandatory purpose");
    for hash in [empty, ordinary] {
        quota.retain_ordinary_entry(hash).unwrap();
    }
    let transcripts = BTreeMap::from([
        (ordinary, vec![transcript(ordinary)]),
        (mandatory, vec![transcript(mandatory)]),
    ]);
    let mut transaction = quota.transaction().unwrap();
    transaction.authorize_governance_purposes();
    transaction
        .replace_entry(ordinary, false, &transcripts[&ordinary])
        .unwrap();
    transaction
        .replace_entry(mandatory, true, &transcripts[&mandatory])
        .unwrap();
    transaction.commit();
    let entries =
        [(empty, false), (ordinary, false), (mandatory, true)].map(|(entry_hash, protocol)| {
            FastpqSourceExecutionEntryV1 {
                entry_hash,
                execution_kind: if protocol {
                    FastpqSourceExecutionKindV1::ProtocolPurpose
                } else {
                    FastpqSourceExecutionKindV1::ExecutionCall
                },
                route: FastpqSourceRouteV1::Unrouted,
                dataspace_id: iroha_model_base::topology::DataSpaceId::UNIVERSAL,
            }
        });
    (quota, entries, transcripts)
}

#[test]
fn retained_dual_quota_requires_complete_reconciliation_and_original_frozen_policy() {
    let (quota, entries, transcripts) = reconciled_dual_pool();
    assert!(
        quota
            .reconcile_and_retain(&entries[1..], &transcripts)
            .is_err()
    );
    let mut missing = transcripts.clone();
    missing.remove(&entries[2].entry_hash);
    assert!(quota.reconcile_and_retain(&entries, &missing).is_err());
    let mut repeated = entries;
    repeated[1] = repeated[0];
    assert!(quota.reconcile_and_retain(&repeated, &transcripts).is_err());
    let seal = quota.reconcile_and_retain(&entries, &transcripts).unwrap();
    assert!(quota.matches_retained(&seal, quota.profile, quota.output));
    let mut wrong_profile = quota.profile;
    wrong_profile.intrinsic.max_deltas += 1;
    assert!(!quota.matches_retained(&seal, wrong_profile, quota.output));
    let mut wrong_output = quota.output;
    wrong_output.max_time_invocations += 1;
    assert!(!quota.matches_retained(&seal, quota.profile, wrong_output));
    let (reconstructed, _, _) = reconciled_dual_pool();
    assert_eq!(quota.ordinary_usage(), reconstructed.ordinary_usage());
    assert_eq!(quota.mandatory_usage(), reconstructed.mandatory_usage());
    assert!(!reconstructed.matches_retained(&seal, quota.profile, quota.output));
}

#[test]
fn retained_dual_quota_preserves_empty_and_dropped_children_but_detects_either_pool_commit() {
    for protocol in [false, true] {
        let (mut quota, entries, transcripts) = reconciled_dual_pool();
        let hash = entries[if protocol { 2 } else { 1 }].entry_hash;
        let seal = quota.reconcile_and_retain(&entries, &transcripts).unwrap();
        let before = (quota.ordinary_usage(), quota.mandatory_usage());
        {
            let mut transaction = quota.transaction().unwrap();
            transaction.authorize_governance_purposes();
            transaction
                .replace_entry(hash, protocol, &transcripts[&hash])
                .unwrap();
        }
        quota.transaction().unwrap().commit();
        assert_eq!((quota.ordinary_usage(), quota.mandatory_usage()), before);
        assert!(quota.matches_retained(&seal, quota.profile, quota.output));
        let mut transaction = quota.transaction().unwrap();
        transaction.authorize_governance_purposes();
        transaction.replace_entry(hash, protocol, []).unwrap();
        transaction
            .replace_entry(hash, protocol, &transcripts[&hash])
            .unwrap();
        transaction.commit();
        assert_eq!((quota.ordinary_usage(), quota.mandatory_usage()), before);
        assert!(!quota.matches_retained(&seal, quota.profile, quota.output));
    }
}

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
    assert_eq!(quota.native_ceiling().executed_entries, 64);
    assert_eq!(quota.native_usage(), SourceUsage::ZERO);
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
fn native_authorization_has_no_entry_until_an_applied_transcript_and_drops_atomically() {
    let mut quota = prepared(1);
    let hash = Hash::new(b"original native source permit");
    {
        let mut attempt = quota.transaction().unwrap();
        attempt.authorize_native_purpose(hash).unwrap();
        assert!(attempt.is_native_purpose(hash));
        assert!(
            attempt
                .require_existing_quantity_capture_entry(hash, true)
                .is_err()
        );
        attempt.commit();
    }
    assert_eq!(quota.native_usage(), SourceUsage::ZERO);
    {
        let mut empty = quota.transaction().unwrap();
        empty.authorize_native_purpose(hash).unwrap();
        assert!(empty.replace_entry(hash, true, std::iter::empty()).is_err());
        assert!(empty.intrinsic_rejected().is_err());
        assert!(!empty.allows_apply());
    }
    assert_eq!(quota.native_usage(), SourceUsage::ZERO);
    {
        let mut attempt = quota.transaction().unwrap();
        attempt.authorize_native_purpose(hash).unwrap();
        attempt
            .replace_entry(hash, true, &[transcript(hash)])
            .unwrap();
        attempt
            .require_existing_quantity_capture_entry(hash, true)
            .unwrap();
        attempt.poison();
    }
    assert_eq!(quota.native_usage(), SourceUsage::ZERO);
    let mut attempt = quota.transaction().unwrap();
    attempt.authorize_native_purpose(hash).unwrap();
    attempt
        .replace_entry(hash, true, &[transcript(hash)])
        .unwrap();
    attempt.commit();
    assert_eq!(
        (
            quota.native_usage().executed_entries,
            quota.native_usage().transcripts,
            quota.native_usage().deltas
        ),
        (1, 1, 1)
    );
    assert_eq!(quota.ordinary_usage(), SourceUsage::ZERO);
    assert_eq!(quota.mandatory_usage(), SourceUsage::ZERO);
    let mut repeated = quota.transaction().unwrap();
    assert!(repeated.authorize_native_purpose(hash).is_err());
}

#[test]
fn native_pool_overflow_cannot_borrow_ordinary_or_governance_reservations() {
    let mut quota = prepared(1);
    for index in 0..64_u32 {
        let hash = Hash::new(index.to_le_bytes());
        let mut attempt = quota.transaction().unwrap();
        attempt.authorize_native_purpose(hash).unwrap();
        attempt
            .replace_entry(hash, true, &[transcript(hash)])
            .unwrap();
        attempt.commit();
    }
    let before = quota.native_usage();
    let overflow = Hash::new(b"native overflow");
    {
        let mut attempt = quota.transaction().unwrap();
        attempt.authorize_native_purpose(overflow).unwrap();
        assert_eq!(
            attempt
                .replace_entry(overflow, true, &[transcript(overflow)])
                .unwrap_err(),
            SOURCE_INTRINSIC_REJECTION
        );
        assert_eq!(attempt.intrinsic_rejected(), Ok(true));
        assert!(!attempt.allows_apply());
    }
    assert_eq!(quota.native_usage(), before);
    assert_eq!(quota.ordinary_usage(), SourceUsage::ZERO);
    assert_eq!(quota.mandatory_usage(), SourceUsage::ZERO);
    quota
        .retain_ordinary_entry(Hash::new(b"Network still reserved"))
        .unwrap();
    let mut mandatory = quota.transaction().unwrap();
    mandatory.authorize_governance_purposes();
    let hash = Hash::new(b"governance still reserved");
    mandatory
        .replace_entry(hash, true, &[transcript(hash)])
        .unwrap();
    mandatory.commit();
    assert_eq!(quota.mandatory_usage().executed_entries, 1);
}

#[test]
fn native_purpose_cannot_use_foreign_identity_or_authorize_governance() {
    let mut quota = prepared(1);
    let native = Hash::new(b"native");
    let foreign = Hash::new(b"foreign purpose");
    {
        let mut attempt = quota.transaction().unwrap();
        attempt.authorize_native_purpose(native).unwrap();
        assert!(
            attempt
                .replace_entry(foreign, true, &[transcript(foreign)])
                .is_err()
        );
        assert!(attempt.intrinsic_rejected().is_err());
    }
    {
        let mut attempt = quota.transaction().unwrap();
        attempt.authorize_native_purpose(native).unwrap();
        attempt.authorize_governance_purposes();
        assert!(!attempt.allows_apply());
    }
    quota.retain_ordinary_entry(native).unwrap();
    let mut attempt = quota.transaction().unwrap();
    assert!(attempt.authorize_native_purpose(native).is_err());
}

#[test]
fn complete_native_and_governance_inventory_reconciles_only_exact_owned_purposes() {
    use iroha_data_model::fastpq::{
        FastpqSourceExecutionEntryV1, FastpqSourceExecutionKindV1, FastpqSourceRouteV1,
    };
    let mut quota = prepared(1);
    let native = Hash::new(b"native renewal");
    let governance = Hash::new(b"retained governance release");
    let bundle = BTreeMap::from([
        (native, vec![transcript(native)]),
        (governance, vec![transcript(governance)]),
    ]);
    {
        let mut attempt = quota.transaction().unwrap();
        attempt.authorize_native_purpose(native).unwrap();
        attempt
            .replace_entry(native, true, &bundle[&native])
            .unwrap();
        attempt.commit();
    }
    {
        let mut attempt = quota.transaction().unwrap();
        attempt.authorize_governance_purposes();
        attempt
            .replace_entry(governance, true, &bundle[&governance])
            .unwrap();
        attempt.commit();
    }
    let entry = |hash| FastpqSourceExecutionEntryV1 {
        entry_hash: hash,
        execution_kind: FastpqSourceExecutionKindV1::ProtocolPurpose,
        route: FastpqSourceRouteV1::Unrouted,
        dataspace_id: iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    };
    let entries = [entry(native), entry(governance)];
    quota.reconcile(&entries, &bundle).unwrap();
    let mut relabeled = entries;
    relabeled[0].execution_kind = FastpqSourceExecutionKindV1::ExecutionCall;
    assert!(quota.reconcile(&relabeled, &bundle).is_err());
    assert!(quota.reconcile(&entries[..1], &bundle).is_err());
    let unknown = Hash::new(b"unknown protocol purpose");
    let forged = BTreeMap::from([(unknown, vec![transcript(unknown)])]);
    assert!(quota.reconcile(&[entry(unknown)], &forged).is_err());
    assert!(quota.reconcile(&entries, &BTreeMap::new()).is_err());
    assert_eq!(quota.native_usage().executed_entries, 1);
    assert_eq!(quota.mandatory_usage().executed_entries, 1);
    assert_eq!(quota.ordinary_usage(), SourceUsage::ZERO);
    // Construct corrupt dual ownership through the private test-only journal,
    // then require final reconciliation to refuse that complete archive.
    {
        let mut duplicate = quota.mandatory_fragment().unwrap();
        let owner = duplicate.open_entry(native).unwrap();
        duplicate.replace_bundle(&owner, &bundle[&native]).unwrap();
        duplicate.commit_after(|| Ok(())).unwrap();
    }
    assert!(quota.reconcile(&entries, &bundle).is_err());
}

#[test]
fn native_intrinsic_overflow_rolls_back_without_poisoning_the_other_pools() {
    let mut quota = prepared(1);
    let hash = Hash::new(b"native intrinsic boundary");
    let bundle = vec![transcript(hash); 17];
    {
        let mut attempt = quota.transaction().unwrap();
        attempt.authorize_native_purpose(hash).unwrap();
        assert_eq!(
            attempt.replace_entry(hash, true, &bundle).unwrap_err(),
            SOURCE_INTRINSIC_REJECTION
        );
        assert_eq!(attempt.intrinsic_rejected(), Ok(true));
        assert!(!attempt.allows_apply());
    }
    assert_eq!(quota.native_usage(), SourceUsage::ZERO);
    assert_eq!(quota.ordinary_usage(), SourceUsage::ZERO);
    assert_eq!(quota.mandatory_usage(), SourceUsage::ZERO);
    let mut retry = quota.transaction().unwrap();
    retry.authorize_native_purpose(hash).unwrap();
    retry
        .replace_entry(hash, true, &[transcript(hash)])
        .unwrap();
    retry.commit();
    assert_eq!(quota.native_usage().transcripts, 1);
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

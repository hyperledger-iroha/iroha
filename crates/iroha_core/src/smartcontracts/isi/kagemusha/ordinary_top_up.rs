//! Distinct ordinary Mint reserve record and closed mathematical admission consumer.
//!
//! Only neutral pooled accounting is shared with other top-ups. The exact ordinary request,
//! separately World-authorized current issuer decision and complete ordinary paired proof are
//! retained. No OEM credential, hardware counter or legacy Mint authorization is reconstructed.

use super::*;
use crate::smartcontracts::isi::kagemusha::ordinary_mint_debit_admission::KagemushaWorldOrdinaryMintDebitDecisionV1;
use crate::state::WorldReadOnly as _;
use iroha_core_zk::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryMintAuthorizationV1;
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryTopUpRequestV1, KagemushaSignedOrdinaryMintDebitDecisionV1,
    kagemusha_ordinary_mint_applied_intent_digest_v1,
};
use mv::storage::StorageReadOnly as _;
use sha2::{Digest as _, Sha256};

/// Exact ordinary debit data persisted in the same actual World transaction as pooled funds.
/// Restoring this data cannot create the independently verified mathematical or issuer owner.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Decode,
    Encode,
    JsonDeserialize,
    JsonSerialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_core::smartcontracts::isi::kagemusha::KagemushaOrdinaryTopUpRecordV1"
)]
pub struct KagemushaOrdinaryTopUpRecordV1 {
    /// Sole first-release layout.
    pub version: u16,
    /// Original Native idempotency identity.
    pub operation_id: [u8; 32],
    /// Sole network, asset and exact incarnation pool.
    pub pool: KagemushaReservePoolKeyV1,
    /// Exact positive atomic units debited.
    pub amount: u128,
    /// Independently governed ordinary proof release.
    pub release_id: [u8; 32],
    /// Actual fixed asset scale.
    pub scale: u32,
    /// Pre-encryption acyclic issuance identity.
    pub issuance_commitment: [u8; 32],
    /// Actual encrypted one-use credit identity.
    pub credit_id: [u8; 32],
    /// Original signed payer; also the actual first-release recipient.
    pub payer: AccountId,
    /// Complete accepted unsigned ordinary request original, including real paired proofs.
    pub request_original: Vec<u8>,
    /// SHA256 of that whole sole canonical request original.
    pub request_original_sha256: [u8; 32],
    /// Exact operation/predecessor-bound independently World-admitted issuer decision original.
    pub issuer_decision_original: Vec<u8>,
    /// Purpose-bound complete request plus original effect-decision commitment.
    pub intent_original_digest: [u8; 32],
    /// Actual post-debit reserve receipt. Its time is supplied only by block execution.
    pub reserve_receipt: KagemushaReserveReceiptV1,
}
impl KagemushaOrdinaryTopUpRecordV1 {
    /// Decode and recheck every persisted original and neutral receipt binding.
    /// This returns data only, never a Mint proof or funding capability.
    /// # Errors
    /// Refuses unsupported, noncanonical or substituted request, decision, pool or receipt.
    pub fn validate_basic(&self) -> Result<(), KagemushaReserveErrorV1> {
        require_version(self.version)?;
        require_nonzero_operation(self.operation_id)?;
        self.pool.validate()?;
        let request =
            KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(&self.request_original)
                .map_err(KagemushaReserveErrorV1::InvalidWire)?;
        let decision = KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(
            &self.issuer_decision_original,
        )
        .map_err(KagemushaReserveErrorV1::InvalidWire)?;
        decision
            .subject
            .selection
            .validate_against_topup(&request)
            .map_err(KagemushaReserveErrorV1::InvalidWire)?;
        let context = &request.authorization.statement.context;
        let runtime = &context.lineage.owner.runtime;
        let statement = request
            .authorization
            .finalized_credit_statement(self.reserve_receipt.committed_at_ms)
            .map_err(KagemushaReserveErrorV1::InvalidWire)?;
        let statement_digest = statement
            .canonical_digest()
            .map_err(map_chain_value_error)?;
        self.reserve_receipt
            .validate()
            .map_err(map_chain_value_error)?;
        if self.intent_original_digest
            != ordinary_intent_digest(&self.request_original, &self.issuer_decision_original)
            || self.request_original_sha256
                != <[u8; 32]>::from(Sha256::digest(&self.request_original))
            || self.operation_id != context.operation_id
            || self.pool.network_id != runtime.network_id
            || self.pool.asset != runtime.asset
            || self.pool.asset_incarnation != runtime.asset_incarnation
            || self.amount != context.amount
            || self.scale != runtime.scale
            || self.release_id != context.release_id
            || self.issuance_commitment
                != context
                    .issuance_commitment()
                    .map_err(KagemushaReserveErrorV1::InvalidWire)?
            || self.credit_id != request.authorization.statement.credit_id
            || self.payer != context.lineage.owner.account_id
            || decision.subject.release_id != self.release_id
            || self.reserve_receipt.committed_at_ms < decision.subject.issued_at_ms
            || self.reserve_receipt.committed_at_ms >= decision.subject.expires_at_ms
            || !receipt_matches(
                &self.reserve_receipt,
                KagemushaOperationKindV1::TopUp,
                self.operation_id,
                self.intent_original_digest,
                statement_digest,
                &self.pool,
                self.scale,
                self.amount,
            )
        {
            return Err(state_invariant(
                "ordinary_top_up_original_or_receipt_mismatch",
            ));
        }
        Ok(())
    }
}

/// Closed actual ordinary proof and separately admitted issuer decision custody.
/// No decoder or caller-provided Boolean can create it.
pub(in crate::smartcontracts::isi) struct AdmittedOrdinaryTopUp {
    proof: KagemushaVerifiedOrdinaryMintAuthorizationV1,
    decision: KagemushaWorldOrdinaryMintDebitDecisionV1,
    request: KagemushaOrdinaryTopUpRequestV1,
}
impl AdmittedOrdinaryTopUp {
    pub(in crate::smartcontracts::isi) fn new(
        proof: KagemushaVerifiedOrdinaryMintAuthorizationV1,
        decision: KagemushaWorldOrdinaryMintDebitDecisionV1,
        transaction: &crate::state::StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> Result<Self, KagemushaReserveErrorV1> {
        let request =
            KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(proof.request_original())
                .map_err(KagemushaReserveErrorV1::InvalidWire)?;
        if proof.authorization() != &request.authorization
            || proof.request_original_sha256()
                != <[u8; 32]>::from(Sha256::digest(proof.request_original()))
            || decision.request_original_sha256() != proof.request_original_sha256()
        {
            return Err(state_invariant("ordinary_top_up_closed_originals_differ"));
        }
        decision
            .recheck(transaction, &request, authority)
            .map_err(KagemushaReserveErrorV1::InvalidWire)?;
        Ok(Self {
            proof,
            decision,
            request,
        })
    }
    pub(in crate::smartcontracts::isi) fn request(&self) -> &KagemushaOrdinaryTopUpRequestV1 {
        &self.request
    }
    pub(in crate::smartcontracts::isi) fn recheck(
        &self,
        transaction: &crate::state::StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> Result<(), KagemushaReserveErrorV1> {
        if self
            .request
            .canonical_bytes()
            .map_err(KagemushaReserveErrorV1::InvalidWire)?
            != self.proof.request_original()
            || self.decision.request_original_sha256() != self.proof.request_original_sha256()
        {
            return Err(state_invariant("ordinary_top_up_retained_originals_differ"));
        }
        self.decision
            .recheck(transaction, &self.request, authority)
            .map_err(KagemushaReserveErrorV1::InvalidWire)
    }
}

pub(in crate::smartcontracts::isi) struct OrdinaryTopUpPlan {
    expected_pool_head: Option<[u8; 32]>,
    next_pool: KagemushaReservePoolV1,
    record: KagemushaOrdinaryTopUpRecordV1,
}
impl OrdinaryTopUpPlan {
    pub(in crate::smartcontracts::isi) fn next_pool(&self) -> &KagemushaReservePoolV1 {
        &self.next_pool
    }
    pub(in crate::smartcontracts::isi) fn record(&self) -> &KagemushaOrdinaryTopUpRecordV1 {
        &self.record
    }
}
pub(in crate::smartcontracts::isi) enum OrdinaryTopUpOutcome {
    Commit(OrdinaryTopUpPlan),
    AlreadyCommitted(KagemushaOrdinaryTopUpRecordV1),
}

pub(in crate::smartcontracts::isi) fn plan_ordinary_top_up(
    admitted: &AdmittedOrdinaryTopUp,
    context: KagemushaReserveCommitContextV1,
    read: KagemushaTopUpReadSetV1<'_>,
) -> Result<OrdinaryTopUpOutcome, KagemushaReserveErrorV1> {
    context.validate()?;
    let request = admitted.request();
    let operation = request.authorization.statement.context.operation_id;
    if let Some(existing) = read.existing_operation {
        return match existing {
            KagemushaReserveOperationRecordV1::OrdinaryTopUp(record)
                if record.request_original == admitted.proof.request_original() =>
            {
                record.validate_basic()?;
                if read.credit_operation != Some(operation)
                    || read.issuance_operation != Some(operation)
                {
                    return Err(state_invariant("ordinary_top_up_reverse_index_mismatch"));
                }
                // A new signed decision never replaces the exact historical original or re-dates
                // a committed effect. Recovery has no second debit or liability increment.
                Ok(OrdinaryTopUpOutcome::AlreadyCommitted(
                    record.as_ref().clone(),
                ))
            }
            _ => Err(KagemushaReserveErrorV1::OperationConflict {
                operation_id: operation,
            }),
        };
    }
    plan_new_ordinary_top_up(
        request,
        admitted.proof.request_original(),
        admitted.decision.decision_original(),
        context,
        read,
    )
}

// Data-only deterministic kernel. Production enters only through the closed cap consumer above.
fn plan_new_ordinary_top_up(
    request: &KagemushaOrdinaryTopUpRequestV1,
    original: &[u8],
    decision_original: &[u8],
    commit: KagemushaReserveCommitContextV1,
    read: KagemushaTopUpReadSetV1<'_>,
) -> Result<OrdinaryTopUpOutcome, KagemushaReserveErrorV1> {
    commit.validate()?;
    if request
        .canonical_bytes()
        .map_err(KagemushaReserveErrorV1::InvalidWire)?
        != original
    {
        return Err(state_invariant("ordinary_top_up_request_original_differs"));
    }
    let context = &request.authorization.statement.context;
    let runtime = &context.lineage.owner.runtime;
    let statement = request
        .authorization
        .finalized_credit_statement(commit.committed_at_ms)
        .map_err(KagemushaReserveErrorV1::InvalidWire)?;
    let pool = KagemushaReservePoolKeyV1::new(
        runtime.network_id,
        runtime.asset.clone(),
        runtime.asset_incarnation,
    )?;
    ensure_entry_unbound(
        read.credit_operation,
        statement.lifecycle.credit_id,
        |credit_id| KagemushaReserveErrorV1::MintCreditConflict { credit_id },
    )?;
    ensure_entry_unbound(
        read.issuance_operation,
        statement.issuance_commitment,
        |issuance_commitment| KagemushaReserveErrorV1::IssuanceConflict {
            issuance_commitment,
        },
    )?;
    let current = read
        .current_pool
        .cloned()
        .unwrap_or_else(|| KagemushaReservePoolV1::empty(pool.clone(), runtime.scale));
    current.validate()?;
    if current.key != pool {
        return Err(state_invariant("ordinary_top_up_read_pool_differs"));
    }
    require_matching_scale(current.scale, runtime.scale)?;
    let mut next_pool = next_top_up_pool(&current, context.amount)?;
    let request_original_sha256 = Sha256::digest(original).into();
    let intent_original_digest = ordinary_intent_digest(original, decision_original);
    let reserve_receipt = receipt_for(
        KagemushaOperationKindV1::TopUp,
        context.operation_id,
        intent_original_digest,
        statement
            .canonical_digest()
            .map_err(map_chain_value_error)?,
        context.amount,
        &current,
        &next_pool,
        commit,
    )?;
    next_pool.latest_receipt = Some(reserve_receipt.clone());
    next_pool.validate()?;
    let record = KagemushaOrdinaryTopUpRecordV1 {
        version: 1,
        operation_id: context.operation_id,
        pool,
        amount: context.amount,
        release_id: context.release_id,
        scale: runtime.scale,
        issuance_commitment: statement.issuance_commitment,
        credit_id: statement.lifecycle.credit_id,
        payer: context.lineage.owner.account_id.clone(),
        request_original: original.to_vec(),
        request_original_sha256,
        issuer_decision_original: decision_original.to_vec(),
        intent_original_digest,
        reserve_receipt,
    };
    record.validate_basic()?;
    Ok(OrdinaryTopUpOutcome::Commit(OrdinaryTopUpPlan {
        expected_pool_head: current.head_digest()?,
        next_pool,
        record,
    }))
}

pub(in crate::smartcontracts::isi) fn validate_ordinary_top_up_commit(
    plan: &OrdinaryTopUpPlan,
    read: KagemushaTopUpReadSetV1<'_>,
) -> Result<Option<KagemushaOrdinaryTopUpRecordV1>, KagemushaReserveErrorV1> {
    plan.record.validate_basic()?;
    plan.next_pool.validate()?;
    if let Some(existing) = read.existing_operation {
        return match existing {
            KagemushaReserveOperationRecordV1::OrdinaryTopUp(record)
                if record.request_original == plan.record.request_original =>
            {
                record.validate_basic()?;
                if read.credit_operation != Some(record.operation_id)
                    || read.issuance_operation != Some(record.operation_id)
                {
                    return Err(state_invariant("ordinary_top_up_reverse_index_mismatch"));
                }
                Ok(Some(record.as_ref().clone()))
            }
            _ => Err(KagemushaReserveErrorV1::OperationConflict {
                operation_id: plan.record.operation_id,
            }),
        };
    }
    let current = read.current_pool.cloned().unwrap_or_else(|| {
        KagemushaReservePoolV1::empty(plan.record.pool.clone(), plan.record.scale)
    });
    current.validate()?;
    if current.head_digest()? != plan.expected_pool_head {
        return Err(KagemushaReserveErrorV1::StalePlan {
            expected: plan.expected_pool_head,
            actual: current.head_digest()?,
        });
    }
    ensure_entry_unbound(read.credit_operation, plan.record.credit_id, |credit_id| {
        KagemushaReserveErrorV1::MintCreditConflict { credit_id }
    })?;
    ensure_entry_unbound(
        read.issuance_operation,
        plan.record.issuance_commitment,
        |issuance_commitment| KagemushaReserveErrorV1::IssuanceConflict {
            issuance_commitment,
        },
    )?;
    let mut expected = next_top_up_pool(&current, plan.record.amount)?;
    expected.latest_receipt = Some(plan.record.reserve_receipt.clone());
    if expected != plan.next_pool {
        return Err(KagemushaReserveErrorV1::InvalidPlan);
    }
    Ok(None)
}

fn ordinary_intent_digest(request: &[u8], decision: &[u8]) -> [u8; 32] {
    kagemusha_ordinary_mint_applied_intent_digest_v1(request, decision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::kagemusha::{
        KagemushaOrdinaryIncomingSelectionV1, KagemushaOrdinaryIncomingSourceSelectionV1,
        KagemushaOrdinaryMintDebitDecisionV1, kagemusha_ordinary_retail_issuer_policy_digest_v1,
    };

    // Inert shared codec/proof fixture plus real known-public account/issuer equations.
    // None of these helpers returns AdmittedOrdinaryTopUp or a mathematical/debit capability.
    fn originals() -> (KagemushaOrdinaryTopUpRequestV1, Vec<u8>, Vec<u8>) {
        let f =
            iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1();
        f.request
            .verify_account_signature(&f.account_consent)
            .unwrap();
        let original = f.request.canonical_bytes().unwrap();
        let c = &f.request.authorization.statement.context;
        let selection = KagemushaOrdinaryIncomingSelectionV1 {
            version: 1,
            lineage: c.lineage.clone(),
            operation_id: c.operation_id,
            predecessor: c.predecessor.clone(),
            source: KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
                topup_request_original_sha256: Sha256::digest(&original).into(),
            },
            credit_id: f.request.authorization.statement.credit_id,
            amount: c.amount,
            scale: c.lineage.owner.runtime.scale,
            recipient_app_credential_digest: c.recipient_app_credential_digest,
            financial_control_original_sha256: c.financial_control_original_sha256,
            clock_context_digest: c.clock_context.binding_digest().unwrap(),
        };
        let mut decision_clock = c.clock_context;
        decision_clock.request_nonce = [93; 32];
        decision_clock.signed_observations_original_digest = [94; 32];
        let subject = KagemushaOrdinaryMintDebitDecisionV1 {
            version: 1,
            selection,
            issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(
                &f.enrollment_fixture.issuer_policy,
            )
            .unwrap(),
            release_id: c.release_id,
            reserved_data_record_original_sha256: [88; 32],
            reserved_data_revision: 11,
            data_incarnation_digest: [89; 32],
            data_policy_epoch: 3,
            data_schema_epoch: 2,
            authority_context_id: [90; 32],
            authority_height: 101,
            world_root: [91; 32],
            current_financial_control_original_sha256: [92; 32],
            decision_clock_context: decision_clock,
            issued_at_ms: decision_clock.lower_at_ms,
            expires_at_ms: decision_clock.upper_at_ms + 100,
        };
        // The FI issuer is distinct from the app-attestation authority (seed 61).
        let issuer = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
        assert_eq!(
            issuer.public_key(),
            &f.enrollment_fixture.issuer_policy.issuer_public_key
        );
        let signed = KagemushaSignedOrdinaryMintDebitDecisionV1 {
            signature: Signature::new(
                issuer.private_key(),
                &subject.issuer_signing_message().unwrap(),
            ),
            subject,
        };
        signed
            .verify_for_request(
                &f.request,
                &signed.subject.selection,
                &f.enrollment_fixture.issuer_policy,
            )
            .unwrap();
        (f.request, original, signed.canonical_bytes().unwrap())
    }
    fn initial_read() -> KagemushaTopUpReadSetV1<'static> {
        KagemushaTopUpReadSetV1 {
            current_pool: None,
            existing_operation: None,
            credit_operation: None,
            issuance_operation: None,
        }
    }
    fn data_plan() -> OrdinaryTopUpPlan {
        let (request, raw, decision) = originals();
        let commit =
            KagemushaReserveCommitContextV1::after_block_context_verification([70; 32], 1005)
                .unwrap();
        match plan_new_ordinary_top_up(&request, &raw, &decision, commit, initial_read()).unwrap() {
            OrdinaryTopUpOutcome::Commit(plan) => plan,
            OrdinaryTopUpOutcome::AlreadyCommitted(_) => {
                panic!("empty data set has no committed operation")
            }
        }
    }
    #[test]
    fn ordinary_top_up_data_kernel_retains_full_request_and_decision_in_receipt() {
        let plan = data_plan();
        let record = plan.record();
        record.validate_basic().unwrap();
        assert_eq!(record.amount, 177);
        assert_eq!(plan.next_pool().total_topups, record.amount);
        assert_eq!(plan.next_pool().total_redemptions, 0);
        assert_ne!(
            record.intent_original_digest,
            record.request_original_sha256
        );
        assert_eq!(
            record.reserve_receipt.request_digest,
            ordinary_intent_digest(&record.request_original, &record.issuer_decision_original)
        );
        assert!(
            validate_ordinary_top_up_commit(&plan, initial_read())
                .unwrap()
                .is_none()
        );
        let decoded: KagemushaOrdinaryTopUpRecordV1 =
            norito::decode_canonical(&norito::encode_canonical(record).unwrap()).unwrap();
        assert_eq!(decoded, *record);
        let mut changed = record.clone();
        changed.reserve_receipt.request_digest = changed.request_original_sha256;
        assert!(
            changed.validate_basic().is_err(),
            "unsigned selector cannot replace full applied intent"
        );
        let mut changed = record.clone();
        changed.request_original.push(0);
        assert!(
            changed.validate_basic().is_err(),
            "complete original framing stays exact"
        );
        let mut changed = record.clone();
        let mut decision = KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(
            &changed.issuer_decision_original,
        )
        .unwrap();
        decision.subject.reserved_data_record_original_sha256[0] ^= 1;
        changed.issuer_decision_original = decision.canonical_bytes().unwrap();
        assert!(
            changed.validate_basic().is_err(),
            "another effect decision cannot replace receipt original"
        );
    }
    #[test]
    fn ordinary_top_up_data_kernel_preserves_original_retry_and_rejects_races() {
        let plan = data_plan();
        let record =
            KagemushaReserveOperationRecordV1::OrdinaryTopUp(Box::new(plan.record().clone()));
        let operation = plan.record().operation_id;
        let read = KagemushaTopUpReadSetV1 {
            current_pool: Some(plan.next_pool()),
            existing_operation: Some(&record),
            credit_operation: Some(operation),
            issuance_operation: Some(operation),
        };
        assert_eq!(
            validate_ordinary_top_up_commit(&plan, read).unwrap(),
            Some(plan.record().clone())
        );
        let missing_index = KagemushaTopUpReadSetV1 {
            issuance_operation: None,
            ..read
        };
        assert!(validate_ordinary_top_up_commit(&plan, missing_index).is_err());
        let changed_head = KagemushaTopUpReadSetV1 {
            current_pool: Some(plan.next_pool()),
            existing_operation: None,
            credit_operation: None,
            issuance_operation: None,
        };
        assert!(matches!(
            validate_ordinary_top_up_commit(&plan, changed_head),
            Err(KagemushaReserveErrorV1::StalePlan { .. })
        ));
        assert!(
            validate_ordinary_top_up_commit(
                &plan,
                KagemushaTopUpReadSetV1 {
                    credit_operation: Some([71; 32]),
                    ..initial_read()
                }
            )
            .is_err()
        );
        assert!(
            validate_ordinary_top_up_commit(
                &plan,
                KagemushaTopUpReadSetV1 {
                    issuance_operation: Some([72; 32]),
                    ..initial_read()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn ordinary_top_up_data_kernel_requires_original_effect_window() {
        let (request, raw, decision) = originals();
        for time in [999, 1110] {
            let commit =
                KagemushaReserveCommitContextV1::after_block_context_verification([70; 32], time)
                    .unwrap();
            assert!(
                plan_new_ordinary_top_up(&request, &raw, &decision, commit, initial_read())
                    .is_err()
            );
        }
        let mut changed = request;
        changed.authorization.proof.eq_proof.push(1);
        let altered_raw = changed.canonical_bytes().unwrap();
        let commit =
            KagemushaReserveCommitContextV1::after_block_context_verification([70; 32], 1005)
                .unwrap();
        assert!(
            plan_new_ordinary_top_up(&changed, &altered_raw, &decision, commit, initial_read())
                .is_err(),
            "shape-valid different proof original cannot reuse decision"
        );
    }
}

/// Produce the complete ordinary funding source from its actual immutable World record
/// and original archived native execution at the selected committed height.
/// The returned object is public data, never an incoming financial or Mint capability.
/// # Errors
/// Refuses wrong record family, unavailable original execution, changed receipts or finality.
pub fn read_finalized_ordinary_top_up_v1(
    view: &impl crate::state::StateReadOnly,
    height: u64,
    operation_id: [u8; 32],
) -> Result<Option<iroha_data_model::kagemusha::KagemushaOrdinaryTopUpFinalizedOriginalV1>, String>
{
    use iroha_data_model::kagemusha::KagemushaOrdinaryTopUpFinalizedOriginalV1;
    let Some(record) = view
        .world()
        .kagemusha_reserve_operations()
        .get(&operation_id)
    else {
        return Ok(None);
    };
    let KagemushaReserveOperationRecordV1::OrdinaryTopUp(record) = record else {
        return Err("operation is not a distinct ordinary top-up".into());
    };
    record.validate_basic().map_err(|error| error.to_string())?;
    let finality =
        crate::query::native_receipts::kagemusha_operation_finality(view, height, operation_id)?
            .ok_or("original certified ordinary top-up receipt is absent")?;
    if finality.finality_proof.height() != height
        || finality.reserve_receipt_witness.receipt != record.reserve_receipt
    {
        return Err("ordinary top-up World/original execution differs".into());
    }
    let original = KagemushaOrdinaryTopUpFinalizedOriginalV1 {
        version: 1,
        request_original: record.request_original.clone(),
        issuer_decision_original: record.issuer_decision_original.clone(),
        finality,
    };
    let anchor = iroha_data_model::isi::kagemusha_v1::KagemushaFinalityTrustAnchorV1 {
        network_id: *view.network_id(),
        checkpoint: crate::sumeragi::finality::build_checkpoint(view, height)
            .map_err(|error| error.to_string())?,
    };
    original.validate_against(&anchor)?;
    // Check the actual complete carrier budget rather than trusting component hashes.
    original.canonical_bytes()?;
    Ok(Some(original))
}

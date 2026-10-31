//! Exact original-pool custody for the entire validated parameter projection.
//!
//! This owner copies an already validated authoring projection. Its interface
//! deliberately provides no decoder, mutable projection or uncharged constructor.
//! The original ingress/decode owner must fund its source separately; freezing
//! does not retroactively admit authoring allocations.

use std::{alloc::Layout, fmt};

use iroha_allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, AllocationReservation, ChargedBuffer,
    ChargedShared,
};
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::{
    account::AccountId,
    block::consensus::{PublicLaneStakingPolicy, RootFeePolicy},
    crypto_admission_policy::CryptoAdmissionPolicyV1,
    fraud::{FraudAdmissionAttesterV1, FraudAdmissionPolicyV1},
    parameter::{CustomParametersAdmissionError, Parameters, SumeragiParameters},
    zk_validity_policy::{ZkProofValidityV1, ZkValidityPolicyV1, ZkVerifyingKeyRefV1},
};
use iroha_primitives::{
    bigint::BigIntAdmissionCloneError,
    numeric::{Numeric, Quantity},
};

use super::{Gas, GasRate, StateParameters};

mod decoded;
pub(super) use decoded::{DecodedParameterParts, ReservedParameterGraph};

/// Original local refusal; no monetary validity rule is weakened by admission.
#[derive(Debug, thiserror::Error)]
pub enum ParameterRetentionError {
    /// The exact original finite pool refused the complete requested backing.
    #[error(transparent)]
    Admission(#[from] AllocationRefusal),
    /// Original physical buffer or reservation refusal.
    #[error(transparent)]
    Buffer(#[from] iroha_allocation::PrepaidBufferError),
    /// Original physical shared-control refusal.
    #[error(transparent)]
    Shared(#[from] iroha_allocation::PrepaidSharedError),
    /// Original custom-map or self-custodial JSON admission refusal.
    #[error(transparent)]
    Custom(#[from] CustomParametersAdmissionError),
    /// Exact native mantissa layout/allocator refusal.
    #[error(transparent)]
    Numeric(#[from] BigIntAdmissionCloneError),
    /// The original codec resource error remains inspectable through this source.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// A different original pool was offered to an existing retained version.
    #[error("parameter owner belongs to a different original allocation pool")]
    ForeignPool,
    /// A sibling update replaced the exact predecessor used for preparation.
    #[error("prepared parameters refer to a different predecessor version")]
    StalePredecessor,
    /// An initialized mandatory policy cannot disappear from a successor.
    #[error("prepared parameters remove an initialized mandatory policy")]
    MissingInitializedPolicy,
    /// The exhaustive private layout plan and copy did not agree.
    #[error("parameter graph allocation plan did not match its exact copy")]
    PlanMismatch,
}

fn overflow() -> ParameterRetentionError {
    ParameterRetentionError::Admission(AllocationRefusal::DemandOverflow)
}

/// An original refusal captured while the real retention operation is observed.
#[derive(Debug, thiserror::Error)]
pub enum ParameterRetentionAttemptError {
    /// Original canonical observer identity, including ordinary invalid input.
    #[error(transparent)]
    Codec(norito::core::DecodeAttemptError),
    /// A concrete pool, allocator or source identity error requiring no codec observer.
    #[error(transparent)]
    Original(ParameterRetentionError),
}

impl ParameterRetentionAttemptError {
    /// Observe the real call before its first codec budget check.
    ///
    /// Merely replaying a saved codec error here does not authenticate it: the
    /// canonical observer rejects origins outside this actual boundary. Non-codec
    /// errors keep their original owners and are never formatted into Norito errors.
    pub(crate) fn observe<T>(
        operation: impl FnOnce() -> Result<T, ParameterRetentionError>,
    ) -> Result<T, Self> {
        let observed = norito::core::classify_decode_attempt(|| match operation() {
            Ok(value) => Ok(Ok(value)),
            Err(
                ParameterRetentionError::Codec(error)
                | ParameterRetentionError::Custom(CustomParametersAdmissionError::Codec(error)),
            ) => Err(error),
            Err(error) => Ok(Err(error)),
        });
        match observed {
            Ok(result) => result.map_err(Self::Original),
            Err(error) => Err(Self::Codec(error)),
        }
    }
}

/// One immutable complete canonical-and-projection graph, shared by MV versions.
///
/// Cloning only retains this exact shared allocation. Each custom JSON backing
/// keeps its own original charge even if a raw Json clone escapes the borrow;
/// the outer ledger never charges those independently self-owned allocations.
pub struct RetainedStateParameters(ChargedShared<ParameterGraph>);

// The exact value always dies before every backing charge, regardless of ingress.
// Copying an already owned authoring graph and decoding directly have different
// leaf custody types, but install the same immutable value and publication owner.
struct ParameterGraph {
    value: StateParameters,
    custody: ParameterGraphCustody,
}
enum ParameterGraphCustody {
    Copy(ChargedBuffer<AllocationCharge>),
    Decoded(decoded::DecodedCustody),
}
impl ParameterGraphCustody {
    fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        match self {
            Self::Copy(ledger) => {
                ledger.belongs_to(budget)
                    && ledger
                        .as_slice()
                        .iter()
                        .all(|charge| charge.belongs_to(budget))
            }
            Self::Decoded(custody) => custody.belongs_to(budget),
        }
    }
}
impl core::ops::Deref for RetainedStateParameters {
    type Target = StateParameters;
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

// Snapshot and authority writers borrow the one canonical source. There is no
// ordinary Deserialize implementation: cold restore must provide its real pool.
impl norito::SerializePayload for RetainedStateParameters {
    fn serialize(&self, encoder: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.get().serialize(encoder)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.get().encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.get().encoded_len_exact()
    }
}
impl norito::json::JsonSerialize for RetainedStateParameters {
    fn json_serialize(&self, out: &mut String) {
        self.get().json_serialize(out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        self.get().json_serialize_to(out)
    }
}

impl Clone for RetainedStateParameters {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl fmt::Debug for RetainedStateParameters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

impl PartialEq for RetainedStateParameters {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl Eq for RetainedStateParameters {}

impl RetainedStateParameters {
    /// Exact production graph shell layout for late-boundary allocator controls.
    #[cfg(test)]
    pub(crate) fn graph_allocation_layout_for_testing() -> Layout {
        ChargedShared::<ParameterGraph>::allocation_layout()
    }

    /// Admit an explicitly authored structural fixture without a runtime transition.
    /// This is limited to component tests for missing/corrupt original policy; it
    /// grants no initialization capability and does not relax prepared successors.
    ///
    /// # Errors
    /// Returns the exact original finite-pool, physical or graph-copy refusal.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn try_copy_for_testing(
        source: &StateParameters,
        budget: &AllocationBudget,
    ) -> Result<Self, ParameterRetentionError> {
        Self::try_copy(source, budget)
    }

    /// Copy the entire validated graph into this original finite execution pool.
    ///
    /// All plain allocations are prepaid before the first clone. Custom array,
    /// key and JSON custody is admitted by its concrete model owner. No original
    /// source is consumed or mutated on any refusal.
    fn try_copy(
        source: &StateParameters,
        budget: &AllocationBudget,
    ) -> Result<Self, ParameterRetentionError> {
        let mut plan = Plan::default();
        visit_graph(source, &mut |layout| plan.add(layout))?;
        let total = plan
            .bytes
            .checked_add(array::<AllocationCharge>(plan.count)?.size())
            .and_then(|bytes| {
                bytes.checked_add(ChargedShared::<ParameterGraph>::allocation_layout().size())
            })
            .ok_or_else(overflow)?;
        let mut reservation = budget.try_reserve_bytes(total)?;
        // The ledger is declared before the graph. Every partial graph therefore
        // drops before its original credits on error or unwind.
        let ledger = ChargedBuffer::from_reservation(plan.count, &mut reservation)?;
        let shell = ChargedShared::reserve_from(&mut reservation)?;
        let mut copy = CopyCustody {
            reservation,
            ledger,
        };
        let graph = copy_graph(source, budget, &mut copy)?;
        if copy.reservation.remaining_bytes() != 0 || copy.ledger.as_slice().len() != plan.count {
            return Err(ParameterRetentionError::PlanMismatch);
        }
        // visit_graph and copy_graph enumerate every plain owned leaf. The
        // private graph destroys its value before this exact immutable ledger;
        // self-custodial CustomParameters/Json leaves are deliberately excluded.
        let graph = ParameterGraph {
            value: graph,
            custody: ParameterGraphCustody::Copy(copy.ledger),
        };
        Ok(Self(shell.initialize(graph)))
    }

    /// Borrow this exact validated graph; no mutable access or decode occurs.
    pub(crate) fn get(&self) -> &StateParameters {
        &self.0.value
    }

    /// Check controls, plain graph custody, and custom/JSON custody together.
    pub(crate) fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.0.belongs_to(budget)
            && self.0.custody.belongs_to(budget)
            && self.get().parameters.custom().belongs_to(budget)
    }

    /// Exact identity of the original retained version, independent of value equality.
    pub(crate) fn same_version(&self, other: &Self) -> bool {
        ChargedShared::ptr_eq(&self.0, &other.0)
    }

    /// Share only within the original pool; independently fund a foreign destination.
    pub(crate) fn try_retain_in(
        &self,
        budget: &AllocationBudget,
    ) -> Result<Self, ParameterRetentionError> {
        if self.belongs_to(budget) {
            return Ok(self.clone());
        }
        Self::try_copy(self.get(), budget)
    }

    /// Bind a directly admitted complete successor to this exact retained version.
    /// The execution owner validates scope/liabilities and prepares events first.
    /// This transition allocates nothing and preserves both owners on refusal.
    pub(crate) fn prepare_admitted(
        &self,
        next: Self,
        budget: &AllocationBudget,
    ) -> Result<PreparedParameters, (Self, ParameterRetentionError)> {
        let error = if !self.belongs_to(budget) || !next.belongs_to(budget) {
            Some(ParameterRetentionError::ForeignPool)
        } else if (self.crypto_policy().is_some() && next.crypto_policy().is_none())
            || (self.staking_policy().is_some() && next.staking_policy().is_none())
            || (self.root_fees().is_some() && next.root_fees().is_none())
        {
            Some(ParameterRetentionError::MissingInitializedPolicy)
        } else {
            None
        };
        if let Some(error) = error {
            return Err((next, error));
        }
        Ok(PreparedParameters {
            predecessor: self.clone(),
            successor: next,
        })
    }

    /// Prepare an explicit component fixture through the exact production reader.
    ///
    /// No production path can call this convenience helper. Its input is authored
    /// by the fixture; the successor follows the same funded source-bound path.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn prepare_fixture_parameter(
        &self,
        parameter: iroha_data_model::parameter::Parameter,
        budget: &AllocationBudget,
    ) -> Result<PreparedParameters, super::ParameterIngressError> {
        self.prepare_parameter(&parameter, budget)
    }

    /// Prepare a complete validated successor before a physical MV writer is opened.
    ///
    /// This binds the exact retained predecessor, not only the enclosing block
    /// publication token: two already applied siblings must not share a stale plan.
    /// The execution owner must already have checked scope, authorization, retained
    /// liabilities and funded its decoder/event backing before calling this method.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn prepare_replace(
        &self,
        next: &StateParameters,
        budget: &AllocationBudget,
    ) -> Result<PreparedParameters, ParameterRetentionError> {
        if !self.belongs_to(budget) {
            return Err(ParameterRetentionError::ForeignPool);
        }
        if (self.get().crypto_policy().is_some() && next.crypto_policy().is_none())
            || (self.get().staking_policy().is_some() && next.staking_policy().is_none())
            || (self.get().root_fees().is_some() && next.root_fees().is_none())
        {
            return Err(ParameterRetentionError::MissingInitializedPolicy);
        }
        self.prepare_admitted(Self::try_copy(next, budget)?, budget)
            .map_err(|(next, error)| {
                drop(next);
                error
            })
    }
}

/// A fully admitted successor bound to the exact predecessor and original pool.
#[derive(Debug)]
pub struct PreparedParameters {
    predecessor: RetainedStateParameters,
    successor: RetainedStateParameters,
}

impl PreparedParameters {
    /// Borrow the fully admitted successor for scope and liability checks.
    /// This never exposes mutable backing or commits a State change.
    pub(crate) fn successor(&self) -> &StateParameters {
        self.successor.get()
    }
    /// Refuse foreign or stale custody before opening/touching the destination Cell.
    pub(crate) fn check_source(
        &self,
        current: &RetainedStateParameters,
        budget: &AllocationBudget,
    ) -> Result<(), ParameterRetentionError> {
        if !current.belongs_to(budget)
            || !self.predecessor.belongs_to(budget)
            || !self.successor.belongs_to(budget)
        {
            return Err(ParameterRetentionError::ForeignPool);
        }
        if !self.predecessor.same_version(current) {
            return Err(ParameterRetentionError::StalePredecessor);
        }
        Ok(())
    }

    /// Extract a fully funded successor after checking the retained current source.
    /// The execution owner must complete its authorization, scope and output
    /// preparation before installing this value; this method creates no authority.
    pub(crate) fn into_successor(
        self,
        current: &RetainedStateParameters,
        budget: &AllocationBudget,
    ) -> Result<RetainedStateParameters, (Self, ParameterRetentionError)> {
        if let Err(error) = self.check_source(current, budget) {
            return Err((self, error));
        }
        Ok(self.successor)
    }

    /// Install with no allocation after the source check and all execution preparation.
    /// Refusal returns the original prepared owner intact and leaves current untouched.
    pub(crate) fn install(
        self,
        current: &mut RetainedStateParameters,
        budget: &AllocationBudget,
    ) -> Result<(), (Self, ParameterRetentionError)> {
        if let Err(error) = self.check_source(current, budget) {
            return Err((self, error));
        }
        *current = self.successor;
        Ok(())
    }
}

#[derive(Default)]
struct Plan {
    count: usize,
    bytes: usize,
}
impl Plan {
    fn add(&mut self, layout: Layout) -> Result<(), ParameterRetentionError> {
        if layout.size() != 0 {
            self.count = self.count.checked_add(1).ok_or_else(overflow)?;
            self.bytes = self.bytes.checked_add(layout.size()).ok_or_else(overflow)?;
        }
        Ok(())
    }
}
fn array<T>(count: usize) -> Result<Layout, ParameterRetentionError> {
    Layout::array::<T>(count).map_err(|_| overflow())
}

type Visitor<'a> = dyn FnMut(Layout) -> Result<(), ParameterRetentionError> + 'a;
fn visit_account(
    account: &AccountId,
    visit: &mut Visitor<'_>,
) -> Result<(), ParameterRetentionError> {
    let mut result = Ok(());
    account.for_each_admission_clone_layout(|layout| {
        if result.is_ok() {
            result = visit(layout);
        }
    })?;
    result
}

fn visit_root_fee(
    policy: &RootFeePolicy,
    visit: &mut Visitor<'_>,
) -> Result<(), ParameterRetentionError> {
    visit_account(&policy.fee_sink_account_id, visit)?;
    visit_account(&policy.sponsor_vault_custody_account_id, visit)?;
    let mut result = Ok(());
    policy
        .successful_claim_fee_exempt_authorities
        .for_each_admission_clone_layout(|layout| {
            if result.is_ok() {
                result = visit(layout);
            }
        })?;
    result?;
    for quantity in [
        &policy.base_fee,
        &policy.per_byte_fee,
        &policy.per_instruction_fee,
        &policy.per_gas_unit_fee,
    ] {
        visit(quantity.admission_clone_layout()?)?;
    }
    Ok(())
}

fn visit_graph(
    source: &StateParameters,
    visit: &mut Visitor<'_>,
) -> Result<(), ParameterRetentionError> {
    visit(array::<Algorithm>(
        source.parameters.sumeragi.key_allowed_algorithms.len(),
    )?)?;
    visit(array::<u8>(source.gas.tech_account_id.len())?)?;
    visit(array::<String>(source.gas.accepted_assets.len())?)?;
    for asset in &source.gas.accepted_assets {
        visit(array::<u8>(asset.len())?)?;
    }
    visit(array::<GasRate>(source.gas.units_per_gas.len())?)?;
    for rate in &source.gas.units_per_gas {
        visit(array::<u8>(rate.asset.len())?)?;
        visit(rate.twap_local_per_xor.admission_clone_layout()?)?;
    }
    visit(array::<FraudAdmissionAttesterV1>(
        source.fraud.attesters.len(),
    )?)?;
    for attester in &source.fraud.attesters {
        visit(array::<u8>(attester.engine_id.len())?)?;
        visit(attester.public_key.retained_allocation_layout())?;
    }
    for key in [
        &source.zk.proofs.kaigi_authorization_vk,
        &source.zk.proofs.kaigi_usage_vk,
    ]
    .into_iter()
    .flatten()
    {
        visit(array::<u8>(key.backend.len())?)?;
        visit(array::<u8>(key.name.len())?)?;
    }
    if let Some(policy) = &source.root_fees {
        visit_root_fee(policy, visit)?;
    }
    if let Some(crypto) = &source.crypto {
        visit(array::<Algorithm>(crypto.allowed_algorithms().len())?)?;
    }
    if let Some(staking) = &source.staking {
        visit(staking.min_validator_stake.admission_clone_layout()?)?;
        visit_account(&staking.stake_escrow_account_id, visit)?;
        visit_account(&staking.slash_sink_account_id, visit)?;
    }
    Ok(())
}

struct CopyCustody {
    reservation: AllocationReservation,
    ledger: ChargedBuffer<AllocationCharge>,
}
impl CopyCustody {
    fn fund(&mut self, layout: Layout) -> Result<(), ParameterRetentionError> {
        if layout.size() != 0 {
            let charge = self
                .reservation
                .try_split(layout)
                .map_err(|_| ParameterRetentionError::PlanMismatch)?;
            self.ledger.push_reserved(charge);
        }
        Ok(())
    }

    #[allow(unsafe_code)]
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, ParameterRetentionError> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let values = ChargedBuffer::<T>::from_reservation(count, &mut self.reservation)?;
        // SAFETY: the exact unchanged Vec allocation moves into the graph and
        // its charge enters this preallocated ledger before any element is copied.
        // Every partial graph is destroyed before this enclosing ledger.
        let (values, charge) = unsafe { values.into_allocation_parts() };
        self.ledger.push_reserved(charge);
        Ok(values)
    }

    fn text(&mut self, source: &str) -> Result<String, ParameterRetentionError> {
        let mut bytes = self.vector::<u8>(source.len())?;
        bytes.extend_from_slice(source.as_bytes());
        String::from_utf8(bytes).map_err(|_| ParameterRetentionError::PlanMismatch)
    }

    fn account(&mut self, source: &AccountId) -> Result<AccountId, ParameterRetentionError> {
        visit_account(source, &mut |layout| self.fund(layout))?;
        source.try_clone_for_admission().map_err(Into::into)
    }

    fn numeric(&mut self, source: &Numeric) -> Result<Numeric, ParameterRetentionError> {
        self.fund(source.admission_clone_layout()?)?;
        source.try_clone_for_admission().map_err(Into::into)
    }

    fn quantity(&mut self, source: &Quantity) -> Result<Quantity, ParameterRetentionError> {
        self.fund(source.admission_clone_layout()?)?;
        source.try_clone_for_admission().map_err(Into::into)
    }

    fn key(&mut self, source: &PublicKey) -> Result<PublicKey, ParameterRetentionError> {
        self.fund(source.retained_allocation_layout())?;
        source.try_clone_for_admission().map_err(Into::into)
    }
}

fn copy_vector<T, U>(
    source: &[T],
    copy: &mut CopyCustody,
    mut element: impl FnMut(&T, &mut CopyCustody) -> Result<U, ParameterRetentionError>,
) -> Result<Vec<U>, ParameterRetentionError> {
    let mut result = copy.vector(source.len())?;
    for value in source {
        result.push(element(value, copy)?);
    }
    Ok(result)
}

fn copy_root_fees(
    source: &RootFeePolicy,
    copy: &mut CopyCustody,
) -> Result<RootFeePolicy, ParameterRetentionError> {
    let mut result = Ok(());
    source
        .successful_claim_fee_exempt_authorities
        .for_each_admission_clone_layout(|layout| {
            if result.is_ok() {
                result = copy.fund(layout);
            }
        })?;
    result?;
    Ok(RootFeePolicy {
        fee_asset_id: source.fee_asset_id.clone(),
        fee_sink_account_id: copy.account(&source.fee_sink_account_id)?,
        sponsor_vault_custody_account_id: copy.account(&source.sponsor_vault_custody_account_id)?,
        successful_claim_fee_exempt_authorities: source
            .successful_claim_fee_exempt_authorities
            .try_clone_for_admission()?,
        base_fee: copy.quantity(&source.base_fee)?,
        per_byte_fee: copy.quantity(&source.per_byte_fee)?,
        per_instruction_fee: copy.quantity(&source.per_instruction_fee)?,
        per_gas_unit_fee: copy.quantity(&source.per_gas_unit_fee)?,
    })
}

fn copy_key_ref(
    source: &ZkVerifyingKeyRefV1,
    copy: &mut CopyCustody,
) -> Result<ZkVerifyingKeyRefV1, ParameterRetentionError> {
    Ok(ZkVerifyingKeyRefV1 {
        backend: copy.text(&source.backend)?,
        name: copy.text(&source.name)?,
    })
}

fn copy_graph(
    source: &StateParameters,
    budget: &AllocationBudget,
    copy: &mut CopyCustody,
) -> Result<StateParameters, ParameterRetentionError> {
    let consensus = &source.parameters.sumeragi;
    let parameters = Parameters {
        sumeragi: SumeragiParameters {
            block_cadence_ms: consensus.block_cadence_ms,
            max_clock_drift_ms: consensus.max_clock_drift_ms,
            due_work_max_lag_ms: consensus.due_work_max_lag_ms,
            key_activation_lead_blocks: consensus.key_activation_lead_blocks,
            key_overlap_grace_blocks: consensus.key_overlap_grace_blocks,
            key_expiry_grace_blocks: consensus.key_expiry_grace_blocks,
            key_allowed_algorithms: copy_vector(
                &consensus.key_allowed_algorithms,
                copy,
                |algorithm, _| Ok(*algorithm),
            )?,
            payload_retry_interval_ms: consensus.payload_retry_interval_ms,
            exec_budget_ms: consensus.exec_budget_ms,
            apply_budget_ms: consensus.apply_budget_ms,
            max_block_bytes: consensus.max_block_bytes,
            epoch_length_blocks: consensus.epoch_length_blocks,
            demotion_window: consensus.demotion_window,
        },
        block: source.parameters.block,
        transaction: source.parameters.transaction,
        executor: source.parameters.executor,
        smart_contract: source.parameters.smart_contract,
        custom: source.parameters.custom().try_retain_in(budget)?,
    };
    let gas = Gas {
        tech_account_id: copy.text(&source.gas.tech_account_id)?,
        accepted_assets: copy_vector(&source.gas.accepted_assets, copy, |asset, copy| {
            copy.text(asset)
        })?,
        units_per_gas: copy_vector(&source.gas.units_per_gas, copy, |rate, copy| {
            Ok(GasRate {
                asset: copy.text(&rate.asset)?,
                units_per_gas: rate.units_per_gas,
                twap_local_per_xor: copy.numeric(&rate.twap_local_per_xor)?,
                liquidity: rate.liquidity,
                volatility: rate.volatility,
            })
        })?,
    };
    let fraud = FraudAdmissionPolicyV1 {
        version: source.fraud.version,
        enabled: source.fraud.enabled,
        required_minimum_band: source.fraud.required_minimum_band,
        missing_assessment_permitted: source.fraud.missing_assessment_permitted,
        attesters: copy_vector(&source.fraud.attesters, copy, |attester, copy| {
            Ok(FraudAdmissionAttesterV1 {
                engine_id: copy.text(&attester.engine_id)?,
                public_key: copy.key(&attester.public_key)?,
            })
        })?,
    };
    let proofs = &source.zk.proofs;
    let zk = ZkValidityPolicyV1 {
        pipa_r: source.zk.pipa_r,
        stark: source.zk.stark,
        sccp: source.zk.sccp,
        proofs: ZkProofValidityV1 {
            max_verify_batch: proofs.max_verify_batch,
            ballot_history_cap: proofs.ballot_history_cap,
            preverify_max_bytes: proofs.preverify_max_bytes,
            preverify_budget_bytes: proofs.preverify_budget_bytes,
            proof_history_cap: proofs.proof_history_cap,
            proof_retention_grace_blocks: proofs.proof_retention_grace_blocks,
            proof_prune_batch: proofs.proof_prune_batch,
            bridge_proof_max_range_len: proofs.bridge_proof_max_range_len,
            bridge_proof_max_past_age_blocks: proofs.bridge_proof_max_past_age_blocks,
            bridge_proof_max_future_drift_blocks: proofs.bridge_proof_max_future_drift_blocks,
            poseidon_params_id: proofs.poseidon_params_id,
            pedersen_params_id: proofs.pedersen_params_id,
            kaigi_authorization_vk: proofs
                .kaigi_authorization_vk
                .as_ref()
                .map(|key| copy_key_ref(key, copy))
                .transpose()?,
            kaigi_usage_vk: proofs
                .kaigi_usage_vk
                .as_ref()
                .map(|key| copy_key_ref(key, copy))
                .transpose()?,
        },
        confidential: source.zk.confidential,
        gas: source.zk.gas,
    };
    let root_fees = source
        .root_fees
        .as_ref()
        .map(|policy| copy_root_fees(policy, copy))
        .transpose()?;
    let crypto = source
        .crypto
        .as_ref()
        .map(|crypto| {
            let algorithms = copy_vector(crypto.allowed_algorithms(), copy, |algorithm, _| {
                Ok(*algorithm)
            })?;
            CryptoAdmissionPolicyV1::new(algorithms)
                .map_err(|_| ParameterRetentionError::PlanMismatch)
        })
        .transpose()?;
    let staking = source
        .staking
        .as_ref()
        .map(|policy| {
            Ok::<_, ParameterRetentionError>(PublicLaneStakingPolicy {
                min_validator_stake: copy.quantity(&policy.min_validator_stake)?,
                max_stake_shares_per_validator: policy.max_stake_shares_per_validator,
                max_pending_unbonds_per_share: policy.max_pending_unbonds_per_share,
                unbonding_delay_ms: policy.unbonding_delay_ms,
                max_slash_bps: policy.max_slash_bps,
                stake_asset_id: policy.stake_asset_id.clone(),
                stake_escrow_account_id: copy.account(&policy.stake_escrow_account_id)?,
                slash_sink_account_id: copy.account(&policy.slash_sink_account_id)?,
            })
        })
        .transpose()?;
    Ok(StateParameters {
        parameters,
        gas,
        pipeline: source.pipeline,
        fraud,
        zk,
        root_fees,
        crypto,
        staking,
    })
}

#[cfg(test)]
pub(super) mod tests;

//! Authoritative deterministic readiness evaluation for mandatory offline cash.
//!
//! The evaluator lives in Core so startup, signed-genesis staging, configuration
//! checks, status, and HTTP readiness can all inspect the same snapshot with the
//! same rules. It performs no filesystem or network I/O: callers must install an
//! already authenticated ABI-21/V4 release catalog on the evaluated state.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error as StdError,
    fmt,
};

use iroha_config::parameters::actual;
use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    asset::{AssetDefinitionAlias, AssetDefinitionId, AssetId},
    block::BlockHeader,
    offline::{
        KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1, KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
        KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4, KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2,
        KAGEMUSHA_VERIFIER_NAMESPACE, KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4,
        KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4, KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2,
        KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2, KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2,
        OFFLINE_ASSET_ENABLED_METADATA_KEY, OfflineActiveTransferVerifier,
        OfflineAuthenticatedArtifactSet, OfflineReadiness, OfflineReadinessBlocker, OfflineStatus,
        OfflineVerifierId, kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v4,
        kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v4,
    },
    peer::PeerId,
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;

use crate::{
    block::ValidBlock,
    smartcontracts::isi::offline::{
        KagemushaAuthenticatedArtifactSetReadinessV4, KagemushaRecursiveReadinessV4,
        KagemushaRecursiveVerifierReadinessV4, KagemushaReleaseCatalogV4,
        ensure_kagemusha_active_release_material_v4, resolve_kagemusha_recursive_readiness_v4,
    },
    state::{StateBlock, StateReadOnly, StateView, WorldReadOnly, ZkAssetVerifierBinding},
};

/// Public, secret-free command-issuer policy used by readiness evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineIssuerReadinessPolicy {
    authority: AccountId,
    signing_public_key: PublicKey,
    minimum_fee_asset_balance: Quantity,
}

impl OfflineIssuerReadinessPolicy {
    /// Account that must submit offline top-up and redemption commands.
    #[must_use]
    pub fn authority(&self) -> &AccountId {
        &self.authority
    }

    /// Exact public key whose private counterpart signs command transactions.
    #[must_use]
    pub fn signing_public_key(&self) -> &PublicKey {
        &self.signing_public_key
    }

    /// Minimum balance required in the live fee asset.
    #[must_use]
    pub fn minimum_fee_asset_balance(&self) -> &Quantity {
        &self.minimum_fee_asset_balance
    }
}

/// Immutable operator-reviewed inputs for mandatory offline readiness.
///
/// This deliberately excludes the command private key and release filesystem
/// paths. The authenticated release catalog is taken from the evaluated state,
/// while the operator escrow bindings remain immutable for replay comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MandatoryOfflinePolicy {
    chain_id: ChainId,
    configured_escrow_accounts: BTreeMap<AssetDefinitionId, AccountId>,
    issuer: OfflineIssuerReadinessPolicy,
}

impl MandatoryOfflinePolicy {
    /// Chain whose staged or committed state may be evaluated by this policy.
    #[must_use]
    pub fn chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    /// Immutable operator-reviewed escrow bindings.
    #[must_use]
    pub fn configured_escrow_accounts(&self) -> &BTreeMap<AssetDefinitionId, AccountId> {
        &self.configured_escrow_accounts
    }

    /// Secret-free command issuer requirements.
    #[must_use]
    pub fn issuer(&self) -> &OfflineIssuerReadinessPolicy {
        &self.issuer
    }
}

/// Invalid mandatory-offline operator configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MandatoryOfflineConfigurationError {
    message: String,
}

impl MandatoryOfflineConfigurationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Deterministic configuration failure text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for MandatoryOfflineConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl StdError for MandatoryOfflineConfigurationError {}

/// A staged or committed state cannot be represented as one exact snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineSnapshotError {
    message: String,
}

impl OfflineSnapshotError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Deterministic snapshot failure text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for OfflineSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl StdError for OfflineSnapshotError {}

/// Complete failure returned when a mandatory-offline status is not ready.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MandatoryOfflineReadinessError {
    blockers: Vec<OfflineReadinessBlocker>,
}

impl MandatoryOfflineReadinessError {
    /// Deterministically ordered blockers across the fleet and every asset.
    #[must_use]
    pub fn blockers(&self) -> &[OfflineReadinessBlocker] {
        &self.blockers
    }
}

impl fmt::Display for MandatoryOfflineReadinessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("mandatory offline readiness failed")?;
        for blocker in &self.blockers {
            write!(formatter, "; {}: {}", blocker.code, blocker.message)?;
        }
        Ok(())
    }
}

impl StdError for MandatoryOfflineReadinessError {}

/// Validate operator configuration and retain only secret-free readiness input.
///
/// Filesystem authentication remains the caller's responsibility. This check
/// requires both non-empty paths so no caller can intentionally substitute an
/// empty release catalog.
///
/// # Errors
///
/// Returns an error when escrow, release, decoded-budget, or issuer
/// configuration is absent or malformed.
pub fn mandatory_offline_policy_from_config(
    chain_id: &ChainId,
    offline: &actual::Offline,
    commands: Option<&actual::ToriiKagemushaCommands>,
) -> Result<MandatoryOfflinePolicy, MandatoryOfflineConfigurationError> {
    if offline.escrow_accounts.is_empty() {
        return Err(MandatoryOfflineConfigurationError::new(format!(
            "chain `{chain_id}` settlement.offline.escrow_accounts must bind at least one required offline asset"
        )));
    }
    match (
        offline.kagemusha_release_policy_path.as_ref(),
        offline.kagemusha_artifact_dir.as_ref(),
    ) {
        (Some(policy), Some(artifacts))
            if !policy.as_os_str().is_empty() && !artifacts.as_os_str().is_empty() => {}
        _ => {
            return Err(MandatoryOfflineConfigurationError::new(format!(
                "chain `{chain_id}` settlement.offline requires a non-empty authenticated release policy and artifact directory"
            )));
        }
    }
    if offline.kagemusha_max_decoded_bytes == 0 {
        return Err(MandatoryOfflineConfigurationError::new(format!(
            "chain `{chain_id}` settlement.offline.kagemusha_max_decoded_bytes must be greater than zero"
        )));
    }
    let commands = commands.ok_or_else(|| {
        MandatoryOfflineConfigurationError::new(format!(
            "chain `{chain_id}` torii.kagemusha_commands is mandatory for offline cash"
        ))
    })?;
    if commands.authority.try_signatory() != Some(commands.key_pair.public_key()) {
        return Err(MandatoryOfflineConfigurationError::new(format!(
            "chain `{chain_id}` offline command signing key does not control its configured authority"
        )));
    }

    Ok(MandatoryOfflinePolicy {
        chain_id: chain_id.clone(),
        configured_escrow_accounts: offline.escrow_accounts.clone(),
        issuer: OfflineIssuerReadinessPolicy {
            authority: commands.authority.clone(),
            signing_public_key: commands.key_pair.public_key().clone(),
            minimum_fee_asset_balance: commands.minimum_xor_balance.clone(),
        },
    })
}

/// Construct the same secret-free mandatory-offline policy from independently
/// reviewed public inputs.
///
/// Deployment semantic tooling uses this after authenticating the release
/// catalog and exact operator documents. It deliberately accepts no private
/// key and still requires the supplied public key to be the authority's sole
/// controller.
///
/// # Errors
///
/// Returns an error for an empty asset catalog or a signer/authority mismatch.
pub fn mandatory_offline_policy_from_reviewed_public_inputs(
    chain_id: &ChainId,
    configured_escrow_accounts: BTreeMap<AssetDefinitionId, AccountId>,
    authority: AccountId,
    signing_public_key: PublicKey,
    minimum_fee_asset_balance: Quantity,
) -> Result<MandatoryOfflinePolicy, MandatoryOfflineConfigurationError> {
    if configured_escrow_accounts.is_empty() {
        return Err(MandatoryOfflineConfigurationError::new(format!(
            "chain `{chain_id}` reviewed offline escrow catalog must bind at least one asset"
        )));
    }
    if authority.try_signatory() != Some(&signing_public_key) {
        return Err(MandatoryOfflineConfigurationError::new(format!(
            "chain `{chain_id}` reviewed offline command public key does not control its authority"
        )));
    }
    Ok(MandatoryOfflinePolicy {
        chain_id: chain_id.clone(),
        configured_escrow_accounts,
        issuer: OfflineIssuerReadinessPolicy {
            authority,
            signing_public_key,
            minimum_fee_asset_balance,
        },
    })
}

/// Evaluate mandatory offline cash against one committed state snapshot.
///
/// The evaluated height, hash, timestamp, fee selector, settlement bindings,
/// world, and authenticated release catalog all come from the same
/// [`StateView`]. A missing local peer identity is represented by a fleet
/// blocker and an empty per-asset `peer_id`; it is not a snapshot error.
///
/// # Errors
///
/// Returns an error only when the committed view cannot identify one canonical
/// non-zero block or belongs to another chain. Protocol readiness failures are
/// returned as deterministic blockers in [`OfflineStatus`].
pub fn evaluate_committed_mandatory_offline(
    state_view: &StateView<'_>,
    policy: &MandatoryOfflinePolicy,
    peer_id: Option<&PeerId>,
) -> Result<OfflineStatus, OfflineSnapshotError> {
    if &state_view.chain_id != policy.chain_id() {
        return Err(OfflineSnapshotError::new(format!(
            "committed offline readiness chain `{}` differs from configured chain `{}`",
            state_view.chain_id,
            policy.chain_id()
        )));
    }
    let evaluated_block = state_view
        .latest_block()
        .ok_or_else(|| OfflineSnapshotError::new("offline readiness requires a committed block"))?;
    let header = evaluated_block.header();
    let evaluated_height = u64::try_from(state_view.height())
        .map_err(|_| OfflineSnapshotError::new("committed offline readiness height exceeds u64"))?;
    if evaluated_height == 0 || header.height().get() != evaluated_height {
        return Err(OfflineSnapshotError::new(
            "committed offline readiness block body and state height disagree",
        ));
    }
    let evaluated_hash = state_view.latest_block_hash().ok_or_else(|| {
        OfflineSnapshotError::new("committed offline readiness has no canonical block hash")
    })?;
    if header.hash() != evaluated_hash {
        return Err(OfflineSnapshotError::new(
            "committed offline readiness block body and hash journal disagree",
        ));
    }
    let evaluated_at_ms = u64::try_from(header.creation_time().as_millis()).map_err(|_| {
        OfflineSnapshotError::new("committed offline readiness timestamp exceeds u64 milliseconds")
    })?;

    Ok(evaluate_snapshot(
        state_view.world(),
        state_view.kagemusha_release_catalog.as_ref(),
        &state_view.settlement.offline,
        &state_view.nexus.fees.fee_asset_id,
        evaluated_height,
        evaluated_hash,
        evaluated_at_ms,
        policy,
        peer_id,
    ))
}

/// Evaluate mandatory offline cash against a validated, uncommitted genesis.
///
/// The caller must pass the exact [`ValidBlock`] whose instructions have
/// already been executed into `staged_genesis`. Core binds evaluation to the
/// height-one header instead of the overlay's still-empty committed parent. A
/// missing local peer identity remains a complete, deterministic unready
/// status rather than aborting evaluation.
///
/// # Errors
///
/// Returns an error when the pair is not a fresh height-one genesis overlay or
/// belongs to another chain. Protocol readiness failures are returned as
/// deterministic blockers in [`OfflineStatus`].
pub fn evaluate_staged_genesis_mandatory_offline(
    valid_genesis: &ValidBlock,
    staged_genesis: &StateBlock<'_>,
    policy: &MandatoryOfflinePolicy,
    peer_id: Option<&PeerId>,
) -> Result<OfflineStatus, OfflineSnapshotError> {
    let signed_genesis = valid_genesis.as_ref();
    let header = signed_genesis.header();
    if !header.is_genesis() || header.height().get() != 1 {
        return Err(OfflineSnapshotError::new(
            "staged offline readiness requires a validated height-one genesis block",
        ));
    }
    ensure_staged_genesis_headers_match(&header, &staged_genesis._curr_block)?;
    if staged_genesis.height() != 0 {
        return Err(OfflineSnapshotError::new(
            "staged offline readiness requires an empty committed parent state",
        ));
    }
    if &staged_genesis.chain_id != policy.chain_id() {
        return Err(OfflineSnapshotError::new(format!(
            "staged offline readiness chain `{}` differs from configured chain `{}`",
            staged_genesis.chain_id,
            policy.chain_id()
        )));
    }
    let evaluated_at_ms = u64::try_from(header.creation_time().as_millis()).map_err(|_| {
        OfflineSnapshotError::new("staged offline readiness timestamp exceeds u64 milliseconds")
    })?;

    Ok(evaluate_snapshot(
        staged_genesis.world(),
        staged_genesis.kagemusha_release_catalog.as_ref(),
        &staged_genesis.settlement.offline,
        &staged_genesis.nexus.fees.fee_asset_id,
        1,
        header.hash(),
        evaluated_at_ms,
        policy,
        peer_id,
    ))
}

fn ensure_staged_genesis_headers_match(
    validated_header: &BlockHeader,
    staged_header: &BlockHeader,
) -> Result<(), OfflineSnapshotError> {
    if staged_header != validated_header {
        return Err(OfflineSnapshotError::new(
            "staged offline readiness block header differs from the validated genesis block",
        ));
    }
    Ok(())
}

/// Fail unless the complete aggregate status is internally consistent and ready.
///
/// # Errors
///
/// Returns all fleet and asset blockers in deterministic order. A malformed
/// caller-constructed status receives an additional consistency blocker.
pub fn ensure_mandatory_offline_ready(
    status: &OfflineStatus,
) -> Result<(), MandatoryOfflineReadinessError> {
    let mut blockers = status.blockers.clone();
    let mut public_status_blockers = public_status_evidence_blockers(status);
    let public_status_valid = public_status_blockers.is_empty();
    let mut every_asset_ready = true;
    for asset in &status.assets {
        blockers.extend(asset.blockers.iter().cloned());
        let mut public_asset_blockers = public_asset_evidence_blockers(status, asset);
        let computed_asset_ready = asset.blockers.is_empty() && public_asset_blockers.is_empty();
        every_asset_ready &= computed_asset_ready;
        if asset.ready != computed_asset_ready {
            public_asset_blockers.push(blocker(
                "offline_asset_status_inconsistent",
                format!(
                    "Asset `{}` publishes a ready flag that disagrees with its complete public evidence and blocker set.",
                    asset.asset_definition_id
                ),
            ));
        }
        blockers.extend(public_asset_blockers);
    }
    blockers.append(&mut public_status_blockers);

    let computed_ready = status.mandatory
        && !status.assets.is_empty()
        && status.blockers.is_empty()
        && public_status_valid
        && every_asset_ready;
    if status.ready != computed_ready {
        blockers.push(blocker(
            "offline_status_inconsistent",
            "The published ready flag disagrees with the complete mandatory-offline blocker set.",
        ));
    }
    sort_and_dedup_blockers(&mut blockers);
    if status.ready && computed_ready && blockers.is_empty() {
        Ok(())
    } else {
        Err(MandatoryOfflineReadinessError { blockers })
    }
}

fn public_status_evidence_blockers(status: &OfflineStatus) -> Vec<OfflineReadinessBlocker> {
    let mut blockers = Vec::new();
    if !status.mandatory {
        blockers.push(blocker(
            "offline_mandatory_flag_missing",
            "Offline cash must be mandatory for this node profile.",
        ));
    }
    if status.cash_handoff_capability != KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1 {
        blockers.push(blocker(
            "offline_cash_handoff_capability_mismatch",
            format!(
                "The published cash-handoff capability must be `{KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1}`."
            ),
        ));
    }
    if status.required_bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4 {
        blockers.push(blocker(
            "offline_bridge_abi_version_mismatch",
            format!(
                "The published native bridge ABI must be {KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4}."
            ),
        ));
    }
    if status.max_hops != KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2 {
        blockers.push(blocker(
            "offline_max_hops_mismatch",
            format!(
                "The published peer-spend hop limit must be {KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2}."
            ),
        ));
    }
    if status.assets.is_empty() {
        blockers.push(blocker(
            "offline_asset_catalog_empty",
            "No required offline asset is present in the effective escrow catalog.",
        ));
        return blockers;
    }

    let first = &status.assets[0];
    let mut asset_ids = BTreeSet::new();
    for asset in &status.assets {
        if asset.peer_id.is_empty() || asset.peer_id.trim() != asset.peer_id {
            blockers.push(blocker(
                "offline_peer_identity_invalid",
                format!(
                    "Asset `{}` does not publish a canonical validator peer identity.",
                    asset.asset_definition_id
                ),
            ));
        } else if asset.peer_id != first.peer_id {
            blockers.push(blocker(
                "offline_snapshot_peer_mismatch",
                format!(
                    "Asset `{}` was not evaluated by the same validator peer as the other assets.",
                    asset.asset_definition_id
                ),
            ));
        }
        if asset.evaluated_block_height != first.evaluated_block_height
            || asset.evaluated_block_hash != first.evaluated_block_hash
        {
            blockers.push(blocker(
                "offline_snapshot_anchor_mismatch",
                format!(
                    "Asset `{}` is not bound to the same evaluated block as the other assets.",
                    asset.asset_definition_id
                ),
            ));
        }
        if !asset_ids.insert(asset.asset_definition_id.as_str()) {
            blockers.push(blocker(
                "offline_asset_definition_duplicate",
                format!(
                    "Asset `{}` occurs more than once in the mandatory readiness catalog.",
                    asset.asset_definition_id
                ),
            ));
        }
    }
    blockers
}

#[derive(Clone, Copy)]
struct ExpectedPublicVerifier<'a> {
    label: &'static str,
    role: &'static str,
    circuit_id: &'static str,
    public_inputs_schema_hash: [u8; 32],
    max_proof_bytes: u32,
    verifier: Option<&'a OfflineActiveTransferVerifier>,
}

fn public_asset_evidence_blockers(
    status: &OfflineStatus,
    asset: &OfflineReadiness,
) -> Vec<OfflineReadinessBlocker> {
    let mut blockers = Vec::new();
    if asset.cash_handoff_capability != status.cash_handoff_capability
        || asset.cash_handoff_capability != KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1
    {
        blockers.push(blocker(
            "offline_asset_cash_handoff_capability_mismatch",
            format!(
                "Asset `{}` does not publish the required `{KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1}` cash-handoff capability.",
                asset.asset_definition_id
            ),
        ));
    }
    if asset.required_bridge_abi_version != status.required_bridge_abi_version
        || asset.required_bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
    {
        blockers.push(blocker(
            "offline_asset_bridge_abi_version_mismatch",
            format!(
                "Asset `{}` does not publish the required ABI-{KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4} bridge identity.",
                asset.asset_definition_id
            ),
        ));
    }
    if asset.max_hops != status.max_hops
        || asset.max_hops != KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
    {
        blockers.push(blocker(
            "offline_asset_max_hops_mismatch",
            format!(
                "Asset `{}` does not publish the required peer-spend hop limit.",
                asset.asset_definition_id
            ),
        ));
    }

    match asset.asset_definition_id.parse::<AssetDefinitionId>() {
        Ok(id) if id.to_string() == asset.asset_definition_id => {}
        _ => blockers.push(blocker(
            "offline_asset_definition_invalid",
            format!(
                "Asset definition `{}` is not a canonical asset identifier.",
                asset.asset_definition_id
            ),
        )),
    }
    match asset.asset_scale {
        Some(scale) if scale <= KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 => {}
        _ => blockers.push(blocker(
            "offline_asset_scale_invalid",
            format!(
                "Asset `{}` does not publish a supported fixed scale.",
                asset.asset_definition_id
            ),
        )),
    }
    if asset.evaluated_block_height == 0
        || !is_canonical_nonzero_sha256_hex(&asset.evaluated_block_hash)
    {
        blockers.push(blocker(
            "offline_snapshot_anchor_invalid",
            format!(
                "Asset `{}` does not publish a canonical non-zero block height and hash anchor.",
                asset.asset_definition_id
            ),
        ));
    }

    let transfer_schema: [u8; 32] =
        Hash::new(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1)
            .into();
    let topup_schema: [u8; 32] =
        Hash::new(crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2)
            .into();
    let unshield_schema: [u8; 32] =
        Hash::new(crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1)
            .into();
    for expected in [
        ExpectedPublicVerifier {
            label: "transfer",
            role: KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2,
            circuit_id: crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
            public_inputs_schema_hash: transfer_schema,
            max_proof_bytes: crate::zk::confidential_v2::CONFIDENTIAL_V2_MAX_PROOF_BYTES,
            verifier: asset.active_transfer_verifier.as_ref(),
        },
        ExpectedPublicVerifier {
            label: "top-up shield",
            role: KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2,
            circuit_id: crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
            public_inputs_schema_hash: topup_schema,
            max_proof_bytes: crate::zk::confidential_v2::CONFIDENTIAL_V2_MAX_PROOF_BYTES,
            verifier: asset.active_topup_shield_verifier.as_ref(),
        },
        ExpectedPublicVerifier {
            label: "unshield",
            role: KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2,
            circuit_id: crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
            public_inputs_schema_hash: unshield_schema,
            max_proof_bytes: crate::zk::confidential_v2::CONFIDENTIAL_V2_MAX_PROOF_BYTES,
            verifier: asset.active_unshield_verifier.as_ref(),
        },
        ExpectedPublicVerifier {
            label: "recursive StepEq",
            role: KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4,
            circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
            public_inputs_schema_hash:
                kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v4(),
            max_proof_bytes: KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
            verifier: asset.active_recursive_step_eq_verifier.as_ref(),
        },
        ExpectedPublicVerifier {
            label: "recursive StepEp",
            role: KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4,
            circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
            public_inputs_schema_hash:
                kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v4(),
            max_proof_bytes: KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
            verifier: asset.active_recursive_step_ep_verifier.as_ref(),
        },
    ] {
        blockers.extend(public_verifier_evidence_blockers(
            asset,
            expected,
            asset.evaluated_block_height,
        ));
    }

    blockers.extend(verifier_role_distinctness_blockers([
        ("transfer", asset.active_transfer_verifier.as_ref()),
        ("topup_shield", asset.active_topup_shield_verifier.as_ref()),
        ("unshield", asset.active_unshield_verifier.as_ref()),
        (
            "recursive_step_eq",
            asset.active_recursive_step_eq_verifier.as_ref(),
        ),
        (
            "recursive_step_ep",
            asset.active_recursive_step_ep_verifier.as_ref(),
        ),
    ]));

    let artifact = asset.artifact_set.as_ref();
    match artifact {
        None => blockers.push(blocker(
            "offline_authenticated_artifact_set_missing",
            format!(
                "Asset `{}` does not publish its authenticated ABI-21/V4 artifact identity.",
                asset.asset_definition_id
            ),
        )),
        Some(artifact) => blockers.extend(public_artifact_evidence_blockers(asset, artifact)),
    }
    if !asset.proof_backend_available {
        blockers.push(blocker(
            "offline_proof_backend_unavailable",
            format!(
                "Asset `{}` does not report the authenticated production proof backend as available.",
                asset.asset_definition_id
            ),
        ));
    }
    if !asset.recursive_lineage_supported {
        blockers.push(blocker(
            "offline_recursive_lineage_unsupported",
            format!(
                "Asset `{}` does not report ABI-21/V4 recursive lineage support.",
                asset.asset_definition_id
            ),
        ));
    }

    if let Some(artifact) = artifact {
        for (label, verifier) in [
            ("StepEq", asset.active_recursive_step_eq_verifier.as_ref()),
            ("StepEp", asset.active_recursive_step_ep_verifier.as_ref()),
        ] {
            if verifier.is_some_and(|verifier| {
                verifier.max_proof_bytes != artifact.max_proof_bytes
                    || verifier.activation_height != artifact.activation_height
                    || verifier.withdrawal_height != Some(artifact.withdrawal_height)
            }) {
                blockers.push(blocker(
                    "offline_recursive_artifact_binding_invalid",
                    format!(
                        "Asset `{}` {label} verifier does not share the authenticated artifact issuance bounds.",
                        asset.asset_definition_id
                    ),
                ));
            }
        }
    }

    sort_and_dedup_blockers(&mut blockers);
    blockers
}

fn public_verifier_evidence_blockers(
    asset: &OfflineReadiness,
    expected: ExpectedPublicVerifier<'_>,
    evaluated_height: u64,
) -> Vec<OfflineReadinessBlocker> {
    let Some(verifier) = expected.verifier else {
        return vec![blocker(
            "offline_required_verifier_missing",
            format!(
                "Asset `{}` does not publish its active {} verifier.",
                asset.asset_definition_id, expected.label
            ),
        )];
    };
    let withdrawal_is_valid = verifier.withdrawal_height.is_none_or(|withdrawal| {
        withdrawal > evaluated_height && withdrawal > verifier.activation_height
    });
    let valid = verifier.id.backend == crate::zk::ZK_BACKEND_HALO2_IPA
        && verifier.id.name == expected.role
        && verifier.version > 0
        && verifier.circuit_id == expected.circuit_id
        && is_canonical_nonzero_sha256_hex(&verifier.commitment)
        && verifier.public_inputs_schema_hash == hex::encode(expected.public_inputs_schema_hash)
        && verifier.max_proof_bytes > 0
        && verifier.max_proof_bytes <= expected.max_proof_bytes
        && verifier.activation_height <= evaluated_height
        && withdrawal_is_valid;
    if valid {
        Vec::new()
    } else {
        vec![blocker(
            "offline_verifier_evidence_invalid",
            format!(
                "Asset `{}` publishes invalid public evidence for its {} verifier.",
                asset.asset_definition_id, expected.label
            ),
        )]
    }
}

fn public_artifact_evidence_blockers(
    asset: &OfflineReadiness,
    artifact: &OfflineAuthenticatedArtifactSet,
) -> Vec<OfflineReadinessBlocker> {
    let digests = [
        artifact.manifest_sha256.as_str(),
        artifact.release_policy_sha256.as_str(),
        artifact.release_attestation_sha256.as_str(),
    ];
    let digests_valid = digests
        .iter()
        .all(|digest| is_canonical_nonzero_sha256_hex(digest))
        && digests.into_iter().collect::<BTreeSet<_>>().len() == 3;
    let valid = !artifact.generation.is_empty()
        && artifact.generation.trim() == artifact.generation
        && digests_valid
        && artifact.activation_height > 0
        && artifact.activation_height <= asset.evaluated_block_height
        && artifact.withdrawal_height > artifact.activation_height
        && asset.evaluated_block_height < artifact.withdrawal_height
        && artifact.max_proof_bytes > 0
        && artifact.max_proof_bytes <= KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
        && asset.asset_scale == Some(artifact.asset_scale)
        && artifact.asset_scale <= KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2;
    if valid {
        Vec::new()
    } else {
        vec![blocker(
            "offline_authenticated_artifact_set_invalid",
            format!(
                "Asset `{}` publishes an invalid authenticated ABI-21/V4 artifact identity or issuance window.",
                asset.asset_definition_id
            ),
        )]
    }
}

fn is_canonical_nonzero_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

#[derive(Clone, Copy)]
struct SnapshotContext {
    evaluated_height: u64,
    evaluated_hash: HashOf<BlockHeader>,
}

fn evaluate_snapshot(
    world: &impl WorldReadOnly,
    release_catalog: &KagemushaReleaseCatalogV4,
    evaluated_offline: &actual::Offline,
    fee_asset_selector: &str,
    evaluated_height: u64,
    evaluated_hash: HashOf<BlockHeader>,
    evaluated_at_ms: u64,
    policy: &MandatoryOfflinePolicy,
    peer_id: Option<&PeerId>,
) -> OfflineStatus {
    let context = SnapshotContext {
        evaluated_height,
        evaluated_hash,
    };
    let mut fleet_blockers = Vec::new();
    if peer_id.is_none() {
        fleet_blockers.push(blocker(
            "peer_identity_unavailable",
            "The validator has no configured local peer identity for this readiness snapshot.",
        ));
    }
    let metadata_bindings = metadata_escrow_bindings(world, policy.chain_id(), &mut fleet_blockers);
    let escrow_bindings = effective_escrow_bindings(
        policy.configured_escrow_accounts(),
        &evaluated_offline.escrow_accounts,
        &metadata_bindings,
        &mut fleet_blockers,
    );

    if let Err(error) =
        ensure_kagemusha_active_release_material_v4(world, release_catalog, evaluated_height)
    {
        fleet_blockers.push(blocker(
            "recursive_v4_release_catalog_invalid",
            format!(
                "The authenticated ABI-21/V4 release catalog is not ready at block {evaluated_height}: {error}"
            ),
        ));
    }
    if let Err(error) =
        crate::smartcontracts::isi::offline::isi::ensure_offline_device_attestation_policy_ready_v1(
            world,
            evaluated_at_ms,
        )
    {
        fleet_blockers.push(blocker(
            "device_attestation_policy_not_ready",
            format!("The governed hardware spend-authority policy is not ready: {error}"),
        ));
    }
    fleet_blockers.extend(issuer_readiness_blockers(
        world,
        policy.issuer(),
        fee_asset_selector,
        evaluated_at_ms,
    ));
    if escrow_bindings.is_empty() {
        fleet_blockers.push(blocker(
            "offline_asset_catalog_empty",
            "No required offline asset is present in the effective escrow catalog.",
        ));
    }
    sort_and_dedup_blockers(&mut fleet_blockers);

    let assets = escrow_bindings
        .iter()
        .map(|(asset_definition_id, escrow_account_id)| {
            evaluate_asset(
                world,
                release_catalog,
                asset_definition_id,
                escrow_account_id,
                context,
                policy,
                peer_id,
                &fleet_blockers,
            )
        })
        .collect::<Vec<_>>();
    let ready = fleet_blockers.is_empty()
        && !assets.is_empty()
        && assets
            .iter()
            .all(|asset| asset.ready && asset.blockers.is_empty());

    OfflineStatus {
        mandatory: true,
        cash_handoff_capability: KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1.to_owned(),
        required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        max_hops: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
        ready,
        assets,
        blockers: fleet_blockers,
    }
}

fn effective_escrow_bindings(
    configured: &BTreeMap<AssetDefinitionId, AccountId>,
    evaluated: &BTreeMap<AssetDefinitionId, AccountId>,
    metadata_derived: &BTreeMap<AssetDefinitionId, AccountId>,
    blockers: &mut Vec<OfflineReadinessBlocker>,
) -> BTreeMap<AssetDefinitionId, AccountId> {
    if evaluated.is_empty() {
        blockers.push(blocker(
            "replayed_offline_escrow_catalog_empty",
            "The evaluated settlement.offline escrow catalog is empty.",
        ));
    }

    let mut effective = evaluated.clone();
    for (asset_definition_id, configured_account_id) in configured {
        match evaluated.get(asset_definition_id) {
            None => blockers.push(blocker(
                "offline_escrow_binding_missing_after_replay",
                format!(
                    "Configured offline escrow binding for `{asset_definition_id}` disappeared from the evaluated state."
                ),
            )),
            Some(evaluated_account_id) if evaluated_account_id != configured_account_id => {
                blockers.push(blocker(
                    "offline_escrow_binding_changed_after_replay",
                    format!(
                        "Configured offline escrow binding for `{asset_definition_id}` is `{configured_account_id}`, but evaluated state selected `{evaluated_account_id}`."
                    ),
                ));
            }
            Some(_) => {}
        }
        // Operator-reviewed bindings remain the evaluation target even after a
        // conflicting replay value has already made the aggregate unready.
        effective.insert(asset_definition_id.clone(), configured_account_id.clone());
    }

    for (asset_definition_id, derived_account_id) in metadata_derived {
        match effective.entry(asset_definition_id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(derived_account_id.clone());
            }
            std::collections::btree_map::Entry::Occupied(entry)
                if entry.get() != derived_account_id =>
            {
                blockers.push(blocker(
                    "offline_metadata_escrow_conflict",
                    format!(
                        "Offline-enabled asset `{asset_definition_id}` requires deterministic escrow `{derived_account_id}`, but evaluated policy selected `{}`.",
                        entry.get()
                    ),
                ));
            }
            std::collections::btree_map::Entry::Occupied(_) => {}
        }
    }
    effective
}

fn metadata_escrow_bindings(
    world: &impl WorldReadOnly,
    chain_id: &ChainId,
    blockers: &mut Vec<OfflineReadinessBlocker>,
) -> BTreeMap<AssetDefinitionId, AccountId> {
    let mut bindings = BTreeMap::new();
    for (asset_definition_id, asset_definition) in world.asset_definitions().iter() {
        match offline_asset_enabled(asset_definition_id, &asset_definition.metadata) {
            Ok(false) => {}
            Ok(true) => {
                bindings.insert(
                    asset_definition_id.clone(),
                    iroha_data_model::offline::offline_escrow_account_id(
                        chain_id,
                        asset_definition_id,
                    ),
                );
            }
            Err(error) => {
                blockers.push(blocker("offline_asset_metadata_invalid", error));
                // An invalid explicit marker is fail-closed and remains visible
                // in the asset catalog instead of silently becoming non-offline.
                bindings.insert(
                    asset_definition_id.clone(),
                    iroha_data_model::offline::offline_escrow_account_id(
                        chain_id,
                        asset_definition_id,
                    ),
                );
            }
        }
    }
    bindings
}

fn offline_asset_enabled(
    asset_definition_id: &AssetDefinitionId,
    metadata: &iroha_data_model::metadata::Metadata,
) -> Result<bool, String> {
    let key: iroha_data_model::name::Name =
        OFFLINE_ASSET_ENABLED_METADATA_KEY
            .parse()
            .map_err(|error| {
                format!(
                    "Offline asset `{asset_definition_id}` has an invalid metadata key: {error}"
                )
            })?;
    let Some(value) = metadata.get(&key) else {
        return Ok(false);
    };
    if let Ok(enabled) = value.try_into_any::<bool>() {
        return Ok(enabled);
    }
    if let Ok(text) = value.try_into_any::<String>() {
        let trimmed = text.trim();
        if trimmed.eq_ignore_ascii_case("true") {
            return Ok(true);
        }
        if trimmed.eq_ignore_ascii_case("false") {
            return Ok(false);
        }
    }
    Err(format!(
        "Offline asset `{asset_definition_id}` metadata `{OFFLINE_ASSET_ENABLED_METADATA_KEY}` must be a boolean or the string `true`/`false`."
    ))
}

fn issuer_readiness_blockers(
    world: &impl WorldReadOnly,
    issuer: &OfflineIssuerReadinessPolicy,
    fee_asset_selector: &str,
    evaluated_at_ms: u64,
) -> Vec<OfflineReadinessBlocker> {
    let mut blockers = Vec::new();
    if world.account(issuer.authority()).is_err() {
        blockers.push(blocker(
            "offline_command_authority_not_registered",
            format!(
                "Offline command authority `{}` is not registered.",
                issuer.authority()
            ),
        ));
    }
    if issuer.authority().try_signatory() != Some(issuer.signing_public_key()) {
        blockers.push(blocker(
            "offline_command_signer_mismatch",
            "The configured offline command signing key does not control its authority.",
        ));
    }
    if !crate::smartcontracts::isi::offline::isi::world_has_offline_escrow_manager_permission(
        world,
        issuer.authority(),
    ) {
        blockers.push(blocker(
            "offline_command_authority_permission_missing",
            "The offline command authority lacks the exact CanManageOfflineEscrow permission.",
        ));
    }

    match resolve_asset_definition_selector(world, fee_asset_selector, evaluated_at_ms) {
        Err(error) => blockers.push(blocker(
            "offline_command_fee_asset_not_ready",
            format!("The offline command fee asset is unavailable: {error}"),
        )),
        Ok(fee_asset_definition) => {
            let fee_asset = AssetId::new(fee_asset_definition, issuer.authority().clone());
            let balance = world
                .asset(&fee_asset)
                .map(|entry| entry.value().as_ref().clone())
                .unwrap_or_else(|_| Quantity::zero());
            if balance < *issuer.minimum_fee_asset_balance() {
                blockers.push(blocker(
                    "offline_command_authority_unfunded",
                    format!(
                        "The offline command authority does not meet its configured minimum fee-asset balance of {}.",
                        issuer.minimum_fee_asset_balance()
                    ),
                ));
            }
        }
    }
    blockers
}

fn resolve_asset_definition_selector(
    world: &impl WorldReadOnly,
    selector: &str,
    evaluated_at_ms: u64,
) -> Result<AssetDefinitionId, String> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err("the fee asset selector is blank".to_owned());
    }
    if let Ok(asset_definition_id) = selector.parse::<AssetDefinitionId>() {
        return world
            .asset_definition(&asset_definition_id)
            .map(|_| asset_definition_id)
            .map_err(|_| "the canonical fee asset definition is not registered".to_owned());
    }
    let alias = selector.parse::<AssetDefinitionAlias>().map_err(|_| {
        "the fee asset selector is neither a canonical id nor an asset alias".to_owned()
    })?;
    world
        .asset_definition_id_by_alias_at(&alias, evaluated_at_ms)
        .ok_or_else(|| "the fee asset alias is absent, expired, or corrupt".to_owned())
}

fn evaluate_asset(
    world: &impl WorldReadOnly,
    release_catalog: &KagemushaReleaseCatalogV4,
    asset_definition_id: &AssetDefinitionId,
    escrow_account_id: &AccountId,
    context: SnapshotContext,
    policy: &MandatoryOfflinePolicy,
    peer_id: Option<&PeerId>,
    fleet_blockers: &[OfflineReadinessBlocker],
) -> OfflineReadiness {
    let mut blockers = fleet_blockers.to_vec();
    let asset_scale = match world.asset_definition(asset_definition_id) {
        Err(error) => {
            blockers.push(blocker(
                "asset_definition_unavailable",
                format!(
                    "Required offline asset definition `{asset_definition_id}` is unavailable: {error}"
                ),
            ));
            None
        }
        Ok(asset_definition) => match asset_definition.spec().scale() {
            None => {
                blockers.push(blocker(
                    "asset_scale_unavailable",
                    "The required offline asset is not fixed-scale.",
                ));
                None
            }
            Some(scale) if scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 => {
                blockers.push(blocker(
                    "asset_scale_unsupported",
                    format!(
                        "The required offline asset scale {scale} exceeds the protocol maximum {KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2}."
                    ),
                ));
                Some(scale)
            }
            Some(scale) => Some(scale),
        },
    };
    if let Err(error) = world.account(escrow_account_id) {
        blockers.push(blocker(
            "offline_escrow_account_unavailable",
            format!(
                "Offline escrow account `{escrow_account_id}` for `{asset_definition_id}` is unavailable: {error}"
            ),
        ));
    }

    let transfer = collect_verifier_result(
        asset_transfer_verifier_record(world, asset_definition_id, context.evaluated_height),
        "transfer_verifier_invalid",
        "transfer_verifier_unavailable",
        "The asset-bound transfer verifier",
        &mut blockers,
    );
    let topup_shield = collect_verifier_result(
        asset_topup_shield_verifier_record(world, asset_definition_id, context.evaluated_height),
        "topup_shield_verifier_invalid",
        "topup_shield_verifier_unavailable",
        "The asset-bound top-up shield verifier",
        &mut blockers,
    );
    let unshield = collect_verifier_result(
        asset_unshield_verifier_record(world, asset_definition_id, context.evaluated_height),
        "unshield_verifier_invalid",
        "unshield_verifier_unavailable",
        "The asset-bound unshield verifier",
        &mut blockers,
    );

    let mut recursive_step_eq = None;
    let mut recursive_step_ep = None;
    let mut artifact_set = None;
    let mut proof_backend_available = false;
    if let Some(scale) = asset_scale
        && scale <= KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
    {
        match resolve_kagemusha_recursive_readiness_v4(
            world,
            release_catalog,
            policy.chain_id(),
            asset_definition_id,
            scale,
            context.evaluated_height,
        ) {
            Err(error) => blockers.push(blocker(
                "recursive_v4_registry_malformed",
                format!(
                    "The authenticated ABI-21/V4 recursive registry failed validation: {error}"
                ),
            )),
            Ok(None) => blockers.push(blocker(
                "recursive_v4_registry_unavailable",
                "No active authenticated ABI-21/V4 Eq/Ep release is installed for this chain, asset, scale, and block.",
            )),
            Ok(Some(recursive)) => {
                let projected = project_recursive_readiness(recursive, &mut blockers);
                recursive_step_eq = Some(projected.step_eq);
                recursive_step_ep = Some(projected.step_ep);
                artifact_set = Some(projected.artifact_set);
                proof_backend_available = projected.proof_backend_available;
            }
        }
    } else {
        blockers.push(blocker(
            "recursive_v4_registry_unavailable",
            "The authenticated ABI-21/V4 recursive release cannot be selected without a supported fixed asset scale.",
        ));
    }

    blockers.extend(verifier_role_distinctness_blockers([
        ("transfer", transfer.as_ref()),
        ("topup_shield", topup_shield.as_ref()),
        ("unshield", unshield.as_ref()),
        ("recursive_step_eq", recursive_step_eq.as_ref()),
        ("recursive_step_ep", recursive_step_ep.as_ref()),
    ]));
    sort_and_dedup_blockers(&mut blockers);
    let recursive_lineage_supported = proof_backend_available
        && artifact_set.is_some()
        && recursive_step_eq.is_some()
        && recursive_step_ep.is_some();
    let ready = blockers.is_empty();

    OfflineReadiness {
        peer_id: peer_id.map_or_else(String::new, ToString::to_string),
        cash_handoff_capability: KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1.to_owned(),
        required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        max_hops: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
        asset_definition_id: asset_definition_id.to_string(),
        asset_scale,
        evaluated_block_height: context.evaluated_height,
        evaluated_block_hash: hex::encode(context.evaluated_hash.as_ref().as_ref()),
        active_transfer_verifier: transfer,
        active_topup_shield_verifier: topup_shield,
        active_unshield_verifier: unshield,
        active_recursive_step_eq_verifier: recursive_step_eq,
        active_recursive_step_ep_verifier: recursive_step_ep,
        artifact_set,
        proof_backend_available,
        recursive_lineage_supported,
        ready,
        blockers,
    }
}

fn collect_verifier_result(
    result: Result<Option<OfflineActiveTransferVerifier>, String>,
    invalid_code: &'static str,
    unavailable_code: &'static str,
    label: &'static str,
    blockers: &mut Vec<OfflineReadinessBlocker>,
) -> Option<OfflineActiveTransferVerifier> {
    match result {
        Ok(Some(verifier)) => Some(verifier),
        Ok(None) => {
            blockers.push(blocker(
                unavailable_code,
                format!("{label} is not active at the evaluated block."),
            ));
            None
        }
        Err(error) => {
            blockers.push(blocker(
                invalid_code,
                format!("{label} is invalid: {error}"),
            ));
            None
        }
    }
}

struct ProjectedRecursiveReadiness {
    step_eq: OfflineActiveTransferVerifier,
    step_ep: OfflineActiveTransferVerifier,
    artifact_set: OfflineAuthenticatedArtifactSet,
    proof_backend_available: bool,
}

fn project_recursive_readiness(
    recursive: KagemushaRecursiveReadinessV4,
    blockers: &mut Vec<OfflineReadinessBlocker>,
) -> ProjectedRecursiveReadiness {
    let proof_backend_available = recursive.proof_backend_error.is_none();
    if let Some(error) = recursive.proof_backend_error {
        blockers.push(blocker(
            "proof_backend_unavailable",
            format!("The authenticated ABI-21/V4 proof backend could not be constructed: {error}"),
        ));
        blockers.push(blocker(
            "recursive_lineage_unavailable",
            "Recursive lineage is unavailable because the authenticated proof backend could not be constructed.",
        ));
    }
    ProjectedRecursiveReadiness {
        step_eq: project_recursive_verifier(recursive.step_eq),
        step_ep: project_recursive_verifier(recursive.step_ep),
        artifact_set: project_artifact_set(recursive.artifact_set),
        proof_backend_available,
    }
}

fn project_recursive_verifier(
    verifier: KagemushaRecursiveVerifierReadinessV4,
) -> OfflineActiveTransferVerifier {
    OfflineActiveTransferVerifier {
        id: OfflineVerifierId {
            backend: verifier.id.backend.as_str().to_owned(),
            name: verifier.id.name,
        },
        version: verifier.version,
        circuit_id: verifier.circuit_id,
        commitment: hex::encode(verifier.commitment),
        public_inputs_schema_hash: hex::encode(verifier.public_inputs_schema_hash),
        max_proof_bytes: verifier.max_proof_bytes,
        activation_height: verifier.activation_height,
        withdrawal_height: verifier.withdrawal_height,
    }
}

fn project_artifact_set(
    artifact_set: KagemushaAuthenticatedArtifactSetReadinessV4,
) -> OfflineAuthenticatedArtifactSet {
    OfflineAuthenticatedArtifactSet {
        generation: artifact_set.generation,
        manifest_sha256: hex::encode(artifact_set.manifest_sha256),
        release_policy_sha256: hex::encode(artifact_set.release_policy_sha256),
        release_attestation_sha256: hex::encode(artifact_set.release_attestation_sha256),
        activation_height: artifact_set.activation_height,
        withdrawal_height: artifact_set.withdrawal_height,
        max_proof_bytes: artifact_set.max_proof_bytes,
        asset_scale: artifact_set.asset_scale,
    }
}

fn asset_transfer_verifier_record(
    world: &impl WorldReadOnly,
    asset: &AssetDefinitionId,
    block_height: u64,
) -> Result<Option<OfflineActiveTransferVerifier>, String> {
    let Some(zk_asset) = world.zk_assets().get(asset) else {
        return Ok(None);
    };
    asset_bound_verifier_record(
        world,
        zk_asset.vk_transfer.as_ref(),
        block_height,
        crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2,
        Hash::new(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1)
            .into(),
        crate::zk::confidential_v2::CONFIDENTIAL_V2_MAX_PROOF_BYTES,
        crate::zk::confidential_v2::ensure_confidential_transfer_v2_canonical_vk_box,
    )
}

fn asset_topup_shield_verifier_record(
    world: &impl WorldReadOnly,
    asset: &AssetDefinitionId,
    block_height: u64,
) -> Result<Option<OfflineActiveTransferVerifier>, String> {
    let Some(zk_asset) = world.zk_assets().get(asset) else {
        return Ok(None);
    };
    asset_bound_verifier_record(
        world,
        zk_asset.vk_shield.as_ref(),
        block_height,
        crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
        KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2,
        Hash::new(crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2)
            .into(),
        crate::zk::confidential_v2::CONFIDENTIAL_V2_MAX_PROOF_BYTES,
        crate::zk::confidential_v2::ensure_kagemusha_topup_shield_v2_canonical_vk_box,
    )
}

fn asset_unshield_verifier_record(
    world: &impl WorldReadOnly,
    asset: &AssetDefinitionId,
    block_height: u64,
) -> Result<Option<OfflineActiveTransferVerifier>, String> {
    let Some(zk_asset) = world.zk_assets().get(asset) else {
        return Ok(None);
    };
    asset_bound_verifier_record(
        world,
        zk_asset.vk_unshield.as_ref(),
        block_height,
        crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
        KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2,
        Hash::new(crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1)
            .into(),
        crate::zk::confidential_v2::CONFIDENTIAL_V2_MAX_PROOF_BYTES,
        crate::zk::confidential_v2::ensure_confidential_unshield_v3_canonical_vk_box,
    )
}

#[allow(clippy::too_many_arguments)]
fn asset_bound_verifier_record<E>(
    world: &impl WorldReadOnly,
    binding: Option<&ZkAssetVerifierBinding>,
    block_height: u64,
    expected_circuit_id: &str,
    expected_role: &str,
    expected_public_inputs_schema_hash: [u8; 32],
    max_allowed_proof_bytes: u32,
    ensure_canonical: impl Fn(&VerifyingKeyBox) -> Result<(), E>,
) -> Result<Option<OfflineActiveTransferVerifier>, String>
where
    E: fmt::Display,
{
    let Some(binding) = binding else {
        return Ok(None);
    };
    let record = world.verifying_keys().get(&binding.id).ok_or_else(|| {
        format!(
            "asset-bound verifier `{}/{}` is missing from the registry",
            binding.id.backend, binding.id.name
        )
    })?;
    let circuit_key = (record.circuit_id.clone(), record.version);
    if record.version == 0
        || record.circuit_id != expected_circuit_id
        || record.namespace != KAGEMUSHA_VERIFIER_NAMESPACE
        || record.backend != BackendTag::Halo2IpaPasta
        || record.curve != "pallas"
        || binding.id.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA
        || binding.id.name != expected_role
        || binding.commitment == [0; 32]
        || binding.commitment != record.commitment
        || !binding.id.is_portable_registry_id()
        || record.public_inputs_schema_hash != expected_public_inputs_schema_hash
        || record.max_proof_bytes == 0
        || record.max_proof_bytes > max_allowed_proof_bytes
    {
        return Err("asset-bound verifier metadata is inconsistent with Kagemusha".to_owned());
    }
    if world.verifying_keys_by_circuit().get(&circuit_key) != Some(&binding.id) {
        return Err(
            "asset-bound verifier is not the registry entry for its circuit version".to_owned(),
        );
    }
    let verifier_key = record
        .key
        .as_ref()
        .ok_or_else(|| "asset-bound verifier key is not available inline".to_owned())?;
    if verifier_key.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA
        || verifier_key.bytes.is_empty()
        || u32::try_from(verifier_key.bytes.len()).ok() != Some(record.vk_len)
        || crate::zk::hash_vk(verifier_key) != record.commitment
    {
        return Err("asset-bound verifier key material is inconsistent".to_owned());
    }
    ensure_canonical(verifier_key)
        .map_err(|error| format!("asset-bound verifier is not canonical: {error}"))?;
    if !record.is_active_at(block_height) {
        return Ok(None);
    }
    Ok(Some(project_verifier_record(&binding.id, record)))
}

fn project_verifier_record(
    id: &VerifyingKeyId,
    record: &VerifyingKeyRecord,
) -> OfflineActiveTransferVerifier {
    OfflineActiveTransferVerifier {
        id: OfflineVerifierId {
            backend: id.backend.as_str().to_owned(),
            name: id.name.clone(),
        },
        version: record.version,
        circuit_id: record.circuit_id.clone(),
        commitment: hex::encode(record.commitment),
        public_inputs_schema_hash: hex::encode(record.public_inputs_schema_hash),
        max_proof_bytes: record.max_proof_bytes,
        activation_height: record.activation_height.unwrap_or(0),
        withdrawal_height: record.withdraw_height,
    }
}

fn verifier_role_distinctness_blockers(
    roles: [(&'static str, Option<&OfflineActiveTransferVerifier>); 5],
) -> Vec<OfflineReadinessBlocker> {
    let mut ids = BTreeMap::<(String, String), &'static str>::new();
    let mut commitments = BTreeMap::<String, &'static str>::new();
    let mut schema_hashes = BTreeMap::<String, &'static str>::new();
    let mut blockers = Vec::new();
    for (role, verifier) in roles {
        let Some(verifier) = verifier else {
            continue;
        };
        let id = (verifier.id.backend.clone(), verifier.id.name.clone());
        if let Some(first_role) = ids.insert(id, role) {
            blockers.push(blocker(
                "offline_verifier_registry_id_reused",
                format!(
                    "Active Kagemusha verifier role `{role}` reuses registry identity from `{first_role}`."
                ),
            ));
        }
        if let Some(first_role) = commitments.insert(verifier.commitment.clone(), role) {
            blockers.push(blocker(
                "offline_verifier_commitment_reused",
                format!(
                    "Active Kagemusha verifier role `{role}` reuses key commitment from `{first_role}`."
                ),
            ));
        }
        if let Some(first_role) =
            schema_hashes.insert(verifier.public_inputs_schema_hash.clone(), role)
        {
            blockers.push(blocker(
                "offline_verifier_schema_hash_reused",
                format!(
                    "Active Kagemusha verifier role `{role}` reuses public-input schema hash from `{first_role}`."
                ),
            ));
        }
    }
    blockers
}

fn blocker(code: &'static str, message: impl Into<String>) -> OfflineReadinessBlocker {
    OfflineReadinessBlocker {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn sort_and_dedup_blockers(blockers: &mut Vec<OfflineReadinessBlocker>) {
    blockers.sort_unstable_by(|left, right| {
        (&left.code, &left.message).cmp(&(&right.code, &right.message))
    });
    blockers.dedup();
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        num::{NonZeroU64, NonZeroUsize},
        path::PathBuf,
        str::FromStr as _,
    };

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        domain::DomainId, name::Name, offline::OfflineStatus, proof::VerifyingKeyId,
    };

    use super::*;

    fn key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("test key pair")
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(key_pair(seed).public_key().clone())
    }

    fn asset(name: &str) -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("offline", "test").expect("test domain"),
            Name::from_str(name).expect("test asset name"),
        )
    }

    fn configured_offline() -> actual::Offline {
        actual::Offline {
            escrow_accounts: BTreeMap::from([(asset("ds"), account(2))]),
            kagemusha_release_policy_path: Some(PathBuf::from("/release/policy.norito")),
            kagemusha_artifact_dir: Some(PathBuf::from("/release/artifacts")),
            kagemusha_max_decoded_bytes: 1,
        }
    }

    fn commands() -> actual::ToriiKagemushaCommands {
        let key_pair = key_pair(1);
        actual::ToriiKagemushaCommands {
            authority: AccountId::new(key_pair.public_key().clone()),
            key_pair,
            minimum_xor_balance: Quantity::from(25_u32),
            max_tx_value: Quantity::from(1_000_u32),
            operation_registry_max_entries: NonZeroUsize::new(1).expect("non-zero"),
            operation_registry_max_bytes: NonZeroUsize::new(1).expect("non-zero"),
        }
    }

    #[test]
    fn policy_constructor_rejects_every_empty_mandatory_input() {
        let chain_id = ChainId::from("offline-readiness-test");
        let commands = commands();

        let mut offline = configured_offline();
        offline.escrow_accounts.clear();
        assert!(
            mandatory_offline_policy_from_config(&chain_id, &offline, Some(&commands))
                .expect_err("empty escrow catalog must reject")
                .message()
                .contains("at least one")
        );

        let mut offline = configured_offline();
        offline.kagemusha_release_policy_path = Some(PathBuf::new());
        assert!(
            mandatory_offline_policy_from_config(&chain_id, &offline, Some(&commands))
                .expect_err("blank policy path must reject")
                .message()
                .contains("non-empty authenticated")
        );

        let mut offline = configured_offline();
        offline.kagemusha_max_decoded_bytes = 0;
        assert!(
            mandatory_offline_policy_from_config(&chain_id, &offline, Some(&commands))
                .expect_err("zero decoded budget must reject")
                .message()
                .contains("greater than zero")
        );

        assert!(
            mandatory_offline_policy_from_config(&chain_id, &configured_offline(), None)
                .expect_err("missing issuer must reject")
                .message()
                .contains("mandatory")
        );
    }

    #[test]
    fn policy_constructor_retains_public_issuer_identity_only() {
        let chain_id = ChainId::from("offline-readiness-test");
        let commands = commands();
        let policy =
            mandatory_offline_policy_from_config(&chain_id, &configured_offline(), Some(&commands))
                .expect("complete operator policy");

        assert_eq!(policy.chain_id(), &chain_id);
        assert_eq!(policy.issuer().authority(), &commands.authority);
        assert_eq!(
            policy.issuer().signing_public_key(),
            commands.key_pair.public_key()
        );
        assert_eq!(
            policy.issuer().minimum_fee_asset_balance(),
            &commands.minimum_xor_balance
        );
    }

    #[test]
    fn policy_constructor_rejects_signer_authority_mismatch() {
        let chain_id = ChainId::from("offline-readiness-test");
        let mut commands = commands();
        commands.authority = account(9);
        assert!(
            mandatory_offline_policy_from_config(&chain_id, &configured_offline(), Some(&commands))
                .expect_err("substituted signer must reject")
                .message()
                .contains("does not control")
        );
    }

    #[test]
    fn reviewed_public_policy_constructor_is_secret_free_and_fail_closed() {
        let chain_id = ChainId::from("offline-readiness-reviewed-test");
        let signer_key_pair = key_pair(7);
        let authority = AccountId::new(signer_key_pair.public_key().clone());
        let escrows = BTreeMap::from([(asset("ds"), account(8))]);
        let policy = mandatory_offline_policy_from_reviewed_public_inputs(
            &chain_id,
            escrows.clone(),
            authority.clone(),
            signer_key_pair.public_key().clone(),
            Quantity::from(1_u32),
        )
        .expect("complete public inputs");
        assert_eq!(policy.chain_id(), &chain_id);
        assert_eq!(policy.configured_escrow_accounts(), &escrows);
        assert_eq!(policy.issuer().authority(), &authority);

        assert!(
            mandatory_offline_policy_from_reviewed_public_inputs(
                &chain_id,
                BTreeMap::new(),
                authority.clone(),
                signer_key_pair.public_key().clone(),
                Quantity::from(1_u32),
            )
            .expect_err("empty catalog must reject")
            .message()
            .contains("at least one")
        );
        assert!(
            mandatory_offline_policy_from_reviewed_public_inputs(
                &chain_id,
                escrows,
                authority,
                key_pair(9).public_key().clone(),
                Quantity::from(1_u32),
            )
            .expect_err("substituted public key must reject")
            .message()
            .contains("does not control")
        );
    }

    #[test]
    fn staged_genesis_readiness_requires_the_exact_validated_header() {
        let validated = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            1_800_000_000_000,
            0,
        );
        ensure_staged_genesis_headers_match(&validated, &validated)
            .expect("the exact staged header must be accepted");

        let substituted = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            1_800_000_000_001,
            0,
        );
        let error = ensure_staged_genesis_headers_match(&validated, &substituted)
            .expect_err("a substituted staged header must reject");
        assert!(
            error
                .message()
                .contains("differs from the validated genesis")
        );
    }

    #[test]
    fn effective_bindings_preserve_operator_binding_and_report_replay_conflict() {
        let definition = asset("ds");
        let configured_account = account(1);
        let replayed_account = account(2);
        let mut blockers = Vec::new();
        let effective = effective_escrow_bindings(
            &BTreeMap::from([(definition.clone(), configured_account.clone())]),
            &BTreeMap::from([(definition.clone(), replayed_account)]),
            &BTreeMap::new(),
            &mut blockers,
        );

        assert_eq!(effective.get(&definition), Some(&configured_account));
        assert_eq!(blockers.len(), 1);
        assert_eq!(
            blockers[0].code,
            "offline_escrow_binding_changed_after_replay"
        );
    }

    fn verifier(role: &str, byte: u8) -> OfflineActiveTransferVerifier {
        OfflineActiveTransferVerifier {
            id: OfflineVerifierId {
                backend: crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                name: role.to_owned(),
            },
            version: 1,
            circuit_id: format!("circuit-{role}"),
            commitment: hex::encode([byte; 32]),
            public_inputs_schema_hash: hex::encode([byte.saturating_add(1); 32]),
            max_proof_bytes: 1,
            activation_height: 1,
            withdrawal_height: None,
        }
    }

    fn exact_verifier(
        role: &str,
        circuit_id: &str,
        public_inputs_schema_hash: [u8; 32],
        commitment_byte: u8,
        max_proof_bytes: u32,
        withdrawal_height: Option<u64>,
    ) -> OfflineActiveTransferVerifier {
        OfflineActiveTransferVerifier {
            id: OfflineVerifierId {
                backend: crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                name: role.to_owned(),
            },
            version: 1,
            circuit_id: circuit_id.to_owned(),
            commitment: hex::encode([commitment_byte; 32]),
            public_inputs_schema_hash: hex::encode(public_inputs_schema_hash),
            max_proof_bytes,
            activation_height: 1,
            withdrawal_height,
        }
    }

    fn complete_public_status() -> OfflineStatus {
        let recursive_max_proof_bytes = 1_024;
        let transfer_schema: [u8; 32] =
            Hash::new(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1)
                .into();
        let topup_schema: [u8; 32] = Hash::new(
            crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2,
        )
        .into();
        let unshield_schema: [u8; 32] =
            Hash::new(crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1)
                .into();
        let readiness = OfflineReadiness {
            peer_id: "peer-1".to_owned(),
            cash_handoff_capability: KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1.to_owned(),
            required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            max_hops: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
            asset_definition_id: asset("ds").to_string(),
            asset_scale: Some(2),
            evaluated_block_height: 5,
            evaluated_block_hash: hex::encode([0xA0; 32]),
            active_transfer_verifier: Some(exact_verifier(
                KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2,
                crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
                transfer_schema,
                1,
                1_024,
                None,
            )),
            active_topup_shield_verifier: Some(exact_verifier(
                KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2,
                crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
                topup_schema,
                2,
                1_024,
                None,
            )),
            active_unshield_verifier: Some(exact_verifier(
                KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2,
                crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
                unshield_schema,
                3,
                1_024,
                None,
            )),
            active_recursive_step_eq_verifier: Some(exact_verifier(
                KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
                kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v4(),
                4,
                recursive_max_proof_bytes,
                Some(10),
            )),
            active_recursive_step_ep_verifier: Some(exact_verifier(
                KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v4(),
                5,
                recursive_max_proof_bytes,
                Some(10),
            )),
            artifact_set: Some(OfflineAuthenticatedArtifactSet {
                generation: "release-1".to_owned(),
                manifest_sha256: hex::encode([0x10; 32]),
                release_policy_sha256: hex::encode([0x11; 32]),
                release_attestation_sha256: hex::encode([0x12; 32]),
                activation_height: 1,
                withdrawal_height: 10,
                max_proof_bytes: recursive_max_proof_bytes,
                asset_scale: 2,
            }),
            proof_backend_available: true,
            recursive_lineage_supported: true,
            ready: true,
            blockers: Vec::new(),
        };
        OfflineStatus {
            mandatory: true,
            cash_handoff_capability: KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1.to_owned(),
            required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            max_hops: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
            ready: true,
            assets: vec![readiness],
            blockers: Vec::new(),
        }
    }

    #[test]
    fn five_verifier_roles_must_have_distinct_ids_commitments_and_schemas() {
        let transfer = verifier("transfer", 1);
        let mut topup = verifier("topup", 2);
        topup.id = transfer.id.clone();
        topup.commitment = transfer.commitment.clone();
        topup.public_inputs_schema_hash = transfer.public_inputs_schema_hash.clone();
        let unshield = verifier("unshield", 3);
        let eq = verifier("eq", 4);
        let ep = verifier("ep", 5);

        let blockers = verifier_role_distinctness_blockers([
            ("transfer", Some(&transfer)),
            ("topup_shield", Some(&topup)),
            ("unshield", Some(&unshield)),
            ("recursive_step_eq", Some(&eq)),
            ("recursive_step_ep", Some(&ep)),
        ]);
        assert_eq!(
            blockers
                .iter()
                .map(|blocker| blocker.code.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "offline_verifier_commitment_reused",
                "offline_verifier_registry_id_reused",
                "offline_verifier_schema_hash_reused",
            ])
        );
    }

    #[test]
    fn readiness_enforcer_rejects_empty_or_inconsistent_status() {
        let mut status = OfflineStatus {
            mandatory: true,
            cash_handoff_capability: KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1.to_owned(),
            required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            max_hops: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
            ready: true,
            assets: Vec::new(),
            blockers: Vec::new(),
        };
        let error =
            ensure_mandatory_offline_ready(&status).expect_err("empty positive status must reject");
        assert!(
            error
                .blockers()
                .iter()
                .any(|blocker| blocker.code == "offline_asset_catalog_empty")
        );
        assert!(
            error
                .blockers()
                .iter()
                .any(|blocker| blocker.code == "offline_status_inconsistent")
        );

        status.ready = false;
        let error =
            ensure_mandatory_offline_ready(&status).expect_err("empty status remains unready");
        assert_eq!(error.blockers().len(), 1);
        assert_eq!(error.blockers()[0].code, "offline_asset_catalog_empty");
    }

    #[test]
    fn readiness_enforcer_accepts_complete_public_abi21_v4_evidence() {
        ensure_mandatory_offline_ready(&complete_public_status())
            .expect("complete public ABI-21/V4 evidence must be ready");
    }

    #[test]
    fn readiness_enforcer_rejects_protocol_or_asset_evidence_substitution() {
        let mut wrong_capability = complete_public_status();
        wrong_capability.cash_handoff_capability = "cash_handoff_v0".to_owned();
        let error = ensure_mandatory_offline_ready(&wrong_capability)
            .expect_err("a substituted fleet capability must reject");
        assert!(
            error
                .blockers()
                .iter()
                .any(|blocker| { blocker.code == "offline_cash_handoff_capability_mismatch" })
        );

        let mut missing_verifier = complete_public_status();
        missing_verifier.assets[0].active_recursive_step_ep_verifier = None;
        let error = ensure_mandatory_offline_ready(&missing_verifier)
            .expect_err("an incomplete five-role verifier projection must reject");
        assert!(
            error
                .blockers()
                .iter()
                .any(|blocker| blocker.code == "offline_required_verifier_missing")
        );

        let mut missing_artifact = complete_public_status();
        missing_artifact.assets[0].artifact_set = None;
        let error = ensure_mandatory_offline_ready(&missing_artifact)
            .expect_err("missing public artifact identity must reject");
        assert!(
            error
                .blockers()
                .iter()
                .any(|blocker| { blocker.code == "offline_authenticated_artifact_set_missing" })
        );

        let mut disabled_backend = complete_public_status();
        disabled_backend.assets[0].proof_backend_available = false;
        disabled_backend.assets[0].recursive_lineage_supported = false;
        let error = ensure_mandatory_offline_ready(&disabled_backend)
            .expect_err("unavailable backend and lineage must reject");
        let codes = error
            .blockers()
            .iter()
            .map(|blocker| blocker.code.as_str())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("offline_proof_backend_unavailable"));
        assert!(codes.contains("offline_recursive_lineage_unsupported"));
    }

    #[test]
    fn blocker_order_is_stable_and_duplicate_free() {
        let mut blockers = vec![
            blocker("z", "second"),
            blocker("a", "first"),
            blocker("z", "second"),
            blocker("a", "another"),
        ];
        sort_and_dedup_blockers(&mut blockers);
        assert_eq!(
            blockers
                .iter()
                .map(|blocker| (blocker.code.as_str(), blocker.message.as_str()))
                .collect::<Vec<_>>(),
            vec![("a", "another"), ("a", "first"), ("z", "second"),]
        );
    }

    #[test]
    fn source_contract_is_unconditional_complete_and_uses_one_evaluator() {
        let source = include_str!("offline_readiness.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        for required in [
            "pub fn evaluate_committed_mandatory_offline",
            "pub fn evaluate_staged_genesis_mandatory_offline",
            "fn evaluate_snapshot",
            "ensure_kagemusha_active_release_material_v4",
            "ensure_offline_device_attestation_policy_ready_v1",
            "world_has_offline_escrow_manager_permission",
            "ensure_confidential_transfer_v2_canonical_vk_box",
            "ensure_kagemusha_topup_shield_v2_canonical_vk_box",
            "ensure_confidential_unshield_v3_canonical_vk_box",
            "verifier_role_distinctness_blockers",
            "state_view.nexus.fees.fee_asset_id",
            "staged_genesis.nexus.fees.fee_asset_id",
        ] {
            assert!(
                production.contains(required),
                "authoritative readiness omitted `{required}`"
            );
        }
        for forbidden in [
            "cfg(feature = \"app_api\")",
            "iroha_torii",
            "KagemushaReleaseCatalogV4::empty",
        ] {
            assert!(
                !production.contains(forbidden),
                "authoritative readiness contains forbidden `{forbidden}`"
            );
        }

        let policy_body = production
            .split("pub struct MandatoryOfflinePolicy")
            .nth(1)
            .and_then(|tail| tail.split("impl MandatoryOfflinePolicy").next())
            .expect("policy struct body");
        assert!(
            !policy_body.contains("KeyPair"),
            "readiness policy must never retain issuer private material"
        );
        assert!(
            production.matches("evaluate_snapshot(").count() >= 3,
            "both snapshot wrappers must call the single evaluator"
        );

        let offline_isi_source = include_str!("smartcontracts/isi/offline.rs");
        let device_readiness = offline_isi_source
            .split("pub fn ensure_offline_device_attestation_policy_ready_v1")
            .nth(1)
            .and_then(|tail| tail.split("/// Derive the canonical").next())
            .expect("device readiness function body");
        assert!(
            device_readiness.contains("validate_offline_attestation_policy_for_release_activation"),
            "device readiness must enforce the production iOS and Android release policy"
        );
    }

    #[test]
    fn test_fixture_verifier_ids_remain_portable() {
        let id = VerifyingKeyId::new(crate::zk::ZK_BACKEND_HALO2_IPA, "readiness-test");
        assert!(id.is_portable_registry_id());
    }
}

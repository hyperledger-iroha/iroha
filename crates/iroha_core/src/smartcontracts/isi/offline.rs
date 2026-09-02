//! Offline Cash V1 pooled-reserve instruction execution.

pub(crate) mod offline_cash_v1_reserve;

use std::{collections::BTreeMap, path::Path};

use super::prelude::*;
use crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with;
use halo2_base::gates::circuit::BaseCircuitParams;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinitionId, AssetId},
    block::consensus_v2::{HeightContextId, finality::V2FinalityArtifact},
    domain::DomainId,
    isi::offline_cash_v1::{
        OFFLINE_CASH_MINT_FINALITY_TREE_DEPTH_V1, OfflineCashMintFinalityEpochRosterV1,
        OfflineCashMintFinalitySealBundleV1, OfflineCashMintFinalitySealMessageV1,
        OfflineCashTopUpLeafV1, OfflineCashTopUpMembershipWitnessV1,
        offline_cash_mint_finality_root_v1,
    },
    isi::{
        OFFLINE_CASH_CHAIN_VERSION_V1, OfflineCashFinalityTrustAnchorV1,
        OfflineCashOperationFinalityV1, OfflineCashRedemptionRequestV1, OfflineCashTopUpRequestV1,
        OfflineCashTopUpResultV1, RedeemOfflineCashV1, TopUpOfflineCashV1,
        error::InstructionExecutionError,
    },
    nexus::AxtAssetIncarnationV1,
    offline::{
        OFFLINE_CASH_V1_REJECTION_REASON_PREFIX, OFFLINE_CASH_WIRE_VERSION_V1,
        OfflineCashAuthenticatedReleaseV1, OfflineCashEnabledProfileV1,
        OfflineCashHardwareProfileV1, OfflineCashInternalValidationReceiptV1,
        OfflineCashLifecycleBindingV1, OfflineCashMintCreditStatementV1, OfflineCashMintCreditV1,
        OfflineCashReleaseAttestationV1, OfflineCashReleaseAuthorityPolicyV1,
        OfflineCashReleaseManifestV1, offline_cash_liability_pool_id_v1,
    },
};
use iroha_primitives::numeric::{Numeric, Quantity};
use norito::JsonDeserialize;
use sha2::{Digest as _, Sha256};

use crate::zk::{
    offline_cash_v1_recursion::{
        OfflineCashAuthenticatedArtifactSetV1, OfflineCashAuthenticatedRecursiveVerifierV1,
        OfflineCashDirectoryArtifactResolverV1, OfflineCashLoadedEpMintAuthorityArtifactsV1,
        OfflineCashLoadedEqMintAuthorityArtifactsV1, OfflineCashMintAuthorityCheckpointV1,
        OfflineCashMintCertificateWitnessV1, OfflineCashRecursiveVerifierProfileV1,
        decode_offline_cash_mint_finality_seal_bundle_v1,
        load_offline_cash_ep_mint_authority_artifacts_v1,
        load_offline_cash_eq_mint_authority_artifacts_v1, offline_cash_mint_finality_empty_root_v1,
        prove_offline_cash_finalized_mint_from_checkpoint_v1,
        prove_offline_cash_mint_authority_bootstrap_v1,
        prove_offline_cash_mint_authority_rotation_from_checkpoint_v1,
        verify_offline_cash_mint_finality_helper_v1,
    },
    offline_cash_v1_state::OfflineCashStateProofReleaseV1,
};

pub use offline_cash_v1_reserve::{
    OfflineCashRedemptionRecordV1, OfflineCashReserveOperationRecordV1, OfflineCashTopUpRecordV1,
};

/// Maximum canonical JSON bytes for one authenticated recursive verifier profile.
pub const OFFLINE_CASH_RECURSIVE_PROFILE_MAX_BYTES_V1: usize = 64 * 1024;

#[derive(Debug, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct OfflineCashBaseCircuitProfileFileV1 {
    k: u64,
    num_advice_per_phase: Vec<u64>,
    num_fixed: u64,
    num_lookup_advice_per_phase: Vec<u64>,
    lookup_bits: Option<u64>,
    num_instance_columns: u64,
}

#[derive(Debug, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct OfflineCashRecursiveVerifierProfileFileV1 {
    state_eq: OfflineCashBaseCircuitProfileFileV1,
    state_ep: OfflineCashBaseCircuitProfileFileV1,
    guard_eq: OfflineCashBaseCircuitProfileFileV1,
    guard_ep: OfflineCashBaseCircuitProfileFileV1,
    wrapper_eq: OfflineCashBaseCircuitProfileFileV1,
    wrapper_ep: OfflineCashBaseCircuitProfileFileV1,
    mint_authorization_eq: OfflineCashBaseCircuitProfileFileV1,
    mint_authorization_ep: OfflineCashBaseCircuitProfileFileV1,
    mint_eq: OfflineCashBaseCircuitProfileFileV1,
    mint_ep: OfflineCashBaseCircuitProfileFileV1,
    mint_eq_protocol_digest: [u8; 32],
    mint_ep_protocol_digest: [u8; 32],
    mint_genesis_roster_id: [u8; 32],
}

/// Non-serializable authority proving that one exact top-up request selected an enabled profile
/// from its authenticated release.
#[derive(Clone, Debug)]
pub(in crate::smartcontracts::isi) struct VerifiedOfflineCashTopUpAuthorizationV1 {
    request_digest: [u8; 32],
    mint_authorization_digest: [u8; 32],
    profile: OfflineCashHardwareProfileV1,
}

impl VerifiedOfflineCashTopUpAuthorizationV1 {
    fn mint_statement(
        &self,
        request: &OfflineCashTopUpRequestV1,
        committed_at_ms: u64,
    ) -> Result<OfflineCashMintCreditStatementV1, String> {
        let request_digest = request
            .canonical_digest()
            .map_err(|error| format!("invalid Offline Cash V1 top-up request: {error}"))?;
        if request_digest != self.request_digest {
            return Err("Offline Cash V1 verified top-up request was substituted".to_owned());
        }
        let mint_authorization_digest = request
            .mint_authorization
            .as_ref()
            .ok_or_else(|| "Offline Cash V1 top-up lacks mint authorization".to_owned())?
            .canonical_digest()
            .map_err(|error| format!("invalid Offline Cash V1 mint authorization: {error}"))?;
        if mint_authorization_digest != self.mint_authorization_digest {
            return Err("Offline Cash V1 verified mint authorization was substituted".to_owned());
        }
        request
            .mint_statement_against_profile(&self.profile, committed_at_ms)
            .map_err(|error| format!("invalid Offline Cash V1 mint statement: {error}"))
    }

    fn mint_statement_digest(
        &self,
        request: &OfflineCashTopUpRequestV1,
        committed_at_ms: u64,
    ) -> Result<[u8; 32], String> {
        let statement = self.mint_statement(request, committed_at_ms)?;
        statement
            .canonical_digest()
            .map_err(|error| format!("invalid Offline Cash V1 mint statement digest: {error}"))
    }
}

/// One-shot authority to debit an online account into the pooled Offline Cash V1 reserve.
pub(in crate::smartcontracts::isi) struct VerifiedOfflineCashTopUpDebitV1 {
    source_authority: AccountId,
    operation_id: [u8; 32],
    source_id: AssetId,
    destination_id: AssetId,
    amount: Quantity,
}

impl VerifiedOfflineCashTopUpDebitV1 {
    fn new(
        source_authority: AccountId,
        operation_id: [u8; 32],
        source_id: AssetId,
        destination_id: AssetId,
        amount: Quantity,
    ) -> Self {
        Self {
            source_authority,
            operation_id,
            source_id,
            destination_id,
            amount,
        }
    }

    pub(in crate::smartcontracts::isi) fn into_parts(
        self,
    ) -> (AccountId, [u8; 32], AssetId, AssetId, Quantity) {
        (
            self.source_authority,
            self.operation_id,
            self.source_id,
            self.destination_id,
            self.amount,
        )
    }
}

/// One-shot authority to debit the pooled reserve after recursive proof verification.
pub(in crate::smartcontracts::isi) struct VerifiedOfflineCashRedemptionDebitV1 {
    operation_id: [u8; 32],
    source_id: AssetId,
    destination_id: AssetId,
    amount: Quantity,
}

impl VerifiedOfflineCashRedemptionDebitV1 {
    fn new(
        operation_id: [u8; 32],
        source_id: AssetId,
        destination_id: AssetId,
        amount: Quantity,
    ) -> Self {
        Self {
            operation_id,
            source_id,
            destination_id,
            amount,
        }
    }

    pub(in crate::smartcontracts::isi) fn into_parts(
        self,
    ) -> ([u8; 32], AssetId, AssetId, Quantity) {
        (
            self.operation_id,
            self.source_id,
            self.destination_id,
            self.amount,
        )
    }
}

/// Node runtime boundary that resolves authenticated Offline Cash V1 releases and artifacts.
///
/// Implementations must resolve the request's `release_id` to threshold-authenticated release
/// metadata and content-addressed artifact bytes. Redemption must return only the opaque token
/// produced by the paired recursive verifier; structural request validation is insufficient.
pub trait OfflineCashV1RuntimeVerifier: Send + Sync {
    /// Return all installed authenticated releases in canonical identifier order.
    fn mint_release_ids(&self) -> Vec<[u8; 32]>;

    /// Admit a top-up only when its exact release and artifact manifest are authenticated.
    fn verify_top_up_authorization(
        &self,
        request: &OfflineCashTopUpRequestV1,
    ) -> Result<VerifiedOfflineCashTopUpAuthorizationV1, String>;

    /// Resolve artifacts and recursively verify an exact redemption request.
    fn verify_redemption_request(
        &self,
        request: OfflineCashRedemptionRequestV1,
    ) -> Result<crate::zk::offline_cash_v1_recursion::VerifiedOfflineCashRedemptionProofV1, String>;

    /// Prove the zero-authority bootstrap for an authenticated release and its pinned roster.
    ///
    /// This runs only when Kura has no durable checkpoint. The proof is generated after release
    /// authentication, then terminally reverified before persistence; it is deliberately absent
    /// from the profile digest so the release identity cannot depend on a proof that embeds that
    /// same release identity.
    fn prove_mint_authority_bootstrap(
        &self,
        release_id: [u8; 32],
        epoch_roster: &OfflineCashMintFinalityEpochRosterV1,
    ) -> Result<OfflineCashMintAuthorityCheckpointV1, String>;

    /// Produce the immutable mint result for one canonical finalized reserve top-up.
    ///
    /// The implementation must recursively verify `authority_checkpoint` and prove the exact
    /// Kura finality evidence in both Pasta circuits. Native finality checks are preflight only.
    fn prove_finalized_top_up(
        &self,
        record: &OfflineCashTopUpRecordV1,
        finality: OfflineCashOperationFinalityV1,
        authority_checkpoint: &OfflineCashMintAuthorityCheckpointV1,
    ) -> Result<OfflineCashTopUpResultV1, String>;

    /// Advance an authority checkpoint at an epoch boundary certified by the current roster.
    fn prove_mint_authority_rotation(
        &self,
        release_id: [u8; 32],
        finality_artifact: &V2FinalityArtifact,
        top_up_membership: Option<OfflineCashTopUpMembershipWitnessV1>,
        authority_checkpoint: &OfflineCashMintAuthorityCheckpointV1,
    ) -> Result<OfflineCashMintAuthorityCheckpointV1, String>;
}

/// Fail-closed runtime used when no authenticated V1 release resolver is configured.
#[derive(Clone, Copy, Debug, Default)]
pub struct RejectAllOfflineCashV1RuntimeVerifier;

impl OfflineCashV1RuntimeVerifier for RejectAllOfflineCashV1RuntimeVerifier {
    fn mint_release_ids(&self) -> Vec<[u8; 32]> {
        Vec::new()
    }

    fn verify_top_up_authorization(
        &self,
        _request: &OfflineCashTopUpRequestV1,
    ) -> Result<VerifiedOfflineCashTopUpAuthorizationV1, String> {
        Err("authenticated Offline Cash V1 release resolver is unavailable".to_owned())
    }

    fn verify_redemption_request(
        &self,
        _request: OfflineCashRedemptionRequestV1,
    ) -> Result<crate::zk::offline_cash_v1_recursion::VerifiedOfflineCashRedemptionProofV1, String>
    {
        Err("authenticated Offline Cash V1 recursive verifier is unavailable".to_owned())
    }

    fn prove_mint_authority_bootstrap(
        &self,
        _release_id: [u8; 32],
        _epoch_roster: &OfflineCashMintFinalityEpochRosterV1,
    ) -> Result<OfflineCashMintAuthorityCheckpointV1, String> {
        Err("authenticated Offline Cash V1 mint authority is unavailable".to_owned())
    }

    fn prove_finalized_top_up(
        &self,
        _record: &OfflineCashTopUpRecordV1,
        _finality: OfflineCashOperationFinalityV1,
        _authority_checkpoint: &OfflineCashMintAuthorityCheckpointV1,
    ) -> Result<OfflineCashTopUpResultV1, String> {
        Err("authenticated Offline Cash V1 mint prover is unavailable".to_owned())
    }

    fn prove_mint_authority_rotation(
        &self,
        _release_id: [u8; 32],
        _finality_artifact: &V2FinalityArtifact,
        _top_up_membership: Option<OfflineCashTopUpMembershipWitnessV1>,
        _authority_checkpoint: &OfflineCashMintAuthorityCheckpointV1,
    ) -> Result<OfflineCashMintAuthorityCheckpointV1, String> {
        Err("authenticated Offline Cash V1 mint rotation prover is unavailable".to_owned())
    }
}

struct AuthenticatedOfflineCashV1ReleaseRuntime {
    artifacts: OfflineCashAuthenticatedArtifactSetV1<OfflineCashDirectoryArtifactResolverV1>,
    verifier: OfflineCashAuthenticatedRecursiveVerifierV1,
    eq_mint_prover: OfflineCashLoadedEqMintAuthorityArtifactsV1,
    ep_mint_prover: OfflineCashLoadedEpMintAuthorityArtifactsV1,
    enabled_profiles: Vec<OfflineCashEnabledProfileV1>,
}

/// Runtime registry of threshold-authenticated Offline Cash V1 proof releases.
///
/// Every installed release reauthenticates its complete content-addressed artifact inventory,
/// recompiles the fixed recursive protocols, and checks the resulting identities against the
/// threshold-signed manifest before it becomes visible to instruction execution.
pub struct AuthenticatedOfflineCashV1RuntimeVerifier {
    releases: BTreeMap<[u8; 32], AuthenticatedOfflineCashV1ReleaseRuntime>,
}

impl AuthenticatedOfflineCashV1RuntimeVerifier {
    /// Construct a registry containing one fully authenticated release.
    ///
    /// # Errors
    ///
    /// Returns an error for an inaccessible artifact directory, malformed release-fixed state,
    /// missing or substituted artifact bytes, or a recursive profile/key identity mismatch.
    pub fn from_authenticated_release(
        release: &OfflineCashAuthenticatedReleaseV1,
        artifact_root: impl AsRef<Path>,
        profile: OfflineCashRecursiveVerifierProfileV1,
    ) -> Result<Self, String> {
        let mut registry = Self {
            releases: BTreeMap::new(),
        };
        registry.install_authenticated_release(release, artifact_root, profile)?;
        Ok(registry)
    }

    /// Add another non-aliasing authenticated release for in-flight wallets during rotation.
    ///
    /// # Errors
    ///
    /// Returns an error for a duplicate release identifier or any release, artifact, or compiled
    /// protocol authentication failure.
    pub fn install_authenticated_release(
        &mut self,
        release: &OfflineCashAuthenticatedReleaseV1,
        artifact_root: impl AsRef<Path>,
        profile: OfflineCashRecursiveVerifierProfileV1,
    ) -> Result<(), String> {
        let release_id = release.release_id();
        if self.releases.contains_key(&release_id) {
            return Err("Offline Cash V1 release is already installed".to_owned());
        }
        let resolver =
            OfflineCashDirectoryArtifactResolverV1::new(artifact_root).map_err(|error| {
                format!("failed to open Offline Cash V1 artifact directory: {error}")
            })?;
        let state_release = OfflineCashStateProofReleaseV1::from_authenticated_release(release)
            .map_err(|error| format!("invalid Offline Cash V1 state proof release: {error}"))?;
        let artifacts = OfflineCashAuthenticatedArtifactSetV1::new(
            release,
            state_release.canonical_empty_effect_digest(),
            resolver,
        )
        .map_err(|error| format!("invalid Offline Cash V1 artifact set: {error}"))?;
        let eq_mint_prover = load_offline_cash_eq_mint_authority_artifacts_v1(
            &artifacts,
            profile.mint_eq.clone(),
        )
        .map_err(|error| format!("failed to load Offline Cash V1 Eq mint prover: {error}"))?;
        let ep_mint_prover = load_offline_cash_ep_mint_authority_artifacts_v1(
            &artifacts,
            profile.mint_ep.clone(),
        )
        .map_err(|error| format!("failed to load Offline Cash V1 Ep mint prover: {error}"))?;
        let verifier = OfflineCashAuthenticatedRecursiveVerifierV1::load(&artifacts, profile)
            .map_err(|error| {
                format!("failed to load Offline Cash V1 recursive verifier: {error}")
            })?;
        self.releases.insert(
            release_id,
            AuthenticatedOfflineCashV1ReleaseRuntime {
                artifacts,
                verifier,
                eq_mint_prover,
                ep_mint_prover,
                enabled_profiles: release.enabled_profiles().to_vec(),
            },
        );
        Ok(())
    }

    /// Return the number of authenticated releases available to replay and new instructions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.releases.len()
    }

    /// Return whether no authenticated proof release has been installed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.releases.is_empty()
    }
}

fn offline_cash_mint_authority_bootstrap_certificate_v1(
    release_id: [u8; 32],
    genesis_roster_id: [u8; 32],
    epoch_roster: &OfflineCashMintFinalityEpochRosterV1,
) -> Result<OfflineCashMintCertificateWitnessV1, String> {
    epoch_roster
        .validate()
        .map_err(|error| format!("invalid Offline Cash genesis finality roster: {error}"))?;
    let actual_roster_id = epoch_roster
        .finality_epoch_id()
        .map_err(|error| format!("failed to digest Offline Cash genesis roster: {error}"))?;
    if release_id == [0; 32] || actual_roster_id != genesis_roster_id {
        return Err(
            "Offline Cash bootstrap roster differs from the authenticated release profile"
                .to_owned(),
        );
    }
    let first_validator = epoch_roster
        .validators
        .first()
        .ok_or_else(|| "Offline Cash bootstrap roster is empty".to_owned())?;
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("offline-cash-bootstrap", "universal")
            .map_err(|error| format!("invalid bootstrap asset domain: {error}"))?,
        "authority"
            .parse()
            .map_err(|error| format!("invalid bootstrap asset name: {error}"))?,
    );
    let network_id = epoch_roster.network_id;
    let binding = |label: &[u8]| {
        let mut hasher = Sha256::new();
        hasher.update(b"iroha:offline-cash:v1:mint-authority-bootstrap");
        hasher.update([0]);
        hasher.update(label);
        hasher.update(network_id.as_bytes());
        hasher.update(genesis_roster_id);
        <[u8; 32]>::from(hasher.finalize())
    };
    let incarnation_bytes: [u8; 32] = Hash::new(binding(b"asset-incarnation")).into();
    let asset_incarnation = AxtAssetIncarnationV1::try_from_bytes(incarnation_bytes)
        .map_err(|error| format!("failed to derive bootstrap asset incarnation: {error}"))?;
    let statement = OfflineCashMintCreditStatementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        lifecycle: OfflineCashLifecycleBindingV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            network_id,
            protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
            suite_id: binding(b"suite"),
            vk_digest: binding(b"vk"),
            release_id,
            asset: asset.clone(),
            asset_incarnation,
            scale: 0,
            liability_pool_id: offline_cash_liability_pool_id_v1(
                &network_id,
                &asset,
                asset_incarnation,
            )
            .map_err(|error| format!("failed to derive bootstrap liability pool: {error}"))?,
            hardware_profile_id: binding(b"hardware-profile"),
            policy_epoch: 1,
            operation_kind: iroha_data_model::offline::OfflineCashOperationKindV1::MintFold,
            request_id: [0; 32],
            acceptance_ticket_id: [0; 32],
            credit_id: [0; 32],
            ciphertext_digest: binding(b"ciphertext"),
        },
        recipient_credential_commitment: binding(b"recipient-credential"),
        authorization_context_digest: binding(b"authorization-context"),
        mint_authorization_digest: binding(b"mint-authorization"),
        amount: 1,
        issuance_commitment: binding(b"issuance"),
        recipient: AccountId::new(first_validator.validator.public_key().clone()),
        credit_commitment: binding(b"credit"),
        minted_at_ms: 1,
    }
    .seal_credit_id()
    .map_err(|error| format!("failed to seal bootstrap statement shape: {error}"))?;
    let statement_digest = statement
        .canonical_digest()
        .map_err(|error| format!("failed to digest bootstrap statement shape: {error}"))?;
    let root = offline_cash_mint_finality_empty_root_v1()
        .map_err(|error| format!("failed to derive bootstrap empty root: {error}"))?;
    let membership = OfflineCashTopUpMembershipWitnessV1 {
        leaf: OfflineCashTopUpLeafV1 {
            version: OFFLINE_CASH_CHAIN_VERSION_V1,
            operation_id: binding(b"operation"),
            reserve_receipt_digest: binding(b"receipt"),
            statement_digest,
            amount: statement.amount,
        },
        leaf_index: 0,
        root,
        siblings: vec![
            iroha_data_model::offline::OfflineCashPastaStateCommitmentV1::ZERO;
            OFFLINE_CASH_MINT_FINALITY_TREE_DEPTH_V1
        ],
    };
    let message = OfflineCashMintFinalitySealMessageV1 {
        version: OFFLINE_CASH_CHAIN_VERSION_V1,
        finality_epoch_id: genesis_roster_id,
        validator_count: u32::try_from(epoch_roster.validators.len())
            .map_err(|_| "Offline Cash bootstrap roster exceeds u32".to_owned())?,
        network_id,
        block_height: 1,
        height_context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(binding(
            b"height-context",
        )))),
        subject_digest: binding(b"subject"),
        execution_commitment_digest: binding(b"execution"),
        offline_cash_top_up_root: offline_cash_mint_finality_root_v1(root),
        offline_cash_top_up_count: 0,
        next_finality_epoch_id: Some(genesis_roster_id),
    };
    let certificate = OfflineCashMintCertificateWitnessV1 {
        statement,
        membership,
        seal_bundle: OfflineCashMintFinalitySealBundleV1 {
            message,
            seals: Vec::new(),
        },
        epoch_roster: epoch_roster.clone(),
    };
    Ok(certificate)
}

impl OfflineCashV1RuntimeVerifier for AuthenticatedOfflineCashV1RuntimeVerifier {
    fn mint_release_ids(&self) -> Vec<[u8; 32]> {
        self.releases.keys().copied().collect()
    }

    fn verify_top_up_authorization(
        &self,
        request: &OfflineCashTopUpRequestV1,
    ) -> Result<VerifiedOfflineCashTopUpAuthorizationV1, String> {
        let runtime = self
            .releases
            .get(&request.release_id)
            .ok_or_else(|| "Offline Cash V1 proof release is not installed".to_owned())?;
        let artifacts = runtime.artifacts.recursion_artifacts();
        if request.artifact_manifest_digest != artifacts.artifact_manifest_digest {
            return Err("Offline Cash V1 artifact manifest identity mismatch".to_owned());
        }
        let enabled = runtime
            .enabled_profiles
            .binary_search_by_key(
                &request.hardware_credential.hardware_profile_id,
                |profile| profile.hardware_profile_id,
            )
            .ok()
            .map(|index| &runtime.enabled_profiles[index])
            .ok_or_else(|| {
                "Offline Cash V1 hardware profile is not enabled by the release".to_owned()
            })?;
        if request.suite_id != enabled.suite_id
            || request.vk_digest != enabled.vk_digest
            || request.hardware_credential.policy_epoch != enabled.policy_epoch
        {
            return Err("Offline Cash V1 release profile binding mismatch".to_owned());
        }
        request
            .validate_against_profile(&enabled.hardware_profile)
            .map_err(|error| format!("invalid Offline Cash V1 hardware credential: {error}"))?;
        let mint_authorization = request
            .mint_authorization
            .as_ref()
            .ok_or_else(|| "Offline Cash V1 top-up lacks mint authorization".to_owned())?;
        runtime
            .verifier
            .verify_mint_authorization(mint_authorization)
            .map_err(|error| {
                format!("invalid Offline Cash V1 paired mint authorization proof: {error}")
            })?;
        let request_digest = request
            .canonical_digest()
            .map_err(|error| format!("invalid Offline Cash V1 top-up request: {error}"))?;
        let mint_authorization_digest = mint_authorization
            .canonical_digest()
            .map_err(|error| format!("invalid Offline Cash V1 mint authorization: {error}"))?;
        Ok(VerifiedOfflineCashTopUpAuthorizationV1 {
            request_digest,
            mint_authorization_digest,
            profile: enabled.hardware_profile.clone(),
        })
    }

    fn verify_redemption_request(
        &self,
        request: OfflineCashRedemptionRequestV1,
    ) -> Result<crate::zk::offline_cash_v1_recursion::VerifiedOfflineCashRedemptionProofV1, String>
    {
        let lifecycle = &request.voucher.statement.lifecycle;
        let release_id = lifecycle.release_id;
        let runtime = self
            .releases
            .get(&release_id)
            .ok_or_else(|| "Offline Cash V1 proof release is not installed".to_owned())?;
        let artifacts = runtime.artifacts.recursion_artifacts();
        if request.voucher.artifact_manifest_digest != artifacts.artifact_manifest_digest {
            return Err("Offline Cash V1 artifact manifest identity mismatch".to_owned());
        }
        let enabled = runtime
            .enabled_profiles
            .binary_search_by_key(&lifecycle.hardware_profile_id, |profile| {
                profile.hardware_profile_id
            })
            .ok()
            .map(|index| &runtime.enabled_profiles[index])
            .ok_or_else(|| {
                "Offline Cash V1 redemption hardware profile is not enabled by the release"
                    .to_owned()
            })?;
        if lifecycle.suite_id != enabled.suite_id
            || lifecycle.vk_digest != enabled.vk_digest
            || lifecycle.policy_epoch != enabled.policy_epoch
        {
            return Err("Offline Cash V1 redemption release profile binding mismatch".to_owned());
        }
        runtime
            .artifacts
            .verify_redemption_request(&runtime.verifier, request)
            .map_err(|error| error.to_string())
    }

    fn prove_mint_authority_bootstrap(
        &self,
        release_id: [u8; 32],
        epoch_roster: &OfflineCashMintFinalityEpochRosterV1,
    ) -> Result<OfflineCashMintAuthorityCheckpointV1, String> {
        let runtime = self
            .releases
            .get(&release_id)
            .ok_or_else(|| "Offline Cash V1 proof release is not installed".to_owned())?;
        let certificate = offline_cash_mint_authority_bootstrap_certificate_v1(
            release_id,
            runtime.verifier.mint_genesis_roster_id(),
            epoch_roster,
        )?;
        let checkpoint = prove_offline_cash_mint_authority_bootstrap_v1(
            &runtime.eq_mint_prover,
            &runtime.ep_mint_prover,
            release_id,
            runtime.verifier.mint_genesis_roster_id(),
            certificate,
        )
        .map_err(|error| format!("failed to prove Offline Cash mint bootstrap: {error}"))?;
        runtime
            .verifier
            .verify_mint_authority_checkpoint(&checkpoint)?;
        Ok(checkpoint)
    }

    fn prove_finalized_top_up(
        &self,
        record: &OfflineCashTopUpRecordV1,
        finality: OfflineCashOperationFinalityV1,
        authority_checkpoint: &OfflineCashMintAuthorityCheckpointV1,
    ) -> Result<OfflineCashTopUpResultV1, String> {
        let request = &record.issuance_intent.request;
        let runtime = self
            .releases
            .get(&record.release_id)
            .ok_or_else(|| "Offline Cash V1 proof release is not installed".to_owned())?;
        if request.release_id != record.release_id
            || request.artifact_manifest_digest
                != runtime
                    .artifacts
                    .recursion_artifacts()
                    .artifact_manifest_digest
        {
            return Err("finalized top-up differs from its authenticated proof release".to_owned());
        }
        let trust_anchor = OfflineCashFinalityTrustAnchorV1 {
            network_id: finality.finality_artifact.height_context.network_id,
            block_height: finality.finality_artifact.height,
            height_context_id: finality.finality_artifact.context_id(),
        };
        finality
            .validate_against(&trust_anchor)
            .map_err(|error| format!("invalid canonical top-up finality: {error}"))?;
        if finality.reserve_receipt_witness.receipt != record.reserve_receipt {
            return Err("canonical finality receipt differs from reserve state".to_owned());
        }
        let verified_authorization = self.verify_top_up_authorization(request)?;
        let statement = verified_authorization
            .mint_statement(request, record.reserve_receipt.committed_at_ms)?;
        let membership = finality
            .top_up_membership_witness
            .clone()
            .ok_or_else(|| "canonical top-up finality lacks its membership witness".to_owned())?;
        let seal_payload = finality
            .finality_artifact
            .commit_qc
            .offline_cash_finality_seal_payload()
            .map_err(|error| format!("invalid Offline Cash CommitQC envelope: {error}"))?
            .ok_or_else(|| "finalized top-up lacks its paired-Pasta seal bundle".to_owned())?;
        let seal_bundle = decode_offline_cash_mint_finality_seal_bundle_v1(seal_payload)
            .map_err(|error| format!("invalid finalized mint seal bundle: {error}"))?;
        let certificate = OfflineCashMintCertificateWitnessV1 {
            statement: statement.clone(),
            membership,
            seal_bundle,
            epoch_roster: finality
                .finality_artifact
                .height_context
                .offline_cash_mint_finality_epoch_roster
                .clone(),
        };
        let generated = prove_offline_cash_finalized_mint_from_checkpoint_v1(
            &runtime.eq_mint_prover,
            &runtime.ep_mint_prover,
            &runtime.verifier,
            certificate,
            authority_checkpoint,
        )
        .map_err(|error| format!("failed to prove finalized Offline Cash mint: {error}"))?;
        let mint_credit = OfflineCashMintCreditV1 {
            version: iroha_data_model::offline::OFFLINE_CASH_WIRE_VERSION_V1,
            statement,
            proof: generated.proof,
            finality_certificate_binding: generated.certificate_binding,
            finality_authority_head: generated.authority_head,
            finality_genesis_roster_id: generated.genesis_roster_id,
            finality_proof_binding_digest: generated.proof_binding_digest,
            encrypted_credit: request.encrypted_credit.clone(),
            artifact_manifest_digest: request.artifact_manifest_digest,
        };
        let _verified_finality = verify_offline_cash_mint_finality_helper_v1(
            &runtime.verifier,
            runtime.artifacts.recursion_artifacts(),
            &mint_credit,
        )
        .map_err(|error| format!("generated finalized-mint proof was rejected: {error}"))?;
        let result = OfflineCashTopUpResultV1 {
            version: OFFLINE_CASH_CHAIN_VERSION_V1,
            request: request.clone(),
            finality,
            mint_credit,
        };
        result
            .validate_against(&trust_anchor)
            .map_err(|error| format!("generated top-up result is invalid: {error}"))?;
        Ok(result)
    }

    fn prove_mint_authority_rotation(
        &self,
        release_id: [u8; 32],
        finality_artifact: &V2FinalityArtifact,
        top_up_membership: Option<OfflineCashTopUpMembershipWitnessV1>,
        authority_checkpoint: &OfflineCashMintAuthorityCheckpointV1,
    ) -> Result<OfflineCashMintAuthorityCheckpointV1, String> {
        let runtime = self
            .releases
            .get(&release_id)
            .ok_or_else(|| "Offline Cash V1 proof release is not installed".to_owned())?;
        finality_artifact
            .verify()
            .map_err(|error| format!("invalid boundary finality artifact: {error}"))?;
        let seal_payload = finality_artifact
            .commit_qc
            .offline_cash_finality_seal_payload()
            .map_err(|error| format!("invalid Offline Cash boundary CommitQC envelope: {error}"))?
            .ok_or_else(|| "epoch boundary lacks its paired-Pasta seal bundle".to_owned())?;
        let seal_bundle = decode_offline_cash_mint_finality_seal_bundle_v1(seal_payload)
            .map_err(|error| format!("invalid boundary mint seal bundle: {error}"))?;
        if seal_bundle.message.next_finality_epoch_id.is_none() {
            return Err("mint authority rotation seal lacks the next roster identifier".to_owned());
        }
        let membership = match top_up_membership {
            Some(membership) => membership,
            None if seal_bundle.message.offline_cash_top_up_count == 0 => {
                let statement_digest = authority_checkpoint
                    .statement
                    .canonical_digest()
                    .map_err(|error| error.to_string())?;
                OfflineCashTopUpMembershipWitnessV1 {
                    leaf: OfflineCashTopUpLeafV1 {
                        version: OFFLINE_CASH_CHAIN_VERSION_V1,
                        operation_id: authority_checkpoint.statement.lifecycle.credit_id,
                        reserve_receipt_digest: authority_checkpoint.certificate_binding,
                        statement_digest,
                        amount: authority_checkpoint.statement.amount,
                    },
                    leaf_index: 0,
                    root: offline_cash_mint_finality_empty_root_v1()
                        .map_err(|error| error.to_string())?,
                    siblings: vec![
                        iroha_data_model::offline::OfflineCashPastaStateCommitmentV1::ZERO;
                        OFFLINE_CASH_MINT_FINALITY_TREE_DEPTH_V1
                    ],
                }
            }
            None => {
                return Err(
                    "non-empty boundary top-up root lacks a canonical membership witness"
                        .to_owned(),
                );
            }
        };
        let certificate = OfflineCashMintCertificateWitnessV1 {
            statement: authority_checkpoint.statement.clone(),
            membership,
            seal_bundle,
            epoch_roster: finality_artifact
                .height_context
                .offline_cash_mint_finality_epoch_roster
                .clone(),
        };
        let checkpoint = prove_offline_cash_mint_authority_rotation_from_checkpoint_v1(
            &runtime.eq_mint_prover,
            &runtime.ep_mint_prover,
            &runtime.verifier,
            certificate,
            authority_checkpoint,
        )
        .map_err(|error| format!("failed to prove Offline Cash mint roster rotation: {error}"))?;
        runtime
            .verifier
            .verify_mint_authority_checkpoint(&checkpoint)?;
        Ok(checkpoint)
    }
}

/// Authenticate one complete proof release and build its accepting runtime verifier.
///
/// The authority policy is a locally configured trust root. All other bytes are authenticated by
/// its threshold attestation, while artifact files are resolved only by their manifest SHA-256
/// address. No request-supplied digest or host verification result grants monetary authority.
///
/// # Errors
///
/// Returns an error for malformed, oversized, non-canonical, mismatched, unauthenticated, or
/// unavailable release inputs.
pub fn load_authenticated_offline_cash_v1_runtime_verifier(
    manifest_bytes: &[u8],
    validation_receipt_bytes: &[u8],
    authority_policy_bytes: &[u8],
    attestation_bytes: &[u8],
    recursive_profile_json: &[u8],
    artifact_root: impl AsRef<Path>,
) -> Result<AuthenticatedOfflineCashV1RuntimeVerifier, String> {
    let manifest = OfflineCashReleaseManifestV1::decode_canonical_exact(manifest_bytes)
        .map_err(|error| format!("invalid Offline Cash V1 release manifest: {error}"))?;
    let receipt =
        OfflineCashInternalValidationReceiptV1::decode_canonical_exact(validation_receipt_bytes)
            .map_err(|error| format!("invalid Offline Cash V1 validation receipt: {error}"))?;
    let policy =
        OfflineCashReleaseAuthorityPolicyV1::decode_canonical_exact(authority_policy_bytes)
            .map_err(|error| format!("invalid Offline Cash V1 authority policy: {error}"))?;
    let attestation = OfflineCashReleaseAttestationV1::decode_canonical_exact(attestation_bytes)
        .map_err(|error| format!("invalid Offline Cash V1 release attestation: {error}"))?;
    if recursive_profile_json.is_empty()
        || recursive_profile_json.len() > OFFLINE_CASH_RECURSIVE_PROFILE_MAX_BYTES_V1
    {
        return Err(format!(
            "Offline Cash V1 recursive profile must contain at most {OFFLINE_CASH_RECURSIVE_PROFILE_MAX_BYTES_V1} bytes"
        ));
    }
    let profile_file: OfflineCashRecursiveVerifierProfileFileV1 =
        norito::json::from_slice(recursive_profile_json)
            .map_err(|error| format!("invalid Offline Cash V1 recursive profile JSON: {error}"))?;
    let profile = profile_file.try_into_profile()?;
    let release = manifest
        .authenticate(&receipt, &policy, &attestation)
        .map_err(|error| format!("unauthenticated Offline Cash V1 proof release: {error}"))?;
    AuthenticatedOfflineCashV1RuntimeVerifier::from_authenticated_release(
        &release,
        artifact_root,
        profile,
    )
}

impl OfflineCashRecursiveVerifierProfileFileV1 {
    fn try_into_profile(self) -> Result<OfflineCashRecursiveVerifierProfileV1, String> {
        Ok(OfflineCashRecursiveVerifierProfileV1 {
            state_eq: self.state_eq.try_into_params("state Eq")?,
            state_ep: self.state_ep.try_into_params("state Ep")?,
            guard_eq: self.guard_eq.try_into_params("GuardBundle Eq")?,
            guard_ep: self.guard_ep.try_into_params("GuardBundle Ep")?,
            wrapper_eq: self.wrapper_eq.try_into_params("commit wrapper Eq")?,
            wrapper_ep: self.wrapper_ep.try_into_params("commit wrapper Ep")?,
            mint_authorization_eq: self
                .mint_authorization_eq
                .try_into_params("mint authorization Eq")?,
            mint_authorization_ep: self
                .mint_authorization_ep
                .try_into_params("mint authorization Ep")?,
            mint_eq: self.mint_eq.try_into_params("mint authority Eq")?,
            mint_ep: self.mint_ep.try_into_params("mint authority Ep")?,
            mint_eq_protocol_digest: self.mint_eq_protocol_digest,
            mint_ep_protocol_digest: self.mint_ep_protocol_digest,
            mint_genesis_roster_id: self.mint_genesis_roster_id,
        })
    }
}

impl OfflineCashBaseCircuitProfileFileV1 {
    fn try_into_params(self, label: &str) -> Result<BaseCircuitParams, String> {
        fn host_usize(value: u64, label: &str, field: &str) -> Result<usize, String> {
            usize::try_from(value).map_err(|_| {
                format!("Offline Cash V1 {label} profile field `{field}` exceeds host usize")
            })
        }
        fn host_usizes(values: Vec<u64>, label: &str, field: &str) -> Result<Vec<usize>, String> {
            if values.len() > 3 {
                return Err(format!(
                    "Offline Cash V1 {label} profile field `{field}` has too many phases"
                ));
            }
            values
                .into_iter()
                .map(|value| host_usize(value, label, field))
                .collect()
        }
        Ok(BaseCircuitParams {
            k: host_usize(self.k, label, "k")?,
            num_advice_per_phase: host_usizes(
                self.num_advice_per_phase,
                label,
                "num_advice_per_phase",
            )?,
            num_fixed: host_usize(self.num_fixed, label, "num_fixed")?,
            num_lookup_advice_per_phase: host_usizes(
                self.num_lookup_advice_per_phase,
                label,
                "num_lookup_advice_per_phase",
            )?,
            lookup_bits: self
                .lookup_bits
                .map(|value| host_usize(value, label, "lookup_bits"))
                .transpose()?,
            num_instance_columns: host_usize(
                self.num_instance_columns,
                label,
                "num_instance_columns",
            )?,
        })
    }
}

fn labeled_invariant(label: &str, message: impl Into<String>) -> InstructionExecutionError {
    let message = message.into();
    let boxed: Box<str> =
        format!("{OFFLINE_CASH_V1_REJECTION_REASON_PREFIX}{label}:{message}").into();
    InstructionExecutionError::InvariantViolation(boxed)
}

fn resolve_offline_reserve_account(
    state_transaction: &mut StateTransaction<'_, '_>,
    definition: &AssetDefinitionId,
) -> Result<AccountId, Error> {
    let asset_definition = state_transaction.world.asset_definition(definition)?;
    // Offline support is a protocol primitive, not an asset enrollment mode.
    // Materialize deterministic reserve custody only when an offline instruction
    // actually needs it. This keeps ordinary asset registration free of
    // offline side effects and removes any process-local catalog dependency.
    crate::smartcontracts::isi::domain::isi::ensure_offline_reserve_account(
        &asset_definition,
        asset_definition.owned_by(),
        state_transaction,
    )?;
    let derived = crate::smartcontracts::isi::domain::isi::offline_cash_reserve_account_id(
        state_transaction.network_id(),
        definition,
    );
    Ok(derived)
}
pub(crate) fn is_offline_reserve_source_asset(
    state_transaction: &StateTransaction<'_, '_>,
    source_id: &AssetId,
) -> Result<bool, Error> {
    state_transaction
        .world
        .asset_definition(source_id.definition())?;
    let derived = crate::smartcontracts::isi::domain::isi::offline_cash_reserve_account_id(
        state_transaction.network_id(),
        source_id.definition(),
    );
    Ok(&derived == source_id.account())
}
fn ensure_distinct_offline_reserve_account(
    reserve_account: &AccountId,
    participant_account: &AccountId,
    participant_role: &str,
    definition_id: &AssetDefinitionId,
) -> Result<(), Error> {
    if reserve_account == participant_account {
        return Err(labeled_invariant(
            "reserve_self_reference",
            format!(
                "Offline Cash reserve account for asset definition `{definition_id}` must be distinct from {participant_role} account `{participant_account}`",
            ),
        )
        .into());
    }
    Ok(())
}
fn canonical_offline_cash_asset_id(
    state_transaction: &StateTransaction<'_, '_>,
    asset: &AssetId,
) -> Result<AssetId, Error> {
    let definition = state_transaction
        .world
        .asset_definition(asset.definition())?;
    let scope = match definition.balance_scope_policy() {
        AssetBalancePolicy::Global => {
            if !matches!(asset.scope(), AssetBalanceScope::Global) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "global assets cannot be addressed with dataspace scope".into(),
                )
                .into());
            }
            AssetBalanceScope::Global
        }
        AssetBalancePolicy::DataspaceRestricted => match asset.scope() {
            AssetBalanceScope::Dataspace(dataspace) => AssetBalanceScope::Dataspace(*dataspace),
            AssetBalanceScope::Global => state_transaction
                .world
                .resolve_asset_balance_scope(asset.definition())?,
        },
    };
    Ok(AssetId::with_scope(
        asset.definition().clone(),
        asset.account().clone(),
        scope,
    ))
}
fn offline_cash_reserve_asset_id(source_asset: &AssetId, reserve_account: AccountId) -> AssetId {
    AssetId::with_scope(
        source_asset.definition().clone(),
        reserve_account,
        source_asset.scope().clone(),
    )
}

/// Execution logic for Offline Cash V1 chain instructions.
pub mod isi {
    use super::offline_cash_v1_reserve::{
        OfflineCashRedemptionReadSetV1, OfflineCashReserveCommitContextV1,
        OfflineCashReserveMutationPlanV1, OfflineCashReserveOperationRecordV1,
        OfflineCashReservePlanOutcomeV1, OfflineCashReservePoolKeyV1, OfflineCashTopUpReadSetV1,
        VerifiedOfflineCashRedemptionV1, VerifiedOfflineCashTopUpIntentV1,
        plan_redemption_from_entries, plan_top_up_from_entries, validate_redemption_commit_entries,
        validate_top_up_commit_entries,
    };
    use super::*;

    /// Return whether `authority` has the exact Offline Cash V1 reserve-management permission.
    ///
    /// Direct grants and permissions inherited through assigned roles are authoritative. Lookup
    /// errors fail closed.
    pub fn world_has_offline_reserve_manager_permission(
        world: &impl WorldReadOnly,
        authority: &AccountId,
    ) -> bool {
        let required: Permission =
            iroha_executor_data_model::permission::offline::CanManageOfflineReserve.into();
        if world
            .account_permissions_iter(authority)
            .is_ok_and(|permissions| {
                permissions
                    .into_iter()
                    .any(|permission| permission == &required)
            })
        {
            return true;
        }
        world.account_roles_iter(authority).any(|role_id| {
            world
                .roles()
                .get(role_id)
                .is_some_and(|role| role.permissions.contains(&required))
        })
    }

    fn offline_cash_v1_error(label: &'static str, error: impl core::fmt::Display) -> Error {
        labeled_invariant(label, error.to_string()).into()
    }

    fn offline_cash_v1_commit_context(
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<OfflineCashReserveCommitContextV1, Error> {
        let transaction_hash = state_transaction.current_tx_hash.ok_or_else(|| {
            labeled_invariant(
                "execution_context_invalid",
                "Offline Cash V1 settlement requires a signed transaction identity",
            )
        })?;
        OfflineCashReserveCommitContextV1::after_block_context_verification(
            *transaction_hash.as_ref(),
            state_transaction.block_unix_timestamp_ms(),
        )
        .map_err(|error| offline_cash_v1_error("execution_context_invalid", error))
    }

    fn offline_cash_v1_amount(
        amount: u128,
        scale: u32,
        definition: &AssetDefinitionId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<Quantity, Error> {
        let spec = state_transaction.numeric_spec_for(definition)?;
        let live_scale = spec.scale().ok_or_else(|| {
            labeled_invariant(
                "amount_scale_invalid",
                "Offline Cash V1 requires an asset with a fixed numeric scale",
            )
        })?;
        if scale != live_scale {
            return Err(labeled_invariant(
                "amount_scale_mismatch",
                "Offline Cash V1 amount scale does not equal the live asset scale",
            )
            .into());
        }
        let amount = Quantity::try_from_numeric(Numeric::new(amount, scale))
            .map_err(|error| offline_cash_v1_error("amount_invalid", error))?;
        assert_numeric_spec_with(amount.as_numeric(), spec)?;
        Ok(amount)
    }

    fn record_offline_cash_v1_receipt_read(
        operation_id: [u8; 32],
        existing: Option<&OfflineCashReserveOperationRecordV1>,
    ) -> Result<(), Error> {
        let encoded = existing
            .map(|record| norito::encode_canonical(record.reserve_receipt()))
            .transpose()
            .map_err(|error| offline_cash_v1_error("receipt_encoding_failed", error))?;
        crate::sumeragi::witness::record_read_offline_cash_reserve_receipt_v1(
            operation_id,
            encoded.as_deref(),
        );
        Ok(())
    }

    fn settle_offline_cash_top_up_v1(
        request: OfflineCashTopUpRequestV1,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        request
            .validate_shape()
            .map_err(|error| offline_cash_v1_error("top_up_invalid", error))?;
        if &request.network_id != state_transaction.network_id() {
            return Err(labeled_invariant(
                "wrong_network",
                "Offline Cash V1 top-up NetworkId does not match this network",
            )
            .into());
        }
        if authority != &request.payer {
            return Err(labeled_invariant(
                "unauthorized_controller",
                "Offline Cash V1 top-up authority must equal the payer",
            )
            .into());
        }
        let live_incarnation = state_transaction
            .world
            .axt_asset_incarnations
            .get(&request.asset)
            .copied()
            .ok_or_else(|| {
                labeled_invariant(
                    "asset_incarnation_missing",
                    "Offline Cash V1 requires an authoritative live asset incarnation",
                )
            })?;
        if live_incarnation != request.asset_incarnation {
            return Err(labeled_invariant(
                "asset_incarnation_mismatch",
                "Offline Cash V1 top-up asset incarnation is not current",
            )
            .into());
        }
        let verified_authorization = state_transaction
            .offline_cash_v1_runtime_verifier
            .verify_top_up_authorization(&request)
            .map_err(|error| offline_cash_v1_error("recursive_release_invalid", error))?;
        let amount = offline_cash_v1_amount(
            request.amount,
            request.scale,
            &request.asset,
            state_transaction,
        )?;
        let source_id = canonical_offline_cash_asset_id(
            state_transaction,
            &AssetId::new(request.asset.clone(), request.payer.clone()),
        )?;
        let reserve_account = resolve_offline_reserve_account(state_transaction, &request.asset)?;
        ensure_distinct_offline_reserve_account(
            &reserve_account,
            &request.payer,
            "payer",
            &request.asset,
        )?;
        let destination_id = offline_cash_reserve_asset_id(&source_id, reserve_account);
        let pool_key = OfflineCashReservePoolKeyV1::new(
            request.network_id,
            request.asset.clone(),
            request.asset_incarnation,
        )
        .map_err(|error| offline_cash_v1_error("reserve_invalid", error))?;
        let context = offline_cash_v1_commit_context(state_transaction)?;
        let verified = VerifiedOfflineCashTopUpIntentV1::after_admission_verification(
            request,
            verified_authorization,
        )
        .map_err(|error| offline_cash_v1_error("top_up_invalid", error))?;
        let operation_id = verified.operation_id();
        let outcome = {
            let existing = state_transaction
                .world
                .offline_cash_reserve_operations
                .get(&operation_id);
            record_offline_cash_v1_receipt_read(operation_id, existing)?;
            plan_top_up_from_entries(
                &verified,
                context,
                OfflineCashTopUpReadSetV1 {
                    current_pool: state_transaction
                        .world
                        .offline_cash_reserve_pools
                        .get(&pool_key.liability_pool_id),
                    existing_operation: existing,
                    credit_operation: state_transaction
                        .world
                        .offline_cash_mint_credit_operations
                        .get(&verified.intent().request.credit_id)
                        .copied(),
                    issuance_operation: state_transaction
                        .world
                        .offline_cash_issuance_operations
                        .get(&verified.intent().request.issuance_commitment)
                        .copied(),
                },
            )
            .map_err(|error| offline_cash_v1_error("reserve_invalid", error))?
        };
        let OfflineCashReservePlanOutcomeV1::Commit(OfflineCashReserveMutationPlanV1::TopUp(plan)) =
            outcome
        else {
            return Ok(());
        };
        {
            let record = plan.record();
            let retry = validate_top_up_commit_entries(
                &plan,
                OfflineCashTopUpReadSetV1 {
                    current_pool: state_transaction
                        .world
                        .offline_cash_reserve_pools
                        .get(&record.pool.liability_pool_id),
                    existing_operation: state_transaction
                        .world
                        .offline_cash_reserve_operations
                        .get(&record.operation_id),
                    credit_operation: state_transaction
                        .world
                        .offline_cash_mint_credit_operations
                        .get(&record.credit_id)
                        .copied(),
                    issuance_operation: state_transaction
                        .world
                        .offline_cash_issuance_operations
                        .get(&record.issuance_commitment)
                        .copied(),
                },
            )
            .map_err(|error| offline_cash_v1_error("reserve_invalid", error))?;
            if retry.is_some() {
                return Ok(());
            }
        }
        crate::smartcontracts::isi::asset::isi::execute_verified_offline_cash_top_up_transfer_v1(
            state_transaction,
            VerifiedOfflineCashTopUpDebitV1::new(
                authority.clone(),
                operation_id,
                source_id,
                destination_id,
                amount,
            ),
        )?;
        let record = plan.record().clone();
        let operation = OfflineCashReserveOperationRecordV1::TopUp(record.clone());
        state_transaction
            .world
            .offline_cash_reserve_pools
            .insert(record.pool.liability_pool_id, plan.next_pool().clone());
        state_transaction
            .world
            .offline_cash_reserve_operations
            .insert(record.operation_id, operation);
        state_transaction
            .world
            .offline_cash_mint_credit_operations
            .insert(record.credit_id, record.operation_id);
        state_transaction
            .world
            .offline_cash_issuance_operations
            .insert(record.issuance_commitment, record.operation_id);
        crate::sumeragi::witness::record_write_offline_cash_reserve_receipt_v1(
            &record.reserve_receipt,
        )
        .map_err(|error| offline_cash_v1_error("receipt_encoding_failed", error))?;
        Ok(())
    }

    fn settle_offline_cash_redemption_v1(
        request: OfflineCashRedemptionRequestV1,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        request
            .validate_shape()
            .map_err(|error| offline_cash_v1_error("redemption_invalid", error))?;
        let verified_proof = state_transaction
            .offline_cash_v1_runtime_verifier
            .verify_redemption_request(request)
            .map_err(|error| offline_cash_v1_error("invalid_recursive_proof", error))?;
        let verified = VerifiedOfflineCashRedemptionV1::after_full_verification(verified_proof);
        let statement = &verified.request().voucher.statement;
        let lifecycle = &statement.lifecycle;
        let live_incarnation = state_transaction
            .world
            .axt_asset_incarnations
            .get(&lifecycle.asset)
            .copied()
            .ok_or_else(|| {
                labeled_invariant(
                    "asset_incarnation_missing",
                    "Offline Cash V1 requires an authoritative live asset incarnation",
                )
            })?;
        if live_incarnation != lifecycle.asset_incarnation {
            return Err(labeled_invariant(
                "asset_incarnation_mismatch",
                "Offline Cash V1 redemption asset incarnation is not current",
            )
            .into());
        }
        if &lifecycle.network_id != state_transaction.network_id() {
            return Err(labeled_invariant(
                "wrong_network",
                "Offline Cash V1 redemption NetworkId does not match this network",
            )
            .into());
        }
        let amount = offline_cash_v1_amount(
            statement.amount,
            lifecycle.scale,
            &lifecycle.asset,
            state_transaction,
        )?;
        let beneficiary_id = canonical_offline_cash_asset_id(
            state_transaction,
            &AssetId::new(lifecycle.asset.clone(), statement.beneficiary.clone()),
        )?;
        let reserve_account = resolve_offline_reserve_account(state_transaction, &lifecycle.asset)?;
        ensure_distinct_offline_reserve_account(
            &reserve_account,
            &statement.beneficiary,
            "beneficiary",
            &lifecycle.asset,
        )?;
        let source_id = offline_cash_reserve_asset_id(&beneficiary_id, reserve_account);
        let pool_key = OfflineCashReservePoolKeyV1::new(
            lifecycle.network_id,
            lifecycle.asset.clone(),
            lifecycle.asset_incarnation,
        )
        .map_err(|error| offline_cash_v1_error("reserve_invalid", error))?;
        let operation_id = verified.operation_id();
        let context = offline_cash_v1_commit_context(state_transaction)?;
        let outcome = {
            let existing = state_transaction
                .world
                .offline_cash_reserve_operations
                .get(&operation_id);
            record_offline_cash_v1_receipt_read(operation_id, existing)?;
            plan_redemption_from_entries(
                &verified,
                context,
                OfflineCashRedemptionReadSetV1 {
                    current_pool: state_transaction
                        .world
                        .offline_cash_reserve_pools
                        .get(&pool_key.liability_pool_id),
                    existing_operation: existing,
                    redemption_operation: state_transaction
                        .world
                        .offline_cash_redemption_id_operations
                        .get(&statement.redemption_id)
                        .copied(),
                    terminal_nullifier_operation: state_transaction
                        .world
                        .offline_cash_terminal_nullifier_operations
                        .get(&statement.terminal_nullifier)
                        .copied(),
                },
            )
            .map_err(|error| offline_cash_v1_error("reserve_invalid", error))?
        };
        let OfflineCashReservePlanOutcomeV1::Commit(OfflineCashReserveMutationPlanV1::Redemption(
            plan,
        )) = outcome
        else {
            return Ok(());
        };
        {
            let record = plan.record();
            let retry = validate_redemption_commit_entries(
                &plan,
                OfflineCashRedemptionReadSetV1 {
                    current_pool: state_transaction
                        .world
                        .offline_cash_reserve_pools
                        .get(&record.pool.liability_pool_id),
                    existing_operation: state_transaction
                        .world
                        .offline_cash_reserve_operations
                        .get(&record.operation_id),
                    redemption_operation: state_transaction
                        .world
                        .offline_cash_redemption_id_operations
                        .get(&record.redemption_id)
                        .copied(),
                    terminal_nullifier_operation: state_transaction
                        .world
                        .offline_cash_terminal_nullifier_operations
                        .get(&record.terminal_nullifier)
                        .copied(),
                },
            )
            .map_err(|error| offline_cash_v1_error("reserve_invalid", error))?;
            if retry.is_some() {
                return Ok(());
            }
        }
        crate::smartcontracts::isi::asset::isi::execute_verified_offline_cash_redemption_transfer_v1(
            state_transaction,
            VerifiedOfflineCashRedemptionDebitV1::new(
                operation_id,
                source_id,
                beneficiary_id,
                amount,
            ),
        )?;
        let record = plan.record().clone();
        let operation = OfflineCashReserveOperationRecordV1::Redemption(record.clone());
        state_transaction
            .world
            .offline_cash_reserve_pools
            .insert(record.pool.liability_pool_id, plan.next_pool().clone());
        state_transaction
            .world
            .offline_cash_reserve_operations
            .insert(record.operation_id, operation);
        state_transaction
            .world
            .offline_cash_redemption_id_operations
            .insert(record.redemption_id, record.operation_id);
        state_transaction
            .world
            .offline_cash_terminal_nullifier_operations
            .insert(record.terminal_nullifier, record.operation_id);
        crate::sumeragi::witness::record_write_offline_cash_reserve_receipt_v1(
            &record.reserve_receipt,
        )
        .map_err(|error| offline_cash_v1_error("receipt_encoding_failed", error))?;
        Ok(())
    }

    impl Execute for TopUpOfflineCashV1 {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            settle_offline_cash_top_up_v1(self.request, authority, state_transaction)
        }
    }

    impl Execute for RedeemOfflineCashV1 {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            settle_offline_cash_redemption_v1(self.request, state_transaction)
        }
    }
}

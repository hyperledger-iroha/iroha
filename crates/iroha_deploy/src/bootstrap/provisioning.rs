//! Release-authenticated public hints and finite allowances for managed parent contexts.

use std::{collections::BTreeSet, time::Instant};

use iroha::config::Config;
use iroha_data_model::{account::AccountId, asset::AssetDefinitionId};
use iroha_model_base::peer::PeerId;
use iroha_primitives::numeric::Quantity;
use norito::{Decode, Encode};

use super::*;

/// Signed endpoint hint for a BLS validator; it cannot grant voting membership.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::bootstrap::ReleasePeerV1")]
pub struct ReleasePeer {
    /// Exact node identity, independently checked against native certified committee state.
    pub node_id: PeerId,
    /// Canonical HTTPS root present in the same release's approved roots.
    pub torii_root: String,
}

/// Testnet faucet authority and explicit per-operation spending caps distributed by the release.
/// These public values authorize no minting: the native prepared envelope and ledger execution
/// still enforce issuer custody, claim work, replay protection, funding and permissions.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::bootstrap::ReleaseFaucetV1")]
pub struct ReleaseFaucet {
    /// Exact approved HTTPS root serving canonical native faucet routes.
    pub torii_root: String,
    /// Independently trusted single-signatory faucet issuer.
    pub issuer: AccountId,
    /// Sole currency available for automatic fees and namespace rent.
    pub asset_definition_id: AssetDefinitionId,
    /// Exact positive allowance delivered by one faucet claim.
    pub amount: Quantity,
    /// Maximum aggregate fees for one managed parent transaction.
    pub max_operation_fee: Quantity,
    /// Maximum combined rent for the one-year SNS dataspace and owner account-alias leases.
    pub max_namespace_rent: Quantity,
}

/// Independently selected public build-registry schema and Torii locations on this network.
/// Provider endpoints and token signer custody must still come from authenticated current state.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::bootstrap::ReleaseBuildRegistryV1")]
pub struct ReleaseBuildRegistry {
    /// Approved parent Torii roots serving registry and provider-discovery evidence.
    pub torii_roots: Vec<String>,
}

pub(super) fn validate(release: &NetworkRelease) -> Result<()> {
    iroha::config::resolve_account_chain_discriminant(
        None,
        Some(release.account_chain_discriminant),
    )
    .map_err(|_| BootstrapError::Invalid("invalid signed account address profile"))?;
    if release.peers.is_empty()
        || release.peers.len() > crate::verify::finality::MAX_OBSERVATION_PEERS
    {
        return Err(BootstrapError::Invalid(
            "invalid signed committee endpoint count",
        ));
    }
    let mut peers = BTreeSet::new();
    for peer in &release.peers {
        if peer.node_id.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
            || !peers.insert(&peer.node_id)
            || !release.torii_roots.contains(&peer.torii_root)
        {
            return Err(BootstrapError::Invalid(
                "invalid signed committee endpoint binding",
            ));
        }
    }
    if let Some(faucet) = &release.faucet {
        iroha::client::AccountFaucetPolicyV1::try_new(
            faucet.issuer.clone(),
            faucet.asset_definition_id.clone(),
            faucet.amount.clone(),
        )
        .map_err(|_| BootstrapError::Invalid("invalid signed faucet authority or allowance"))?;
        if !release.torii_roots.contains(&faucet.torii_root)
            || faucet.max_operation_fee.is_zero()
            || faucet.max_namespace_rent.is_zero()
            || faucet.max_operation_fee > faucet.amount
            || faucet.max_namespace_rent > faucet.amount
        {
            return Err(BootstrapError::Invalid(
                "invalid signed developer spending allowance",
            ));
        }
    }
    if let Some(registry) = &release.build_registry {
        let roots = registry.torii_roots.iter().collect::<BTreeSet<_>>();
        if roots.is_empty()
            || roots.len() != registry.torii_roots.len()
            || roots.iter().any(|root| !release.torii_roots.contains(root))
        {
            return Err(BootstrapError::Invalid(
                "build registry must select distinct approved parent roots",
            ));
        }
    }
    Ok(())
}

impl AuthenticatedBootstrap {
    /// Select the release's preferred public wallet context without constructing credentials.
    /// A managed wallet imports the retained child owner through its native custody API.
    ///
    /// # Errors
    /// The release's exact chain/profile/endpoint cannot form a canonical SDK wallet context.
    pub fn wallet_network(&self) -> Result<iroha_wallet::WalletNetwork> {
        let release = self.release();
        iroha_wallet::WalletNetwork::new(
            release.network_id,
            release
                .chain_id
                .parse()
                .map_err(|_| BootstrapError::Invalid("invalid signed parent chain"))?,
            release.torii_roots[0]
                .parse()
                .map_err(|_| BootstrapError::Invalid("invalid signed parent endpoint"))?,
            release.account_chain_discriminant,
        )
        .map_err(|_| BootstrapError::Invalid("signed parent wallet context is inconsistent"))
    }

    /// Obtain finite automatic registration/anchoring fees from the authenticated testnet policy.
    /// A deployment without this policy needs an explicitly funded context; it cannot silently
    /// use a queried faucet authority or authorize unlimited charges.
    ///
    /// # Errors
    /// No authenticated faucet allowance is installed or the scheduling deadline elapsed.
    pub fn private_operation_options(
        &self,
        deadline: Instant,
    ) -> Result<iroha_wallet::operations::BoundedTransactionOptions> {
        let faucet = self
            .release()
            .faucet
            .as_ref()
            .ok_or(BootstrapError::Invalid(
                "signed network release has no automatic funding allowance",
            ))?;
        if Instant::now() >= deadline {
            return Err(BootstrapError::Invalid("parent operation deadline elapsed"));
        }
        Ok(iroha_wallet::operations::BoundedTransactionOptions {
            fee_payment: iroha_data_model::transaction::FeePaymentIntent::authority(vec![], None),
            max_total_fees: [(
                faucet.asset_definition_id.clone(),
                faucet.max_operation_fee.clone(),
            )]
            .into_iter()
            .collect(),
            deadline,
        })
    }

    /// Construct credential-free parent endpoint contexts from signed release hints.
    ///
    /// The supplied wallet already targets one approved parent root. Its child listener token,
    /// Basic Auth and arbitrary operator credentials cannot be copied to other endpoints.
    /// The generated clients retain only this exact parent's signer and native SDK defaults.
    /// Fresh committee readiness still requires [`ParentFinalityStore::observe`].
    ///
    /// # Errors
    /// Rejects changed network/profile, any transport credentials, unapproved wallet endpoint,
    /// an exhausted deadline, or an invalid endpoint/client before observation.
    pub fn parent_http_source(
        &self,
        config: &Config,
        deadline: Instant,
    ) -> std::result::Result<
        crate::verify::http::HttpFinalitySource,
        crate::verify::http::HttpFinalityError,
    > {
        use crate::verify::http::HttpFinalityError;
        let release = self.release();
        if config.network_id != release.network_id
            || config.chain.to_string() != release.chain_id
            || config.account_chain_discriminant != release.account_chain_discriminant
            || config.api_token.is_some()
            || config.basic_auth.is_some()
            || !release
                .torii_roots
                .iter()
                .any(|root| root == config.torii_api_url.as_str())
        {
            return Err(HttpFinalityError::Invalid(
                "wallet differs from signed public parent context",
            ));
        }
        let make_client = |root: &str| {
            let mut selected = config.clone();
            selected.torii_api_url = root
                .parse()
                .map_err(|_| HttpFinalityError::Invalid("signed parent endpoint"))?;
            iroha::client::Client::builder(selected)
                .build()
                .map_err(|_| HttpFinalityError::Invalid("signed parent SDK context"))
        };
        let proofs = release
            .torii_roots
            .iter()
            .map(|root| make_client(root))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let peers = release
            .peers
            .iter()
            .map(|peer| Ok((peer.node_id.clone(), make_client(&peer.torii_root)?)))
            .collect::<std::result::Result<Vec<_>, HttpFinalityError>>()?;
        self.http_source(proofs, peers, deadline)
    }
}

#[cfg(test)]
mod tests;

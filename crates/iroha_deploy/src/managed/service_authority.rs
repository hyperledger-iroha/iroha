//! Original generated service authority, signed genesis and private operation custody.

use super::{
    PreparedLocalnet, Result,
    native_operation::{invalid, read_selected_peers},
};
use crate::{
    localnet::service_authorities::{
        NetworkServiceAuthorityRole, ProviderServiceInventory, RetainedProviderServicePlan,
        StreamTokenAuthorityManifest, StreamTokenAuthorityRole,
    },
    verify::finality::{FinalityVerifier, GenesisAnchor},
};
use iroha::{client::Client, config::Config};
use iroha_data_model::{
    NetworkId,
    account::{AccountId, address::ChainDiscriminantGuard},
    sorafs::{
        capacity::ProviderId,
        reserve::{ReserveAuthorityPolicyV1, account_proof::VerifiedReserveAccountStateV1},
    },
    sumeragi_finality::{FinalityValidator, genesis_epoch},
};
use iroha_fs::PrivateDirectory;
use iroha_model_base::peer::PeerId;
use std::{collections::BTreeSet, fs::File, time::Instant};

#[path = "service_authority/inventory.rs"]
mod inventory;
pub(super) use inventory::ServiceChildInventory;

pub(super) enum NetworkPurpose {
    BuildRegistry,
    InitialReservePolicy,
    InitialReputationPolicy,
    ServiceBootstrap,
    GeneratedRuntime,
}
impl NetworkPurpose {
    fn directory_name(self) -> &'static str {
        match self {
            Self::BuildRegistry => "build-registry",
            Self::InitialReservePolicy => "initial-reserve-policy",
            Self::InitialReputationPolicy => "initial-reputation-policy",
            Self::ServiceBootstrap => "service-bootstrap",
            Self::GeneratedRuntime => "generated-service-runtime",
        }
    }
}

pub(super) enum ProviderPurpose {
    ProviderAdvertisement,
    Custody,
    ReserveAccountRegistration,
    ReserveTopUpRequest,
    ReserveTopUpApproval,
    InitialProviderCredit,
    ProviderCapacityDeclaration,
    ProviderFundingBootstrap,
    GatewayCompliance,
    InitialGatewaySetup,
    InitialProviderIngestAuthority,
}
impl ProviderPurpose {
    fn directory_name(self) -> &'static str {
        match self {
            Self::ProviderAdvertisement => "provider-advertisement",
            Self::Custody => "stream-token-custody",
            Self::ReserveAccountRegistration => "reserve-account-registration",
            Self::ReserveTopUpRequest => "reserve-top-up-request",
            Self::ReserveTopUpApproval => "reserve-top-up-approval",
            Self::InitialProviderCredit => "initial-provider-credit",
            Self::ProviderCapacityDeclaration => "provider-capacity-declaration",
            Self::ProviderFundingBootstrap => "provider-funding-bootstrap",
            Self::GatewayCompliance => "gateway-compliance",
            Self::InitialGatewaySetup => "initial-gateway-setup",
            Self::InitialProviderIngestAuthority => "initial-provider-ingest-authority",
        }
    }
}

#[derive(Clone, Copy)]
enum Scope {
    Network,
    Provider { provider: ProviderId, slot: u8 },
}

pub(super) struct ServiceAuthority {
    scope: Scope,
    pub(super) prepared: PreparedLocalnet,
    pub(super) directory: PrivateDirectory,
    pub(super) _lock: File,
    pub(super) manifest: StreamTokenAuthorityManifest,
    pub(super) config: Config,
    pub(super) genesis: GenesisAnchor,
    pub(super) peers: Vec<(PeerId, Client)>,
}

fn operation_directory(
    generation: &PrivateDirectory,
    scope: Scope,
    purpose: &str,
    create: bool,
) -> Result<Option<PrivateDirectory>> {
    let child = |parent: &PrivateDirectory, name: &str| -> Result<Option<PrivateDirectory>> {
        if create {
            return parent.ensure_child(name).map(Some).map_err(Into::into);
        }
        match parent.open_child(name) {
            Ok(directory) => Ok(Some(directory)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                parent.revalidate()?;
                Ok(None)
            }
            Err(error) => Err(error.into()),
        }
    };
    let Some(runtime) = child(generation, "runtime")? else {
        return Ok(None);
    };
    let Some(operations) = child(&runtime, "service-operations")? else {
        return Ok(None);
    };
    let selected = match scope {
        Scope::Network => child(&operations, "network")?,
        Scope::Provider { slot, .. } => {
            let Some(providers) = child(&operations, "providers")? else {
                return Ok(None);
            };
            child(&providers, &slot.to_string())?
        }
    };
    let Some(selected) = selected else {
        return Ok(None);
    };
    child(&selected, purpose)
}

impl ServiceAuthority {
    pub(super) fn open_network(
        prepared: &PreparedLocalnet,
        purpose: NetworkPurpose,
    ) -> Result<Self> {
        Self::open(prepared, None, purpose.directory_name(), true)?
            .ok_or_else(|| invalid("new native operation custody was not created"))
    }

    pub(super) fn open_provider(
        prepared: &PreparedLocalnet,
        provider: ProviderId,
        purpose: ProviderPurpose,
    ) -> Result<Self> {
        Self::open(prepared, Some(provider), purpose.directory_name(), true)?
            .ok_or_else(|| invalid("new native operation custody was not created"))
    }

    pub(super) fn open_network_existing(
        prepared: &PreparedLocalnet,
        purpose: NetworkPurpose,
    ) -> Result<Option<Self>> {
        Self::open(prepared, None, purpose.directory_name(), false)
    }

    pub(super) fn open_provider_existing(
        prepared: &PreparedLocalnet,
        provider: ProviderId,
        purpose: ProviderPurpose,
    ) -> Result<Option<Self>> {
        Self::open(prepared, Some(provider), purpose.directory_name(), false)
    }

    fn open(
        prepared: &PreparedLocalnet,
        provider: Option<ProviderId>,
        purpose: &'static str,
        create: bool,
    ) -> Result<Option<Self>> {
        #[cfg(test)]
        inventory::record_authority_open();
        let manifest = prepared.stream_token_authorities()?.ok_or_else(|| {
            invalid("managed native operation requires its original StreamTokenAuthorities profile")
        })?;
        // Resolve the original provider before any operation directory or lock is created.
        let scope = match provider {
            Some(provider) => Scope::Provider {
                slot: manifest.provider(provider)?.slot,
                provider,
            },
            None => Scope::Network,
        };
        let config = prepared.context.load_client_config()?;
        let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
        let path = prepared
            .context
            .client_config
            .parent()
            .ok_or_else(|| invalid("managed native operation has no generation"))?;
        let generation = PrivateDirectory::open_exact(path)?;
        let bytes = generation.read(
            "genesis.signed.nrt",
            iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1,
        )?;
        let genesis = iroha_data_model::block::decode_framed_signed_block(&bytes)
            .map_err(|_| invalid("invalid original signed service genesis"))?;
        let epoch = genesis_epoch(&genesis)
            .map_err(|_| invalid("cannot authenticate original service genesis"))?;
        if epoch.network_id != config.network_id
            || NetworkId::from_genesis_hash(genesis.hash()) != manifest.network_id
            || prepared.context.dataspace_id != 0
            || manifest.manager != config.account
        {
            return Err(invalid("original service network or manager differs"));
        }
        let validators = epoch
            .committee
            .iter()
            .map(|member| FinalityValidator {
                public_key: member.validator.public_key().clone(),
                proof_of_possession: member.proof_of_possession.clone(),
            })
            .collect();
        let expected: BTreeSet<_> = epoch
            .committee
            .iter()
            .map(|member| member.validator.clone())
            .collect();
        let mut peers = Vec::new();
        for peer in &prepared.peers {
            let bytes = iroha_fs::read_private(&peer.config_path, 1024 * 1024)?;
            let rendered = std::str::from_utf8(&bytes)
                .map_err(|_| invalid("invalid retained service peer configuration"))?;
            let table = crate::secret_toml::parse_table(rendered, "managed native operation peer")
                .map_err(|_| invalid("invalid retained service peer configuration"))?;
            let reader = iroha_config::node_config::open_node_config(
                iroha_config::node_config::NodeFile::Verified {
                    path: peer.config_path.clone(),
                    table,
                },
                iroha_config::node_config::NodeConfigOptions::default(),
            )
            .map_err(|_| invalid("cannot resolve retained service peer"))?;
            let (user, _) = reader
                .read()
                .map_err(|_| invalid("cannot read retained service peer"))?;
            let actual = user
                .parse()
                .map_err(|_| invalid("invalid retained service peer"))?;
            let id = PeerId::new(actual.common.key_pair.public_key().clone());
            if actual.genesis.expected_hash != genesis.hash() || !expected.contains(&id) {
                return Err(invalid(
                    "service peer differs from original genesis committee",
                ));
            }
            let mut selected = config.clone();
            selected.torii_api_url = peer
                .torii_url
                .parse()
                .map_err(|_| invalid("invalid service peer endpoint"))?;
            let client = Client::builder(selected)
                .build()
                .map_err(|_| invalid("cannot construct service peer client"))?;
            peers.push((id, client));
        }
        if peers.len() != expected.len()
            || peers
                .iter()
                .map(|(peer, _)| peer.clone())
                .collect::<BTreeSet<_>>()
                != expected
        {
            return Err(invalid(
                "service endpoints do not cover the exact original committee",
            ));
        }
        let Some(directory) = operation_directory(&generation, scope, purpose, create)? else {
            return Ok(None);
        };
        let lock = if create {
            directory.open_lock("operation.lock")?
        } else {
            match directory.open_existing_lock("operation.lock") {
                Ok(lock) => lock,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    // A crash before lock publication can leave only this exact empty directory.
                    super::native_operation::require_empty(&directory)?;
                    return Ok(None);
                }
                Err(error) => return Err(error.into()),
            }
        };
        lock.try_lock()
            .map_err(|_| invalid("another managed native operation holds this generation"))?;
        directory.revalidate()?;
        Ok(Some(Self {
            scope,
            prepared: prepared.clone(),
            directory,
            _lock: lock,
            manifest,
            config: config.clone(),
            genesis: GenesisAnchor {
                network_id: config.network_id,
                chain_id: config.chain.to_string(),
                genesis,
                validators,
            },
            peers,
        }))
    }

    /// Select authenticated public intent without acquiring another live child lock.
    pub(super) fn provider_inventory(
        &self,
        provider: ProviderId,
    ) -> Result<&ProviderServiceInventory> {
        if let Scope::Provider {
            provider: selected, ..
        } = self.scope
            && provider != selected
        {
            return Err(invalid("provider differs from selected service scope"));
        }
        self.manifest.provider(provider)
    }

    pub(super) fn provider_id(&self) -> Result<ProviderId> {
        match self.scope {
            Scope::Provider { provider, .. } => Ok(provider),
            Scope::Network => Err(invalid("network service has no selected provider")),
        }
    }

    pub(super) fn provider_role(&self, role: StreamTokenAuthorityRole) -> Result<&AccountId> {
        Ok(&self
            .provider_inventory(self.provider_id()?)?
            .authority(role)?
            .account)
    }

    pub(super) fn network_role(&self, role: NetworkServiceAuthorityRole) -> Result<&AccountId> {
        Ok(&self.manifest.network.authority(role)?.account)
    }

    pub(super) fn provider_plan(&self) -> Result<RetainedProviderServicePlan> {
        self.prepared
            .provider_service_plan(self.provider_id()?)?
            .ok_or_else(|| invalid("selected service provider plan is absent"))
    }
    /// Select only the original generated issuer operator after authenticating the whole profile.
    /// The manager and independently selected finality peers stay unchanged. This is a local
    /// signing configuration, not evidence of current native permission, registration or funds.
    pub(super) fn issuer_operator_config(&self) -> Result<Config> {
        self.validate_profile()
            .map_err(|_| invalid("invalid original issuer-operator profile"))?;
        let key = crate::localnet::service_authorities::issuer_operator_key(
            &self.prepared,
            &self.manifest,
            self.provider_id()?,
        )?;
        let mut config = self.config.clone();
        config.account = self
            .provider_role(StreamTokenAuthorityRole::IssuerOperator)?
            .clone();
        config.key_pair = key;
        Ok(config)
    }

    /// Select the original shared reserve operator, preserving manager finality clients.
    pub(super) fn reserve_operations_config(&self) -> Result<Config> {
        self.validate_profile()
            .map_err(|_| invalid("invalid original reserve-operations profile"))?;
        let key = crate::localnet::service_authorities::reserve_operations_key(
            &self.prepared,
            &self.manifest,
        )?;
        let mut config = self.config.clone();
        config.account = self
            .network_role(NetworkServiceAuthorityRole::ReserveOperations)?
            .clone();
        config.key_pair = key;
        Ok(config)
    }

    fn validate_operation_custody(&self) -> Result<()> {
        self.directory.revalidate()?;
        if iroha_fs::FileIdentity::of(&self.directory.open_read("operation.lock")?)?
            != iroha_fs::FileIdentity::of(&self._lock)?
        {
            return Err(invalid("managed native operation lock was replaced"));
        }
        Ok(())
    }

    pub(super) fn validate_profile(&self) -> Result<()> {
        self.validate_operation_custody()?;
        if self.prepared.stream_token_authorities()?.as_ref() != Some(&self.manifest) {
            return Err(invalid("original service authority profile changed"));
        }
        if let Scope::Provider { provider, slot } = self.scope
            && self.manifest.provider(provider)?.slot != slot
        {
            return Err(invalid("original provider operation scope changed"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

// The original registration reader is shared without a parallel transport or proof verifier.
impl ServiceAuthority {
    pub(super) fn read_reserve_account(
        &self,
        policy: &ReserveAuthorityPolicyV1,
        verifier: &FinalityVerifier,
        deadline: Instant,
    ) -> Result<VerifiedReserveAccountStateV1> {
        let block = verifier
            .verified_tip()
            .map_err(|_| invalid("invalid certified reserve tip"))?;
        let schema = iroha_core::state::State::native_world_schema_hash_v1()
            .map_err(|_| invalid("native reserve schema unavailable"))?;
        // Keep the original manager clients intact for finality. Rebind only the account
        // signer of each independently selected original committee endpoint for this purpose.
        let operator = self.reserve_operations_config()?;
        let provider = self.provider_id()?;
        let owner = self.provider_role(StreamTokenAuthorityRole::IssuerOperator)?;
        read_selected_peers(&self.peers, deadline, |client, deadline| {
            let mut builder = client.to_builder();
            builder.account = operator.account.clone();
            builder.key_pair = operator.key_pair.clone();
            let client = builder
                .build()
                .map_err(|_| invalid("invalid reserve operator client"))?;
            client
                .with_request_deadline(deadline)
                .get_reserve_account_state(
                    &policy.operations_authority,
                    provider,
                    owner,
                    policy,
                    schema,
                    &block,
                )
                .map_err(|_| invalid("native reserve provider candidate unavailable or invalid"))
        })
    }
}

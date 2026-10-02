//! Retained native token authority prerequisites for managed global genesis.
//!
//! This profile creates funded signers, initial capabilities, two non-signing reserve accounts and
//! one genesis-bound provider owner.
//! It does not admit a provider, configure custody or gateway policies, enroll a signer, or enable
//! token services.
use super::*;
use crate::managed::{Error, PreparedLocalnet};
use iroha_data_model::{NetworkId, sorafs::capacity::ProviderId};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsStreamToken, CanDeclareSorafsCapacity, CanManageSorafsReputationJournalPolicy,
    CanManageSorafsStreamTokenCustody, CanManageSorafsStreamTokenGateway,
    CanOperateSorafsStreamToken, CanRecordSorafsReputationJournal, CanSetSorafsReservePolicy,
    CanUpsertSorafsProviderCredit,
};
use norito::{JsonDeserialize, JsonSerialize};

const DIRECTORY: &str = "stream-token-authorities";
const MANIFEST: &str = "authorities.json";
const MAX_MANIFEST: usize = 16 * 1024;
const PROFILE_METADATA: &str = "localnet_service_profile_v1";
const ROLE_METADATA: &str = "localnet_stream_token_authority_role_v1";

pub(super) fn append_profile(
    genesis: RawGenesisTransaction,
    profile: LocalnetServiceProfile,
    manager: &AccountId,
) -> Result<RawGenesisTransaction> {
    genesis
        .into_builder()
        .append_instruction(SetKeyValue::account(
            manager.clone(),
            PROFILE_METADATA
                .parse()
                .expect("static profile metadata key"),
            Json::new(profile),
        ))
        .build_raw()
}

fn role_metadata(role: impl norito::json::JsonSerialize) -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        ROLE_METADATA
            .parse()
            .expect("static authority metadata key"),
        Json::new(role),
    );
    metadata
}

/// Closed preparation profile, retained exactly with the signed genesis identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(
    tag = "profile",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LocalnetServiceProfile {
    /// Ordinary configuration-free localnet without token service authority prerequisites.
    #[default]
    Standard,
    /// Prepare native token service identities and initial permissions; services stay disabled.
    StreamTokenAuthorities,
}

/// One distinct native token service credential purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, JsonSerialize, JsonDeserialize)]
#[norito(
    tag = "role",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum StreamTokenAuthorityRole {
    /// Provider owner and issuer operation transaction signer.
    IssuerOperator,
    /// Independent issuer Check transaction signer.
    IssuerObserver,
    /// Purpose-bound token body signer; never the operation transaction signer.
    TokenSigner,
    /// Independent signer-custody attester.
    CustodyAttester,
    /// Gateway admission and acknowledgement transaction signer.
    GatewayOperator,
    /// Independent gateway Check transaction signer.
    GatewayObserver,
    /// Exact governed reputation append transaction signer.
    ReputationRecorder,
}
impl StreamTokenAuthorityRole {
    /// Fixed basename beneath `runtime/stream-token-authorities`; never caller-controlled.
    #[must_use]
    pub const fn credential_filename(self) -> &'static str {
        match self {
            Self::IssuerOperator => "issuer-operator.key",
            Self::IssuerObserver => "issuer-observer.key",
            Self::TokenSigner => "token-signer.key",
            Self::CustodyAttester => "custody-attester.key",
            Self::GatewayOperator => "gateway-operator.key",
            Self::GatewayObserver => "gateway-observer.key",
            Self::ReputationRecorder => "reputation-recorder.key",
        }
    }
    const fn transacts(self) -> bool {
        !matches!(self, Self::TokenSigner | Self::CustodyAttester)
    }
}
const ROLES: [StreamTokenAuthorityRole; 7] = [
    StreamTokenAuthorityRole::IssuerOperator,
    StreamTokenAuthorityRole::IssuerObserver,
    StreamTokenAuthorityRole::TokenSigner,
    StreamTokenAuthorityRole::CustodyAttester,
    StreamTokenAuthorityRole::GatewayOperator,
    StreamTokenAuthorityRole::GatewayObserver,
    StreamTokenAuthorityRole::ReputationRecorder,
];

/// Public account and fixed credential purpose; contains no secret bytes or configurable path.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenAuthority {
    /// Purpose whose fixed filename is exposed by [`StreamTokenAuthorityRole::credential_filename`].
    pub role: StreamTokenAuthorityRole,
    /// Registered universal single-key account for this purpose.
    pub account: AccountId,
}

/// Distinct protocol-only reserve accounts; neither has a generated signing credential.
///
/// Initial registration gives them no funds or permission. An ordinary governed reserve policy
/// must select them before native reserve movements can use them.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenReserveAccounts {
    /// Pooled native reserve custody, initially unfunded.
    pub custody: AccountId,
    /// Native reserve treasury, initially unfunded and distinct from custody.
    pub treasury: AccountId,
}

#[derive(Clone, Copy, JsonSerialize)]
#[norito(tag = "role", content = "value", rename_all = "snake_case")]
enum ReserveAccountRole {
    ReserveCustody,
    ReserveTreasury,
}

// These are local profile identities, not a new protocol-wide provider/account allocation rule.
// The canonical public-point derivation exposes no signing scalar, including for a seeded fixture.
fn reserve_accounts(provider: ProviderId) -> StreamTokenReserveAccounts {
    StreamTokenReserveAccounts {
        custody: AccountId::new(iroha_crypto::derive_non_signing_ed25519_public_key(
            b"iroha:localnet:stream-token-reserve-custody:v1",
            &[provider.as_bytes()],
        )),
        treasury: AccountId::new(iroha_crypto::derive_non_signing_ed25519_public_key(
            b"iroha:localnet:stream-token-reserve-treasury:v1",
            &[provider.as_bytes()],
        )),
    }
}

/// Identity-bound public authority inventory, not proof of current policy or provider admission.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenAuthorityManifest {
    /// Network derived from the exact retained signed genesis.
    pub network_id: NetworkId,
    /// Existing retained ledger client holding the initial management permissions.
    pub manager: AccountId,
    /// Locally allocated future provider scope; no provider is admitted by this inventory.
    pub provider_id: ProviderId,
    /// Exactly seven distinct registered accounts in the canonical role order.
    pub authorities: Vec<StreamTokenAuthority>,
    /// Mandatory original non-signing reserve account identities.
    pub reserve_accounts: StreamTokenReserveAccounts,
}

pub(super) struct GeneratedAuthorities {
    authorities: Vec<(StreamTokenAuthorityRole, LocalnetClientIdentity)>,
    provider_id: ProviderId,
}

pub(super) fn validate_selection(opts: &LocalnetOptions, managed: bool, taira: bool) -> Result<()> {
    if opts.service_profile == LocalnetServiceProfile::StreamTokenAuthorities {
        ensure!(
            managed
                && !taira
                && opts.sora_profile.is_none()
                && opts.peers.get() == 4
                && opts.consensus_mode == SumeragiConsensusMode::Permissioned,
            "native service authorities require the stock managed global four-validator profile"
        );
    }
    Ok(())
}

// ProviderId is protocol-assigned opaque bytes, not an account address. This is only the local
// generator's stable scope allocation, not a new network-wide key-to-provider identity rule.
fn provider_scope(operator: &iroha_crypto::PublicKey) -> Result<ProviderId> {
    let key = norito::encode_canonical(operator)?;
    let mut material = b"iroha.localnet.stream-token-provider.v1\0".to_vec();
    material.extend_from_slice(&key);
    Ok(ProviderId::new(*Hash::new(material).as_ref()))
}

pub(super) fn generate(
    profile: LocalnetServiceProfile,
    root: &Path,
    seed: Option<&[u8]>,
    manager: &LocalnetClientIdentity,
    http: &LocalnetClientIdentity,
    onboarding: &LocalnetClientIdentity,
) -> Result<Option<GeneratedAuthorities>> {
    if profile == LocalnetServiceProfile::Standard {
        return Ok(None);
    }
    let directory = root.join(LOCALNET_RUNTIME_DIRECTORY).join(DIRECTORY);
    custody::prepare_empty_private_directory(&directory)?;
    let mut unique = BTreeSet::from([
        manager.public_key.clone(),
        http.public_key.clone(),
        onboarding.public_key.clone(),
    ]);
    let mut authorities = Vec::with_capacity(ROLES.len());
    for role in ROLES {
        let label = format!("native-token-authorities/{}", role.credential_filename());
        let identity = localnet_ephemeral_identity(seed, label.as_bytes())?;
        ensure!(
            unique.insert(identity.public_key.clone()),
            "service credential identities must be distinct"
        );
        write_private_key_sidecar(
            &directory.join(role.credential_filename()),
            identity.private_key.as_str(),
        )?;
        authorities.push((role, identity));
    }
    let provider_id = provider_scope(&authorities[0].1.public_key)?;
    let reserves = reserve_accounts(provider_id);
    for account in [&reserves.custody, &reserves.treasury] {
        ensure!(
            unique.insert(
                account
                    .try_signatory()
                    .expect("derived single-key account")
                    .clone()
            ),
            "reserve accounts must be distinct from every generated signer"
        );
    }
    Ok(Some(GeneratedAuthorities {
        authorities,
        provider_id,
    }))
}

fn grants(
    manager: &AccountId,
    provider_id: ProviderId,
    authorities: &[StreamTokenAuthority],
) -> Vec<(AccountId, Permission)> {
    vec![
        (manager.clone(), CanSetSorafsReservePolicy.into()),
        (manager.clone(), CanUpsertSorafsProviderCredit.into()),
        (manager.clone(), CanManageSorafsStreamTokenGateway.into()),
        (
            manager.clone(),
            CanManageSorafsReputationJournalPolicy.into(),
        ),
        (
            manager.clone(),
            CanManageSorafsStreamTokenCustody { provider_id }.into(),
        ),
        (
            authorities[0].account.clone(),
            CanOperateSorafsStreamToken { provider_id }.into(),
        ),
        (
            authorities[0].account.clone(),
            CanDeclareSorafsCapacity.into(),
        ),
        (
            authorities[1].account.clone(),
            CanCheckSorafsStreamToken { provider_id }.into(),
        ),
        (
            authorities[6].account.clone(),
            CanRecordSorafsReputationJournal.into(),
        ),
    ]
}
impl GeneratedAuthorities {
    // Used only by the unpublished bootstrap and final config render, before genesis signing.
    // State seeds this exact map only before its first block; later owner changes need governance.
    pub(super) fn seed_provider_owner(
        &self,
        rendered: &str,
        chain_discriminant: Option<u16>,
    ) -> Result<Zeroizing<String>> {
        let mut table = crate::secret_toml::Table::new(crate::secret_toml::parse_table(
            rendered,
            "generated service authority config",
        )?);
        let gov = table
            .entry("gov")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| eyre!("generated governance config is not a table"))?;
        ensure!(
            !gov.contains_key("sorafs_provider_owners"),
            "generated provider owner seed is already present"
        );
        let mut owners = toml::Table::new();
        owners.insert(
            hex::encode(self.provider_id.as_bytes()),
            toml::Value::String(account_id_runtime_literal(
                &self.authorities[0].1.account_id,
                chain_discriminant,
            )),
        );
        gov.insert("sorafs_provider_owners".into(), toml::Value::Table(owners));
        toml::to_string(&*table)
            .map(Zeroizing::new)
            .map_err(|_| eyre!("cannot encode service authority config"))
    }

    fn manifest(&self, network_id: NetworkId, manager: &AccountId) -> StreamTokenAuthorityManifest {
        StreamTokenAuthorityManifest {
            network_id,
            manager: manager.clone(),
            provider_id: self.provider_id,
            reserve_accounts: reserve_accounts(self.provider_id),
            authorities: self
                .authorities
                .iter()
                .map(|(role, identity)| StreamTokenAuthority {
                    role: *role,
                    account: identity.account_id.clone(),
                })
                .collect(),
        }
    }
    pub(super) fn append_genesis(
        &self,
        genesis: RawGenesisTransaction,
        manager: &AccountId,
    ) -> Result<RawGenesisTransaction> {
        let mut accounts: Vec<_> = self
            .authorities
            .iter()
            .map(|(role, identity)| {
                Account::new(identity.account_id.clone()).with_metadata(role_metadata(*role))
            })
            .collect();
        let reserves = reserve_accounts(self.provider_id);
        accounts.extend([
            Account::new(reserves.custody)
                .with_metadata(role_metadata(ReserveAccountRole::ReserveCustody)),
            Account::new(reserves.treasury)
                .with_metadata(role_metadata(ReserveAccountRole::ReserveTreasury)),
        ]);
        let genesis = append_localnet_service_accounts(genesis, &accounts)?;
        let mut builder = genesis.into_builder();
        for (role, identity) in &self.authorities {
            if role.transacts() {
                builder = builder.append_instruction(Mint::asset_quantity(
                    LOCALNET_ALIAS_SETUP_PAYER_BALANCE,
                    AssetId::new(
                        localnet_xor_asset_definition_id(),
                        identity.account_id.clone(),
                    ),
                ));
            }
        }
        // Only the accounts/provider are consumed here; no genesis-derived gateway scope is used.
        let authorities: Vec<_> = self
            .authorities
            .iter()
            .map(|(role, identity)| StreamTokenAuthority {
                role: *role,
                account: identity.account_id.clone(),
            })
            .collect();
        for (account, permission) in grants(manager, self.provider_id, &authorities) {
            builder = builder.append_instruction(Grant::account_permission(permission, account));
        }
        builder.build_raw()
    }
    pub(super) fn publish(
        &self,
        root: &Path,
        genesis: HashOf<BlockHeader>,
        manager: &AccountId,
    ) -> Result<()> {
        let manifest = self.manifest(NetworkId::from_genesis_hash(genesis), manager);
        let bytes = norito::json::to_vec(&manifest)?;
        ensure!(
            bytes.len() <= MAX_MANIFEST,
            "authority manifest exceeds byte bound"
        );
        custody::write(
            root.join(LOCALNET_RUNTIME_DIRECTORY)
                .join(DIRECTORY)
                .join(MANIFEST),
            bytes,
        )
    }
}

impl PreparedLocalnet {
    /// Load exact retained authority prerequisites without claiming current eligibility.
    ///
    /// # Errors
    /// Rejects wrong profile/root/genesis, substituted accounts or credentials, invalid grants,
    /// malformed bounded inventory or token services enabled outside a future provisioning owner.
    pub fn stream_token_authorities(
        &self,
    ) -> crate::managed::Result<Option<StreamTokenAuthorityManifest>> {
        validate_retained(self)
    }
}

// Callers authenticate original genesis with genesis_epoch first. Reusing the already decoded
// block keeps private-root validation within its existing read; no positive result is cached.
pub(super) fn validate_signed_profile(
    prepared: &PreparedLocalnet,
    block: &iroha_data_model::block::SignedBlock,
) -> crate::managed::Result<std::collections::BTreeMap<AccountId, Json>> {
    let invalid = || Error::Invalid("retained signed localnet service profile differs".into());
    block
        .validate_output_merkle_cache()
        .map_err(|_| invalid())?;
    if !block.output_results().all(|result| result.as_ref().is_ok())
        || NetworkId::from_genesis_hash(block.hash()).to_string() != prepared.context.network_id
    {
        return Err(invalid());
    }
    let metadata = iroha_data_model::sumeragi_finality::signed_genesis_consensus_metadata(block)
        .map_err(|_| invalid())?;
    if metadata.sumeragi_context.root_scope.dataspace_id().as_u64() != prepared.context.dataspace_id
        || (metadata.sumeragi_context.root_scope == SumeragiRootScope::Global
            && prepared.context.dataspace_alias != "universal")
    {
        return Err(invalid());
    }
    // The retained literal owns its address rendering. A parent SDK operation may have
    // installed a different ambient prefix; it cannot reinterpret this domainless identity.
    let discriminant = iroha_data_model::account::address::AccountAddress::i105_discriminant(
        &prepared.context.account_id,
    )
    .map_err(|_| invalid())?;
    let _address_profile = ChainDiscriminantGuard::enter(discriminant);
    let manager = AccountId::parse_encoded(&prepared.context.account_id).map_err(|_| invalid())?;
    let expected_profile = Json::new(prepared.service_profile);
    let mut found_profile = false;
    let mut registered_roles = std::collections::BTreeMap::new();
    let role_key: iroha_model_base::name::Name = ROLE_METADATA
        .parse()
        .expect("static authority metadata key");
    for transaction in block.external_transactions() {
        for instruction in transaction.instructions().explicit_instructions() {
            if let Some(iroha_data_model::isi::SetKeyValueBox::Account(set)) =
                instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::SetKeyValueBox>()
                && set.key().as_ref() == PROFILE_METADATA
            {
                if found_profile || set.object() != &manager || set.value() != &expected_profile {
                    return Err(invalid());
                }
                found_profile = true;
            }
            if let Some(RegisterBox::Account(register)) =
                instruction.as_any().downcast_ref::<RegisterBox>()
                && let Some(role) = register.object.metadata.get(&role_key)
                && registered_roles
                    .insert(register.object.id.clone(), role.clone())
                    .is_some()
            {
                return Err(invalid());
            }
        }
    }
    if !found_profile {
        return Err(invalid());
    }
    if prepared.service_profile == LocalnetServiceProfile::Standard {
        if !registered_roles.is_empty() {
            return Err(invalid());
        }
        let directory = prepared
            .context
            .client_config
            .parent()
            .ok_or_else(invalid)?
            .join(LOCALNET_RUNTIME_DIRECTORY)
            .join(DIRECTORY);
        match fs::symlink_metadata(&directory) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(invalid()),
        }
    }
    Ok(registered_roles)
}

pub(crate) fn validate_retained(
    prepared: &PreparedLocalnet,
) -> crate::managed::Result<Option<StreamTokenAuthorityManifest>> {
    let invalid =
        || Error::Invalid("retained stream-token authority prerequisites are invalid".into());
    let root = prepared
        .context
        .client_config
        .parent()
        .ok_or_else(invalid)?;
    let directory = root.join(LOCALNET_RUNTIME_DIRECTORY).join(DIRECTORY);
    if prepared.context.client_config.file_name() != Some(std::ffi::OsStr::new("client.toml"))
        || prepared.peers.len() != 4
        || prepared
            .peers
            .iter()
            .enumerate()
            .any(|(index, peer)| peer.config_path != root.join(format!("peer{index}.toml")))
    {
        return Err(invalid());
    }
    init_instruction_registry();
    let bytes =
        iroha_fs::read_private(root.join("genesis.signed.nrt"), SIGNED_GENESIS_MAX_BYTES_V1)?;
    let block =
        iroha_data_model::block::decode_framed_signed_block(&bytes).map_err(|_| invalid())?;
    iroha_data_model::sumeragi_finality::genesis_epoch(&block).map_err(|_| invalid())?;
    let registered_roles = validate_signed_profile(prepared, &block)?;
    if prepared.service_profile == LocalnetServiceProfile::Standard {
        return Ok(None);
    }
    if prepared.context.dataspace_id != 0
        || prepared.context.dataspace_alias != "universal"
        || registered_roles.len() != ROLES.len() + 2
    {
        return Err(invalid());
    }
    let client = prepared.context.load_client_config()?;
    let _address_profile = ChainDiscriminantGuard::enter(client.account_chain_discriminant);
    let retained = iroha_fs::PrivateDirectory::open_exact(&directory)?;
    let expected_files = std::iter::once(std::ffi::OsString::from(MANIFEST))
        .chain(
            ROLES
                .into_iter()
                .map(|role| std::ffi::OsString::from(role.credential_filename())),
        )
        .collect::<BTreeSet<_>>();
    if retained
        .entries(expected_files.len())?
        .into_iter()
        .collect::<BTreeSet<_>>()
        != expected_files
    {
        return Err(invalid());
    }
    let bytes = retained.read(MANIFEST, MAX_MANIFEST)?;
    let manifest: StreamTokenAuthorityManifest =
        norito::json::from_slice(&bytes).map_err(|_| invalid())?;
    if manifest.authorities.len() != ROLES.len()
        || manifest.network_id.to_string() != prepared.context.network_id
    {
        return Err(invalid());
    }
    if manifest.manager != client.account || manifest.network_id != client.network_id {
        return Err(invalid());
    }
    if manifest.reserve_accounts != reserve_accounts(manifest.provider_id) {
        return Err(invalid());
    }
    let mut unique = BTreeSet::from([manifest.manager.clone()]);
    for (account, role) in [
        (
            &manifest.reserve_accounts.custody,
            ReserveAccountRole::ReserveCustody,
        ),
        (
            &manifest.reserve_accounts.treasury,
            ReserveAccountRole::ReserveTreasury,
        ),
    ] {
        if !unique.insert(account.clone())
            || registered_roles.get(account) != Some(&Json::new(role))
        {
            return Err(invalid());
        }
    }
    for (authority, role) in manifest.authorities.iter().zip(ROLES) {
        if authority.role != role
            || !unique.insert(authority.account.clone())
            || registered_roles.get(&authority.account) != Some(&Json::new(role))
        {
            return Err(invalid());
        }
        let bytes = retained.read(role.credential_filename(), 256)?;
        let text = bytes
            .strip_suffix(b"\n")
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .ok_or_else(invalid)?;
        let secret = Zeroizing::new(text.to_owned());
        let exposed: ExposedPrivateKey = secret.parse().map_err(|_| invalid())?;
        if Zeroizing::new(exposed.try_to_multihash_string().map_err(|_| invalid())?).as_str()
            != text
        {
            return Err(invalid());
        }
        let key = KeyPair::from_private_key(exposed.0).map_err(|_| invalid())?;
        if key.public_key().algorithm() != iroha_crypto::Algorithm::Ed25519
            || AccountId::new(key.public_key().clone()) != authority.account
        {
            return Err(invalid());
        }
    }
    if provider_scope(
        manifest.authorities[0]
            .account
            .try_signatory()
            .ok_or_else(invalid)?,
    )
    .map_err(|_| invalid())?
        != manifest.provider_id
    {
        return Err(invalid());
    }
    let metadata = iroha_data_model::sumeragi_finality::signed_genesis_consensus_metadata(&block)
        .map_err(|_| invalid())?;
    if metadata.sumeragi_context.root_scope != SumeragiRootScope::Global
        || NetworkId::from_genesis_hash(block.hash()) != manifest.network_id
    {
        return Err(invalid());
    }
    let mut accounts = BTreeSet::new();
    let mut remaining = grants(
        &manifest.manager,
        manifest.provider_id,
        &manifest.authorities,
    );
    let mut funded = BTreeSet::new();
    for transaction in block.external_transactions() {
        for instruction in transaction.instructions().explicit_instructions() {
            if let Some(RegisterBox::Account(register)) =
                instruction.as_any().downcast_ref::<RegisterBox>()
            {
                accounts.insert(register.object.id.clone());
            }
            if let Some(GrantBox::Permission(grant)) =
                instruction.as_any().downcast_ref::<GrantBox>()
            {
                remaining.retain(|(account, permission)| {
                    grant.destination() != account || grant.object() != permission
                });
                if matches!(
                    grant.object().name().as_ref(),
                    "CanOperateSorafsStreamTokenGateway" | "CanCheckSorafsStreamTokenGateway"
                ) {
                    return Err(invalid());
                }
            }
            if let Some(MintBox::Asset(mint)) = instruction.as_any().downcast_ref::<MintBox>() {
                for authority in &manifest.authorities {
                    if mint.destination()
                        == &AssetId::new(
                            localnet_xor_asset_definition_id(),
                            authority.account.clone(),
                        )
                        && mint.object() == &Quantity::from(LOCALNET_ALIAS_SETUP_PAYER_BALANCE)
                    {
                        funded.insert(authority.account.clone());
                    }
                }
            }
        }
    }
    if !remaining.is_empty()
        || manifest.authorities.iter().any(|entry| {
            !accounts.contains(&entry.account)
                || entry.role.transacts() && !funded.contains(&entry.account)
        })
    {
        return Err(invalid());
    }
    for peer in &prepared.peers {
        let bytes = iroha_fs::read_private(&peer.config_path, 1024 * 1024)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| invalid())?;
        let config =
            parse_localnet_peer_config(text, Some(&peer.config_path)).map_err(|_| invalid())?;
        if config.genesis.expected_hash != block.hash()
            || config.gov.sorafs_provider_owners
                != std::collections::BTreeMap::from([(
                    manifest.provider_id,
                    manifest.authorities[0].account.clone(),
                )])
            || configured_execution_policy(&config).map_err(|_| invalid())?
                != Hash::prehashed(metadata.sumeragi_context.execution_policy_hash)
            || config.torii.sorafs_storage.stream_tokens.enabled
            || config.torii.sorafs_storage.stream_tokens.signer.is_some()
            || config
                .torii
                .sorafs_storage
                .stream_tokens
                .admission_native
                .is_some()
        {
            return Err(invalid());
        }
    }
    retained.revalidate()?;
    Ok(Some(manifest))
}

// Reuse the canonical runtime policy owners; no native genesis execution or cached authority is
// needed on a retained reopen. The closed profile has no external compliance policy source.
fn configured_execution_policy(config: &actual::Root) -> Result<Hash> {
    ensure!(
        !config.nexus.compliance.enabled,
        "service authority profile cannot select an external compliance policy"
    );
    let manifests = iroha_core::governance::manifest::LaneManifestRegistry::from_config(
        &config.nexus.lane_catalog,
        &config.nexus.governance,
        &config.nexus.registry,
    );
    manifests
        .validate_active_coverage_for_catalog(&config.nexus.lane_catalog)
        .map_err(|_| eyre!("service authority lane policies are invalid"))?;
    let nexus = actual::nexus_consensus_policy_digest_with_runtime_policies(
        &config.nexus,
        None,
        Some(manifests.baseline_consensus_policy_digest()),
    )
    .map_err(|_| eyre!("service authority Nexus policy is invalid"))?;
    Ok(Hash::prehashed(actual::execution_policy_digest_v1(
        &config.pipeline,
        &config.oracle,
        &config.crypto,
        &config.fraud_monitoring,
        &config.gov,
        &config.content,
        &config.settlement,
        nexus,
        iroha_core::state::compute_zk_consensus_policy_hash(&config.zk),
    )))
}

#[cfg(test)]
mod tests;

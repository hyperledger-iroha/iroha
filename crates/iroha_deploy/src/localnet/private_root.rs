//! Independently signed, owner-operated private-root preparation.

use super::*;
use crate::managed::{Error, LocalnetPorts, ManagedContext, ManagedPeer, PreparedLocalnet};
use iroha_data_model::{
    NetworkId,
    block::consensus::SumeragiRootScope,
    hijiri::HijiriParametersV1,
    nexus::{DataSpaceCatalog, DataSpaceMetadata, RuntimeDataSpaceAdditionV1},
    parameter::{Parameter, Parameters},
};
use norito::{JsonDeserialize, JsonSerialize};

const PREPARED: &str = "private-root-prepared.json";

/// Exact parent catalog identity for an independently operated private ledger.
///
/// The caller must authenticate the parent catalog admission of this descriptor. Decoding or
/// validating this value proves structural identity only. The stable catalog manifest hash is
/// separate from the child genesis and from the native lane governance manifest; it must be
/// supplied from genuine parent admission, never synthesized to obtain a chosen identifier.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct PrivateRootSpec {
    /// Genesis-bound identity of the parent network.
    pub parent_network_id: NetworkId,
    /// Full-width identifier assigned by the authenticated parent catalog.
    pub dataspace_id: DataSpaceId,
    /// Canonical parent-reserved namespace.
    pub dataspace_alias: String,
    /// Stable parent catalog identity hash whose first eight bytes derive `dataspace_id`.
    pub manifest_hash: [u8; 32],
}

impl PrivateRootSpec {
    /// Validate the exact parent catalog hash/identifier/alias binding.
    ///
    /// # Errors
    /// Rejects reserved identity, malformed aliases and a hash deriving a different identifier.
    pub fn validate(&self) -> Result<()> {
        let addition = RuntimeDataSpaceAdditionV1 {
            descriptor: DataSpaceMetadata {
                id: self.dataspace_id,
                alias: self.dataspace_alias.clone(),
                description: None,
                fault_tolerance: 1,
            },
            manifest_hash: self.manifest_hash,
        };
        addition.validate_structure()?;
        let catalog = DataSpaceCatalog::new(vec![addition.descriptor])?;
        ResolvedDataSpaceV1::resolve_catalog(&self.dataspace_alias, &catalog)?;
        DomainId::parse_fully_qualified(&format!("app.{}", self.dataspace_alias))?;
        self.scope().validate()?;
        Ok(())
    }

    /// Immutable native root identity embedded in the signed child genesis.
    #[must_use]
    pub const fn scope(&self) -> SumeragiRootScope {
        SumeragiRootScope::Dataspace {
            parent_network_id: self.parent_network_id,
            dataspace_id: self.dataspace_id,
        }
    }
}

#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RetainedPrivateRoot {
    spec: PrivateRootSpec,
    prepared: PreparedLocalnet,
}

/// Prepare four loopback validators for an independent, owner-operated private root.
///
/// This only prepares local artifacts. It neither registers with the parent nor starts a node.
/// The generated ledger owner is returned in the context and retained for parent registration.
/// Reopening a completed generation returns that exact identity. An interrupted incomplete
/// generation is retained and rejected; it is never silently replaced with new signing keys.
/// TODO: Resume partially prepared artifacts through the parent-admission operation journal.
///
/// # Errors
/// Rejects unsafe custody, conflicting identity, incomplete preparation or genuine Core genesis
/// execution failure. Parent admission and runtime readiness remain separate required steps.
pub fn prepare_private_root(
    name: &str,
    directory: &Path,
    ports: &LocalnetPorts,
    spec: &PrivateRootSpec,
) -> crate::managed::Result<PreparedLocalnet> {
    spec.validate()
        .map_err(|error| Error::Invalid(format!("private-root catalog is invalid: {error}")))?;
    if directory.exists() {
        let root = iroha_fs::PrivateDirectory::open(directory)?;
        match root.read(PREPARED, 1024 * 1024) {
            Ok(bytes) => {
                let retained: RetainedPrivateRoot = norito::json::from_slice(&bytes)
                    .map_err(|_| Error::Invalid("retained private root is invalid".into()))?;
                if retained.spec != *spec || retained.prepared.context.name != name {
                    return Err(Error::Invalid(
                        "retained private-root identity differs".into(),
                    ));
                }
                verify_retained(root.path(), &retained.prepared, spec)?;
                return Ok(retained.prepared);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let prepared = prepare_fresh(name, directory, ports, spec)
        .map_err(|error| Error::Invalid(format!("private-root preparation failed: {error:#}")))?;
    verify_retained(&directory.canonicalize()?, &prepared, spec)?;
    let retained = RetainedPrivateRoot {
        spec: spec.clone(),
        prepared: prepared.clone(),
    };
    let root = iroha_fs::PrivateDirectory::open(directory)?;
    root.write_atomic(
        PREPARED,
        norito::json::to_json(&retained)
            .map_err(|_| Error::Invalid("cannot encode prepared private root".into()))?
            .as_bytes(),
        iroha_fs::PublishMode::CreateNew,
    )?;
    Ok(prepared)
}

fn verify_retained(
    root: &Path,
    prepared: &PreparedLocalnet,
    spec: &PrivateRootSpec,
) -> crate::managed::Result<()> {
    let invalid = || Error::Invalid("retained private-root artifact binding differs".into());
    if prepared.context.client_config != root.join("client.toml")
        || prepared.context.dataspace_id != spec.dataspace_id.as_u64()
        || prepared.context.dataspace_alias != spec.dataspace_alias
        || prepared.peers.len() != 4
        || prepared
            .peers
            .iter()
            .enumerate()
            .any(|(index, peer)| peer.config_path != root.join(format!("peer{index}.toml")))
    {
        return Err(invalid());
    }
    let client = prepared.context.load_client_config()?;
    prepared.load_operator_key_pair()?;
    verify_private_credentials(prepared)?;
    let bytes = iroha_fs::read_private(
        &root.join("genesis.signed.nrt"),
        SIGNED_GENESIS_MAX_BYTES_V1,
    )?;
    let block =
        iroha_data_model::block::decode_framed_signed_block(&bytes).map_err(|_| invalid())?;
    iroha_data_model::sumeragi_finality::genesis_epoch(&block).map_err(|_| invalid())?;
    let metadata = iroha_data_model::sumeragi_finality::signed_genesis_consensus_metadata(&block)
        .map_err(|_| invalid())?;
    if metadata.sumeragi_context.root_scope != spec.scope()
        || NetworkId::from_genesis_hash(block.hash()).to_string() != prepared.context.network_id
    {
        return Err(invalid());
    }
    let management = Permission::from(CanManageSmartContractCode);
    let mut found_owner = false;
    for transaction in block.external_transactions() {
        for instruction in transaction.instructions().explicit_instructions() {
            if let Some(GrantBox::Permission(grant)) =
                instruction.as_any().downcast_ref::<GrantBox>()
                && grant.object() == &management
            {
                if found_owner || grant.destination() != &client.account {
                    return Err(invalid());
                }
                found_owner = true;
            }
        }
    }
    if !found_owner {
        return Err(invalid());
    }
    Ok(())
}

fn verify_private_credentials(prepared: &PreparedLocalnet) -> crate::managed::Result<()> {
    let invalid = || Error::Invalid("private-root listener credential binding is invalid".into());
    let client = iroha_fs::read_private(&prepared.context.client_config, 1024 * 1024)?;
    let client = crate::secret_toml::Table::new(
        crate::secret_toml::parse_table(
            std::str::from_utf8(&client).map_err(|_| invalid())?,
            "private-root client",
        )
        .map_err(|_| invalid())?,
    );
    let token = client
        .get("api_token")
        .and_then(toml::Value::as_str)
        .ok_or_else(invalid)?;
    if token.len() != 64
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid());
    }
    for peer in &prepared.peers {
        let bytes = iroha_fs::read_private(&peer.config_path, 1024 * 1024)?;
        let table = crate::secret_toml::Table::new(
            crate::secret_toml::parse_table(
                std::str::from_utf8(&bytes).map_err(|_| invalid())?,
                "private-root validator",
            )
            .map_err(|_| invalid())?,
        );
        let torii = table
            .get("torii")
            .and_then(toml::Value::as_table)
            .ok_or_else(invalid)?;
        if torii
            .get("require_api_token")
            .and_then(toml::Value::as_bool)
            != Some(true)
            || !matches!(torii.get("api_tokens").and_then(toml::Value::as_array), Some(tokens) if tokens.len() == 1 && tokens[0].as_str() == Some(token))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn prepare_fresh(
    name: &str,
    directory: &Path,
    ports: &LocalnetPorts,
    spec: &PrivateRootSpec,
) -> Result<PreparedLocalnet> {
    init_instruction_registry();
    let root = custody::prepare_empty_private_directory(directory)?;
    let chain = resolve_localnet_chain_id(None)?;
    let hosts = CanonicalHost::parse("127.0.0.1", "private-root loopback")?;
    let peers = build_peers(4, None, ports.base_api, ports.base_p2p)?;
    let owner = localnet_ephemeral_identity(None, b"private-root-owner")?;
    let operator = localnet_ephemeral_identity(None, b"private-root-http-operator")?;
    let mut token_bytes = Zeroizing::new([0_u8; 32]);
    OsRng
        .try_fill_bytes(token_bytes.as_mut())
        .map_err(|error| eyre!("private-root token generation failed: {error}"))?;
    let api_token = Zeroizing::new(hex::encode(&*token_bytes));
    let runtime = root.join(LOCALNET_RUNTIME_DIRECTORY);
    custody::create_directory(&runtime)?;
    custody::write(
        &runtime.join(LOCALNET_OPERATOR_SIGNER_KEY_FILE),
        Zeroizing::new(format!("{}\n", operator.private_key.as_str())).as_bytes(),
    )?;
    custody::write(
        &runtime.join(LOCALNET_LEDGER_SIGNER_KEY_FILE),
        Zeroizing::new(format!("{}\n", owner.private_key.as_str())).as_bytes(),
    )?;
    write_managed_mint_finality_seeds(&root, &peers)?;
    let (genesis_public, genesis_private) = generate_genesis_key_pair(None, GENESIS_SEED)?;
    write_genesis_key_files(
        &root.join(GENESIS_PUBLIC_KEY_FILE),
        &root.join(GENESIS_PRIVATE_KEY_FILE),
        &genesis_public,
        &genesis_private,
    )?;
    let gas = localnet_gas_account_id(&genesis_public);
    let genesis = private_genesis(
        spec,
        &chain,
        &genesis_public,
        &owner.account_id,
        &gas,
        &peers,
    )?;
    let rans = copy_rans_tables(&root)?;
    let signed_path = root.join("genesis.signed.nrt");
    let trusted = peers
        .iter()
        .map(|peer| format!("{}@{}", peer.public_key, hosts.addr_literal(peer.p2p_port)))
        .collect::<Vec<_>>();
    let urls = peers
        .iter()
        .map(|peer| hosts.torii_url(peer.api_port))
        .collect::<Vec<_>>();
    let bls = peers
        .iter()
        .map(|peer| BlsEntry {
            bls_pk: peer.bls_public_key.to_string(),
            pop_hex: format!("0x{}", hex::encode(&peer.bls_pop)),
        })
        .collect::<Vec<_>>();
    let owner_literal = owner.account_id.to_string();
    let gas_literal = gas.to_string();
    let render = |index: usize, identity| -> Result<Zeroizing<String>> {
        let paths = LocalnetPeerStoragePaths::new(&root, index);
        let raw = render_peer_config(
            &peers[index],
            &trusted,
            &urls,
            &genesis_public,
            &signed_path,
            identity,
            &bls,
            &paths,
            Some(&rans),
            &chain,
            None,
            (&hosts, &hosts),
            RenderPeerFeatures {
                mcp_enabled: false,
                npos_bootstrap: false,
                taira: false,
                operator_account: &owner_literal,
                operator_public_key: &operator.public_key,
                onboarding_account: &owner_literal,
                runtime: None,
            },
            None,
            None,
            None,
            &gas_literal,
            localnet_tx_gossip_overrides(LOCALNET_PIPELINE_TIME_MS),
            None,
            None,
            LOCALNET_QUEUE_CAPACITY,
        );
        let configured = private_peer_config(&raw, spec, &api_token)?;
        managed_peer_config(&configured, &managed_node_dir(&root, index))
    };
    let bootstrap = render(
        0,
        LocalnetGenesisIdentitySource::BootstrapInline(HashOf::from_untyped_unchecked(Hash::new(
            b"private-root staged policy binding",
        ))),
    )?;
    let config = parse_localnet_peer_config(&bootstrap, Some(&root.join("peer0.toml")))?;
    let expected = write_genesis(GenesisWriteContext {
        manifest: &genesis,
        public_key: &genesis_public,
        private_key: genesis_private,
        config: &config,
        chain_discriminant: None,
        json_path: &root.join("genesis.json"),
        signed_path: &signed_path,
        policies: GenesisConsensusPolicies {
            da_proof_policies: Some(resolve_localnet_da_proof_policies(&config)),
            confidential_policy_hash: iroha_core::state::compute_genesis_confidential_policy_hash(
                &config.zk,
            ),
        },
    })?;
    write_and_validate_genesis_expected_hash(
        &root.join(GENESIS_EXPECTED_HASH_FILE),
        &signed_path,
        expected,
    )?;
    for index in 0..4 {
        let paths = LocalnetPeerStoragePaths::new(&root, index);
        for path in [
            &paths.kura,
            &paths.state,
            &paths.tiered_state,
            &paths.da_store,
        ] {
            custody::ensure_directory(path)?;
        }
        let path = root.join(format!("peer{index}.toml"));
        let rendered = render(index, LocalnetGenesisIdentitySource::PublishedFile)?;
        let parsed = parse_localnet_peer_config(&rendered, Some(&path))?;
        ensure!(
            parsed.genesis.expected_hash == expected,
            "private-root genesis binding changed"
        );
        custody::write(&path, rendered.as_bytes())?;
    }
    write_client_config(&root, ports.base_api, &hosts, &chain, None, &owner)?;
    let client_path = root.join("client.toml");
    let client = iroha_fs::read_private(&client_path, 1024 * 1024)?;
    let mut table = crate::secret_toml::Table::new(crate::secret_toml::parse_table(
        std::str::from_utf8(&client)?,
        "private-root client",
    )?);
    table.remove("basic_auth");
    table.insert(
        "api_token".into(),
        toml::Value::String(api_token.as_str().into()),
    );
    table
        .get_mut("account")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| eyre!("private-root client account is absent"))?
        .insert(
            "domain".into(),
            toml::Value::String(format!("app.{}", spec.dataspace_alias)),
        );
    custody::replace(
        &client_path,
        Zeroizing::new(toml::to_string(&*table)?).as_bytes(),
    )?;
    custody::validate_private_tree(&root, &[])?;
    Ok(PreparedLocalnet {
        context: ManagedContext {
            name: name.into(),
            chain_id: chain,
            network_id: NetworkId::from_genesis_hash(expected).to_string(),
            account_id: owner_literal,
            dataspace_id: spec.dataspace_id.as_u64(),
            dataspace_alias: spec.dataspace_alias.clone(),
            torii_url: urls[0].clone(),
            client_config: client_path,
        },
        peers: (0..4)
            .map(|index| ManagedPeer {
                config_path: root.join(format!("peer{index}.toml")),
                torii_url: urls[index].clone(),
                log_name: format!("peer{index}.log"),
            })
            .collect(),
    })
}

fn private_peer_config(
    raw: &str,
    spec: &PrivateRootSpec,
    api_token: &str,
) -> Result<Zeroizing<String>> {
    use toml::{Table, Value};
    let mut root = crate::secret_toml::Table::new(crate::secret_toml::parse_table(
        raw,
        "private-root validator",
    )?);
    let nexus = root
        .get_mut("nexus")
        .and_then(Value::as_table_mut)
        .ok_or_else(|| eyre!("private-root Nexus config absent"))?;
    let mut dataspace = Table::new();
    dataspace.insert("alias".into(), Value::String(spec.dataspace_alias.clone()));
    // Use the authenticated complete hash. No TOML integer conversion can truncate its u64 ID.
    dataspace.insert(
        "manifest_hash".into(),
        Value::String(hex::encode(spec.manifest_hash)),
    );
    dataspace.insert("fault_tolerance".into(), Value::Integer(1));
    nexus.insert(
        "dataspace_catalog".into(),
        Value::Array(vec![Value::Table(dataspace)]),
    );
    let mut lane = Table::new();
    lane.insert("index".into(), Value::Integer(0));
    lane.insert("alias".into(), Value::String(spec.dataspace_alias.clone()));
    lane.insert(
        "dataspace".into(),
        Value::String(spec.dataspace_alias.clone()),
    );
    lane.insert("visibility".into(), Value::String("restricted".into()));
    lane.insert("storage".into(), Value::String("full_replica".into()));
    nexus.insert("lane_count".into(), Value::Integer(1));
    nexus.insert(
        "lane_catalog".into(),
        Value::Array(vec![Value::Table(lane)]),
    );
    let mut routing = Table::new();
    routing.insert("default_lane".into(), Value::Integer(0));
    routing.insert(
        "default_dataspace".into(),
        Value::String(spec.dataspace_alias.clone()),
    );
    nexus.insert("routing_policy".into(), Value::Table(routing));
    let torii = root
        .entry("torii")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| eyre!("private-root Torii config is invalid"))?;
    torii.insert("require_api_token".into(), Value::Boolean(true));
    torii.insert(
        "api_tokens".into(),
        Value::Array(vec![Value::String(api_token.into())]),
    );
    Ok(Zeroizing::new(toml::to_string(&*root)?))
}

fn private_genesis(
    spec: &PrivateRootSpec,
    chain: &str,
    genesis_key: &iroha_crypto::PublicKey,
    owner: &AccountId,
    gas: &AccountId,
    peers: &[Peer],
) -> Result<RawGenesisTransaction> {
    let genesis_authority = AccountId::new(genesis_key.clone());
    let domain = DomainId::parse_fully_qualified(&format!("app.{}", spec.dataspace_alias))?;
    let mut context = SumeragiGenesisContextParameters::recommended();
    context.root_scope = spec.scope();
    let mut builder = GenesisBuilder::new_without_executor(chain.parse()?, PathBuf::from("."))
        .with_sumeragi_context_parameters(context)
        .with_kagemusha_mint_finality_genesis_parameters(
            localnet_kagemusha_mint_finality_genesis_parameters(peers)?,
        );
    let mut parameters = Parameters::default();
    parameters.set_parameter(Parameter::Custom(
        HijiriParametersV1::first_release_genesis().into_custom_parameter(),
    ));
    for parameter in parameters.parameters() {
        builder = builder.append_parameter(parameter);
    }
    builder = builder
        .append_instruction(Register::account(Account::new(owner.clone())))
        .append_instruction(Register::account(Account::new(gas.clone())));
    for peer in peers {
        builder = builder.append_instruction(Register::account(Account::new(
            peer.validator_account_id(false),
        )));
    }
    let temporary_role: RoleId = "private_root_genesis_alias_setup".parse()?;
    builder = builder
        .next_transaction()
        .append_instruction(Register::role(
            Role::new(temporary_role.clone(), genesis_authority)
                .add_permission(CanManageAccountAlias {
                    scope: AccountAliasPermissionScope::Dataspace(spec.dataspace_id),
                })
                .add_permission(CanManageAccountAlias {
                    scope: AccountAliasPermissionScope::Domain(domain.clone()),
                }),
        ));
    let quote = AliasQuoteGuardV1 {
        expected_policy_version: LOCALNET_ALIAS_SETUP_POLICY_VERSION,
        expected_payment_asset: AssetDefinitionId::derive_from_components(
            domain.clone(),
            "gas".parse()?,
        ),
        max_amount: LOCALNET_PRIVATE_SNS_LEASE_PAYMENT
            .parse()
            .map_err(|error| eyre!("private bootstrap quote: {error}"))?,
        valid_until_ms: u64::MAX,
    };
    for intent in [
        AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
            dataspace: ResolvedDataSpaceV1::new(spec.dataspace_alias.parse()?, spec.dataspace_id),
            owner: owner.clone(),
        }),
        AliasIntentV1::Domain(AliasDomainIntentV1 {
            domain: ResolvedDomainV1::new(domain.clone(), spec.dataspace_id),
            owner: owner.clone(),
        }),
    ] {
        builder = builder.append_instruction(EnsureAlias::new(
            intent,
            AliasLeaseAcquisitionV1::new(1, None),
            quote.clone(),
        ));
    }
    builder = builder
        .append_instruction(Unregister::role(temporary_role))
        .next_transaction();
    // Sample holdings belong to this root's sole dataspace. No global balance bucket or
    // public-network fee asset is created by the private development recipe.
    for (name, label) in [("sample", "Private sample"), ("gas", "Private gas")] {
        let definition = AssetDefinitionId::derive_from_components(domain.clone(), name.parse()?);
        builder = builder
            .append_instruction(Register::asset_definition(AssetDefinition::new(
                definition.clone(),
                label.to_owned(),
                NumericSpec::fractional(LOCALNET_FEE_ASSET_SCALE),
                iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
                Some(domain.clone()),
            )))
            .append_instruction(SetAssetDefinitionAlias::bind(
                definition.clone(),
                format!("{name}#app.{}", spec.dataspace_alias).parse()?,
                None,
            ))
            .append_instruction(Mint::asset_quantity(
                LOCALNET_REQUESTED_ASSET_INITIAL_QUANTITY,
                AssetId::with_scope(
                    definition.clone(),
                    owner.clone(),
                    iroha_data_model::asset::AssetBalanceScope::Dataspace(spec.dataspace_id),
                ),
            ))
            .append_instruction(Transfer::asset_definition(
                AccountId::new(genesis_key.clone()),
                definition,
                owner.clone(),
            ));
    }
    builder = builder.next_transaction();
    for permission in [
        Permission::from(CanManageSmartContractCode),
        Permission::from(CanGrantSmartContractCodeManagement),
        Permission::from(CanReadAllLedgerData),
        Permission::from(CanReadRestrictedDataspace {
            dataspace: spec.dataspace_id,
        }),
        Permission::from(CanRegisterAccount { domain }),
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(spec.dataspace_id),
        }),
        Permission::from(CanPublishSpaceDirectoryManifest {
            dataspace: spec.dataspace_id,
        }),
    ] {
        builder = builder.append_instruction(Grant::account_permission(permission, owner.clone()));
    }
    let genesis = builder
        .build_raw()?
        .with_consensus_mode(SumeragiConsensusMode::Permissioned);
    let genesis = apply_parameter_overrides(
        genesis,
        NonZeroU16::new(4).unwrap(),
        Some(LOCALNET_PIPELINE_TIME_MS),
        LOCALNET_BLOCK_MAX_TRANSACTIONS,
        SumeragiConsensusMode::Permissioned,
    )?;
    let genesis = append_peer_pop(genesis, peers)?;
    Ok(apply_localnet_crypto_overrides(genesis)?.with_consensus_meta())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> PrivateRootSpec {
        // An opaque admitted catalog identity in this fixture. Select an actual hash above
        // the signed TOML integer range so accidental integer narrowing cannot hide in tests.
        let manifest_hash = (0_u64..)
            .map(|nonce| <[u8; 32]>::from(Hash::new(nonce.to_le_bytes())))
            .find(|hash| DataSpaceId::from_hash(hash).as_u64() > i64::MAX as u64)
            .unwrap();
        PrivateRootSpec {
            parent_network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new(b"private-root parent fixture"),
            )),
            dataspace_id: DataSpaceId::from_hash(&manifest_hash),
            dataspace_alias: "privateapp".into(),
            manifest_hash,
        }
    }

    #[test]
    fn exact_catalog_identity_and_scope_preserve_all_64_bits() {
        let spec = spec();
        spec.validate().unwrap();
        assert_eq!(spec.scope().dataspace_id(), spec.dataspace_id);
        let encoded = norito::json::to_json(&spec).unwrap();
        assert_eq!(
            norito::json::from_str::<PrivateRootSpec>(&encoded).unwrap(),
            spec
        );
        let mut wrong = spec.clone();
        wrong.dataspace_id = DataSpaceId::new(spec.dataspace_id.as_u64() ^ 1);
        assert!(wrong.validate().is_err());
        wrong = spec.clone();
        wrong.dataspace_alias = "universal".into();
        assert!(wrong.validate().is_err());
        wrong = spec;
        wrong.dataspace_id = DataSpaceId::UNIVERSAL;
        assert!(wrong.validate().is_err());
    }

    #[test]
    fn private_config_has_one_restricted_lane_and_unmodified_catalog_hash() {
        let spec = spec();
        let config = private_peer_config("[nexus]\n", &spec, "private-fixture-token").unwrap();
        let parsed: toml::Table = config.parse().unwrap();
        let nexus = parsed["nexus"].as_table().unwrap();
        let dataspaces = nexus["dataspace_catalog"].as_array().unwrap();
        assert_eq!(dataspaces.len(), 1);
        assert!(dataspaces[0].get("id").is_none());
        assert_eq!(
            dataspaces[0]["manifest_hash"].as_str(),
            Some(hex::encode(spec.manifest_hash).as_str())
        );
        let lanes = nexus["lane_catalog"].as_array().unwrap();
        assert_eq!(lanes.len(), 1);
        assert_eq!(lanes[0]["index"].as_integer(), Some(0));
        assert_eq!(lanes[0]["visibility"].as_str(), Some("restricted"));
        assert_eq!(
            lanes[0]["dataspace"].as_str(),
            Some(spec.dataspace_alias.as_str())
        );
        assert_eq!(
            nexus["routing_policy"]["default_dataspace"].as_str(),
            Some(spec.dataspace_alias.as_str())
        );
        assert_eq!(parsed["torii"]["require_api_token"].as_bool(), Some(true));
        assert_eq!(
            parsed["torii"]["api_tokens"][0].as_str(),
            Some("private-fixture-token")
        );
    }

    #[test]
    fn private_root_preparation_executes_signed_genesis_and_retains_owner_on_reopen() {
        let _guard = crate::managed::native_test_guard();
        let parent = tempfile::tempdir().unwrap();
        let directory = parent.path().join("private-root");
        let ports = LocalnetPorts::reserve().unwrap();
        let spec = spec();
        let prepared = prepare_private_root("private", &directory, &ports, &spec).unwrap();
        assert_eq!(prepared.context.dataspace_id, spec.dataspace_id.as_u64());
        assert_eq!(prepared.peers.len(), 4);
        verify_retained(&directory.canonicalize().unwrap(), &prepared, &spec).unwrap();
        let owner = prepared.context.load_client_config().unwrap().account;
        for peer in &prepared.peers {
            let bytes = iroha_fs::read_private(&peer.config_path, 1024 * 1024).unwrap();
            let table: toml::Table = std::str::from_utf8(&bytes).unwrap().parse().unwrap();
            assert!(table["torii"].get("faucet").is_none());
            assert!(table["torii"].get("account_onboarding").is_none());
            let config = parse_localnet_peer_config(
                std::str::from_utf8(&bytes).unwrap(),
                Some(&peer.config_path),
            )
            .unwrap();
            assert_eq!(config.nexus.lane_catalog.lanes().len(), 1);
            assert_eq!(
                config.nexus.lane_catalog.lanes()[0].dataspace_id,
                spec.dataspace_id
            );
        }
        let manifest = RawGenesisTransaction::from_path(&directory.join("genesis.json")).unwrap();
        assert_eq!(
            manifest.sumeragi_context_parameters().root_scope,
            spec.scope()
        );
        assert!(
            !manifest
                .effective_parameters()
                .unwrap()
                .custom()
                .contains_key(&PrivateDataspaceAdmissionPolicy::parameter_id()),
            "a private root cannot enable the parent admission registry"
        );
        let code_grants = manifest
            .instructions()
            .filter_map(|instruction| {
                let GrantBox::Permission(grant) =
                    instruction.as_any().downcast_ref::<GrantBox>()?
                else {
                    return None;
                };
                (grant.object() == &Permission::from(CanManageSmartContractCode))
                    .then(|| grant.destination().clone())
            })
            .collect::<Vec<_>>();
        assert_eq!(code_grants, vec![owner.clone()]);
        let definitions = manifest
            .instructions()
            .filter_map(|instruction| {
                let RegisterBox::AssetDefinition(registration) =
                    instruction.as_any().downcast_ref::<RegisterBox>()?
                else {
                    return None;
                };
                Some(registration.object())
            })
            .collect::<Vec<_>>();
        assert_eq!(definitions.len(), 2);
        for definition in definitions {
            assert_eq!(
                definition.balance_scope_policy,
                iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted
            );
            assert_eq!(
                definition.owning_domain.as_ref(),
                Some(
                    &DomainId::parse_fully_qualified(&format!("app.{}", spec.dataspace_alias))
                        .unwrap()
                )
            );
        }
        let mints = manifest
            .instructions()
            .filter_map(|instruction| {
                let MintBox::Asset(mint) = instruction.as_any().downcast_ref::<MintBox>()? else {
                    return None;
                };
                Some(mint.destination())
            })
            .collect::<Vec<_>>();
        assert_eq!(mints.len(), 2);
        for destination in mints {
            assert_eq!(destination.account(), &owner);
            assert_eq!(
                destination.scope(),
                &iroha_data_model::asset::AssetBalanceScope::Dataspace(spec.dataspace_id)
            );
        }
        let repeated = prepare_private_root("private", &directory, &ports, &spec).unwrap();
        assert_eq!(repeated, prepared);
        let peer_path = &prepared.peers[0].config_path;
        let bytes = iroha_fs::read_private(peer_path, 1024 * 1024).unwrap();
        let mut table = crate::secret_toml::Table::new(
            crate::secret_toml::parse_table(
                std::str::from_utf8(&bytes).unwrap(),
                "test private validator",
            )
            .unwrap(),
        );
        table["torii"]
            .as_table_mut()
            .unwrap()
            .insert("require_api_token".into(), toml::Value::Boolean(false));
        custody::replace(
            peer_path,
            Zeroizing::new(toml::to_string(&*table).unwrap()).as_bytes(),
        )
        .unwrap();
        assert!(verify_private_credentials(&prepared).is_err());
        custody::replace(peer_path, bytes.as_slice()).unwrap();
        let mut foreign = spec;
        foreign.parent_network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
            Hash::new(b"foreign parent"),
        ));
        assert!(prepare_private_root("private", &directory, &ports, &foreign).is_err());
    }

    #[test]
    fn incomplete_private_generation_never_overwrites_retained_keys() {
        let _guard = crate::managed::native_test_guard();
        let parent = tempfile::tempdir().unwrap();
        let directory =
            iroha_fs::PrivateDirectory::open_or_create(parent.path().join("private-root")).unwrap();
        directory
            .write_atomic(
                "retained-key",
                b"already-created",
                iroha_fs::PublishMode::CreateNew,
            )
            .unwrap();
        let ports = LocalnetPorts::reserve().unwrap();
        assert!(prepare_private_root("private", directory.path(), &ports, &spec()).is_err());
        assert_eq!(
            &*directory.read("retained-key", 32).unwrap(),
            b"already-created"
        );
    }
}

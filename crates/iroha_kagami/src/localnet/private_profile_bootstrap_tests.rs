#[test]
#[allow(clippy::too_many_lines)]
fn private_profiles_seed_exact_sns_owners_and_least_privilege_permissions() {
    struct Case {
        profile: SoraProfile,
        alias: &'static str,
        dataspace_id: u64,
        domains: &'static [&'static str],
    }
    for case in [
        Case {
            profile: SoraProfile::PrivateSbp,
            alias: "sbp",
            dataspace_id: LOCALNET_PAYNET_ALIAS_DATASPACE_ID,
            domains: SBP_BOOTSTRAP_DOMAINS,
        },
        Case {
            profile: SoraProfile::PrivateCbuae,
            alias: "cbuae",
            dataspace_id: LOCALNET_CBUAE_ALIAS_DATASPACE_ID,
            domains: &[],
        },
    ] {
        let seed = format!("private-profile-sns-bootstrap-{}", case.alias);
        let opts = LocalnetOptions {
            sora_profile: Some(case.profile),
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some(seed.clone()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29_080,
            base_p2p_port: 33_337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        let (genesis_public_key, _) =
            generate_genesis_key_pair(Some(seed.as_bytes()), GENESIS_SEED)
                .expect("derive test genesis account");
        let genesis_account_id = AccountId::new(genesis_public_key);
        let client_identity = localnet_client_identity(Some(seed.as_bytes()), true)
            .expect("derive fresh private-profile client");
        let client_account_id = client_identity.account_id;
        let payment_amount: Quantity = LOCALNET_PRIVATE_SNS_LEASE_PAYMENT
            .parse()
            .expect("parse expected SNS lease payment");
        let manifest = localnet_genesis_for_opts_and_client(&opts, &client_account_id);
        let expected_normalized = manifest
            .clone()
            .normalize()
            .expect("normalize in-memory private-profile genesis");
        let manifest_json =
            json::to_json(&manifest).expect("serialize private-profile genesis manifest");
        let manifest: RawGenesisTransaction =
            json::from_str(&manifest_json).expect("deserialize private-profile genesis manifest");
        let normalized = manifest
            .clone()
            .normalize()
            .expect("normalize private-profile genesis");
        let expected_boundary_lengths = expected_normalized
            .transactions
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>();
        let persisted_boundary_lengths = normalized
            .transactions
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>();
        assert_eq!(
            persisted_boundary_lengths, expected_boundary_lengths,
            "persisted private genesis JSON must preserve every routing boundary"
        );
        assert!(
            normalized.transactions.len() <= 16,
            "persisted private genesis must remain within the protocol transaction limit"
        );
        let alias_setup_batches = normalized
            .transactions
            .iter()
            .enumerate()
            .filter(|(_, instructions)| {
                instructions
                    .iter()
                    .any(|instruction| instruction.as_any().downcast_ref::<EnsureAlias>().is_some())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            alias_setup_batches.len(),
            1,
            "private profile must emit exactly one declarative alias bootstrap transaction"
        );
        let (alias_setup_batch_index, alias_setup_batch) = alias_setup_batches[0];
        let private_dataspace = DataSpaceId::new(case.dataspace_id);
        let mut expected_intents = vec![AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
            dataspace: ResolvedDataSpaceV1::new(
                case.alias.parse().expect("private dataspace alias"),
                private_dataspace,
            ),
            owner: client_account_id.clone(),
        })];
        expected_intents.extend(case.domains.iter().map(|domain| {
            AliasIntentV1::Domain(AliasDomainIntentV1 {
                domain: ResolvedDomainV1::new(
                    DomainId::parse_fully_qualified(domain)
                        .expect("static private-profile domain must parse"),
                    private_dataspace,
                ),
                owner: client_account_id.clone(),
            })
        }));
        let resource_count = expected_intents.len();
        assert_eq!(
            alias_setup_batch.len(),
            resource_count.saturating_add(3),
            "private bootstrap must contain one temporary role, the ensures, role cleanup, and one restricted-read grant"
        );
        assert_eq!(
            manifest
                .instructions()
                .filter_map(|instruction| instruction.as_any().downcast_ref::<RegisterBox>())
                .filter(|register| matches!(
                    register,
                    RegisterBox::Account(register)
                        if register.object().id == client_account_id
                ))
                .count(),
            1,
            "the universal AccountId must be registered exactly once across the private-profile genesis"
        );
        let expected_temporary_permissions = expected_intents
            .iter()
            .map(|intent| match intent {
                AliasIntentV1::Dataspace(intent) => Permission::from(CanManageAccountAlias {
                    scope: AccountAliasPermissionScope::Dataspace(intent.dataspace.dataspace_id),
                }),
                AliasIntentV1::Domain(intent) => Permission::from(CanManageAccountAlias {
                    scope: AccountAliasPermissionScope::Domain(
                        intent.domain.canonical_name.clone(),
                    ),
                }),
                AliasIntentV1::AccountAlias(_) => {
                    panic!("private bootstrap unexpectedly contains an account alias")
                }
            })
            .collect::<Vec<_>>();
        let RegisterBox::Role(temporary_role) = alias_setup_batch[0]
            .as_any()
            .downcast_ref::<RegisterBox>()
            .expect("private bootstrap must start with its temporary setup role")
        else {
            panic!("private bootstrap must start with role registration");
        };
        let expected_temporary_role_id: RoleId = format!(
            "private_{}_dataspace_{}_alias_bootstrap",
            case.alias,
            private_dataspace.as_u64()
        )
        .parse()
        .expect("temporary role id");
        assert_eq!(
            temporary_role.object().inner().id,
            expected_temporary_role_id
        );
        assert_eq!(temporary_role.object().grant_to(), &genesis_account_id);
        assert_eq!(
            temporary_role
                .object()
                .inner()
                .permissions()
                .cloned()
                .collect::<Vec<_>>(),
            expected_temporary_permissions,
            "genesis authority's ephemeral role must contain only the exact setup scopes"
        );
        let ensure_start = 1_usize;
        let revoke_start = ensure_start.saturating_add(resource_count);
        let restricted_read_index = revoke_start.saturating_add(1);
        let ensures = alias_setup_batch[ensure_start..revoke_start]
            .iter()
            .map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<EnsureAlias>()
                    .cloned()
                    .expect("declarative setup vector must contain only EnsureAlias")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            ensures
                .iter()
                .map(|ensure| ensure.intent.clone())
                .collect::<Vec<_>>(),
            expected_intents,
            "private setup intents must remain ordered dataspace then domains"
        );
        for ensure in &ensures {
            assert_eq!(ensure.acquisition, AliasLeaseAcquisitionV1::new(1, None));
            assert_eq!(
                ensure.quote_guard,
                AliasQuoteGuardV1 {
                    expected_policy_version: LOCALNET_ALIAS_SETUP_POLICY_VERSION,
                    expected_payment_asset: localnet_fee_asset_definition_id(),
                    max_amount: payment_amount.clone(),
                    valid_until_ms: u64::MAX,
                }
            );
        }
        let UnregisterBox::Role(unregister_temporary_role) = alias_setup_batch[revoke_start]
            .as_any()
            .downcast_ref::<UnregisterBox>()
            .expect("temporary setup role must be unregistered after alias repair")
        else {
            panic!("temporary setup cleanup must unregister a role");
        };
        assert_eq!(
            unregister_temporary_role.object(),
            &expected_temporary_role_id,
            "temporary setup authority must not survive the bootstrap transaction"
        );
        let restricted_read_grant = alias_setup_batch[restricted_read_index]
            .as_any()
            .downcast_ref::<GrantBox>()
            .expect("private alias bootstrap must end with the restricted-read grant");
        let GrantBox::Permission(restricted_read_grant) = restricted_read_grant else {
            panic!("private alias bootstrap must end with an account permission grant");
        };
        let restricted_read_permission = Permission::from(CanReadRestrictedDataspace {
            dataspace: private_dataspace,
        });
        assert_eq!(restricted_read_grant.destination(), &client_account_id);
        assert_eq!(restricted_read_grant.object(), &restricted_read_permission);
        let expected_universal_permissions = [
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
            }),
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
            }),
        ];
        let mut expected_private_permissions = vec![
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(private_dataspace),
            }),
            Permission::from(CanDelegateAccountAliasResolution {
                scope: AccountAliasPermissionScope::Dataspace(private_dataspace),
            }),
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(private_dataspace),
            }),
        ];
        for domain in case.domains {
            let domain = DomainId::parse_fully_qualified(domain)
                .expect("static private-profile domain must parse");
            expected_private_permissions.push(Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(domain.clone()),
            }));
            expected_private_permissions.push(Permission::from(
                CanDelegateAccountAliasResolution {
                    scope: AccountAliasPermissionScope::Domain(domain.clone()),
                },
            ));
            expected_private_permissions.push(Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Domain(domain),
            }));
        }
        assert_eq!(
            ensures
                .iter()
                .flat_map(|ensure| {
                    iroha_core::alias_setup::exact_alias_permission_bundle(&ensure.intent)
                })
                .collect::<Vec<_>>(),
            expected_private_permissions,
            "EnsureAlias must derive the exact manage/delegate/resolve owner bundle"
        );
        let observed_permissions = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<GrantBox>())
            .filter_map(|grant| match grant {
                GrantBox::Permission(grant) if grant.destination() == &client_account_id => {
                    let permission = grant.object();
                    matches!(
                        permission.name(),
                        "CanManageAccountAlias"
                            | "CanResolveAccountAlias"
                            | "CanReadRestrictedDataspace"
                    )
                    .then(|| permission.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let observed_permission_inventory = observed_permissions
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            observed_permission_inventory.len(),
            observed_permissions.len(),
            "direct private-profile permissions must be unique in staged genesis"
        );
        let expected_direct_permissions = vec![
            expected_universal_permissions[0].clone(),
            restricted_read_permission.clone(),
            expected_universal_permissions[1].clone(),
        ];
        assert_eq!(
            observed_permissions, expected_direct_permissions,
            "direct grants must contain the universal ancillary permissions and one private restricted-read capability"
        );
        let observed_restricted_read_grants = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<GrantBox>())
            .filter_map(|grant| match grant {
                GrantBox::Permission(grant)
                    if grant.object().name() == "CanReadRestrictedDataspace" =>
                {
                    Some((grant.destination().clone(), grant.object().clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            observed_restricted_read_grants,
            vec![(
                client_account_id.clone(),
                restricted_read_permission.clone(),
            )],
            "fresh private genesis must materialize one exact direct read grant in the private execution world"
        );
        let expected_reader_role_id =
            crate::genesis::private_dataspace_reader_role_id(case.alias, private_dataspace);
        let observed_reader_roles = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<RegisterBox>())
            .filter_map(|register| match register {
                RegisterBox::Role(register) => Some(register.object()),
                _ => None,
            })
            .filter(|role| role.inner().id == expected_reader_role_id)
            .collect::<Vec<_>>();
        assert_eq!(
            observed_reader_roles.len(),
            1,
            "fresh private genesis must register exactly one deterministic ingress reader role"
        );
        let reader_role = observed_reader_roles[0];
        assert_eq!(reader_role.grant_to(), &client_account_id);
        assert_eq!(
            reader_role
                .inner()
                .permissions()
                .cloned()
                .collect::<Vec<_>>(),
            vec![restricted_read_permission.clone()],
            "the ingress role must hold only the exact restricted-dataspace read capability"
        );
        let universal_permission_batch = normalized
            .transactions
            .get(alias_setup_batch_index.saturating_add(1))
            .expect("alias bootstrap must be followed by its universal ingress transaction");
        assert_eq!(
            universal_permission_batch.len(),
            expected_universal_permissions.len(),
            "the universal ingress transaction must contain one reader role plus the missing deduplicated ancillary grants"
        );
        let RegisterBox::Role(universal_reader_role) = universal_permission_batch[0]
            .as_any()
            .downcast_ref::<RegisterBox>()
            .expect("universal ingress transaction starts with role registration")
        else {
            panic!("universal ingress transaction must start with role registration");
        };
        assert_eq!(
            universal_reader_role.object().inner().id,
            expected_reader_role_id
        );
        assert_eq!(
            universal_reader_role.object().grant_to(),
            &client_account_id
        );
        assert_eq!(
            universal_reader_role
                .object()
                .inner()
                .permissions()
                .cloned()
                .collect::<Vec<_>>(),
            vec![restricted_read_permission]
        );
        assert_eq!(
            universal_permission_batch[1..]
                .iter()
                .map(|instruction| {
                    let grant = instruction
                        .as_any()
                        .downcast_ref::<GrantBox>()
                        .expect("universal ingress transaction must contain only grants after its role");
                    let GrantBox::Permission(grant) = grant else {
                        panic!("universal ingress transaction must contain account grants after its role");
                    };
                    assert_eq!(grant.destination(), &client_account_id);
                    grant.object().clone()
                })
                .collect::<Vec<_>>(),
            expected_universal_permissions[1..].to_vec(),
            "the existing universal manage grant must remain deduplicated while missing grants preserve order"
        );
        let forbidden_domains = case
            .domains
            .iter()
            .map(|domain| {
                DomainId::parse_fully_qualified(domain)
                    .expect("static private-profile domain must parse")
            })
            .collect::<BTreeSet<_>>();
        assert!(
            !manifest.instructions().any(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterBox>()
                    .is_some_and(|register| match register {
                        RegisterBox::Domain(register) => {
                            forbidden_domains.contains(&register.object().id)
                        }
                        _ => false,
                    })
            }),
            "private-profile app domains must be created only by declarative EnsureAlias execution"
        );
    }
}
#[test]
fn private_profiles_stage_and_sign_role_based_restricted_read_bootstrap() {
    for (profile, alias, base_api_port, base_p2p_port) in [
        (SoraProfile::PrivateSbp, "sbp", 39_080, 43_337),
        (SoraProfile::PrivateCbuae, "cbuae", 49_080, 53_337),
    ] {
        let temp = tempfile::tempdir().expect("create private-profile signing directory");
        let opts = LocalnetOptions {
            sora_profile: Some(profile),
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some(format!("private-profile-staged-sign-{alias}")),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port,
            base_p2p_port,
            out_dir: temp.path().join(alias),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
            .unwrap_or_else(|error| panic!("stage and sign {alias} private genesis: {error:#}"));
        let start_script = fs::read_to_string(opts.out_dir.join("start.sh"))
            .expect("read private-profile start script");
        assert!(
            start_script.contains("IROHA_SORA_MODE=\"1\"")
                && start_script.contains(" --sora --config "),
            "an explicit Sora profile must keep requesting the matching daemon profile"
        );
        for peer_index in 0..opts.peers.get() {
            let config: toml::Value = toml::from_str(
                &fs::read_to_string(opts.out_dir.join(format!("peer{peer_index}.toml")))
                    .expect("read private-profile peer config"),
            )
            .expect("parse private-profile peer config");
            let storage = config
                .get("sorafs")
                .and_then(toml::Value::as_table)
                .and_then(|sorafs| sorafs.get("storage"))
                .and_then(toml::Value::as_table)
                .expect("explicit Sora profile must render sorafs.storage");
            assert_eq!(
                storage.get("enabled").and_then(toml::Value::as_bool),
                Some(false),
                "localnet must explicitly preserve disabled embedded SoraFS storage when --sora is applied"
            );
            let expected_sorafs_dir = fs::canonicalize(&opts.out_dir)
                .expect("canonical private-profile output directory")
                .join("state")
                .join(format!("peer{peer_index}"))
                .join("sorafs");
            assert_eq!(
                storage
                    .get("data_dir")
                    .and_then(toml::Value::as_str)
                    .map(Path::new),
                Some(expected_sorafs_dir.as_path()),
                "each Sora profile peer must reserve its own SoraFS data root"
            );
        }
        let signed = fs::read(opts.out_dir.join("genesis.signed.nrt"))
            .expect("read staged and signed private genesis");
        assert!(
            !signed.is_empty(),
            "{alias} staged genesis signer must emit a framed block"
        );
    }
}

// Test body included from command.rs to keep the production source budget bounded.
use std::{
    io::{Read as _, Write as _},
    net::TcpListener,
    thread,
    time::Duration,
};

use clap::CommandFactory as _;
use iroha::crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    ChainId,
    account::{AccountId, address::ChainDiscriminantGuard},
    musubi::{
        MusubiDescriptionV1, MusubiInvitationStateV1, MusubiKeywordV1,
        MusubiMaintainerInvitationV1, MusubiNamespaceBindingV1, MusubiOrderedPackageEntryV1,
        MusubiOrderedPackagePageV1, MusubiOrderedPrefixQueryV1, MusubiOrderedPrefixV1,
        MusubiPackageIdV1, MusubiPackageMemberV1, MusubiPackageScopeV1, MusubiRegistrySnapshotV1,
        MusubiSearchHitV1, MusubiSearchPageV1, MusubiSearchSnapshotV1,
    },
    nexus::DataSpaceId,
};
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

use super::*;
use crate::lockfile::LockedRootV1;

fn command_names(command: clap::Command) -> BTreeSet<String> {
    command
        .get_subcommands()
        .map(|command| command.get_name().to_owned())
        .collect()
}

fn command_aliases(command: &clap::Command) -> BTreeSet<String> {
    let mut aliases = command
        .get_all_aliases()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    for subcommand in command.get_subcommands() {
        aliases.extend(command_aliases(subcommand));
    }
    aliases
}

fn command_long_options(command: &clap::Command) -> BTreeSet<String> {
    let mut options = command
        .get_arguments()
        .filter_map(|argument| argument.get_long().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    for subcommand in command.get_subcommands() {
        options.extend(command_long_options(subcommand));
    }
    options
}

fn create_test_package(temp: &TempDir) -> (PathBuf, PathBuf) {
    let root = temp.path().join("demo");
    let invocation = invoke([
        OsString::from("musubi"),
        OsString::from("new"),
        root.as_os_str().to_owned(),
        OsString::from("--namespace"),
        OsString::from("apps.sora"),
        OsString::from("--export"),
        OsString::from("run"),
    ]);
    assert_eq!(invocation.output.exit_code(), 0);
    let manifest = root.join(MANIFEST_FILE_NAME);
    (root, manifest)
}

fn test_account(seed: u8) -> AccountId {
    let keypair =
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("derive test account");
    AccountId::new(keypair.public_key().clone())
}

fn serve_json_sequence(responses: Vec<Vec<u8>>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback registry listener");
    let address = listener.local_addr().expect("loopback registry address");
    let server = thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().expect("registry query connection");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("registry read timeout");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 2_048];
            let (body_start, content_length) = loop {
                let read = stream.read(&mut buffer).expect("read registry query");
                assert_ne!(read, 0, "registry request ended before its headers");
                request.extend_from_slice(&buffer[..read]);
                let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers =
                    std::str::from_utf8(&request[..header_end]).expect("HTTP request headers");
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().expect("content length"))
                    })
                    .unwrap_or(0);
                break (header_end + 4, content_length);
            };
            while request.len() < body_start + content_length {
                let read = stream.read(&mut buffer).expect("read registry query body");
                assert_ne!(read, 0, "registry request ended before its body");
                request.extend_from_slice(&buffer[..read]);
            }
            write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.len()
                )
                .expect("write registry response headers");
            stream
                .write_all(&response)
                .expect("write registry response body");
        }
    });
    (format!("http://{address}/"), server)
}

fn retention_snapshot() -> MusubiRegistrySnapshotV1 {
    MusubiRegistrySnapshotV1 {
        finalized_height: 17,
        finalized_block_hash: [0x71; 32],
        index_revision: 19,
    }
}

fn retention_page(
    chain: &str,
    archive_ids: &[iroha_data_model::musubi::ArchiveId],
    prunable: &BTreeSet<iroha_data_model::musubi::ArchiveId>,
) -> iroha_data_model::musubi::MusubiArchiveRetentionPageV1 {
    let snapshot = retention_snapshot();
    iroha_data_model::musubi::MusubiArchiveRetentionPageV1 {
        chain_id: chain.parse::<ChainId>().expect("fixture chain id"),
        genesis_hash: [0x81; 32],
        items: archive_ids
            .iter()
            .map(|archive_id| {
                if prunable.contains(archive_id) {
                    iroha_data_model::musubi::MusubiArchiveRetentionDecisionV1 {
                        archive_id: *archive_id,
                        disposition: MusubiArchiveRetentionDispositionV1::PruneUnreferenced,
                        active_releases: 0,
                        yanked_releases: 0,
                        taken_down_releases: 0,
                        storage: Some(iroha_data_model::musubi::MusubiArchiveAvailabilityV1 {
                            archive_id: *archive_id,
                            availability: MusubiStorageAvailabilityV1::Unavailable,
                            healthy_replicas: 0,
                            active_locations: 0,
                            finalized_height: snapshot.finalized_height,
                            finalized_block_hash: snapshot.finalized_block_hash,
                            index_revision: snapshot.index_revision,
                        }),
                    }
                } else {
                    iroha_data_model::musubi::MusubiArchiveRetentionDecisionV1 {
                        archive_id: *archive_id,
                        disposition: MusubiArchiveRetentionDispositionV1::RetainUnknown,
                        active_releases: 0,
                        yanked_releases: 0,
                        taken_down_releases: 0,
                        storage: None,
                    }
                }
            })
            .collect(),
        snapshot,
        finalized_time_ms: 1_700_000_000_000,
    }
}

#[cfg(unix)]
fn create_cache_archive_directory(
    cache: &MusubiCache,
    archive_id: iroha_data_model::musubi::ArchiveId,
) -> PathBuf {
    let path = cache
        .source_path(&archive_id)
        .parent()
        .expect("source path has archive parent")
        .to_path_buf();
    fs::create_dir(&path).expect("create archive cache directory");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
        .expect("secure archive cache directory");
    path
}

#[cfg(unix)]
#[test]
fn cache_prune_dry_run_reports_without_mutating() {
    let temporary = TempDir::new().expect("temporary cache root");
    let cache = MusubiCache::open(temporary.path().join("cache")).expect("private cache");
    let archive_id = iroha_data_model::musubi::ArchiveId::new([0x21; 32]);
    let archive_path = create_cache_archive_directory(&cache, archive_id);
    let prunable = BTreeSet::from([archive_id]);
    let page = retention_page("musubi-prune-test", &[archive_id], &prunable);
    let (torii_url, server) = serve_json_sequence(vec![
        norito::json::to_vec(&page).expect("retention response JSON"),
    ]);
    let registry = RegistryReadClientV1::new(
        torii_url.parse().expect("loopback URL"),
        Duration::from_secs(2),
        753,
    )
    .expect("signer-free registry client");

    let result = prune_cache_targets(&cache, &[archive_id], &registry, true)
        .expect("dry-run retention proof");
    assert_eq!(result.message, "would prune 1 cached archive(s)");
    assert!(archive_path.exists(), "dry-run must not rename or delete");
    assert_eq!(
        result
            .data
            .get("removed")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        result
            .data
            .get("candidates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    server.join().expect("registry server");
}

#[cfg(unix)]
#[test]
fn cache_prune_live_removes_only_explicit_finalized_candidates() {
    let temporary = TempDir::new().expect("temporary cache root");
    let cache = MusubiCache::open(temporary.path().join("cache")).expect("private cache");
    let pruned_id = iroha_data_model::musubi::ArchiveId::new([0x31; 32]);
    let retained_id = iroha_data_model::musubi::ArchiveId::new([0x32; 32]);
    let pruned_path = create_cache_archive_directory(&cache, pruned_id);
    let retained_path = create_cache_archive_directory(&cache, retained_id);
    let archive_ids = [pruned_id, retained_id];
    let prunable = BTreeSet::from([pruned_id]);
    let page = retention_page("musubi-prune-test", &archive_ids, &prunable);
    let (torii_url, server) = serve_json_sequence(vec![
        norito::json::to_vec(&page).expect("retention response JSON"),
    ]);
    let registry = RegistryReadClientV1::new(
        torii_url.parse().expect("loopback URL"),
        Duration::from_secs(2),
        753,
    )
    .expect("signer-free registry client");

    let result = prune_cache_targets(&cache, &archive_ids, &registry, false)
        .expect("live finalized retention prune");
    assert_eq!(result.message, "pruned 1 cached archive(s)");
    assert!(
        !pruned_path.exists(),
        "the exact prunable archive is removed"
    );
    assert!(
        retained_path.exists(),
        "an unknown fail-closed archive remains untouched"
    );
    assert_eq!(
        result
            .data
            .get("removed")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    server.join().expect("registry server");
}

#[cfg(unix)]
#[test]
fn cache_prune_rejects_cross_batch_deployment_drift_before_mutation() {
    let temporary = TempDir::new().expect("temporary cache root");
    let cache = MusubiCache::open(temporary.path().join("cache")).expect("private cache");
    let archive_ids = (1_u8..=101)
        .map(|seed| iroha_data_model::musubi::ArchiveId::new([seed; 32]))
        .collect::<Vec<_>>();
    let candidate = archive_ids[0];
    let archive_path = create_cache_archive_directory(&cache, candidate);
    let prunable = BTreeSet::from([candidate]);
    let first = retention_page(
        "musubi-prune-test",
        &archive_ids[..MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1],
        &prunable,
    );
    let second = retention_page(
        "different-chain",
        &archive_ids[MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1..],
        &BTreeSet::new(),
    );
    let responses = vec![
        norito::json::to_vec(&first).expect("first retention batch JSON"),
        norito::json::to_vec(&second).expect("second retention batch JSON"),
    ];
    let (torii_url, server) = serve_json_sequence(responses);
    let registry = RegistryReadClientV1::new(
        torii_url.parse().expect("loopback URL"),
        Duration::from_secs(2),
        753,
    )
    .expect("signer-free registry client");

    let error = prune_cache_targets(&cache, &archive_ids, &registry, false)
        .expect_err("deployment drift must fail closed");
    assert_eq!(error.code(), ErrorCode::Registry);
    assert!(
        archive_path.exists(),
        "no batch may mutate before full proof"
    );
    server.join().expect("registry server");
}

fn write_test_lock(root: &Path) {
    let lock = LockfileV1::new(
        "musubi-cli-test".parse().expect("chain id"),
        [1; 32],
        MusubiRegistrySnapshotV1 {
            finalized_height: 7,
            finalized_block_hash: [2; 32],
            index_revision: 3,
        },
        vec![LockedRootV1 {
            package: "apps.sora/demo".parse().expect("root package selector"),
            dependencies: Vec::new(),
        }],
        Vec::new(),
    )
    .expect("valid test lock");
    let bytes = lock.render().expect("render lock").into_bytes();
    LockfileV1::parse(std::str::from_utf8(&bytes).expect("UTF-8 lock"))
        .expect("rendered lock parses");
    let lock_path = root.join(LOCK_FILE_NAME);
    fs::write(&lock_path, &bytes).expect("write lock");
    LockfileV1::read(&lock_path).expect("written lock parses");
}

#[test]
fn top_level_and_nested_command_inventory_is_exact() {
    let command = Cli::command();
    assert_eq!(
        command_aliases(&command),
        BTreeSet::new(),
        "Musubi V1 must not retain hidden or visible command aliases"
    );
    assert_eq!(
        command_names(command.clone()),
        BTreeSet::from_iter(
            [
                "add", "alias", "build", "cache", "check", "fetch", "info", "init", "metadata",
                "new", "owner", "package", "publish", "remove", "search", "test", "tree", "unyank",
                "update", "versions", "yank",
            ]
            .map(str::to_owned)
        )
    );
    let owner = command
        .get_subcommands()
        .find(|command| command.get_name() == "owner")
        .expect("owner command");
    assert_eq!(
        command_names(owner.clone()),
        BTreeSet::from_iter(["accept", "invite", "list", "remove", "set-role"].map(str::to_owned))
    );
    let alias = command
        .get_subcommands()
        .find(|command| command.get_name() == "alias")
        .expect("alias command");
    assert_eq!(
        command_names(alias.clone()),
        BTreeSet::from_iter(["history", "info", "register", "resolve"].map(str::to_owned))
    );
    let cache = command
        .get_subcommands()
        .find(|command| command.get_name() == "cache")
        .expect("cache command");
    assert_eq!(
        command_names(cache.clone()),
        BTreeSet::from_iter(["prune", "repair", "verify"].map(str::to_owned))
    );
}

#[test]
fn retired_commands_and_subcommands_are_rejected() {
    for argv in [
        vec!["musubi", "install"],
        vec!["musubi", "pack"],
        vec!["musubi", "alias", "set"],
        vec!["musubi", "cache", "list"],
        vec!["musubi", "cache", "import"],
        vec!["musubi", "cache", "fetch"],
        vec!["musubi", "publish", "--wait"],
    ] {
        let invocation = invoke(argv);
        assert_eq!(invocation.output.exit_code(), ErrorCode::Usage.exit_code());
    }
}

#[test]
fn argv_has_no_secret_or_arbitrary_cache_source_controls() {
    let options = command_long_options(&Cli::command());
    for secret_fragment in [
        "authorization",
        "credential",
        "password",
        "private-key",
        "provider-url",
        "secret",
        "source-plan",
        "token",
    ] {
        assert!(
            options
                .iter()
                .all(|option| !option.contains(secret_fragment)),
            "secret-bearing --*{secret_fragment}* option is reachable"
        );
    }
    for forbidden in ["cache-dir", "cache-path", "cache-root"] {
        assert!(
            !options.contains(forbidden),
            "arbitrary cache control --{forbidden} is reachable"
        );
    }
}

#[test]
fn frozen_combines_locked_and_offline_at_typed_boundary() {
    let cli = Cli::try_parse_from(["musubi", "fetch", "--frozen"]).expect("parse frozen fetch");
    let Command::Fetch(args) = cli.command else {
        panic!("expected fetch");
    };
    assert!(args.mode.effective_locked());
    assert!(args.mode.effective_offline());
}

#[test]
fn graph_commands_accept_an_explicit_signer_free_registry_config() {
    let cli = Cli::try_parse_from(["musubi", "build", "--config", "/platform/client.toml"])
        .expect("parse registry config");
    let Command::Build(args) = cli.command else {
        panic!("expected build");
    };
    assert_eq!(
        args.registry.config.as_deref(),
        Some(Path::new("/platform/client.toml"))
    );
}

#[test]
fn search_uses_the_signer_free_finalized_projection_route() {
    let selector: MusubiPackageSelectorV1 = "apps.sora/proofs".parse().expect("selector");
    let page = MusubiSearchPageV1 {
        query: MusubiSearchQueryV1 {
            query: "proof systems".to_owned(),
            page: MusubiSearchPageRequestV1 {
                limit: 1,
                cursor: None,
            },
        },
        items: vec![MusubiSearchHitV1 {
            package: MusubiPackageIdV1::new(
                DataSpaceId::new(7),
                MusubiPackageScopeV1::Domain("apps".parse().expect("domain")),
                selector.name,
            ),
            claimed_namespace: selector.namespace,
            description: Some(
                MusubiDescriptionV1::new("Proof systems for Kotodama").expect("description"),
            ),
            keywords: vec![MusubiKeywordV1::from_str("proof-systems").expect("keyword")],
            metadata_revision: 3,
        }],
        next_cursor: None,
        snapshot: MusubiSearchSnapshotV1 {
            finalized_height: 9,
            finalized_block_hash: [4; 32],
            projection_revision: 5,
        },
    };
    page.validate().expect("valid search page");
    let response = {
        let _chain_discriminant = ChainDiscriminantGuard::enter(753);
        norito::json::to_vec(&page).expect("search page JSON")
    };
    let (torii_url, server) = serve_json_sequence(vec![response]);
    let temporary = TempDir::new().expect("temporary config directory");
    let config = temporary.path().join("client.toml");
    fs::write(
            &config,
            format!(
                "torii_url = \"{torii_url}\"\ntorii_request_timeout_ms = 2000\n[account]\nchain_discriminant = 753\n"
            ),
        )
        .expect("write public config");

    let invocation = invoke([
        OsString::from("musubi"),
        OsString::from("--format"),
        OsString::from("json"),
        OsString::from("search"),
        OsString::from("proof systems"),
        OsString::from("--limit"),
        OsString::from("1"),
        OsString::from("--config"),
        config.into_os_string(),
    ]);
    assert_eq!(invocation.output.exit_code(), 0);
    let rendered = invocation
        .output
        .render(invocation.format)
        .expect("search JSON output");
    assert!(rendered.stderr().is_empty());
    let document: Value = norito::json::from_str(rendered.stdout()).expect("JSON document");
    assert_eq!(
        document
            .pointer("/data/items")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    server.join().expect("registry server");
}

#[test]
fn owner_roles_require_explicit_nonempty_maintainer_permissions() {
    assert_eq!(
        owner_role(RoleArg::Owner, MaintainerPermissionArgs::default()).expect("plain owner role"),
        MusubiPackageRoleV1::Owner
    );
    assert!(
        owner_role(
            RoleArg::Owner,
            MaintainerPermissionArgs {
                publish: true,
                ..MaintainerPermissionArgs::default()
            },
        )
        .is_err()
    );
    assert!(
        owner_role(RoleArg::Maintainer, MaintainerPermissionArgs::default()).is_err(),
        "a maintainer role must not silently receive implicit permissions"
    );
    assert_eq!(
        owner_role(
            RoleArg::Maintainer,
            MaintainerPermissionArgs {
                publish: true,
                metadata: true,
                ..MaintainerPermissionArgs::default()
            },
        )
        .expect("explicit maintainer permissions"),
        MusubiPackageRoleV1::Maintainer(MusubiMaintainerPermissionsV1 {
            publish: true,
            yank: false,
            metadata: true,
            archive_locations: false,
        })
    );
}

#[test]
fn workspace_test_failures_keep_their_stable_boundary_codes() {
    assert_eq!(
        test_runner_diagnostic(WorkspaceTestErrorV1::Workspace("invalid".to_owned())).code(),
        ErrorCode::WorkspaceInvalid
    );
    assert_eq!(
        test_runner_diagnostic(WorkspaceTestErrorV1::Lock("invalid".to_owned())).code(),
        ErrorCode::LockfileInvalid
    );
    assert_eq!(
        test_runner_diagnostic(WorkspaceTestErrorV1::Cache("invalid".to_owned())).code(),
        ErrorCode::CacheCorrupt
    );
    assert_eq!(
        test_runner_diagnostic(WorkspaceTestErrorV1::Runner("failed".to_owned())).code(),
        ErrorCode::Compiler
    );
}

#[test]
fn invitation_ids_and_compare_and_set_revisions_are_canonical() {
    let invitation = format!("{}1", "0".repeat(63));
    assert_eq!(
        hex::encode(
            parse_invite_id(&invitation)
                .expect("canonical invitation")
                .as_bytes()
        ),
        invitation
    );
    for invalid in [
        "0".repeat(64),
        "A".repeat(64),
        "1".repeat(63),
        format!("{}g", "0".repeat(63)),
    ] {
        assert!(parse_invite_id(&invalid).is_err(), "accepted {invalid}");
    }
    require_nonzero_revision(1, "revision").expect("non-zero revision");
    assert!(require_nonzero_revision(0, "revision").is_err());
}

#[test]
fn owner_invite_parser_requires_explicit_identity_expiry_and_permissions() {
    let invitation = format!("{}1", "0".repeat(63));
    let parsed = Cli::try_parse_from([
        "musubi",
        "owner",
        "invite",
        "apps.sora/demo",
        "ed0120deadbeef",
        "--role",
        "maintainer",
        "--invitation",
        &invitation,
        "--expires-at-height",
        "42",
        "--publish",
        "--expected-revision",
        "7",
    ])
    .expect("complete invite command");
    let Command::Owner(OwnerArgs {
        command:
            OwnerCommand::Invite {
                expires_at_height,
                expected_revision,
                permissions,
                ..
            },
    }) = parsed.command
    else {
        panic!("owner invite command expected");
    };
    assert_eq!(expires_at_height, 42);
    assert_eq!(expected_revision, 7);
    assert!(permissions.publish);

    assert!(
        Cli::try_parse_from([
            "musubi",
            "owner",
            "invite",
            "apps.sora/demo",
            "ed0120deadbeef",
            "--role",
            "maintainer",
            "--expires-at-height",
            "42",
            "--publish",
            "--expected-revision",
            "7",
        ])
        .is_err(),
        "invitation ids must never be synthesized by the CLI"
    );
}

#[test]
fn owner_remove_selects_exactly_one_member_or_pending_invitation() {
    let invitation = format!("{}1", "0".repeat(63));
    let accepted = Cli::try_parse_from([
        "musubi",
        "owner",
        "remove",
        "apps.sora/demo",
        "ed0120deadbeef",
        "--expected-revision",
        "7",
    ])
    .expect("accepted-member removal");
    let Command::Owner(OwnerArgs {
        command:
            OwnerCommand::Remove {
                account,
                invitation: pending,
                ..
            },
    }) = accepted.command
    else {
        panic!("owner remove command expected");
    };
    assert_eq!(account.as_deref(), Some("ed0120deadbeef"));
    assert!(pending.is_none());

    let pending = Cli::try_parse_from([
        "musubi",
        "owner",
        "remove",
        "apps.sora/demo",
        "--invitation",
        &invitation,
        "--expected-revision",
        "7",
    ])
    .expect("pending-invitation revocation");
    let Command::Owner(OwnerArgs {
        command:
            OwnerCommand::Remove {
                account,
                invitation: pending,
                ..
            },
    }) = pending.command
    else {
        panic!("owner remove command expected");
    };
    assert!(account.is_none());
    assert_eq!(pending.as_deref(), Some(invitation.as_str()));

    assert!(
        Cli::try_parse_from([
            "musubi",
            "owner",
            "remove",
            "apps.sora/demo",
            "--expected-revision",
            "7",
        ])
        .is_err(),
        "a removal target is required"
    );
    assert!(
        Cli::try_parse_from([
            "musubi",
            "owner",
            "remove",
            "apps.sora/demo",
            "ed0120deadbeef",
            "--invitation",
            &invitation,
            "--expected-revision",
            "7",
        ])
        .is_err(),
        "accepted and pending targets are mutually exclusive"
    );
}

#[test]
fn owner_list_is_signer_free_and_includes_pending_invitations() {
    let selector: MusubiPackageSelectorV1 = "apps.sora/demo".parse().expect("selector");
    let binding = MusubiNamespaceBindingV1 {
        namespace: selector.namespace.clone(),
        home_dataspace: DataSpaceId::new(7),
        scope: MusubiPackageScopeV1::Domain("apps".parse().expect("domain")),
        generation: 1,
    };
    let package = MusubiPackageIdV1::new(
        binding.home_dataspace,
        binding.scope.clone(),
        selector.name.clone(),
    );
    let snapshot = MusubiRegistrySnapshotV1 {
        finalized_height: 10,
        finalized_block_hash: [9; 32],
        index_revision: 11,
    };
    let directory = MusubiOrderedPackagePageV1 {
        query: MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new("apps.sora/").expect("namespace prefix"),
            page: MusubiPageRequestV1 {
                limit: 1,
                cursor: None,
            },
        },
        chain_id: ChainId::from("musubi-owner-list-test"),
        genesis_hash: [8; 32],
        namespace_binding: binding,
        items: vec![MusubiOrderedPackageEntryV1 {
            selector: selector.clone(),
            package: package.clone(),
            latest_selectable: Some("1.0.0".parse().expect("version")),
            metadata_revision: 2,
            index_revision: snapshot.index_revision,
        }],
        next_cursor: None,
        snapshot: snapshot.clone(),
    };
    let owner = test_account(31);
    let invited = test_account(32);
    let mut entries = vec![
        MusubiMaintainerDirectoryEntryV1::Accepted(MusubiPackageMemberV1 {
            package: package.clone(),
            account: owner.clone(),
            role: MusubiPackageRoleV1::Owner,
            accepted_at_height: 3,
            governance_revision: 4,
        }),
        MusubiMaintainerDirectoryEntryV1::PendingInvitation(MusubiMaintainerInvitationV1 {
            invite_id: MusubiInviteIdV1::new([5; 32]),
            package: package.clone(),
            invited_by: owner,
            invited_account: invited,
            role: MusubiPackageRoleV1::Maintainer(MusubiMaintainerPermissionsV1 {
                publish: true,
                yank: false,
                metadata: false,
                archive_locations: false,
            }),
            expected_governance_revision: 4,
            expires_at_height: 50,
            state: MusubiInvitationStateV1::Pending,
        }),
    ];
    entries.sort_by_key(MusubiMaintainerDirectoryEntryV1::key);
    let maintainers = iroha_data_model::musubi::MusubiMaintainerPageV1 {
        query: MusubiPackagePageQueryV1 {
            package,
            page: MusubiPageRequestV1 {
                limit: 50,
                cursor: None,
            },
        },
        items: entries,
        next_cursor: None,
        snapshot,
    };
    let responses = {
        let _chain_discriminant = ChainDiscriminantGuard::enter(753);
        vec![
            norito::json::to_vec(&directory).expect("directory JSON"),
            norito::json::to_vec(&maintainers).expect("maintainer JSON"),
        ]
    };
    let (torii_url, server) = serve_json_sequence(responses);
    let temporary = TempDir::new().expect("temporary config directory");
    let config = temporary.path().join("client.toml");
    fs::write(
            &config,
            format!(
                "torii_url = \"{torii_url}\"\ntorii_request_timeout_ms = 2000\n[account]\nchain_discriminant = 753\n"
            ),
        )
        .expect("write public config");

    let invocation = invoke([
        OsString::from("musubi"),
        OsString::from("--format"),
        OsString::from("json"),
        OsString::from("owner"),
        OsString::from("list"),
        OsString::from(selector.to_string()),
        OsString::from("--config"),
        config.into_os_string(),
    ]);
    assert_eq!(invocation.output.exit_code(), 0);
    let rendered = invocation
        .output
        .render(invocation.format)
        .expect("owner-list JSON output");
    assert!(rendered.stderr().is_empty());
    let document: Value = norito::json::from_str(rendered.stdout()).expect("JSON document");
    assert_eq!(
        document.pointer("/data/accepted").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        document.pointer("/data/pending").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        document
            .pointer("/data/entries")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    server.join().expect("registry server");
}

#[test]
fn targeted_update_parser_requires_structural_package_and_exact_version() {
    let target = "std/math@1.2.3"
        .parse::<UpdateTarget>()
        .expect("valid targeted update");
    assert_eq!(target.package.to_string(), "std/math");
    assert_eq!(
        target.locked_version.as_ref().map(ToString::to_string),
        Some("1.2.3".to_owned())
    );
    assert!("math@1.2.3".parse::<UpdateTarget>().is_err());
    assert!("std/math@1.2.3+local".parse::<UpdateTarget>().is_err());
}

#[test]
fn json_parse_failure_is_one_stdout_document() {
    let invocation = invoke(["musubi", "--format", "json", "install"]);
    let rendered = invocation
        .output
        .render(invocation.format)
        .expect("render JSON failure");
    assert_eq!(rendered.exit_code(), ErrorCode::Usage.exit_code());
    assert!(rendered.stderr().is_empty());
    assert_eq!(rendered.stdout().matches('\n').count(), 1);
    let value: Value = norito::json::from_str(rendered.stdout()).expect("JSON document");
    assert_eq!(value.get("ok").and_then(Value::as_bool), Some(false));
}

#[test]
fn local_new_add_metadata_tree_remove_roundtrip() {
    let temp = TempDir::new().expect("temporary directory");
    let (_root, manifest_path) = create_test_package(&temp);
    let manifest = fs::read_to_string(&manifest_path).expect("new manifest");
    parse_manifest(&manifest).expect("strict generated manifest");

    let add = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("add"),
        OsString::from("std/math"),
        OsString::from("--version"),
        OsString::from("^1.0.0"),
        OsString::from("--rename"),
        OsString::from("math"),
    ]);
    assert_eq!(add.output.exit_code(), 0);

    let add_local_dev = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("add"),
        OsString::from("local-test-support"),
        OsString::from("--path"),
        OsString::from("."),
        OsString::from("--dev"),
    ]);
    assert_eq!(add_local_dev.output.exit_code(), 0);

    let metadata = invoke([
        OsString::from("musubi"),
        OsString::from("--format"),
        OsString::from("json"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("metadata"),
    ]);
    assert_eq!(
        metadata.output.exit_code(),
        0,
        "{}",
        metadata
            .output
            .render(OutputFormat::Human)
            .expect("metadata diagnostic")
            .stderr()
    );
    let rendered = metadata
        .output
        .render(metadata.format)
        .expect("metadata JSON");
    let document: Value = norito::json::from_str(rendered.stdout()).expect("metadata document");
    assert_eq!(
        document
            .pointer("/data/packages/0/package")
            .and_then(Value::as_str),
        Some("apps.sora/demo")
    );

    let tree = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("tree"),
    ]);
    assert_eq!(tree.output.exit_code(), 0);
    let tree = tree.output.render(tree.format).expect("tree output");
    assert!(tree.stdout().contains("math -> std/math ^1.0.0"));
    assert!(tree.stdout().contains("[dev] local-test-support ->"));

    let remove = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("remove"),
        OsString::from("math"),
    ]);
    assert_eq!(remove.output.exit_code(), 0);
    let manifest = fs::read_to_string(&manifest_path).expect("edited manifest");
    assert!(
        !parse_manifest(&manifest)
            .expect("manifest after remove")
            .dependencies
            .contains_key("math")
    );
}

#[test]
fn metadata_and_tree_include_only_the_validated_exact_lock_graph() {
    let temp = TempDir::new().expect("temporary directory");
    let (root, manifest_path) = create_test_package(&temp);
    write_test_lock(&root);

    let metadata = invoke([
        OsString::from("musubi"),
        OsString::from("--format"),
        OsString::from("json"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("metadata"),
    ]);
    assert_eq!(metadata.output.exit_code(), 0);
    let rendered = metadata
        .output
        .render(metadata.format)
        .expect("metadata JSON");
    let document: Value = norito::json::from_str(rendered.stdout()).expect("JSON document");
    assert_eq!(
        document
            .pointer("/data/lock/schema")
            .and_then(Value::as_str),
        Some("musubi-lock")
    );
    assert_eq!(
        document
            .pointer("/data/lock/finalized_height")
            .and_then(Value::as_u64),
        Some(7)
    );
    for forbidden in ["cache_path", "provider_url", "credential", "bearer"] {
        assert!(!rendered.stdout().contains(forbidden));
    }

    let tree = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("tree"),
    ]);
    assert_eq!(tree.output.exit_code(), 0);
    let rendered = tree.output.render(tree.format).expect("tree output");
    assert!(
        rendered
            .stdout()
            .contains("apps.sora/demo exact lock graph")
    );
}

#[test]
fn locked_fetch_rejects_legacy_lock_without_rewriting_it() {
    let temp = TempDir::new().expect("temporary directory");
    let (root, manifest_path) = create_test_package(&temp);
    let lock_path = root.join(LOCK_FILE_NAME);
    let legacy = b"schema = \"musubi-lock\"\nversion = 2\n";
    fs::write(&lock_path, legacy).expect("write legacy lock");

    let fetch = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("fetch"),
        OsString::from("--locked"),
    ]);
    assert_eq!(
        fetch.output.exit_code(),
        ErrorCode::LockfileLegacy.exit_code()
    );
    assert_eq!(fs::read(lock_path).expect("read lock"), legacy);
    let rendered = fetch
        .output
        .render(OutputFormat::Human)
        .expect("legacy diagnostic");
    assert!(
        rendered.stderr().contains("MUSUBI_E_LOCKFILE_LEGACY"),
        "{}",
        rendered.stderr()
    );
    assert!(rendered.stderr().contains("never rewrites retired formats"));
}

#[test]
fn consumer_lock_is_not_used_as_package_or_cache_authentication() {
    let temp = TempDir::new().expect("temporary directory");
    let (root, manifest_path) = create_test_package(&temp);
    write_test_lock(&root);

    let package = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("package"),
        OsString::from("--list"),
        OsString::from("--offline"),
    ]);
    assert_eq!(
        package.output.exit_code(),
        ErrorCode::OfflineMiss.exit_code()
    );
    let rendered = package
        .output
        .render(OutputFormat::Human)
        .expect("package diagnostic");
    assert!(rendered.stderr().contains("cached Musubi resolver index"));
    assert!(rendered.stderr().contains("resolver cache"));

    let verify = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("cache"),
        OsString::from("verify"),
    ]);
    assert_eq!(verify.output.exit_code(), ErrorCode::Registry.exit_code());
    let rendered = verify
        .output
        .render(OutputFormat::Human)
        .expect("cache diagnostic");
    assert!(
        rendered
            .stderr()
            .contains("MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID")
    );
    assert!(!rendered.stderr().contains("healthy"));
}

#[cfg(unix)]
#[test]
fn empty_cache_maintenance_is_signer_and_network_free() {
    let temp = TempDir::new().expect("temporary directory");
    let cache = MusubiCache::open(temp.path().join("cache")).expect("private cache");
    let targets = BTreeSet::new();

    let verified = verify_cache_targets(&cache, &targets, None).expect("empty verification");
    assert_eq!(verified.message, "verified 0 cached archive(s)");
    let repaired = repair_cache_targets(&cache, &targets, None, true).expect("empty repair");
    assert_eq!(repaired.message, "repaired 0 cached archive(s)");

    let graph = ResolvedWorkspaceGraphV1 {
        lock: LockfileV1::new(
            "empty-cache-test".parse().expect("chain id"),
            [1; 32],
            MusubiRegistrySnapshotV1 {
                finalized_height: 1,
                finalized_block_hash: [2; 32],
                index_revision: 1,
            },
            vec![LockedRootV1 {
                package: "apps.sora/demo".parse().expect("root package"),
                dependencies: Vec::new(),
            }],
            Vec::new(),
        )
        .expect("empty external graph"),
        registry: None,
        cached_source: None,
        account_chain_discriminant: 753,
    };
    assert!(
        ensure_graph_archives(&cache, &graph, GraphModeArgs::default(), None)
            .expect("empty graph fetch")
            .is_empty()
    );
}

#[test]
fn package_output_writer_creates_confined_target_directory() {
    let temp = TempDir::new().expect("temporary directory");
    let (root, _manifest_path) = create_test_package(&temp);
    let workspace = load_workspace(&root).expect("workspace");
    let writer = package_output_writer(&workspace).expect("package writer");
    writer
        .replace(Path::new("target/package/demo.car"), b"canonical-car")
        .expect("confined package write");
    assert_eq!(
        fs::read(root.join("target/package/demo.car")).expect("package bytes"),
        b"canonical-car"
    );
}

#[test]
fn offline_fetch_with_a_valid_lock_requires_authenticated_cache_inputs() {
    let temp = TempDir::new().expect("temporary directory");
    let (root, manifest_path) = create_test_package(&temp);
    write_test_lock(&root);

    let fetch = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("fetch"),
        OsString::from("--offline"),
    ]);
    assert_eq!(fetch.output.exit_code(), ErrorCode::OfflineMiss.exit_code());
    let rendered = fetch
        .output
        .render(OutputFormat::Human)
        .expect("offline diagnostic");
    assert!(rendered.stderr().contains("cached Musubi resolver index"));
    assert!(rendered.stderr().contains("resolver cache"));
}

#[test]
fn compiler_command_requires_an_authenticated_v1_graph_offline() {
    let temp = TempDir::new().expect("temporary directory");
    let (root, manifest_path) = create_test_package(&temp);
    write_test_lock(&root);
    let invocation = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest_path.as_os_str().to_owned(),
        OsString::from("check"),
        OsString::from("--offline"),
    ]);
    assert_eq!(
        invocation.output.exit_code(),
        ErrorCode::OfflineMiss.exit_code()
    );
    let rendered = invocation
        .output
        .render(OutputFormat::Human)
        .expect("authenticated graph diagnostic");
    assert!(rendered.stderr().contains("resolver cache"));
    assert!(!rendered.stderr().contains("install"));
}

#[test]
fn publish_resume_accepts_only_a_canonical_detached_operation() {
    let operation = "0101010101010101010101010101010101010101010101010101010101010101";
    let parsed = Cli::try_parse_from(["musubi", "publish", "--resume", operation])
        .expect("canonical resume command");
    let Command::Publish(arguments) = parsed.command else {
        panic!("publish command expected");
    };
    assert_eq!(arguments.resume.expect("operation").to_string(), operation);
    assert!(!arguments.detach);

    assert!(
        Cli::try_parse_from(["musubi", "publish", "--resume", operation, "--detach"]).is_err(),
        "detach and resume are mutually exclusive"
    );
    assert!(
        Cli::try_parse_from([
            "musubi",
            "publish",
            "--resume",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ])
        .is_err(),
        "operation id uses canonical lowercase hex"
    );
}

#[test]
fn fresh_publish_selects_one_explicit_workspace_package() {
    let parsed = Cli::try_parse_from([
        "musubi",
        "publish",
        "-p",
        "apps.sora/demo",
        "--config",
        "/platform/client.toml",
        "--detach",
    ])
    .expect("fresh publication command");
    let Command::Publish(arguments) = parsed.command else {
        panic!("publish command expected");
    };
    assert_eq!(
        arguments.selection.packages,
        vec![
            "apps.sora/demo"
                .parse::<MusubiPackageSelectorV1>()
                .expect("package selector")
        ]
    );
    assert_eq!(
        arguments.network.config.as_deref(),
        Some(Path::new("/platform/client.toml"))
    );
    assert!(arguments.detach);
    assert!(arguments.resume.is_none());
}

#[test]
fn prepared_car_validation_stream_is_bounded_and_exact() {
    let bytes = b"canonical-publication-car";
    let size = u64::try_from(bytes.len()).expect("fixture length fits u64");
    let digest = MusubiContentDigestV1::new(*blake3::hash(bytes).as_bytes());
    validate_prepared_car_stream(&mut bytes.as_slice(), size, digest)
        .expect("exact stream validates");

    let length_error = validate_prepared_car_stream(
        &mut bytes.as_slice(),
        size.checked_sub(1).expect("non-empty fixture"),
        digest,
    )
    .expect_err("oversized stream must fail before accepting trailing bytes");
    assert_eq!(length_error.code(), "PACKAGE_VALIDATION_LENGTH_INVALID");

    let digest_error = validate_prepared_car_stream(
        &mut bytes.as_slice(),
        size,
        MusubiContentDigestV1::new([0x55; 32]),
    )
    .expect_err("substituted digest must fail");
    assert_eq!(digest_error.code(), "PACKAGE_VALIDATION_CAR_MISMATCH");
}

#[test]
fn publication_compiler_evidence_and_nonce_are_domain_bound() {
    let interface = MusubiContentDigestV1::new([1; 32]);
    let release = iroha_data_model::musubi::MusubiReleaseDigestV1::new([2; 32]);
    let lock = iroha_data_model::musubi::MusubiVerificationLockDigestV1::new([3; 32]);
    let digest = publication_compiler_output_digest(interface, release, lock);
    assert!(!digest.is_zero());
    assert_ne!(
        digest,
        publication_compiler_output_digest(MusubiContentDigestV1::new([4; 32]), release, lock,)
    );

    let first = unpredictable_publication_nonce();
    let second = unpredictable_publication_nonce();
    assert!(first.iter().any(|byte| *byte != 0));
    assert!(second.iter().any(|byte| *byte != 0));
    assert_ne!(first, second);
}

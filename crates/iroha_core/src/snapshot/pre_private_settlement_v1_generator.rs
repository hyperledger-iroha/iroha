// Include this file verbatim from `crates/iroha_core/src/snapshot/support_policy_tests.rs` in a
// detached checkout of revision 1bdec3b88c348a84776241839fb0e8ad71738b3e, then run the command
// recorded in `pre_private_settlement_v1.provenance`. Compiling this `.rs` source and hashing
// the same path at runtime binds the recorded generator digest to the code that emitted the
// artifact. The test deliberately uses only deterministic keys, identities, timestamps, state,
// and canonical serializers from that exact revision.
const PRE_PRIVATE_SETTLEMENT_SOURCE_REVISION_V1: &str = "1bdec3b88c348a84776241839fb0e8ad71738b3e";
const PRE_PRIVATE_SETTLEMENT_WORLD_SCHEMA_ORDER_SHA256_V1: &str =
    "09c478f61330137e92bdfa98d5d532b9f234b3dca8f0f00b6bdc5bf4fac01fe2";
const PRE_PRIVATE_SETTLEMENT_INJECTION_PATH_V1: &str =
    "crates/iroha_core/src/snapshot/support_policy_tests.rs";
const PRE_PRIVATE_SETTLEMENT_INJECTION_SUFFIX_V1: &[u8] = concat!(
    "\n// Local-only exact-revision fixture emitter. Runtime provenance binds the external include bytes.\n",
    "const PRE_PRIVATE_SETTLEMENT_GENERATOR_SOURCE_V1: &[u8] = include_bytes!(env!(\"IROHA_PRE_PRIVATE_SETTLEMENT_GENERATOR_PATH\"));\n",
    "include!(env!(\"IROHA_PRE_PRIVATE_SETTLEMENT_GENERATOR_PATH\"));\n",
)
.as_bytes();
const PRE_PRIVATE_SETTLEMENT_COMPILED_GENERATOR_PATH_V1: Option<&str> =
    option_env!("IROHA_PRE_PRIVATE_SETTLEMENT_GENERATOR_PATH");
const PRE_PRIVATE_SETTLEMENT_COMPILED_GENERATOR_SHA256_V1: Option<&str> =
    option_env!("IROHA_PRE_PRIVATE_SETTLEMENT_GENERATOR_SHA256");

fn predecessor_fixture_command_stdout(
    program: &str,
    arguments: &[&str],
) -> std::io::Result<String> {
    let output = std::process::Command::new(program)
        .args(arguments)
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "{program} {} exited with {}: {}",
            arguments.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim_end().to_owned())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn validate_predecessor_fixture_source_revision(actual: &str) -> Result<(), String> {
    if actual == PRE_PRIVATE_SETTLEMENT_SOURCE_REVISION_V1 {
        Ok(())
    } else {
        Err(format!(
            "fixture generator must run at exact predecessor revision {}; got {actual}",
            PRE_PRIVATE_SETTLEMENT_SOURCE_REVISION_V1
        ))
    }
}

fn validate_predecessor_fixture_generator_digest(
    generator_bytes: &[u8],
    supplied_sha256: &str,
) -> Result<String, String> {
    let actual_sha256 = hex::encode(Sha256::digest(generator_bytes));
    if supplied_sha256 == actual_sha256 {
        Ok(actual_sha256)
    } else {
        Err(format!(
            "supplied generator SHA-256 {supplied_sha256} does not match actual source {actual_sha256}"
        ))
    }
}

fn validate_predecessor_fixture_world_order(
    field_count: usize,
    order_sha256: &str,
) -> Result<(), String> {
    if field_count != 180 {
        return Err(format!(
            "predecessor World must have exactly 180 fields; got {field_count}"
        ));
    }
    if order_sha256 != PRE_PRIVATE_SETTLEMENT_WORLD_SCHEMA_ORDER_SHA256_V1 {
        return Err(format!(
            "predecessor World order SHA-256 must be {}; got {order_sha256}",
            PRE_PRIVATE_SETTLEMENT_WORLD_SCHEMA_ORDER_SHA256_V1
        ));
    }
    Ok(())
}

fn validate_predecessor_fixture_checkout_patch() -> Result<String, String> {
    let object = format!("HEAD:{}", PRE_PRIVATE_SETTLEMENT_INJECTION_PATH_V1);
    let baseline = std::process::Command::new("git")
        .args(["show", object.as_str()])
        .output()
        .map_err(|error| format!("read exact predecessor source from Git: {error}"))?;
    if !baseline.status.success() {
        return Err(format!(
            "git show {object} exited with {}: {}",
            baseline.status,
            String::from_utf8_lossy(&baseline.stderr).trim()
        ));
    }
    let mut expected = baseline.stdout;
    expected.extend_from_slice(PRE_PRIVATE_SETTLEMENT_INJECTION_SUFFIX_V1);
    let repository_root =
        predecessor_fixture_command_stdout("git", &["rev-parse", "--show-toplevel"])
            .map_err(|error| format!("resolve predecessor repository root: {error}"))?;
    let injection_path =
        std::path::Path::new(&repository_root).join(PRE_PRIVATE_SETTLEMENT_INJECTION_PATH_V1);
    let actual = std::fs::read(&injection_path)
        .map_err(|error| format!("read injected predecessor test source: {error}"))?;
    if actual != expected {
        return Err(
            "exact predecessor checkout differs from HEAD beyond the reviewed generator include"
                .to_owned(),
        );
    }
    let status = predecessor_fixture_command_stdout(
        "git",
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )
    .map_err(|error| format!("read predecessor checkout status: {error}"))?;
    let expected_status = format!(" M {PRE_PRIVATE_SETTLEMENT_INJECTION_PATH_V1}");
    if status != expected_status {
        return Err(format!(
            "predecessor fixture checkout must contain only the reviewed generator include; got {status:?}"
        ));
    }
    Ok(hex::encode(Sha256::digest(&actual)))
}

struct PrePrivateSettlementFixtureStaging {
    path: Option<std::path::PathBuf>,
}

impl PrePrivateSettlementFixtureStaging {
    fn create(fixture_parent: &std::path::Path) -> std::io::Result<Self> {
        let final_dir = fixture_parent.join("snapshot");
        if final_dir.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "refuse to replace an existing predecessor fixture directory",
            ));
        }
        let path = fixture_parent.join(format!(
            ".snapshot-pre-private-settlement-v1-{}",
            std::process::id()
        ));
        std::fs::create_dir(&path)?;
        Ok(Self { path: Some(path) })
    }

    fn path(&self) -> &std::path::Path {
        self.path
            .as_deref()
            .expect("unpublished predecessor fixture has a staging path")
    }

    fn publish(mut self, fixture_parent: &std::path::Path) -> std::io::Result<()> {
        let final_dir = fixture_parent.join("snapshot");
        if final_dir.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "refuse to replace an existing predecessor fixture directory",
            ));
        }
        let path = self
            .path
            .as_ref()
            .expect("unpublished predecessor fixture has a staging path");
        std::fs::rename(path, &final_dir)?;
        self.path = None;
        Ok(())
    }
}

impl Drop for PrePrivateSettlementFixtureStaging {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

#[test]
fn pre_private_settlement_fixture_staging_is_fail_closed() {
    let fixture_parent = tempfile::tempdir().expect("create fixture-staging test directory");

    let failed = PrePrivateSettlementFixtureStaging::create(fixture_parent.path())
        .expect("create failure-path staging directory");
    let failed_path = failed.path().to_owned();
    std::fs::write(failed.path().join("partial"), b"partial")
        .expect("write deliberately partial staging artifact");
    assert!(
        !fixture_parent.path().join("snapshot").exists(),
        "a partial hidden staging directory must never look published"
    );
    drop(failed);
    assert!(
        !failed_path.exists(),
        "ordinary generator failure must remove its partial staging directory"
    );

    let staged = PrePrivateSettlementFixtureStaging::create(fixture_parent.path())
        .expect("create collision-path staging directory");
    let staged_path = staged.path().to_owned();
    let final_dir = fixture_parent.path().join("snapshot");
    std::fs::create_dir(&final_dir).expect("create pre-existing frozen fixture directory");
    std::fs::write(final_dir.join("sentinel"), b"frozen").expect("write frozen fixture sentinel");
    let error = staged
        .publish(fixture_parent.path())
        .expect_err("publication must not replace an existing frozen fixture directory");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        std::fs::read(final_dir.join("sentinel")).expect("read frozen fixture sentinel"),
        b"frozen"
    );
    assert!(
        !staged_path.exists(),
        "a failed collision publication must clean its staging directory"
    );
}

#[test]
fn pre_private_settlement_fixture_provenance_is_fail_closed() {
    validate_predecessor_fixture_source_revision(PRE_PRIVATE_SETTLEMENT_SOURCE_REVISION_V1)
        .expect("the frozen predecessor revision must validate");
    assert!(
        validate_predecessor_fixture_source_revision("c495d41e00000000000000000000000000000000")
            .is_err(),
        "a caller-supplied successor revision must fail closed"
    );
    let checkout_revision = predecessor_fixture_command_stdout("git", &["rev-parse", "HEAD"])
        .expect("read the checkout revision used by the test");
    let checkout_validation = validate_predecessor_fixture_source_revision(&checkout_revision);
    if checkout_revision == PRE_PRIVATE_SETTLEMENT_SOURCE_REVISION_V1 {
        checkout_validation.expect("the exact predecessor checkout must be admitted");
    } else {
        assert!(
            checkout_validation.is_err(),
            "a current or unrelated checkout must not emit predecessor evidence"
        );
    }

    let generator_bytes = b"reviewed generator source";
    let generator_sha256 = hex::encode(Sha256::digest(generator_bytes));
    validate_predecessor_fixture_generator_digest(generator_bytes, &generator_sha256)
        .expect("the actual generator digest must validate");
    assert!(
        validate_predecessor_fixture_generator_digest(generator_bytes, &["00"; 32].concat())
            .is_err(),
        "a caller-supplied generator digest must not be trusted"
    );

    validate_predecessor_fixture_world_order(
        180,
        PRE_PRIVATE_SETTLEMENT_WORLD_SCHEMA_ORDER_SHA256_V1,
    )
    .expect("the frozen predecessor World order must validate");
    assert!(
        validate_predecessor_fixture_world_order(
            179,
            PRE_PRIVATE_SETTLEMENT_WORLD_SCHEMA_ORDER_SHA256_V1,
        )
        .is_err(),
        "a missing predecessor World field must fail closed"
    );
    assert!(
        validate_predecessor_fixture_world_order(180, &["00"; 32].concat()).is_err(),
        "a reordered predecessor World must fail closed"
    );
}

#[test]
#[ignore]
fn emit_pre_private_settlement_fixture_v1() {
    let fixture_parent = std::path::PathBuf::from(
        std::env::var("IROHA_PRE_PRIVATE_SETTLEMENT_FIXTURE_PARENT")
            .expect("set IROHA_PRE_PRIVATE_SETTLEMENT_FIXTURE_PARENT"),
    );
    let source_revision = predecessor_fixture_command_stdout("git", &["rev-parse", "HEAD"])
        .expect("read the actual generator checkout revision");
    validate_predecessor_fixture_source_revision(&source_revision)
        .expect("fixture generation must run at the exact frozen predecessor revision");
    let checkout_injection_sha256 = validate_predecessor_fixture_checkout_patch()
        .expect("fixture checkout must equal the exact revision plus one reviewed include");
    let supplied_source_revision = std::env::var("IROHA_PRE_PRIVATE_SETTLEMENT_SOURCE_REVISION")
        .expect("set IROHA_PRE_PRIVATE_SETTLEMENT_SOURCE_REVISION");
    assert_eq!(supplied_source_revision, source_revision);
    let generator_path = std::path::PathBuf::from(
        std::env::var("IROHA_PRE_PRIVATE_SETTLEMENT_GENERATOR_PATH")
            .expect("set IROHA_PRE_PRIVATE_SETTLEMENT_GENERATOR_PATH"),
    );
    let compiled_generator_path = PRE_PRIVATE_SETTLEMENT_COMPILED_GENERATOR_PATH_V1
        .expect("compile the exact-revision emitter with its generator path");
    assert_eq!(
        generator_path,
        std::path::PathBuf::from(compiled_generator_path),
        "runtime generator path must equal the source included by rustc"
    );
    let generator_bytes =
        std::fs::read(&generator_path).expect("read the exact generator .rs source");
    let supplied_generator_sha256 = std::env::var("IROHA_PRE_PRIVATE_SETTLEMENT_GENERATOR_SHA256")
        .expect("set IROHA_PRE_PRIVATE_SETTLEMENT_GENERATOR_SHA256");
    let compiled_generator_sha256 =
        hex::encode(Sha256::digest(PRE_PRIVATE_SETTLEMENT_GENERATOR_SOURCE_V1));
    assert_eq!(
        Some(supplied_generator_sha256.as_str()),
        PRE_PRIVATE_SETTLEMENT_COMPILED_GENERATOR_SHA256_V1,
        "runtime generator digest must equal the digest supplied when rustc included it"
    );
    assert_eq!(
        supplied_generator_sha256, compiled_generator_sha256,
        "caller-supplied digest must equal the generator bytes embedded by rustc"
    );
    assert_eq!(
        generator_bytes.as_slice(),
        PRE_PRIVATE_SETTLEMENT_GENERATOR_SOURCE_V1,
        "runtime generator bytes must equal the source embedded by rustc"
    );
    let generator_sha256 =
        validate_predecessor_fixture_generator_digest(&generator_bytes, &supplied_generator_sha256)
            .expect("bind provenance to the actual generator source bytes");
    let rustc_verbose = predecessor_fixture_command_stdout("rustc", &["-Vv"])
        .expect("read the actual fixture compiler version")
        .replace('\n', "\\n");
    let cargo_version = predecessor_fixture_command_stdout("cargo", &["-V"])
        .expect("read the actual fixture Cargo version");

    let authority_key = checked_seeded_keypair(0x71, Algorithm::Ed25519);
    let authority = AccountId::new(authority_key.public_key().clone());
    let domain_id =
        DomainId::try_new("predecessor", "universal").expect("deterministic predecessor domain id");
    let domain = iroha_data_model::domain::Domain::new(domain_id).build(&authority);
    let account = iroha_data_model::account::Account::new(authority.clone()).build(&authority);
    let mut world = crate::state::World::with([domain], [account], []);
    let mut parameters = iroha_data_model::parameter::Parameters::default();
    let mut npos = iroha_data_model::parameter::system::SumeragiNposParameters::default();
    npos.slashing_delay_blocks = 3_599;
    parameters.set_parameter(iroha_data_model::parameter::Parameter::Custom(
        npos.into_custom_parameter(),
    ));
    world.parameters = Cell::new(parameters);

    let kura = Kura::blank_kura_for_testing();
    let state = State::new_with_chain_and_network_id_for_testing(
        world,
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        ChainId::from("pre-private-settlement-fixture-v1"),
        snapshot_test_network_id(),
    );

    let mut transaction = TransactionBuilder::new(
        snapshot_test_network_id(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    transaction.set_creation_time(std::time::Duration::from_millis(1_000));
    let transaction = transaction
        .with_instructions([Log::new(
            Level::INFO,
            "pre-private-settlement snapshot continuity fixture".to_owned(),
        )])
        .sign(authority_key.private_key());
    let transaction = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
    let (_, block_time) =
        iroha_primitives::time::TimeSource::new_mock(std::time::Duration::from_millis(2_000));
    let block_signer = checked_seeded_keypair(0x72, Algorithm::BlsNormal);
    let block: SignedBlock = BlockBuilder::new_with_time_source(vec![transaction], block_time)
        .chain(0, None)
        .sign(block_signer.private_key())
        .unpack(|_| {})
        .into();
    let mut state_block = state.block(block.header());
    let valid =
        crate::block::ValidBlock::validate_unchecked(block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let block = Arc::new(committed.as_ref().clone());
    kura.store_block(Arc::clone(&block))
        .expect("persist predecessor block before WSV publication");
    let _events = state_block.apply_without_execution(&committed, Vec::new());
    state_block
        .commit()
        .expect("apply the predecessor block to the synthetic fixture WSV");
    let finality = signed_complete_wire_finality_for_snapshot_blocks(
        &state.network_id,
        std::slice::from_ref(&block),
    )
    .into_iter()
    .next()
    .expect("one predecessor finality artifact");
    store_complete_snapshot_commit_evidence(&state, &kura, &block, &finality);

    let state_bytes = exact_snapshot_payload_bytes(&state);
    let state_text = std::str::from_utf8(&state_bytes).expect("predecessor State is UTF-8");
    let world_bytes = borrowed_json_object_members(state_text)
        .expect("parse predecessor State")
        .into_iter()
        .find(|member| member.key == "world")
        .expect("predecessor State carries World")
        .value
        .as_bytes();
    let block_wire = block
        .encode_wire()
        .expect("encode exact predecessor SignedBlockWire");
    let wsv_hash = canonical_state_snapshot_hash(&state);
    let wsv_artifact = format!("{}\n", hex::encode(wsv_hash.as_ref()));
    let wsv_checkpoint = kura
        .wsv_checkpoint(1)
        .expect("read predecessor WSV checkpoint")
        .expect("predecessor WSV checkpoint exists")
        .encode();
    let commit_manifest = kura
        .commit_manifest(1)
        .expect("read predecessor commit manifest")
        .expect("predecessor commit manifest exists")
        .encode();
    let finality_artifact = finality.encode();
    let world_schema_order = borrowed_json_object_members(
        std::str::from_utf8(world_bytes).expect("predecessor World is UTF-8"),
    )
    .expect("parse predecessor World field order")
    .into_iter()
    .map(|member| member.key)
    .collect::<Vec<_>>();
    let world_field_count = world_schema_order.len();
    let world_schema_order = world_schema_order.join("\n");
    let sha256 = |bytes: &[u8]| hex::encode(Sha256::digest(bytes));
    let world_schema_order_sha256 = sha256(world_schema_order.as_bytes());
    validate_predecessor_fixture_world_order(world_field_count, &world_schema_order_sha256)
        .expect("the generated World must have the exact frozen predecessor field order");
    let provenance = format!(
        concat!(
            "format_version=1\n",
            "source_revision={}\n",
            "generator_path=crates/iroha_core/src/snapshot/pre_private_settlement_v1_generator.rs\n",
            "generator_sha256={}\n",
            "checkout_injection_sha256={}\n",
            "generation_command=IROHA_PRE_PRIVATE_SETTLEMENT_FIXTURE_PARENT=\"$FIXTURE_PARENT\" ",
            "IROHA_PRE_PRIVATE_SETTLEMENT_SOURCE_REVISION=\"$(git rev-parse HEAD)\" ",
            "IROHA_PRE_PRIVATE_SETTLEMENT_GENERATOR_PATH=\"$GENERATOR\" ",
            "IROHA_PRE_PRIVATE_SETTLEMENT_GENERATOR_SHA256=\"$(shasum -a 256 \"$GENERATOR\" | cut -d ' ' -f 1)\" ",
            "CARGO_TARGET_DIR=\"$TARGET_DIR\" cargo test --locked -p iroha_core --lib ",
            "snapshot::tests::emit_pre_private_settlement_fixture_v1 ",
            "-- --ignored --exact\n",
            "rustc_verbose={}\n",
            "cargo_version={}\n",
            "world_field_count=180\n",
            "world_schema_order_sha256={}\n",
            "world_sha256={}\n",
            "state_sha256={}\n",
            "block_wire_sha256={}\n",
            "wsv_checkpoint_sha256={}\n",
            "commit_manifest_sha256={}\n",
            "finality_artifact_sha256={}\n",
            "wsv_artifact_sha256={}\n",
            "wsv_hash={}\n"
        ),
        source_revision,
        generator_sha256,
        checkout_injection_sha256,
        rustc_verbose,
        cargo_version,
        world_schema_order_sha256,
        sha256(world_bytes),
        sha256(&state_bytes),
        sha256(&block_wire),
        sha256(&wsv_checkpoint),
        sha256(&commit_manifest),
        sha256(&finality_artifact),
        sha256(wsv_artifact.as_bytes()),
        hex::encode(wsv_hash.as_ref()),
    );

    let staging = PrePrivateSettlementFixtureStaging::create(&fixture_parent)
        .expect("create staged predecessor fixture directory without replacing frozen evidence");

    std::fs::write(
        staging.path().join("pre_private_settlement_state_v1.json"),
        &state_bytes,
    )
    .expect("write predecessor State");
    std::fs::write(
        staging.path().join("pre_private_settlement_world_v1.json"),
        world_bytes,
    )
    .expect("write predecessor World");
    std::fs::write(
        staging
            .path()
            .join("pre_private_settlement_block_v1.norito"),
        block_wire,
    )
    .expect("write predecessor SignedBlockWire");
    std::fs::write(
        staging
            .path()
            .join("pre_private_settlement_wsv_checkpoint_v1.norito"),
        wsv_checkpoint,
    )
    .expect("write predecessor WSV checkpoint");
    std::fs::write(
        staging
            .path()
            .join("pre_private_settlement_commit_manifest_v1.norito"),
        commit_manifest,
    )
    .expect("write predecessor commit manifest");
    std::fs::write(
        staging
            .path()
            .join("pre_private_settlement_finality_v1.norito"),
        finality_artifact,
    )
    .expect("write predecessor finality artifact");
    std::fs::write(
        staging.path().join("pre_private_settlement_state_v1.wsv"),
        wsv_artifact,
    )
    .expect("write predecessor WSV hash");
    std::fs::write(
        staging.path().join("pre_private_settlement_v1.provenance"),
        provenance,
    )
    .expect("write predecessor provenance");
    staging
        .publish(&fixture_parent)
        .expect("publish the complete predecessor fixture directory atomically");
}

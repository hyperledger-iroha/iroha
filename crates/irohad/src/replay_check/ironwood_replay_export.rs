//! Produce private Ironwood preparation evidence on the operator's disposable replay State.
//! The consumer authenticates both originals against its independently verified block and schema.

use super::*;
use crate::replay_check::world_payout_export::{Prepared, read_request_bytes, retain_native};
use iroha_core::sumeragi::certified_chain::{CurrentCutSourceLimits, read_current_certified_cut};
use iroha_crypto::HashOf;
use iroha_data_model::{
    account::AccountId,
    asset::AssetDefinitionId,
    block::{BlockHeader, consensus::SumeragiRootScope},
    privacy::PrivacyPoolIdV1,
    sumeragi::finality::{
        NATIVE_FINALITY_MAX_BLOCK_BYTES, NATIVE_FINALITY_MAX_BLOCK_COUNT,
        NATIVE_FINALITY_MAX_JOURNAL_BYTES,
    },
    sumeragi_finality::MAX_WORLD_STATE_SNAPSHOT_BYTES_V1,
};

const MIB: usize = 1024 * 1024;
const ORIGINALS_MAX: usize = 4 * MIB;

#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ContextRequest {
    schema: String,
    network_id: NetworkId,
    chain_id: String,
    genesis_hash: HashOf<BlockHeader>,
    height: u64,
    block_hash: HashOf<BlockHeader>,
    world_schema_hash_hex: String,
    selection_id_hex: String,
    pool_id_hex: String,
    asset_definition: AssetDefinitionId,
    payer: AccountId,
    reserve: AccountId,
}

fn exact_hex(value: &str) -> Result<[u8; 32], String> {
    let bytes: [u8; 32] = hex::decode(value)
        .map_err(|err| err.to_string())?
        .try_into()
        .map_err(|_| "expected 32 bytes".to_owned())?;
    if bytes == [0; 32] || hex::encode(bytes) != value {
        return Err("expected nonzero canonical lower-case hex".into());
    }
    Ok(bytes)
}

impl ContextRequest {
    fn pool(&self) -> Result<PrivacyPoolIdV1, String> {
        if self.schema != "iroha.ironwood.replayed-context-request.v1"
            || self.height < 2
            || self.chain_id.is_empty()
            || self.chain_id.len() > 1024
            || self.network_id.as_genesis_hash() != &self.genesis_hash
            || self.payer == self.reserve
            || self.world_schema_hash_hex
                != hex::encode(
                    iroha_data_model::sumeragi_finality::ironwood_world_schema_v1().as_ref(),
                )
        {
            return Err("invalid Ironwood replay context request".into());
        }
        for value in [&self.world_schema_hash_hex, &self.selection_id_hex] {
            exact_hex(value)?;
        }
        Ok(PrivacyPoolIdV1::new(exact_hex(&self.pool_id_hex)?))
    }
}

fn retain_context_originals(
    state: &State,
    tip: &iroha_core::sumeragi::certified_chain::CurrentCertifiedCut,
    request: &ContextRequest,
    world: &AllocationBudget,
    output_budget: &AllocationBudget,
) -> Result<Vec<super::world_payout_export::Original>, String> {
    state
        .with_replayed_ironwood_context_v1(
            tip,
            request.pool()?,
            &request.asset_definition,
            &request.payer,
            &request.reserve,
            world,
            |snapshot, typed| {
                if hex::encode(snapshot.schema_hash.as_ref()) != request.world_schema_hash_hex
                    || snapshot.schema_hash
                        != iroha_data_model::sumeragi_finality::ironwood_world_schema_v1()
                    || hex::encode(typed.selection.id().map_err(|err| err.to_string())?.0)
                        != request.selection_id_hex
                {
                    return Err(
                        "World snapshot schema differs from the independently selected schema"
                            .into(),
                    );
                }
                Ok(vec![
                    retain_native(
                        "world.nrt".into(),
                        snapshot,
                        MAX_WORLD_STATE_SNAPSHOT_BYTES_V1,
                        output_budget,
                    )?,
                    retain_native("originals.nrt".into(), typed, ORIGINALS_MAX, output_budget)?,
                ])
            },
        )
        .map_err(|e| e.to_string())
}

fn prepare(
    matched: &MatchedReplay<'_>,
    request: &ContextRequest,
    request_bytes: &[u8],
) -> Result<Prepared, String> {
    request.pool()?;
    if !matches!(matched.report.outcome, ReplayCheckOutcome::Matched)
        || matched.report.start.height() >= matched.tip_height
        || matched.report.replayed_to != request.height
        || matched.tip_height != request.height
        || matched.genesis_hash != request.genesis_hash
    {
        return Err(
            "Ironwood export needs a complete nonempty matched replay at the selected cut".into(),
        );
    }
    // Disjoint source, decoded graph, live snapshot/typed inputs, and retained output credits.
    let reservation = matched
        .state
        .ivm_execution_budget()
        .try_reserve_bytes(896 * MIB)
        .map_err(|e| e.to_string())?;
    let frames = AllocationBudget::new(128 * MIB);
    let graphs = AllocationBudget::new(256 * MIB);
    let world = AllocationBudget::new(384 * MIB);
    let output_budget = AllocationBudget::new(128 * MIB);
    let view = matched.state.try_view_once().map_err(|e| e.to_string())?;
    if view.network_id != request.network_id
        || view.chain_id.to_string() != request.chain_id
        || u64::try_from(view.height()).ok() != Some(request.height)
        || view.latest_block_hash() != Some(request.block_hash)
    {
        return Err("replayed ledger differs from selected chain, network or tip".into());
    }
    let tip = read_current_certified_cut(
        &view,
        request.height,
        SumeragiRootScope::Global,
        CurrentCutSourceLimits::new(
            NATIVE_FINALITY_MAX_BLOCK_BYTES,
            NATIVE_FINALITY_MAX_JOURNAL_BYTES,
            NATIVE_FINALITY_MAX_BLOCK_COUNT,
        )?,
        &frames,
        &graphs,
    )
    .map_err(|e| e.to_string())?;
    drop(view);
    if tip.height() != request.height || tip.block_hash() != request.block_hash {
        return Err(
            "certified context differs from the independently selected public block".into(),
        );
    }
    let originals = retain_context_originals(matched.state, &tip, request, &world, &output_budget)?;
    #[derive(JsonSerialize)]
    struct Manifest<'a> {
        schema: &'static str,
        selector: &'a ContextRequest,
        request_sha256: String,
        core_hash_hex: String,
        context_id_hex: String,
        world_root_hex: String,
        files: Vec<&'a super::world_payout_export::Descriptor>,
    }
    let manifest = norito::json::to_json_bounded_boxed(
        &Manifest {
            schema: "iroha.ironwood.operator-replayed-context.v1",
            selector: request,
            request_sha256: hex::encode(iroha_crypto::sha256(request_bytes)),
            core_hash_hex: hex::encode(tip.core_hash().0),
            context_id_hex: hex::encode(tip.id().0.as_ref()),
            world_root_hex: hex::encode(tip.world_state_root().as_ref()),
            files: originals.iter().map(|row| &row.descriptor).collect(),
        },
        64 * 1024,
    )
    .map_err(|e| e.to_string())?;
    Ok(Prepared {
        originals,
        manifest,
        _reservation: reservation,
    })
}

fn cut_name(request: &ContextRequest) -> String {
    format!(
        "cut-{:020}-{}",
        request.height,
        hex::encode(request.block_hash.as_ref())
    )
}

fn publication_parent(
    output: &Path,
    request: &ContextRequest,
) -> Result<iroha_fs::PrivateDirectory, String> {
    if output.file_name() != Some(std::ffi::OsStr::new(&cut_name(request))) {
        return Err("Ironwood evidence directory must name its exact certified cut".into());
    }
    let parent = output
        .parent()
        .ok_or("operator output needs a private parent")?;
    iroha_fs::PrivateDirectory::open(parent).map_err(|err| err.to_string())
}

fn publish(
    parent: &iroha_fs::PrivateDirectory,
    name: &str,
    prepared: &Prepared,
) -> Result<(), String> {
    let [world, originals] = prepared.originals.as_slice() else {
        return Err("Ironwood export requires exactly two native originals".into());
    };
    for (item, expected_name, maximum) in [
        (world, "world.nrt", MAX_WORLD_STATE_SNAPSHOT_BYTES_V1),
        (originals, "originals.nrt", ORIGINALS_MAX),
    ] {
        if item.descriptor.name != expected_name
            || item.bytes.is_empty()
            || item.bytes.len() > maximum
        {
            return Err("Ironwood export has an invalid original name or size".into());
        }
    }
    // The maintained retained-directory primitive syncs both complete files and the private
    // staging directory, then atomically renames with NOREPLACE through the original parent.
    // An error after the native rename requires reconciliation and never licenses a retry.
    let published = parent
        .publish_private_child(
            name,
            &[
                ("world.nrt", &world.bytes),
                ("originals.nrt", &originals.bytes),
            ],
        )
        .map_err(|err| err.to_string())?;
    published.revalidate().map_err(|err| err.to_string())
}

#[derive(JsonSerialize)]
struct RunReport {
    replay: ReplayCheckReportV1,
    context: Option<norito::json::Value>,
    export_failure: Option<String>,
}

/// Write one fresh private evidence directory after an offline replay at C.
///
/// # Errors
/// Fails on invalid selection, wrong cut, missing pool evidence, resource refusal or custody loss.
/// Publication errors require operator reconciliation before any retry.
pub fn run_ironwood_replay_export(
    config: &Config,
    request: &Path,
    output: &Path,
    config_path: Option<&Path>,
) -> ReportResult<(), MainError> {
    let bytes = read_request_bytes(request).map_err(failure)?;
    let expected: ContextRequest =
        norito::json::from_slice(&bytes).map_err(|err| failure(err.to_string()))?;
    expected.pool().map_err(failure)?;
    let output = sorafs_authority_inspect::output_destination(config, output, config_path)
        .map_err(failure)?;
    let parent = publication_parent(&output, &expected).map_err(failure)?;
    let mut context = None;
    let mut export_failure = None;
    let replay = check_replay_with_matched(config, |matched| {
        let outcome = prepare(&matched, &expected, &bytes).and_then(|prepared| {
            let manifest =
                norito::json::from_slice(&prepared.manifest).map_err(|err| err.to_string())?;
            // This guard rechecks configured store overlap. Publication itself uses only retained
            // parent authority, so a pathname substitution cannot redirect either original.
            sorafs_authority_inspect::output_destination(config, &output, config_path)?;
            publish(&parent, &cut_name(&expected), &prepared)?;
            Ok(manifest)
        });
        match outcome {
            Ok(manifest) => context = Some(manifest),
            Err(reason) => export_failure = Some(reason),
        }
        Ok(())
    })?;
    if context.is_none() && export_failure.is_none() {
        export_failure = Some("export requires a complete nonempty matched replay".into());
    }
    let report = RunReport {
        replay,
        context,
        export_failure,
    };
    println!(
        "{}",
        norito::json::to_json(&report).map_err(|err| failure(err.to_string()))?
    );
    if report.replay.is_ok() && report.context.is_some() {
        Ok(())
    } else {
        Err(failure(
            report
                .export_failure
                .unwrap_or_else(|| report.replay.failure_summary()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;
    #[test]
    fn ironwood_context_export_requires_replay_and_an_exclusive_complete_pair() {
        let options = [
            "--ironwood-context-export-request",
            "/input",
            "--ironwood-context-export-output",
            "/output",
        ];
        assert!(
            crate::Args::try_parse_from(["iroha3d", "--check-replay"].into_iter().chain(options))
                .is_ok()
        );
        assert!(crate::Args::try_parse_from(["iroha3d"].into_iter().chain(options)).is_err());
        for flag in [
            "--ironwood-context-export-request",
            "--ironwood-context-export-output",
        ] {
            assert!(
                crate::Args::try_parse_from(["iroha3d", "--check-replay", flag, "/path"]).is_err()
            );
        }
        for more in [
            vec!["--check-config"],
            vec!["--check-storage"],
            vec!["--rehearse-taira-activation", "/pool"],
            vec![
                "--rehearse-sorafs-repair",
                "/signed",
                "--repair-execution-time-ms",
                "12",
            ],
            vec![
                "--runtime-addition-review-request",
                "/req",
                "--runtime-addition-review-output",
                "/dir",
            ],
            vec![
                "--world-payout-export-request",
                "/req",
                "--world-payout-export-output",
                "/dir",
            ],
            vec![
                "--managed-listeners",
                "/listener",
                "--managed-listener-deadline-ns",
                "50",
            ],
        ] {
            assert!(
                crate::Args::try_parse_from(
                    ["iroha3d", "--check-replay"]
                        .into_iter()
                        .chain(options)
                        .chain(more)
                )
                .is_err()
            );
        }
    }

    #[test]
    fn ironwood_context_selector_refuses_noncanonical_or_zero_digest() {
        let good = "12".repeat(32);
        assert_eq!(exact_hex(&good).unwrap(), [0x12; 32]);
        for bad in [
            "00".repeat(32),
            "AB".repeat(32),
            "ab".repeat(31),
            format!("{good}00"),
        ] {
            assert!(exact_hex(&bad).is_err());
        }
    }

    fn request() -> ContextRequest {
        let genesis =
            HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"operator-export-selector"));
        let mut asset = [1u8; 16];
        asset[6] = 0x41;
        asset[8] = 0x81;
        ContextRequest {
            schema: "iroha.ironwood.replayed-context-request.v1".into(),
            network_id: NetworkId::from_genesis_hash(genesis),
            genesis_hash: genesis,
            chain_id: "operator-export-selector".into(),
            height: 2,
            block_hash: genesis,
            world_schema_hash_hex: hex::encode(
                iroha_data_model::sumeragi_finality::ironwood_world_schema_v1().as_ref(),
            ),
            selection_id_hex: "44".repeat(32),
            pool_id_hex: "55".repeat(32),
            asset_definition: AssetDefinitionId::from_uuid_bytes(asset).unwrap(),
            payer: iroha_test_samples::ALICE_ID.clone(),
            reserve: iroha_test_samples::BOB_ID.clone(),
        }
    }

    #[test]
    fn closed_context_request_refuses_ambiguous_or_foreign_selection() {
        let selected = request();
        selected.pool().unwrap();
        for field in ["unknown", "height"] {
            let mut json = norito::json::to_json(&selected).unwrap();
            json.insert_str(1, &format!("\"{field}\":2,"));
            assert!(norito::json::from_str::<ContextRequest>(&json).is_err());
        }
        let mut bad = selected.clone();
        bad.height = 1;
        assert!(bad.pool().is_err());
        let mut bad = selected.clone();
        bad.schema.push('x');
        assert!(bad.pool().is_err());
        let mut bad = selected.clone();
        bad.chain_id.clear();
        assert!(bad.pool().is_err());
        let mut bad = selected.clone();
        bad.payer = bad.reserve.clone();
        assert!(bad.pool().is_err());
        let mut bad = selected.clone();
        bad.selection_id_hex = "00".repeat(32);
        assert!(bad.pool().is_err());
        let mut bad = selected;
        bad.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"foreign genesis"),
        ));
        assert!(bad.pool().is_err());
    }

    // Publication fixtures exercise only custody. They are never passed off as authenticated World.
    fn publication_fixture() -> Prepared {
        let bytes = AllocationBudget::new(1024);
        Prepared {
            originals: vec![
                retain_native("world.nrt".into(), &1u64, 1024, &bytes).unwrap(),
                retain_native("originals.nrt".into(), &2u64, 1024, &bytes).unwrap(),
            ],
            manifest: Box::from(&b"{}"[..]),
            _reservation: AllocationBudget::new(1024).try_reserve_bytes(1024).unwrap(),
        }
    }

    #[test]
    #[cfg(unix)]
    fn atomic_private_cut_publishes_both_originals_and_never_clobbers() {
        use std::os::unix::fs::PermissionsExt as _;
        let temporary = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temporary.path()).unwrap();
        let parent = iroha_fs::PrivateDirectory::open_or_create(root.join("private")).unwrap();
        let selected = request();
        let name = cut_name(&selected);
        let output = parent.path().join(&name);
        let prepared = publication_fixture();
        publication_parent(&output, &selected).unwrap();
        publish(&parent, &name, &prepared).unwrap();
        let child = parent.open_child(&name).unwrap();
        assert_eq!(
            child.entries(2).unwrap(),
            ["originals.nrt", "world.nrt"].map(std::ffi::OsString::from)
        );
        assert_eq!(
            std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o700
        );
        for row in &prepared.originals {
            assert_eq!(
                &*child.read(&row.descriptor.name, 1024).unwrap(),
                &row.bytes
            );
            assert_eq!(
                std::fs::metadata(output.join(&row.descriptor.name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let before = crate::replay_check::tests::tree(parent.path());
        assert!(publish(&parent, &name, &publication_fixture()).is_err());
        assert_eq!(crate::replay_check::tests::tree(parent.path()), before);
    }

    #[test]
    fn incomplete_or_misnamed_pair_never_creates_the_selected_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temporary.path()).unwrap();
        let parent = iroha_fs::PrivateDirectory::open_or_create(root.join("private")).unwrap();
        let name = cut_name(&request());
        for wrong_name in [false, true] {
            let mut prepared = publication_fixture();
            if wrong_name {
                prepared.originals[1].descriptor.name = "wrong.nrt".into();
            } else {
                prepared.originals.pop();
            }
            assert!(publish(&parent, &name, &prepared).is_err());
            assert!(parent.entries(1).unwrap().is_empty());
        }
    }

    #[test]
    #[cfg(unix)]
    fn private_parent_refuses_wrong_cut_or_shared_permissions() {
        use std::os::unix::fs::PermissionsExt as _;
        let temporary = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temporary.path()).unwrap();
        let parent = iroha_fs::PrivateDirectory::open_or_create(root.join("private")).unwrap();
        let selected = request();
        let output = parent.path().join(cut_name(&selected));
        assert!(publication_parent(&parent.path().join("cut-wrong"), &selected).is_err());
        std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(publication_parent(&output, &selected).is_err());
    }

    #[test]
    fn changed_retained_parent_cannot_redirect_original_publication() {
        let temporary = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temporary.path()).unwrap();
        let selected = root.join("private");
        let parent = iroha_fs::PrivateDirectory::open_or_create(&selected).unwrap();
        std::fs::rename(&selected, root.join("original")).unwrap();
        let replacement = iroha_fs::PrivateDirectory::open_or_create(&selected).unwrap();
        assert!(publish(&parent, &cut_name(&request()), &publication_fixture()).is_err());
        assert!(replacement.entries(1).unwrap().is_empty());
        assert!(
            std::fs::read_dir(root.join("original"))
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[test]
    fn original_bound_and_budget_refusal_release_all_retained_credits() {
        let budget = AllocationBudget::new(1024);
        assert!(retain_native("world.nrt".into(), &1u64, 0, &budget).is_err());
        assert_eq!(budget.reserved_bytes(), 0);
        assert!(retain_native("world.nrt".into(), &1u64, 1024, &AllocationBudget::new(0)).is_err());
        let original = retain_native("world.nrt".into(), &1u64, 1024, &budget).unwrap();
        assert_eq!(budget.reserved_bytes(), original.bytes.len());
        drop(original);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn unauthenticated_empty_replay_does_not_publish_context() {
        use crate::replay_check::tests::{store_config, tree};
        let temporary = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temporary.path()).unwrap();
        let parent = iroha_fs::PrivateDirectory::open_or_create(root.join("private")).unwrap();
        let selected = request();
        let output = parent.path().join(cut_name(&selected));
        let input = root.join("request.json");
        std::fs::write(&input, norito::json::to_vec(&selected).unwrap()).unwrap();
        let before = tree(&root);
        assert!(run_ironwood_replay_export(&store_config(&root), &input, &output, None).is_err());
        assert!(!output.exists());
        assert_eq!(tree(&root), before);
    }
}

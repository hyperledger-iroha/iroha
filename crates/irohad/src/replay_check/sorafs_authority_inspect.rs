//! Read-only SoraFS authority inputs, exported while the matched replay still owns State.
//!
//! This package is an unsigned historical observation, not a State proof or an activation
//! permit. Request selectors are equality assertions, never constructors of native State.

use super::*;
use iroha_core::state::{StorageReadOnly as _, WorldReadOnly};
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::BlockHeader,
    parameter::{CustomParameter, runtime_upgrade_policy::RuntimeUpgradePolicyV1},
    permission::Permission,
    role::{Role, RoleId, RoleIdWithOwner},
    runtime::{
        definition::{DefinitionDescriptorV1, DefinitionId, RootSelectionV1, SelectionId},
        lifecycle::{ChangeSetId, ChangeSetRecordV1, ChangeSetStatusV1},
    },
};
use iroha_executor_data_model::permission::{
    governance::{CanManageParliament, CanProposeRuntimeChangeSet},
    parameter::CanSetParameters,
    role::CanManageRoles,
};
use std::io::{Read as _, Write as _};

const REQUEST_SCHEMA: &str = "iroha.sorafs.authority.inspect-request.v1";
const MODULE: &str = "iroha.sorafs.authority_governance.v1";
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 16 * 1024;
const MAX_INVENTORY_ROWS: usize = 16 * 1024;
const COMPLIANCE_ROLE: &str = "sorafs_gateway_compliance_operator";
const AUTHORITY_PERMISSION_NAMES: [&str; 7] = [
    "CanCompleteSorafsReplicationOrder",
    "CanSetSorafsReservePolicy",
    "CanUpsertSorafsProviderCredit",
    "CanManageParliament",
    "CanProposeRuntimeChangeSet",
    "CanSetParameters",
    "CanManageRoles",
];
const AUTHORITY_LIMITS: &str = "Unsigned historical inspection only; not a State proof, qualification, governance approval, proposal admission, or activation permit. No draft or transaction is produced. Recheck current State before any later action; original replay inputs and independent build provenance remain necessary.";

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct InspectRequestV1 {
    schema: String,
    mode: String,
    module: String,
    expected_cut: ExpectedCutV1,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ExpectedCutV1 {
    network_id: NetworkId,
    genesis_hash: HashOf<BlockHeader>,
    height: u64,
    tip_hash: HashOf<BlockHeader>,
    selection_id: SelectionId,
}

impl InspectRequestV1 {
    fn validate(&self) -> Result<(), String> {
        if self.schema != REQUEST_SCHEMA || self.mode != "inspect" || self.module != MODULE {
            return Err("only the closed v1 SoraFS authority inspect request is supported; draft inputs are not accepted".into());
        }
        if self.expected_cut.height == 0
            || self.expected_cut.network_id.as_genesis_hash() != &self.expected_cut.genesis_hash
        {
            return Err(
                "expected cut must be nonempty and network must identify its exact genesis".into(),
            );
        }
        Ok(())
    }
}

/// Borrow native serializers without cloning State rows. Some native types implement
/// `JsonSerialize` directly rather than the narrower `FastJsonWrite` reference blanket.
struct NativeJsonRef<'a, T>(&'a T);

impl<T: norito::json::JsonSerialize> norito::json::JsonSerialize for NativeJsonRef<'_, T> {
    fn json_serialize(&self, output: &mut String) {
        self.0.json_serialize(output);
    }

    fn json_serialize_to(
        &self,
        output: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        self.0.json_serialize_to(output)
    }
}

#[derive(JsonSerialize)]
struct DefinitionEntry<'a> {
    id: DefinitionId,
    descriptor: NativeJsonRef<'a, DefinitionDescriptorV1>,
}

#[derive(JsonSerialize)]
struct ChangeSetEntry<'a> {
    id: ChangeSetId,
    record: NativeJsonRef<'a, ChangeSetRecordV1>,
}

#[derive(JsonSerialize)]
struct DirectAuthorityGrant<'a> {
    account: NativeJsonRef<'a, AccountId>,
    account_exists: bool,
    permission: NativeJsonRef<'a, Permission>,
    /// `None` for scoped SoraFS tokens; name matching alone never asserts validity.
    exact_administrative_token: Option<bool>,
}

#[derive(JsonSerialize)]
struct AuthorityRole<'a> {
    id: NativeJsonRef<'a, RoleId>,
    /// Native Role JSON retains all tokens but omits epochs, which are exported separately.
    definition: NativeJsonRef<'a, Role>,
    permission_epochs: Vec<RolePermissionEpoch<'a>>,
    exact_administrative_tokens: Vec<&'static str>,
}

#[derive(JsonSerialize)]
struct RolePermissionEpoch<'a> {
    permission: NativeJsonRef<'a, Permission>,
    epoch: u64,
}

#[derive(JsonSerialize)]
struct AuthorityMembership<'a> {
    membership: NativeJsonRef<'a, RoleIdWithOwner>,
    account_exists: bool,
    role_exists: bool,
}

#[derive(JsonSerialize)]
struct ExecutorObservation {
    kind: &'static str,
    /// Core's borrowed semantic identity; excludes loaded VM caches and pool history.
    semantic_hash: iroha_crypto::Hash,
    permission_model_empty: bool,
    relevant_declared_permissions: Vec<&'static str>,
}

#[derive(JsonSerialize)]
struct AuthorityObservation<'a> {
    interpretation: &'static str,
    direct_grant_epochs: &'static str,
    permission_names: Vec<&'static str>,
    reserved_role_name: &'static str,
    scan_complete: bool,
    accounts_scanned: usize,
    direct_permission_sets_scanned: usize,
    direct_permissions_scanned: usize,
    roles_scanned: usize,
    role_permissions_scanned: usize,
    memberships_scanned: usize,
    direct_grants: Vec<DirectAuthorityGrant<'a>>,
    roles: Vec<AuthorityRole<'a>>,
    memberships: Vec<AuthorityMembership<'a>>,
    executor: ExecutorObservation,
}

fn relevant_permission(permission: &Permission) -> bool {
    AUTHORITY_PERMISSION_NAMES.contains(&permission.name())
}

fn push_inventory_row<T>(rows: &mut Vec<T>, row: T, remaining: &mut usize) -> Result<(), String> {
    *remaining = remaining.checked_sub(1).ok_or(
        "authority observation exceeds the local row limit; no complete inventory produced",
    )?;
    rows.try_reserve(1).map_err(|error| error.to_string())?;
    rows.push(row);
    Ok(())
}

/// Project stored native rows from the same held view, never from request-supplied accounts.
/// Exact unit-token comparisons describe possession, not execution authorization.
fn inspect_authority<'a>(
    world: &'a impl WorldReadOnly,
    max_rows: usize,
) -> Result<AuthorityObservation<'a>, String> {
    let administrative: [Permission; 4] = [
        CanManageParliament.into(),
        CanProposeRuntimeChangeSet.into(),
        CanSetParameters.into(),
        CanManageRoles.into(),
    ];
    let compliance_role: RoleId = COMPLIANCE_ROLE
        .parse()
        .map_err(|error| format!("{error}"))?;
    let mut remaining = max_rows;
    let mut direct_grants = Vec::new();
    let mut direct_permission_sets_scanned = 0;
    let mut direct_permissions_scanned = 0usize;
    for (account, tokens) in world.account_permissions().iter() {
        direct_permission_sets_scanned += 1;
        direct_permissions_scanned = direct_permissions_scanned
            .checked_add(tokens.len())
            .ok_or("direct permission inventory count overflow")?;
        for token in tokens.iter().filter(|token| relevant_permission(token)) {
            push_inventory_row(
                &mut direct_grants,
                DirectAuthorityGrant {
                    account: NativeJsonRef(account),
                    account_exists: world.accounts().get(account).is_some(),
                    permission: NativeJsonRef(token),
                    exact_administrative_token: administrative
                        .iter()
                        .find(|required| required.name() == token.name())
                        .map(|required| required == token),
                },
                &mut remaining,
            )?;
        }
    }
    let mut roles = Vec::new();
    let mut roles_scanned = 0;
    let mut role_permissions_scanned = 0usize;
    for (id, role) in world.roles().iter() {
        roles_scanned += 1;
        role_permissions_scanned = role_permissions_scanned
            .checked_add(role.permissions().len())
            .ok_or("role permission inventory count overflow")?;
        if id != &compliance_role && !role.permissions().any(relevant_permission) {
            continue;
        }
        let mut permission_epochs = Vec::new();
        for (permission, epoch) in role.permission_epochs() {
            push_inventory_row(
                &mut permission_epochs,
                RolePermissionEpoch {
                    permission: NativeJsonRef(permission),
                    epoch: *epoch,
                },
                &mut remaining,
            )?;
        }
        let exact_administrative_tokens = administrative
            .iter()
            .zip([
                "CanManageParliament",
                "CanProposeRuntimeChangeSet",
                "CanSetParameters",
                "CanManageRoles",
            ])
            .filter_map(|(required, name)| {
                role.permissions()
                    .any(|token| token == required)
                    .then_some(name)
            })
            .collect();
        push_inventory_row(
            &mut roles,
            AuthorityRole {
                id: NativeJsonRef(id),
                definition: NativeJsonRef(role),
                permission_epochs,
                exact_administrative_tokens,
            },
            &mut remaining,
        )?;
    }
    let mut memberships = Vec::new();
    let mut memberships_scanned = 0;
    for (membership, ()) in world.account_roles().iter() {
        memberships_scanned += 1;
        // B+tree order is RoleId order; all relevant roles above are retained in that order.
        let relevant_role = roles
            .binary_search_by(|role| role.id.0.cmp(&membership.id))
            .is_ok();
        if membership.id != compliance_role && !relevant_role {
            continue;
        }
        push_inventory_row(
            &mut memberships,
            AuthorityMembership {
                membership: NativeJsonRef(membership),
                account_exists: world.accounts().get(&membership.account).is_some(),
                role_exists: world.roles().get(&membership.id).is_some(),
            },
            &mut remaining,
        )?;
    }
    let declared = world.executor_data_model().permissions();
    let executor = ExecutorObservation {
        kind: match world.executor() {
            iroha_core::executor::Executor::Initial => "Initial",
            iroha_core::executor::Executor::UserProvided(_) => "UserProvided",
        },
        semantic_hash: iroha_core::executor::executor_norito::net_state_hash(world.executor())?,
        permission_model_empty: declared.is_empty(),
        relevant_declared_permissions: AUTHORITY_PERMISSION_NAMES
            .into_iter()
            .filter(|name| declared.contains(*name))
            .collect(),
    };
    Ok(AuthorityObservation {
        interpretation: "Complete stored-row observation for the named permissions and reserved role at this replay cut. Exact administrative-token flags compare canonical native Permission values. Scoped SoraFS token validity is not asserted. Role origins and direct grants are retained separately; neither token possession nor executor identity grants execution authority, and UserProvided executor authorization has not been evaluated.",
        direct_grant_epochs: "The native direct-grant table stores Permission sets without grant epochs; no epoch is invented. Role permission epochs are retained separately from Role JSON.",
        permission_names: AUTHORITY_PERMISSION_NAMES.to_vec(),
        reserved_role_name: COMPLIANCE_ROLE,
        scan_complete: true,
        accounts_scanned: world.accounts().iter().count(),
        direct_permission_sets_scanned,
        direct_permissions_scanned,
        roles_scanned,
        role_permissions_scanned,
        memberships_scanned,
        direct_grants,
        roles,
        memberships,
        executor,
    })
}

#[derive(JsonSerialize)]
struct InspectionMaterial<'a> {
    schema: &'static str,
    authority_limits: &'static str,
    module: &'static str,
    cut: ExpectedCutV1,
    chain_id: &'a iroha_model_base::chain::ChainId,
    tip_creation_time_ms: u64,
    replay_start: ReplayStartV1,
    replayed_to: u64,
    replay_outcome: &'static str,
    selection: NativeJsonRef<'a, RootSelectionV1>,
    published_definitions: Vec<DefinitionEntry<'a>>,
    committed_policy_parameter: Option<NativeJsonRef<'a, CustomParameter>>,
    decoded_policy: Option<RuntimeUpgradePolicyV1>,
    policy_decode_failure: Option<String>,
    runtime_change_sets: Vec<ChangeSetEntry<'a>>,
    scheduled_change_sets: usize,
    activation_inventory: ReplaySorafsAuthorityInventoryV1,
    authority_observation: AuthorityObservation<'a>,
}

struct PreparedInspection {
    material: Box<[u8]>,
    selection: Vec<u8>,
}

/// All assertions, native history reads and serialization precede destination creation.
fn prepare_inspection(
    matched: &MatchedReplay<'_>,
    request: &InspectRequestV1,
    request_len: usize,
) -> Result<PreparedInspection, String> {
    request.validate()?;
    if !matches!(matched.report.outcome, ReplayCheckOutcome::Matched)
        || matched.tip_height == 0
        || matched.report.replayed_to != matched.tip_height
    {
        return Err("inspection requires a complete nonempty native replay match".into());
    }
    let view = matched
        .state
        .try_view_once()
        .map_err(|error| error.to_string())?;
    let height = u64::try_from(view.height()).map_err(|error| error.to_string())?;
    if height != matched.tip_height || view.network_id.as_genesis_hash() != &matched.genesis_hash {
        return Err("matched replay owner and final State disagree on height or genesis".into());
    }
    let world = view.world();
    let selection = world.runtime_selection();
    let cut = ExpectedCutV1 {
        network_id: view.network_id,
        genesis_hash: matched.genesis_hash,
        height,
        tip_hash: view
            .latest_block_hash()
            .ok_or("final State has no canonical tip")?,
        selection_id: selection.id().map_err(|error| error.to_string())?,
    };
    if cut != request.expected_cut {
        return Err(
            "expected network/genesis/height/tip/selection does not match the native replay cut"
                .into(),
        );
    }
    let history = view.canonical_history();
    let tip = history
        .block(NonZeroUsize::new(view.height()).ok_or("empty native State")?)
        .map_err(|error| error.to_string())?;
    if tip.hash() != cut.tip_hash || tip.header().height().get() != cut.height {
        return Err("canonical history and final State disagree on the tip hash or height".into());
    }
    let tip_creation_time_ms = u64::try_from(tip.header().creation_time().as_millis())
        .map_err(|error| error.to_string())?;
    let definition_count = world.runtime_definitions().iter().count();
    let record_count = world.runtime_change_sets().iter().count();
    if definition_count.saturating_add(record_count) > MAX_INVENTORY_ROWS {
        return Err("inspection inventory exceeds the local row limit".into());
    }
    let mut definitions = Vec::new();
    definitions
        .try_reserve_exact(definition_count)
        .map_err(|error| error.to_string())?;
    for (id, descriptor) in world.runtime_definitions().iter() {
        if descriptor.id().map_err(|error| error.to_string())? != *id {
            return Err("published definition key disagrees with its native identity".into());
        }
        definitions.push(DefinitionEntry {
            id: *id,
            descriptor: NativeJsonRef(descriptor),
        });
    }
    definitions.sort_by_key(|entry| entry.id);
    let mut records = Vec::new();
    records
        .try_reserve_exact(record_count)
        .map_err(|error| error.to_string())?;
    let mut scheduled_change_sets = 0;
    for (id, record) in world.runtime_change_sets().iter() {
        if record.change_set.id().map_err(|error| error.to_string())? != *id {
            return Err("change-set record key disagrees with its native identity".into());
        }
        scheduled_change_sets += usize::from(matches!(record.status, ChangeSetStatusV1::Scheduled));
        records.push(ChangeSetEntry {
            id: *id,
            record: NativeJsonRef(record),
        });
    }
    records.sort_by_key(|entry| entry.id);
    let policy = world
        .parameters()
        .custom()
        .get(RuntimeUpgradePolicyV1::parameter_id());
    let (decoded_policy, policy_decode_failure) = match policy {
        Some(parameter) => match RuntimeUpgradePolicyV1::from_custom_parameter(parameter) {
            Ok(policy) => (Some(policy), None),
            Err(error) => (None, Some(error.to_string())),
        },
        None => (
            None,
            Some("the runtime upgrade policy is not committed; no change set is admitted".into()),
        ),
    };
    let failure_reason =
        iroha_core::runtime::validate_sorafs_authority_activation_inventory(world).err();
    let activation_inventory = ReplaySorafsAuthorityInventoryV1 {
        height,
        selection_id: hex::encode(cut.selection_id.0),
        inventory_clear: failure_reason.is_none(),
        failure_reason,
    };
    let authority_observation = inspect_authority(world, MAX_INVENTORY_ROWS)?;
    let selection_len =
        norito::canonical_frame_len(selection).map_err(|error| error.to_string())?;
    let material_limit = MAX_PACKAGE_BYTES
        .checked_sub(request_len)
        .and_then(|remaining| remaining.checked_sub(MAX_MANIFEST_BYTES))
        .and_then(|remaining| remaining.checked_sub(selection_len))
        .ok_or("inspection exceeds the local package byte limit")?;
    let material = InspectionMaterial {
        schema: "iroha.sorafs.authority.inspection.v1",
        authority_limits: AUTHORITY_LIMITS,
        module: MODULE,
        cut,
        chain_id: &view.chain_id,
        tip_creation_time_ms,
        replay_start: ReplayStartV1 {
            height: matched.report.start.height(),
            source: match matched.report.start {
                ReplayCheckStart::Genesis => START_GENESIS,
                ReplayCheckStart::Snapshot(_) => START_SNAPSHOT,
            }
            .into(),
        },
        replayed_to: matched.report.replayed_to,
        replay_outcome: OUTCOME_MATCHED,
        selection: NativeJsonRef(selection),
        published_definitions: definitions,
        committed_policy_parameter: policy.map(NativeJsonRef),
        decoded_policy,
        policy_decode_failure,
        runtime_change_sets: records,
        scheduled_change_sets,
        activation_inventory,
        authority_observation,
    };
    // Native bounded JSON counts before allocating its exact destination.
    let material = norito::json::to_json_bounded_boxed(&material, material_limit)
        .map_err(|error| error.to_string())?;
    let selection_bytes = {
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        norito::core::to_bytes_bounded(selection, selection_len)
            .map_err(|error| error.to_string())?
    };
    if selection_bytes.len() != selection_len {
        return Err("native selection encoding changed after its counted pass".into());
    }
    let decoded: RootSelectionV1 =
        norito::decode_canonical(&selection_bytes).map_err(|error| error.to_string())?;
    if &decoded != selection {
        return Err("native selection failed canonical roundtrip".into());
    }
    Ok(PreparedInspection {
        material,
        selection: selection_bytes,
    })
}

fn read_request(path: &Path) -> Result<(InspectRequestV1, Vec<u8>), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_REQUEST_BYTES as u64 {
        return Err(
            "inspection request must be a regular non-symlink file of at most 64 KiB".into(),
        );
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    let mut file = options.open(path).map_err(|error| error.to_string())?;
    let opened = file.metadata().map_err(|error| error.to_string())?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err("inspection request changed while opening".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_REQUEST_BYTES + 1)
        .map_err(|error| error.to_string())?;
    std::io::Read::by_ref(&mut file)
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() != opened.len() as usize || bytes.len() > MAX_REQUEST_BYTES {
        return Err("inspection request changed or exceeded its byte limit".into());
    }
    let request: InspectRequestV1 =
        norito::json::from_slice(&bytes).map_err(|error| error.to_string())?;
    request.validate()?;
    Ok((request, bytes))
}

/// Normalize an absolute path while refusing symlinks in every existing component.
fn checked_path(path: &Path) -> Result<PathBuf, String> {
    use std::path::Component;
    if !path.is_absolute() {
        return Err("inspection paths must be absolute".into());
    }
    let mut checked = PathBuf::new();
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err("inspection paths cannot contain parent traversal".into());
        }
        checked.push(component.as_os_str());
        match std::fs::symlink_metadata(&checked) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("inspection path has a symlink component".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(checked)
}

pub(super) fn output_destination(
    config: &Config,
    output: &Path,
    config_path: Option<&Path>,
) -> Result<PathBuf, String> {
    let output = checked_path(output)?;
    if output.try_exists().map_err(|error| error.to_string())? {
        return Err("inspection destination must not already exist".into());
    }
    let parent = output
        .parent()
        .ok_or("inspection destination needs an existing parent")?;
    if !parent.is_dir() || output.file_name().is_none() {
        return Err("inspection destination needs an existing parent directory".into());
    }
    let mut excluded = vec![
        config.kura.store_dir.resolve_relative_path(),
        config.snapshot.store_dir.resolve_relative_path(),
    ];
    excluded.extend(config.tiered_state.cold_store_root.iter().cloned());
    excluded.extend(config.tiered_state.da_store_root.iter().cloned());
    if let Some(data_dir) = &config.data_dir {
        excluded.push(data_dir.root().to_path_buf());
    }
    if let Some(config_path) = config_path {
        let config_path = std::fs::canonicalize(config_path).map_err(|error| error.to_string())?;
        excluded.push(
            config_path
                .parent()
                .ok_or("configuration has no parent")?
                .to_path_buf(),
        );
    }
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    for protected in excluded {
        let absolute = if protected.is_absolute() {
            protected
        } else {
            cwd.join(protected)
        };
        // Resolve existing aliases before containment; a missing store still keeps its location reserved.
        let protected = if absolute.exists() {
            std::fs::canonicalize(&absolute).map_err(|error| error.to_string())?
        } else {
            checked_path(&absolute)?
        };
        if output.starts_with(&protected) || protected.starts_with(&output) {
            return Err(
                "inspection output overlaps node data or the configuration directory".into(),
            );
        }
    }
    if output
        .components()
        .any(|part| part.as_os_str() == "secrets")
    {
        return Err("inspection output cannot be under a secrets directory".into());
    }
    Ok(output)
}

#[derive(Debug, JsonSerialize)]
struct Artifact {
    path: &'static str,
    bytes: usize,
    artifact_sha256: String,
}

#[derive(JsonSerialize)]
struct Manifest {
    schema: &'static str,
    authority_limits: &'static str,
    files: Vec<Artifact>,
}

#[derive(Debug, JsonSerialize)]
struct Publication {
    manifest_artifact_sha256: String,
}

fn write_package(
    output: &Path,
    request: &[u8],
    prepared: &PreparedInspection,
) -> Result<Publication, String> {
    let files: [(&str, &[u8]); 3] = [
        ("request.json", request),
        ("inspection.json", &prepared.material),
        ("selection.norito", &prepared.selection),
    ];
    let manifest = Manifest {
        schema: "iroha.sorafs.authority.inspection-package.v1",
        authority_limits: AUTHORITY_LIMITS,
        files: files
            .iter()
            .map(|(path, bytes)| Artifact {
                path,
                bytes: bytes.len(),
                artifact_sha256: hex::encode(iroha_crypto::sha256(bytes)),
            })
            .collect(),
    };
    let manifest = norito::json::to_json_bounded_boxed(&manifest, MAX_MANIFEST_BYTES)
        .map_err(|error| error.to_string())?;
    let size = files
        .iter()
        .try_fold(manifest.len(), |total, (_, bytes)| {
            total.checked_add(bytes.len())
        })
        .ok_or("inspection package size overflow")?;
    if size > MAX_PACKAGE_BYTES {
        return Err("inspection package exceeds 64 MiB".into());
    }
    let mut directory = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        directory.mode(0o700);
    }
    #[cfg(not(unix))]
    return Err("private inspection package publication requires Unix file custody".into());
    directory
        .create(output)
        .map_err(|error| error.to_string())?;
    // A failure leaves an incomplete private directory without manifest.json. Existing files
    // are never replaced. The manifest is the atomic completeness marker, published last.
    for (name, bytes) in files {
        publish_file(output, name, bytes)?;
    }
    publish_file(output, "manifest.json", &manifest)?;
    Ok(Publication {
        manifest_artifact_sha256: hex::encode(iroha_crypto::sha256(&manifest)),
    })
}

pub(super) fn publish_file(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let mut staged =
        tempfile::NamedTempFile::new_in(directory).map_err(|error| error.to_string())?;
    staged.write_all(bytes).map_err(|error| error.to_string())?;
    staged
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    staged
        .persist_noclobber(directory.join(name))
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(JsonSerialize)]
struct InspectionRunReport {
    replay: ReplayCheckReportV1,
    inspection: Option<Publication>,
    inspection_failure: Option<String>,
}

/// Export native SoraFS authority inputs after a matched replay, without drafting or signing.
///
/// # Errors
/// Refuses invalid requests/destinations, incomplete replay, an expected-cut mismatch, native
/// read/encoding failure or local resource refusal. Inspection failure preserves the replay
/// verdict in the printed report; the explicitly requested inspection exits unsuccessfully.
pub fn run_sorafs_authority_inspection(
    config: &Config,
    request_path: &Path,
    output: &Path,
    config_path: Option<&Path>,
) -> ReportResult<(), MainError> {
    let (request, bytes) = read_request(request_path).map_err(failure)?;
    let output = output_destination(config, output, config_path).map_err(failure)?;
    let mut inspection = None;
    let mut inspection_failure = None;
    let replay = check_replay_with_matched(config, |matched| {
        match prepare_inspection(&matched, &request, bytes.len()).and_then(|prepared| {
            // Replay may take a long time: recheck destination custody immediately before
            // publication, then use create-new directory/files and a no-clobber manifest.
            let output = output_destination(config, &output, config_path)?;
            write_package(&output, &bytes, &prepared)
        }) {
            Ok(publication) => inspection = Some(publication),
            Err(reason) => inspection_failure = Some(reason),
        }
        Ok(())
    })?;
    if inspection.is_none() && inspection_failure.is_none() {
        inspection_failure = Some("inspection requires a complete nonempty matched replay".into());
    }
    let report = InspectionRunReport {
        replay,
        inspection,
        inspection_failure,
    };
    println!(
        "{}",
        norito::json::to_json(&report).map_err(|error| failure(error.to_string()))?
    );
    if report.replay.is_ok() && report.inspection.is_some() {
        Ok(())
    } else {
        Err(failure(
            report
                .inspection_failure
                .unwrap_or_else(|| report.replay.failure_summary()),
        ))
    }
}

#[cfg(test)]
mod tests;

//! Selection, grant authority and certified row matching for scoped State reads.
use super::catalog::CatalogError;
use crate::state::{StorageReadOnly, WorldReadOnly};
use iroha_data_model::{
    NetworkId,
    account::{AccountId, OpaqueAccountId},
    asset::{AssetDefinitionId, AssetId},
    escrow::{AssetEscrowRecord, EscrowId},
    identifier::{IdentifierClaimRecord, IdentifierPolicyId},
    isi::error::InstructionExecutionError,
    permission::Permission,
    runtime::{
        definition::{
            DefinitionDescriptorV1, DefinitionError, DefinitionId, DefinitionLookup,
            PermissionDescriptorV1, RootSelectionV1, SchemaV1,
        },
        lifecycle::{ChangeSetV1, RootScopeV1, SelectionTransitionV1},
    },
};
use iroha_executor_data_model::permission::query::{
    CanReadAllLedgerData, CanReadStateScope, StateReadScope,
};
use iroha_model_base::state_path::StatePath;
use std::sync::LazyLock;

/// Direct grant name in State.
pub const NAME: &str = "CanReadStateScope";
/// Governed addition for the scoped permission.
pub const MODULE: &str = "iroha.permission.state_scope.v1";

/// Immutable definition published after the keyed-State cutover.
pub fn descriptor() -> DefinitionDescriptorV1 {
    let layout = crate::state::authority_registry::V1_LAYOUT;
    DefinitionDescriptorV1::Permission(PermissionDescriptorV1 {
        name: NAME.into(),
        payload: SchemaV1::Codec(format!(
            "semantic:iroha.state-scope-read.json.v1@{}.{}.{}",
            layout.major, layout.minor, layout.flags
        )),
    })
}

static DEFINITION: LazyLock<Result<DefinitionId, DefinitionError>> =
    LazyLock::new(|| descriptor().id());

/// Whether this definition belongs to the committed selection.
pub fn selected(selection: &RootSelectionV1) -> bool {
    DEFINITION
        .as_ref()
        .is_ok_and(|id| selection.active.binary_search(id).is_ok())
}

/// Whether this keyed root currently admits reads and new grants.
pub fn active(selection: &RootSelectionV1) -> bool {
    super::keyed_state::selected(selection)
        && selected(selection)
        && DEFINITION.as_ref().is_ok_and(|id| {
            !selection
                .restrictions
                .iter()
                .any(|entry| &entry.definition == id)
        })
}

/// Whether the selected executor can still remove a direct permission.
pub(super) fn revocation_executable(selection: &RootSelectionV1) -> bool {
    iroha_data_model::isi::wire_id_of::<iroha_data_model::isi::RevokeBox>()
        .is_some_and(|wire| super::obligations::executable(selection, wire))
}

pub(crate) fn validate_activation_inventory(world: &impl WorldReadOnly) -> Result<(), String> {
    let named = |p: &Permission| p.name() == NAME;
    if world
        .account_permissions()
        .iter()
        .any(|(_, ps)| ps.iter().any(named))
        || world
            .roles()
            .iter()
            .any(|(_, role)| role.permissions().any(named))
    {
        return Err(format!(
            "remove all existing {NAME} account and role grants before changing this definition"
        ));
    }
    Ok(())
}

pub(crate) fn guard_account_change(
    world: &impl WorldReadOnly,
    caller: &AccountId,
    token: &Permission,
    revoke: bool,
) -> Result<(), InstructionExecutionError> {
    if token.name() != NAME || !selected(world.runtime_selection()) {
        return Ok(());
    }
    let invalid = || {
        InstructionExecutionError::InvariantViolation(
            "invalid scoped State read grant or issuer".into(),
        )
    };
    let reader = CanReadStateScope::try_from(token).map_err(|_| invalid())?;
    if !reader.is_valid()
        || iroha_primitives::json::Json::try_new(reader).map_err(|_| invalid())? != *token.payload()
        || (!revoke
            && (!active(world.runtime_selection())
                || !revocation_executable(world.runtime_selection())))
    {
        return Err(invalid());
    }
    let root = CanReadAllLedgerData.into();
    if world.account_contains_inherent_permission(caller, &root)
        || world.account_roles_iter(caller).any(|id| {
            world
                .roles()
                .get(id)
                .is_some_and(|role| role.permissions().any(|p| p == &root))
        })
    {
        Ok(())
    } else {
        Err(invalid())
    }
}

pub(crate) fn guard_role_grant(
    world: &impl WorldReadOnly,
    token: &Permission,
) -> Result<(), InstructionExecutionError> {
    if token.name() == NAME && selected(world.runtime_selection()) {
        return Err(InstructionExecutionError::InvariantViolation(
            "scoped State read grants must be direct".into(),
        ));
    }
    Ok(())
}

/// Draft the permission-only transition on an already keyed root.
///
/// # Errors
/// Rejects an ineligible predecessor or an unavailable definition.
pub fn activation(
    current: &RootSelectionV1,
    published: &impl DefinitionLookup,
    network: NetworkId,
    effective_height: u64,
    qualification: [u8; 32],
) -> Result<ChangeSetV1, CatalogError> {
    let fail = |reason: String| CatalogError::Addition {
        module: MODULE.to_owned(),
        reason,
    };
    if !super::keyed_state::selected(current) {
        return Err(fail("a selected keyed State profile is required".into()));
    }
    if selected(current) {
        return Err(fail(format!("{NAME} is already active")));
    }
    let id = descriptor().id()?;
    let descriptor = super::release_catalog()?
        .definitions
        .get(&id)
        .cloned()
        .ok_or_else(|| fail(format!("this release does not publish {NAME}")))?;
    let mut successor = current.clone();
    successor.active.push(id);
    successor.active.sort_unstable();
    let change_set = ChangeSetV1 {
        network,
        root: RootScopeV1::Universal,
        predecessor: current.id()?,
        successor: successor.id()?,
        effective_height,
        publish: if published.definition(&id).is_none() {
            vec![descriptor]
        } else {
            Vec::new()
        },
        transitions: vec![SelectionTransitionV1::Activate(id)],
        qualification,
    };
    change_set
        .apply(current, published)
        .map_err(|error| fail(error.to_string()))?;
    Ok(change_set)
}

/// Canonical compact bare encodings are the keys in the keyed State registry.
fn decode_row<T: norito::codec::Decode>(bytes: &[u8]) -> Option<T> {
    let decoded =
        norito::codec::decode_bare_with_layout(bytes, norito::core::WireLayout::Compact).ok()?;
    norito::codec::verify_exact_payload(&decoded, bytes).ok()?;
    Some(decoded)
}

/// Classify a requested row once, before consulting the reader's direct grants.
/// Hash-only identifiers owe their membership to the record stored under that key.
enum RowFamily {
    Asset(AssetDefinitionId),
    Path(StatePath),
    Escrow(AssetDefinitionId),
    Claim(IdentifierPolicyId),
}

impl RowFamily {
    fn current(world: &impl WorldReadOnly, table: &str, key: &[u8]) -> Option<Self> {
        match table {
            "world.assets" => Some(Self::Asset(
                decode_row::<AssetId>(key)?.definition().clone(),
            )),
            "world.smart_contract_state" => Some(Self::Path(decode_row(key)?)),
            "world.asset_escrows" => {
                let id = decode_row::<EscrowId>(key)?;
                let row = world.asset_escrows().get(&id)?;
                (row.id == id).then(|| Self::Escrow(row.asset_definition.clone()))
            }
            "world.identifier_claims" => {
                let id = decode_row::<OpaqueAccountId>(key)?;
                let row = world.identifier_claims().get(&id)?;
                (row.opaque_id == id).then(|| Self::Claim(row.policy_id.clone()))
            }
            _ => None,
        }
    }

    fn covered_by(&self, grant: &StateReadScope) -> bool {
        match (self, grant) {
            (Self::Asset(id), StateReadScope::AssetBalances(definition))
            | (Self::Escrow(id), StateReadScope::AssetEscrows(definition)) => id == definition,
            (Self::Claim(id), StateReadScope::IdentifierClaims(policy)) => id == policy,
            (Self::Path(path), StateReadScope::ContractStatePrefix(prefix)) => {
                path.as_ref().starts_with(prefix.as_ref())
            }
            (Self::Path(path), StateReadScope::MultisigRecords(account)) => {
                crate::smartcontracts::isi::multisig::is_multisig_effect_key(path, account)
            }
            _ => false,
        }
    }
}

/// Select a current direct scoped grant for the exact table and canonical bare key.
pub fn read_grant<'world>(
    world: &'world impl WorldReadOnly,
    account: &AccountId,
    table: &str,
    key: &[u8],
) -> Option<&'world Permission> {
    if !active(world.runtime_selection()) {
        return None;
    }
    let mut candidates = world
        .account_inherent_permissions(account)
        .filter(|token| token.name() == NAME)
        .peekable();
    candidates.peek()?;
    let family = RowFamily::current(world, table, key)?;
    candidates.find(|token| {
        CanReadStateScope::try_from(*token)
            .is_ok_and(|grant| grant.is_valid() && family.covered_by(&grant.scope))
    })
}

/// Recheck record-dependent membership against the value in the actual certified cut.
/// An absent opaque key has no verifiable asset definition or policy.
pub fn admits_certified_value(
    grant: &CanReadStateScope,
    table: &str,
    key: &[u8],
    value: Option<&[u8]>,
) -> bool {
    let family = match table {
        "world.assets" => {
            decode_row::<AssetId>(key).map(|id| RowFamily::Asset(id.definition().clone()))
        }
        "world.smart_contract_state" => decode_row(key).map(RowFamily::Path),
        "world.asset_escrows" => value
            .and_then(decode_row::<AssetEscrowRecord>)
            .filter(|row| norito::codec::verify_exact_payload(&row.id, key).is_ok())
            .map(|row| RowFamily::Escrow(row.asset_definition)),
        "world.identifier_claims" => value
            .and_then(decode_row::<IdentifierClaimRecord>)
            .filter(|row| norito::codec::verify_exact_payload(&row.opaque_id, key).is_ok())
            .map(|row| RowFamily::Claim(row.policy_id)),
        _ => None,
    };
    grant.is_valid() && family.is_some_and(|row| row.covered_by(&grant.scope))
}

#[cfg(test)]
mod tests;

//! One-time SoraFS initialization under the separately selected Taira repair definition.
//!
//! The owner-authorized boundary authenticates all four scheduled validators and installs the
//! exact successor in the same transaction overlay. This typed handler applies only its signed
//! payload and retains a receipt. The predecessor threshold-key lifecycle is unchanged.

use std::collections::BTreeSet;

use iroha_data_model::{
    account::AccountId,
    isi::{
        Grant, Register,
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::SorafsProviderGovernanceActionV1,
        sorafs_repair::ApplySorafsRepairV1,
    },
    musubi::MusubiProviderLocationKeyV1,
    permission::Permission,
    role::{Role, RoleId},
    runtime::sorafs_repair::SorafsRepairCertificateV1,
    sorafs::{
        bootstrap::{
            SORAFS_BOOTSTRAP_COMPLIANCE_OPERATOR_ROLE_V1, SORAFS_BOOTSTRAP_RECORD_PATH_V1,
            SorafsBootstrapPayloadV1, SorafsBootstrapRecordV1,
        },
        capacity::ProviderId,
        provider_admission::governance::ProviderAdmissionGovernanceActionV1 as Action,
    },
};
use iroha_model_base::state_path::StatePath;
use mv::storage::StorageReadOnly;

use super::sorafs::apply_governed_provider_owner_action;
use crate::{
    smartcontracts::Execute,
    state::{StateTransaction, WorldReadOnly},
};

fn rejected(error: impl std::fmt::Display) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(format!(
        "SoraFS bootstrap rejected: {error}"
    )))
}

fn record_path() -> StatePath {
    SORAFS_BOOTSTRAP_RECORD_PATH_V1
        .parse()
        .expect("fixed native bootstrap record path")
}

fn certificate_path() -> StatePath {
    "sorafs/repair/certificate"
        .parse()
        .expect("fixed repair receipt path")
}

fn compliance_role() -> RoleId {
    SORAFS_BOOTSTRAP_COMPLIANCE_OPERATOR_ROLE_V1
        .parse()
        .expect("fixed gateway compliance operator role")
}

/// Read the retained bootstrap record, if this chain executed one.
///
/// # Errors
/// Returns an error if a retained record is not canonical.
pub fn read_sorafs_bootstrap_record_v1(
    world: &impl WorldReadOnly,
) -> Result<Option<SorafsBootstrapRecordV1>, InstructionExecutionError> {
    world
        .smart_contract_state()
        .get(&record_path())
        .map(|bytes| SorafsBootstrapRecordV1::decode_frame(bytes).map_err(rejected))
        .transpose()
}

/// Read the retained repair certificate, if this chain executed the repair.
///
/// # Errors
/// Returns an error if a retained certificate is not canonical and fully signed.
pub(crate) fn read_retained_repair_certificate(
    world: &impl WorldReadOnly,
) -> Result<Option<SorafsRepairCertificateV1>, InstructionExecutionError> {
    world
        .smart_contract_state()
        .get(&certificate_path())
        .map(|bytes| SorafsRepairCertificateV1::from_bytes(bytes).map_err(rejected))
        .transpose()
}

/// Resolve one allowlisted grant to its exact canonical permission and optional provider scope.
fn canonical_grant(
    permission: &Permission,
) -> Result<Option<ProviderId>, InstructionExecutionError> {
    use iroha_executor_data_model::permission::sorafs as token;
    let malformed = || rejected("grant payload does not decode as its named permission");
    macro_rules! unit {
        ($ty:ty) => {{
            let decoded = <$ty>::try_from(permission).map_err(|_| malformed())?;
            (Permission::from(decoded), None)
        }};
    }
    macro_rules! scoped {
        ($ty:ty) => {{
            let decoded = <$ty>::try_from(permission).map_err(|_| malformed())?;
            let provider = decoded.provider_id;
            (Permission::from(decoded), Some(provider))
        }};
    }
    let (canonical, provider) = match permission.name() {
        "CanBindSorafsAlias" => unit!(token::CanBindSorafsAlias),
        "CanDeclareSorafsCapacity" => unit!(token::CanDeclareSorafsCapacity),
        "CanFileSorafsCapacityDispute" => unit!(token::CanFileSorafsCapacityDispute),
        "CanIssueSorafsReplicationOrder" => unit!(token::CanIssueSorafsReplicationOrder),
        "CanManageSorafsProofOutcomePolicy" => unit!(token::CanManageSorafsProofOutcomePolicy),
        "CanManageSorafsReputationJournalPolicy" => {
            unit!(token::CanManageSorafsReputationJournalPolicy)
        }
        "CanRecordSorafsReputationJournal" => unit!(token::CanRecordSorafsReputationJournal),
        "CanResolveSorafsCapacityDispute" => unit!(token::CanResolveSorafsCapacityDispute),
        "CanSetSorafsPricing" => unit!(token::CanSetSorafsPricing),
        "CanSetSorafsReservePolicy" => unit!(token::CanSetSorafsReservePolicy),
        "CanSubmitSorafsTelemetry" => unit!(token::CanSubmitSorafsTelemetry),
        "CanUpsertSorafsProviderCredit" => unit!(token::CanUpsertSorafsProviderCredit),
        "CanCompleteSorafsReplicationOrder" => {
            scoped!(token::CanCompleteSorafsReplicationOrder)
        }
        "CanOperateSorafsRepair" => scoped!(token::CanOperateSorafsRepair),
        "CanRecordSorafsProofOutcome" => scoped!(token::CanRecordSorafsProofOutcome),
        _ => return Err(rejected("grant is not an allowlisted SoraFS permission")),
    };
    if canonical != *permission {
        return Err(rejected(
            "grant payload is not the exact canonical permission",
        ));
    }
    Ok(provider)
}

/// Check that `SoraFS` is uninitialized and that every payload entry fills an empty root.
///
/// Returns the strictly ordered provider identities named by the payload envelopes.
fn check_uninitialized(
    payload: &SorafsBootstrapPayloadV1,
    tx: &StateTransaction<'_, '_>,
) -> Result<Vec<ProviderId>, InstructionExecutionError> {
    let world = tx.world();
    if world
        .smart_contract_state()
        .get(&certificate_path())
        .is_some()
    {
        return Err(rejected("the retained repair certificate already exists"));
    }
    let prefix: StatePath = "sorafs/provider_admission/"
        .parse()
        .expect("fixed native prefix");
    if world
        .smart_contract_state()
        .range(prefix.clone()..)
        .next()
        .is_some_and(|(path, _)| path.as_ref().starts_with(prefix.as_ref()))
    {
        return Err(rejected(
            "provider admission is already initialized by genesis or Parliament",
        ));
    }
    let mut providers = Vec::with_capacity(payload.providers.len());
    for entry in &payload.providers {
        let provider = Action::Admit(entry.envelope.clone())
            .provider_id()
            .map_err(rejected)?
            .ok_or_else(|| rejected("admission envelope names no provider"))?;
        if providers
            .last()
            .is_some_and(|previous| *previous >= provider)
        {
            return Err(rejected(
                "providers must be strictly ordered by provider id",
            ));
        }
        world.account(&entry.owner)?;
        if world
            .provider_owners()
            .get(&provider)
            .is_some_and(|owner| owner != &entry.owner)
        {
            return Err(rejected(
                "provider owner conflicts with an existing binding",
            ));
        }
        if world.provider_owners().get(&provider).is_none()
            && world
                .musubi_locations_by_provider()
                .range(MusubiProviderLocationKeyV1::provider_range(provider))
                .next()
                .is_some()
        {
            return Err(rejected(
                "a new provider owner would change existing Musubi locations",
            ));
        }
        providers.push(provider);
    }
    let mut names = BTreeSet::new();
    for grant in &payload.grants {
        world.account(&grant.account)?;
        if let Some(provider) = canonical_grant(&grant.permission)?
            && providers.binary_search(&provider).is_err()
        {
            return Err(rejected(
                "a provider-scoped grant names a provider outside this bootstrap",
            ));
        }
        names.insert(grant.permission.name().to_owned());
    }
    // First grants only: a permission that already has a holder already has a root, and its
    // holder delegates it through the executor.
    let held = |permission: &Permission| names.contains(permission.name());
    if world
        .account_permissions()
        .iter()
        .any(|(_, permissions)| permissions.iter().any(held))
        || world
            .roles()
            .iter()
            .any(|(_, role)| role.permissions().any(held))
    {
        return Err(rejected("a granted SoraFS permission already has a holder"));
    }
    if !payload.compliance_operators.is_empty() {
        if world.roles().get(&compliance_role()).is_some() {
            return Err(rejected(
                "the gateway compliance operator role already exists",
            ));
        }
        for operator in &payload.compliance_operators {
            world.account(operator)?;
        }
    }
    Ok(providers)
}

impl Execute for ApplySorafsRepairV1 {
    fn execute(
        self,
        authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        apply_certified_sorafs_bootstrap_v1(&self.certificate, authority, tx)
    }
}

/// Apply exactly the payload admitted by the exceptional signed transaction boundary.
pub(crate) fn apply_certified_sorafs_bootstrap_v1(
    certificate: &SorafsRepairCertificateV1,
    authority: &AccountId,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let plan = &certificate.plan;
    let session_id = plan.id().map_err(rejected)?;
    if tx.sorafs_repair_entrypoint != Some(session_id)
        || tx.world.runtime_selection().id().map_err(rejected)? != plan.change_set.successor
        || tx.block_height() != plan.change_set.effective_height
        || read_sorafs_bootstrap_record_v1(&*tx.world)?.is_some()
    {
        return Err(rejected(
            "bootstrap requires the exact unconsumed signed repair boundary",
        ));
    }
    let payload = plan.payload.clone();
    let payload_digest = payload.canonical_digest().map_err(rejected)?;
    let providers = check_uninitialized(&payload, tx)?;
    let now = tx.block_unix_timestamp_ms();
    if now == 0 || now == u64::MAX {
        return Err(rejected("invalid block time"));
    }
    let record = SorafsBootstrapRecordV1 {
        network_id: *tx.network_id.as_bytes(),
        height: tx.block_height(),
        recorded_at_unix_ms: now,
        session_id,
        payload_digest,
        roster_hash: iroha_data_model::runtime::lifecycle::committee_digest(&plan.committee)
            .map_err(rejected)?,
    };
    if !apply_governed_provider_owner_action(
        SorafsProviderGovernanceActionV1::Admission(Action::ConfigureCouncil(payload.council)),
        tx,
    )? {
        return Err(rejected("the admission council was not configured"));
    }
    for (provider_id, entry) in providers.into_iter().zip(payload.providers) {
        if tx.world.provider_owners().get(&provider_id).is_none() {
            // This new definition initializes only its signed owner binding. Ordinary
            // governed Establish also grants the owner a repair permission and refreshes
            // Musubi projections; neither is an implicit effect of this certificate.
            // Preflight excludes locations requiring a refresh, and every permission
            // addition below comes exclusively from the signed grant list.
            tx.world.provider_owners.insert(provider_id, entry.owner);
        }
        if !apply_governed_provider_owner_action(
            SorafsProviderGovernanceActionV1::Admission(Action::Admit(entry.envelope)),
            tx,
        )? {
            return Err(rejected("a provider was not admitted"));
        }
    }
    for grant in payload.grants {
        Grant::account_permission(grant.permission, grant.account).execute(authority, tx)?;
    }
    let mut operators = payload.compliance_operators.into_iter();
    if let Some(first) = operators.next() {
        let role = compliance_role();
        Register::role(Role::new(role.clone(), first)).execute(authority, tx)?;
        for operator in operators {
            Grant::account_role(role.clone(), operator).execute(authority, tx)?;
        }
    }
    let frame = norito::encode_canonical(&record).map_err(rejected)?;
    tx.world.smart_contract_state.insert(record_path(), frame);
    // Retain the complete certificate as the once-only authority receipt, independently of the
    // signed transaction's membership in history. It cannot be replaced by a later plan.
    tx.world.smart_contract_state.insert(
        certificate_path(),
        certificate.to_bytes().map_err(rejected)?,
    );
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;

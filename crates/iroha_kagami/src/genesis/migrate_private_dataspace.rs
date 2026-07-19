//! Fail-closed migration for retained private-dataspace localnet genesis manifests.

use std::{
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::{Args as ClapArgs, ValueEnum};
use color_eyre::eyre::{Result, WrapErr as _, eyre};
use iroha_data_model::{
    isi::{GrantBox, MintBox, sns::RegisterSnsName},
    nexus::DataSpaceId,
    parameter::system::SumeragiConsensusMode,
    prelude::*,
    sns::{
        DATASPACE_ALIAS_SUFFIX_ID, DOMAIN_NAME_SUFFIX_ID, NameControllerV1, NameSelectorV1,
        RegisterNameRequestV1, SuffixId,
    },
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias, CanResolveAccountAlias},
    domain::CanRegisterDomain,
    query::CanReadRestrictedDataspace,
};
use iroha_genesis::RawGenesisTransaction;
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, Quantity},
};
use norito::codec::Decode;

use crate::{Outcome, RunArgs, tui};

const PRIVATE_SBP_DATASPACE_ID: u64 = 10;
const PRIVATE_CBUAE_DATASPACE_ID: u64 = 12;
const PRIVATE_SBP_DOMAINS: &[&str] = &["hbl.sbp", "ubl.sbp"];
const PRIVATE_SNS_LEASE_PAYMENT: &str = "0.5";
const FEE_ASSET_DOMAIN: &str = "universal.universal";
const FEE_ASSET_NAME: &str = "xor";
// Keep this migration fail-closed against the structural limit enforced by
// `iroha_core::block::check_genesis_block`.
const PROTOCOL_MAX_GENESIS_TRANSACTIONS: usize = 16;

/// Legacy private localnet profile whose retained genesis should be migrated.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Profile {
    /// State Bank of Pakistan private dataspace (dataspace 10).
    #[value(name = "private-sbp", alias = "sbp")]
    PrivateSbp,
    /// Central Bank of the UAE private dataspace (dataspace 12).
    #[value(name = "private-cbuae", alias = "cbuae")]
    PrivateCbuae,
}

#[derive(Clone, Copy, Debug)]
struct ProfileSpec {
    profile_name: &'static str,
    alias: &'static str,
    dataspace_id: u64,
    domains: &'static [&'static str],
}

impl Profile {
    const fn spec(self) -> ProfileSpec {
        match self {
            Self::PrivateSbp => ProfileSpec {
                profile_name: "private-sbp",
                alias: "sbp",
                dataspace_id: PRIVATE_SBP_DATASPACE_ID,
                domains: PRIVATE_SBP_DOMAINS,
            },
            Self::PrivateCbuae => ProfileSpec {
                profile_name: "private-cbuae",
                alias: "cbuae",
                dataspace_id: PRIVATE_CBUAE_DATASPACE_ID,
                domains: &[],
            },
        }
    }
}

/// Rewrite the exact legacy mixed private-dataspace permission tail.
#[derive(ClapArgs, Clone, Debug)]
pub struct Args {
    /// Retained raw genesis JSON manifest to inspect.
    genesis_file: PathBuf,
    /// Exact private localnet profile represented by the manifest.
    #[arg(long, value_enum)]
    profile: Profile,
    /// Destination for the migrated canonical raw genesis JSON.
    #[arg(long)]
    out_file: PathBuf,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        if self.genesis_file == self.out_file {
            return Err(eyre!(
                "refusing to overwrite the retained genesis in place; choose a distinct --out-file"
            ));
        }

        tui::status("Migrating retained private-dataspace genesis");
        let source = fs::read_to_string(&self.genesis_file).wrap_err_with(|| {
            format!(
                "failed to read retained genesis {}",
                self.genesis_file.display()
            )
        })?;
        // Parse the raw manifest directly instead of `from_path`: signing resolves relative
        // paths later, while this boundary-only migration must preserve those raw path fields.
        let manifest: RawGenesisTransaction = norito::json::from_str(&source).map_err(|error| {
            eyre!(
                "failed to deserialize retained genesis {}: {error}",
                self.genesis_file.display()
            )
        })?;
        let migrated = migrate(manifest, self.profile)?;
        let json = norito::json::to_json_pretty(&migrated)
            .wrap_err("failed to serialize migrated raw genesis")?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.out_file)
            .wrap_err_with(|| {
                format!(
                    "failed to create new migrated genesis {}; existing outputs are never overwritten",
                    self.out_file.display()
                )
            })?;
        output.write_all(json.as_bytes()).wrap_err_with(|| {
            format!(
                "failed to write migrated genesis {}",
                self.out_file.display()
            )
        })?;
        writeln!(
            writer,
            "migrated_profile: {}\nout_file: {}",
            self.profile.spec().profile_name,
            self.out_file.display()
        )?;
        tui::success("Retained private-dataspace genesis migrated");
        Ok(())
    }
}

/// Migrate one exact legacy private-profile manifest.
///
/// The legacy generator mixed the universal payment-reserve mint with private
/// SNS registrations, then placed universal and private permission grants in
/// one final transaction. Target routing cannot execute either mixed batch in
/// both worlds. This migration preserves every instruction and raw manifest
/// field while splitting the SNS batch into a universal setup transaction and a
/// private-SNS transaction, and replacing the permission tail with a
/// private-only transaction. The universal setup combines the reserve with the
/// universal permission grants and assigns a restricted-reader role to the
/// client. This remains one lane-homogeneous transaction and keeps the migrated
/// genesis within the protocol transaction limit. The private permission batch
/// begins with a label-less registration of the same client so the private
/// world has an account row before its direct grants execute.
pub fn migrate(
    mut manifest: RawGenesisTransaction,
    profile: Profile,
) -> Result<RawGenesisTransaction> {
    let spec = profile.spec();
    if manifest.consensus_mode() != SumeragiConsensusMode::Npos {
        return Err(eyre!(
            "{} retained private-dataspace genesis must declare NPoS consensus; refusing to migrate {:?}",
            spec.profile_name,
            manifest.consensus_mode()
        ));
    }
    let tx_count = manifest.transactions().len();
    if tx_count < 2 {
        return Err(eyre!(
            "{profile_name} retained genesis must end with an SNS bootstrap transaction and the legacy mixed permission transaction",
            profile_name = spec.profile_name
        ));
    }
    let tail_index = tx_count - 1;
    let tail = manifest.transactions()[tail_index].instructions();
    let (universal_permissions, private_permissions) = expected_target_permissions(spec)?;
    let legacy_permissions =
        expected_legacy_permission_order(&universal_permissions, &private_permissions, spec);
    if tail.len() != legacy_permissions.len() {
        return Err(eyre!(
            "{profile_name} legacy mixed permission tail must contain exactly {expected} grants; found {actual}",
            profile_name = spec.profile_name,
            expected = legacy_permissions.len(),
            actual = tail.len()
        ));
    }

    let (client_account_id, first_permission) =
        account_permission_grant(&tail[0]).ok_or_else(|| {
            eyre!(
                "{} legacy mixed permission tail must begin with an account permission grant",
                spec.profile_name
            )
        })?;
    if first_permission != &legacy_permissions[0] {
        return Err(eyre!(
            "{} legacy mixed permission tail begins with an unexpected permission",
            spec.profile_name
        ));
    }
    let client_account_id = client_account_id.clone();
    for (instruction_index, (instruction, expected_permission)) in
        tail.iter().zip(&legacy_permissions).enumerate()
    {
        let Some((destination, permission)) = account_permission_grant(instruction) else {
            return Err(eyre!(
                "{} legacy mixed permission tail instruction {instruction_index} is not an account permission grant",
                spec.profile_name
            ));
        };
        if destination != &client_account_id || permission != expected_permission {
            return Err(eyre!(
                "{} legacy mixed permission tail instruction {instruction_index} does not match the exact destination and permission order",
                spec.profile_name
            ));
        }
    }

    validate_prior_permission_inventory(
        &manifest,
        tail_index,
        &client_account_id,
        &legacy_permissions,
        spec,
    )?;
    let legacy_sns_index = tail_index - 1;
    validate_sns_bootstrap(
        manifest.transactions()[legacy_sns_index].instructions(),
        &client_account_id,
        spec,
    )?;

    let legacy_sns = manifest.transactions()[legacy_sns_index].instructions();
    let reserve_mint = legacy_sns[0].clone();
    let private_sns_batch = legacy_sns[1..].to_vec();

    let mut universal_batch = Vec::with_capacity(universal_permissions.len() + 2);
    universal_batch.push(reserve_mint);
    universal_batch.extend(
        universal_permissions.iter().cloned().map(|permission| {
            Grant::account_permission(permission, client_account_id.clone()).into()
        }),
    );
    let private_dataspace = DataSpaceId::new(spec.dataspace_id);
    let restricted_read = Permission::from(CanReadRestrictedDataspace {
        dataspace: private_dataspace,
    });
    universal_batch.push(
        Register::role(
            Role::new(
                crate::genesis::private_dataspace_reader_role_id(spec.alias, private_dataspace),
                client_account_id.clone(),
            )
            .add_permission(restricted_read),
        )
        .into(),
    );
    let mut private_batch = Vec::with_capacity(private_permissions.len() + 1);
    private_batch.push(Register::account(Account::new(client_account_id.clone())).into());
    private_batch.extend(
        private_permissions.iter().cloned().map(|permission| {
            Grant::account_permission(permission, client_account_id.clone()).into()
        }),
    );

    manifest.replace_instruction_only_transaction(
        legacy_sns_index,
        vec![universal_batch, private_sns_batch],
    )?;
    // Splitting the transaction immediately before the legacy tail shifts that tail forward by
    // exactly one slot.
    let private_permission_index = tail_index + 1;
    manifest.replace_instruction_only_transaction(private_permission_index, vec![private_batch])?;
    if manifest.transactions().len() > PROTOCOL_MAX_GENESIS_TRANSACTIONS {
        return Err(eyre!(
            "{} migrated genesis would contain {} transactions, exceeding the protocol maximum of {PROTOCOL_MAX_GENESIS_TRANSACTIONS}",
            spec.profile_name,
            manifest.transactions().len()
        ));
    }
    validate_migrated_sns_bootstrap(&manifest, legacy_sns_index, &client_account_id, spec)?;
    validate_migrated_permission_tail(
        &manifest,
        legacy_sns_index,
        private_permission_index,
        &client_account_id,
        &universal_permissions,
        &private_permissions,
        spec,
    )?;
    Ok(manifest)
}

fn expected_target_permissions(spec: ProfileSpec) -> Result<(Vec<Permission>, Vec<Permission>)> {
    // The universal Manage grant is intentionally absent: the ordinary localnet bootstrap
    // already granted it and the legacy private-profile append path deduplicated it.
    // Torii authorizes at universal ingress and re-authorizes the forwarded signed query on the
    // receiving private peer. The universal hop receives its restricted-read capability through
    // a role registered in that transaction; only the private hop uses a direct account grant.
    let private_dataspace = DataSpaceId::new(spec.dataspace_id);
    let mut universal = vec![Permission::from(CanResolveAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
    })];
    if !spec.domains.is_empty() {
        universal.push(Permission::from(CanRegisterDomain));
    }

    let mut private = vec![
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(private_dataspace),
        }),
        Permission::from(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(private_dataspace),
        }),
    ];
    for domain in spec.domains {
        let domain = DomainId::parse_fully_qualified(domain).wrap_err_with(|| {
            format!(
                "invalid static {} private domain `{domain}`",
                spec.profile_name
            )
        })?;
        private.push(Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(domain.clone()),
        }));
        private.push(Permission::from(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Domain(domain),
        }));
    }
    private.push(Permission::from(CanReadRestrictedDataspace {
        dataspace: private_dataspace,
    }));
    Ok((universal, private))
}

fn expected_legacy_permission_order(
    universal: &[Permission],
    target_private: &[Permission],
    spec: ProfileSpec,
) -> Vec<Permission> {
    // The retained SBP generator appended CanRegisterDomain after every scoped alias grant,
    // even though the corrected universal-only replacement must place it beside universal
    // Resolve. Both retained profiles predate CanReadRestrictedDataspace, so their exact input
    // order excludes the new role and private direct grant. CBUAE has no
    // CanRegisterDomain grant, so its retained order is simply universal Resolve followed by its
    // legacy private grants.
    debug_assert_eq!(universal.len(), if spec.domains.is_empty() { 1 } else { 2 });
    let restricted_read = Permission::from(CanReadRestrictedDataspace {
        dataspace: DataSpaceId::new(spec.dataspace_id),
    });
    debug_assert_eq!(target_private.last(), Some(&restricted_read));
    let legacy_private = &target_private[..target_private.len() - 1];
    let mut legacy = Vec::with_capacity(universal.len() + legacy_private.len());
    legacy.push(universal[0].clone());
    legacy.extend(legacy_private.iter().cloned());
    legacy.extend(universal[1..].iter().cloned());
    legacy
}

fn validate_migrated_permission_tail(
    manifest: &RawGenesisTransaction,
    universal_index: usize,
    private_index: usize,
    client_account_id: &AccountId,
    expected_universal: &[Permission],
    expected_private: &[Permission],
    spec: ProfileSpec,
) -> Result<()> {
    let transactions = manifest.transactions();
    if transactions.len() != private_index + 1 || private_index != universal_index + 2 {
        return Err(eyre!(
            "{} migrated genesis must end with one private SNS transaction between its universal setup and private permission transaction",
            spec.profile_name
        ));
    }

    let universal = transactions[universal_index].instructions();
    if universal.len() != expected_universal.len() + 2 {
        return Err(eyre!(
            "{} migrated universal setup transaction has an unexpected instruction count",
            spec.profile_name
        ));
    }
    for (instruction_index, (instruction, expected_permission)) in
        universal[1..].iter().zip(expected_universal).enumerate()
    {
        let Some((destination, permission)) = account_permission_grant(instruction) else {
            return Err(eyre!(
                "{} migrated universal permission instruction {instruction_index} is not an account permission grant",
                spec.profile_name
            ));
        };
        if destination != client_account_id || permission != expected_permission {
            return Err(eyre!(
                "{} migrated universal permission instruction {instruction_index} does not match the exact target destination and order",
                spec.profile_name
            ));
        }
    }
    let Some(RegisterBox::Role(register)) = universal
        .last()
        .and_then(|instruction| instruction.as_any().downcast_ref::<RegisterBox>())
    else {
        return Err(eyre!(
            "{} migrated universal permission transaction must end with restricted-reader role registration",
            spec.profile_name
        ));
    };
    let role = register.object();
    let private_dataspace = DataSpaceId::new(spec.dataspace_id);
    let expected_role_id =
        crate::genesis::private_dataspace_reader_role_id(spec.alias, private_dataspace);
    let expected_read = Permission::from(CanReadRestrictedDataspace {
        dataspace: private_dataspace,
    });
    if role.inner().id != expected_role_id
        || role.grant_to() != client_account_id
        || role.inner().permissions().ne([&expected_read])
    {
        return Err(eyre!(
            "{} migrated universal restricted-reader role does not exactly match its id, owner, and permission",
            spec.profile_name
        ));
    }

    let private = transactions[private_index].instructions();
    if private.len() != expected_private.len() + 1 {
        return Err(eyre!(
            "{} migrated private permission transaction has an unexpected instruction count",
            spec.profile_name
        ));
    }
    let Some(RegisterBox::Account(register)) = private[0].as_any().downcast_ref::<RegisterBox>()
    else {
        return Err(eyre!(
            "{} migrated private permission transaction must begin with account registration",
            spec.profile_name
        ));
    };
    let account = register.object();
    if account.id != *client_account_id
        || account.metadata != Metadata::default()
        || account.label.is_some()
        || account.uaid.is_some()
        || !account.opaque_ids.is_empty()
    {
        return Err(eyre!(
            "{} migrated private account registration does not match the exact target client",
            spec.profile_name
        ));
    }
    for (instruction_index, (instruction, expected_permission)) in
        private[1..].iter().zip(expected_private).enumerate()
    {
        let Some((destination, permission)) = account_permission_grant(instruction) else {
            return Err(eyre!(
                "{} migrated private permission instruction {instruction_index} is not an account permission grant",
                spec.profile_name
            ));
        };
        if destination != client_account_id || permission != expected_permission {
            return Err(eyre!(
                "{} migrated private permission instruction {instruction_index} does not match the exact target destination and order",
                spec.profile_name
            ));
        }
    }
    Ok(())
}

fn account_permission_grant(instruction: &InstructionBox) -> Option<(&AccountId, &Permission)> {
    let GrantBox::Permission(grant) = instruction.as_any().downcast_ref::<GrantBox>()? else {
        return None;
    };
    Some((grant.destination(), grant.object()))
}

fn validate_prior_permission_inventory(
    manifest: &RawGenesisTransaction,
    tail_index: usize,
    client_account_id: &AccountId,
    legacy_permissions: &[Permission],
    spec: ProfileSpec,
) -> Result<()> {
    let expected_existing_manage = Permission::from(CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
    });
    let private_dataspace = DataSpaceId::new(spec.dataspace_id);
    let expected_reader_role =
        crate::genesis::private_dataspace_reader_role_id(spec.alias, private_dataspace);
    let expected_read = Permission::from(CanReadRestrictedDataspace {
        dataspace: private_dataspace,
    });
    let mut existing_manage_count = 0_usize;
    for (tx_index, transaction) in manifest.transactions()[..tail_index].iter().enumerate() {
        for (instruction_index, instruction) in transaction.instructions().iter().enumerate() {
            if let Some(RegisterBox::Role(register)) =
                instruction.as_any().downcast_ref::<RegisterBox>()
            {
                let role = register.object();
                if role.inner().id == expected_reader_role
                    || role
                        .inner()
                        .permissions()
                        .any(|permission| permission == &expected_read)
                {
                    return Err(eyre!(
                        "{} retained genesis contains an unexpected prior restricted-reader role at transaction {tx_index}, instruction {instruction_index}",
                        spec.profile_name
                    ));
                }
            }
            let Some((destination, permission)) = account_permission_grant(instruction) else {
                continue;
            };
            if destination != client_account_id {
                continue;
            }
            if permission == &expected_existing_manage {
                existing_manage_count += 1;
                continue;
            }
            if legacy_permissions.contains(&permission)
                || matches!(
                    permission.name(),
                    "CanManageAccountAlias"
                        | "CanResolveAccountAlias"
                        | "CanRegisterDomain"
                        | "CanReadRestrictedDataspace"
                )
            {
                return Err(eyre!(
                    "{} retained genesis contains an unexpected prior private-bootstrap permission at transaction {tx_index}, instruction {instruction_index}",
                    spec.profile_name
                ));
            }
        }
    }
    if existing_manage_count != 1 {
        return Err(eyre!(
            "{} retained genesis must contain exactly one prior universal CanManageAccountAlias grant for the legacy client; found {existing_manage_count}",
            spec.profile_name
        ));
    }
    Ok(())
}

fn validate_sns_bootstrap(
    instructions: &[InstructionBox],
    client_account_id: &AccountId,
    spec: ProfileSpec,
) -> Result<()> {
    let expected_registration_count = spec.domains.len() + 1;
    if instructions.len() != expected_registration_count + 1 {
        return Err(eyre!(
            "{} SNS bootstrap transaction must contain exactly one reserve mint and {expected_registration_count} registrations",
            spec.profile_name
        ));
    }

    let expected_selectors = std::iter::once((DATASPACE_ALIAS_SUFFIX_ID, spec.alias))
        .chain(
            spec.domains
                .iter()
                .map(|domain| (DOMAIN_NAME_SUFFIX_ID, *domain)),
        )
        .collect::<Vec<(SuffixId, &str)>>();
    let expected_controller =
        NameControllerV1::account(&client_account_id.to_account_address().map_err(|error| {
            eyre!(
                "failed to derive {} legacy client SNS controller: {error}",
                spec.profile_name
            )
        })?);
    let expected_payment: Quantity = PRIVATE_SNS_LEASE_PAYMENT
        .parse()
        .wrap_err("invalid static private SNS payment")?;
    let expected_null = Json::from_string_unchecked("null".to_owned());
    let expected_fee_asset = fee_asset_definition_id();
    let expected_fee_asset_literal = expected_fee_asset.canonical_address();
    let mut payer = None;

    for (offset, ((suffix_id, label), instruction)) in expected_selectors
        .iter()
        .zip(&instructions[1..])
        .enumerate()
    {
        let register = instruction
            .as_any()
            .downcast_ref::<RegisterSnsName>()
            .ok_or_else(|| {
                eyre!(
                    "{} SNS bootstrap instruction {} is not RegisterSnsName",
                    spec.profile_name,
                    offset + 1
                )
            })?;
        let mut encoded = register.request.as_slice();
        let request = RegisterNameRequestV1::decode(&mut encoded).map_err(|error| {
            eyre!(
                "failed to decode {} SNS bootstrap instruction {}: {error}",
                spec.profile_name,
                offset + 1
            )
        })?;
        if !encoded.is_empty() {
            return Err(eyre!(
                "{} SNS bootstrap instruction {} contains trailing request bytes",
                spec.profile_name,
                offset + 1
            ));
        }
        let expected_selector = NameSelectorV1::new(*suffix_id, *label).map_err(|error| {
            eyre!(
                "invalid static {} SNS selector `{label}`: {error}",
                spec.profile_name
            )
        })?;
        if request.selector != expected_selector
            || request.owner != *client_account_id
            || request.controllers != [expected_controller.clone()]
            || request.term_years != 1
            || request.pricing_class_hint.is_some()
            || request.payment.asset_id != expected_fee_asset_literal
            || request.payment.gross_amount != expected_payment
            || request.payment.net_amount != expected_payment
            || request.payment.settlement_tx != expected_null
            || request.payment.signature != expected_null
            || request.governance.is_some()
            || request.metadata != Metadata::default()
        {
            return Err(eyre!(
                "{} SNS bootstrap instruction {} does not match the exact legacy request",
                spec.profile_name,
                offset + 1
            ));
        }
        match &payer {
            None => payer = Some(request.payment.payer),
            Some(expected_payer) if expected_payer == &request.payment.payer => {}
            Some(_) => {
                return Err(eyre!(
                    "{} SNS bootstrap registrations do not share one payer",
                    spec.profile_name
                ));
            }
        }
    }

    let payer = payer.expect("private profile always has at least one SNS registration");
    let reserve_mint = instructions[0]
        .as_any()
        .downcast_ref::<MintBox>()
        .ok_or_else(|| {
            eyre!(
                "{} SNS bootstrap transaction must begin with its reserve mint",
                spec.profile_name
            )
        })?;
    let MintBox::Asset(reserve_mint) = reserve_mint else {
        return Err(eyre!(
            "{} SNS bootstrap reserve must mint an asset quantity",
            spec.profile_name
        ));
    };
    let registration_count = u64::try_from(expected_registration_count)
        .expect("private profile registration count fits in u64");
    let expected_reserve = expected_payment
        .try_mul_decimal(&Numeric::from(registration_count))
        .wrap_err("static private SNS reserve overflow")?;
    if reserve_mint.destination() != &AssetId::new(expected_fee_asset, payer)
        || reserve_mint.object().as_numeric() != expected_reserve.as_numeric()
    {
        return Err(eyre!(
            "{} SNS bootstrap reserve mint does not match the exact legacy request set",
            spec.profile_name
        ));
    }
    Ok(())
}

fn validate_migrated_sns_bootstrap(
    manifest: &RawGenesisTransaction,
    reserve_index: usize,
    client_account_id: &AccountId,
    spec: ProfileSpec,
) -> Result<()> {
    let transactions = manifest.transactions();
    let universal_setup = transactions
        .get(reserve_index)
        .ok_or_else(|| {
            eyre!(
                "{} migrated universal setup transaction is absent",
                spec.profile_name
            )
        })?
        .instructions();
    if universal_setup.is_empty() {
        return Err(eyre!(
            "{} migrated universal setup transaction must begin with the reserve mint",
            spec.profile_name
        ));
    }
    let registrations = transactions
        .get(reserve_index + 1)
        .ok_or_else(|| eyre!("{} migrated SNS transaction is absent", spec.profile_name))?
        .instructions();
    let expected_registration_count = spec.domains.len() + 1;
    if registrations.len() != expected_registration_count
        || registrations.iter().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterSnsName>()
                .is_none()
        })
    {
        return Err(eyre!(
            "{} migrated SNS transaction must contain exactly {expected_registration_count} registrations",
            spec.profile_name
        ));
    }

    // Reuse the exact legacy inventory validator after proving the new transaction boundary.
    // This demonstrates that the split changes ordering boundaries only, not instruction data.
    let combined = universal_setup[..1]
        .iter()
        .chain(registrations)
        .cloned()
        .collect::<Vec<_>>();
    validate_sns_bootstrap(&combined, client_account_id, spec)
}

fn fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::parse_fully_qualified(FEE_ASSET_DOMAIN)
            .expect("static fee asset domain must remain valid"),
        FEE_ASSET_NAME
            .parse()
            .expect("static fee asset name must remain valid"),
    )
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs};

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        isi::{RegisterBox, sns::RegisterSnsName},
        sns::{PaymentProofV1, RegisterNameRequestV1},
    };
    use iroha_genesis::{GenesisBuilder, init_instruction_registry};
    use tempfile::tempdir;

    use super::*;

    fn fixture_account() -> AccountId {
        AccountId::new(
            KeyPair::from_seed(
                b"retained-private-client-fixture".to_vec(),
                Algorithm::Ed25519,
            )
            .public_key()
            .clone(),
        )
    }

    fn fixture_payer() -> AccountId {
        AccountId::new(
            KeyPair::from_seed(
                b"retained-private-genesis-fixture".to_vec(),
                Algorithm::Ed25519,
            )
            .public_key()
            .clone(),
        )
    }

    fn sns_registration(
        suffix_id: SuffixId,
        label: &str,
        owner: &AccountId,
        payer: &AccountId,
        payment: &Quantity,
    ) -> InstructionBox {
        let controller = NameControllerV1::account(
            &owner
                .to_account_address()
                .expect("derive fixture SNS controller"),
        );
        RegisterSnsName::new(RegisterNameRequestV1 {
            selector: NameSelectorV1::new(suffix_id, label).expect("valid fixture selector"),
            owner: owner.clone(),
            controllers: vec![controller],
            term_years: 1,
            pricing_class_hint: None,
            payment: PaymentProofV1 {
                asset_id: fee_asset_definition_id().canonical_address(),
                gross_amount: payment.clone(),
                net_amount: payment.clone(),
                settlement_tx: Json::from_string_unchecked("null".to_owned()),
                payer: payer.clone(),
                signature: Json::from_string_unchecked("null".to_owned()),
            },
            governance: None,
            metadata: Metadata::default(),
        })
        .into()
    }

    fn legacy_manifest(profile: Profile) -> RawGenesisTransaction {
        init_instruction_registry();
        let spec = profile.spec();
        let client = fixture_account();
        let payer = fixture_payer();
        let payment: Quantity = PRIVATE_SNS_LEASE_PAYMENT
            .parse()
            .expect("parse fixture payment");
        let registration_count = u64::try_from(spec.domains.len() + 1).expect("count fits");
        let reserve = payment
            .try_mul_decimal(&Numeric::from(registration_count))
            .expect("fixture reserve fits");
        let existing_manage = Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
        });
        let mut builder = GenesisBuilder::new_without_executor(
            ChainId::from(format!("fixture-{}", spec.profile_name)),
            PathBuf::from("relative-ivm"),
        )
        .append_instruction(Register::account(Account::new(client.clone())))
        .append_instruction(Grant::account_permission(existing_manage, client.clone()))
        .next_transaction()
        .append_instruction(Mint::asset_quantity(
            reserve,
            AssetId::new(fee_asset_definition_id(), payer.clone()),
        ))
        .append_instruction(sns_registration(
            DATASPACE_ALIAS_SUFFIX_ID,
            spec.alias,
            &client,
            &payer,
            &payment,
        ));
        for domain in spec.domains {
            builder = builder.append_instruction(sns_registration(
                DOMAIN_NAME_SUFFIX_ID,
                domain,
                &client,
                &payer,
                &payment,
            ));
        }
        builder = builder.next_transaction();
        let (universal, private) = expected_target_permissions(spec).expect("fixture permissions");
        for permission in expected_legacy_permission_order(&universal, &private, spec) {
            builder =
                builder.append_instruction(Grant::account_permission(permission, client.clone()));
        }
        builder
            .build_raw()
            .with_consensus_mode(SumeragiConsensusMode::Npos)
    }

    fn pad_legacy_manifest_to_transaction_count(
        mut manifest: RawGenesisTransaction,
        target_count: usize,
    ) -> RawGenesisTransaction {
        let current_count = manifest.transactions().len();
        assert!(target_count >= current_count);
        let padding_count = target_count - current_count;
        let sns_index = current_count - 2;
        let original_sns = manifest.transactions()[sns_index].instructions().to_vec();
        let mut replacement = Vec::with_capacity(padding_count + 1);
        for index in 0..padding_count {
            let domain =
                DomainId::parse_fully_qualified(&format!("migration-padding-{index}.universal"))
                    .expect("valid migration padding domain");
            replacement.push(vec![Register::domain(Domain::new(domain)).into()]);
        }
        // Keep the exact SNS transaction immediately before the legacy permission tail.
        replacement.push(original_sns);
        manifest
            .replace_instruction_only_transaction(sns_index, replacement)
            .expect("pad retained fixture with ordinary transactions");
        assert_eq!(manifest.transactions().len(), target_count);
        manifest
    }

    fn grant_permissions(instructions: &[InstructionBox]) -> Vec<Permission> {
        instructions
            .iter()
            .map(|instruction| {
                account_permission_grant(instruction)
                    .expect("expected account permission grant")
                    .1
                    .clone()
            })
            .collect()
    }

    #[test]
    fn sbp_fixture_matches_exact_retained_legacy_order() {
        let manifest = legacy_manifest(Profile::PrivateSbp);
        let observed = grant_permissions(
            manifest
                .transactions()
                .last()
                .expect("fixture has legacy tail")
                .instructions(),
        );
        let (universal, private) =
            expected_target_permissions(Profile::PrivateSbp.spec()).expect("expected permissions");
        assert_eq!(universal.len(), 2);
        assert_eq!(private.len(), 7);
        let expected = vec![
            universal[0].clone(),
            private[0].clone(),
            private[1].clone(),
            private[2].clone(),
            private[3].clone(),
            private[4].clone(),
            private[5].clone(),
            universal[1].clone(),
        ];
        assert_eq!(
            observed, expected,
            "retained SBP tail is Resolve(universal), its pre-read-grant private scopes, then CanRegisterDomain"
        );
        assert!(
            !observed
                .iter()
                .any(|permission| permission.name() == "CanReadRestrictedDataspace"),
            "legacy fixture must model the retained input before restricted-read grants existed"
        );
    }

    #[test]
    fn rejects_permissioned_manifest_before_tail_migration() {
        let permissioned = legacy_manifest(Profile::PrivateSbp)
            .with_consensus_mode(SumeragiConsensusMode::Permissioned);
        let error = migrate(permissioned, Profile::PrivateSbp)
            .expect_err("private retained genesis must already declare NPoS");
        assert!(
            error.to_string().contains("must declare NPoS consensus"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn migrates_exact_legacy_profiles_and_adds_the_read_grant_to_both_worlds() {
        for profile in [Profile::PrivateSbp, Profile::PrivateCbuae] {
            let legacy = legacy_manifest(profile);
            let before_transactions = legacy.transactions().len();
            let before_prefix = legacy.transactions()[..before_transactions - 2]
                .iter()
                .map(|transaction| {
                    transaction
                        .instructions()
                        .iter()
                        .map(iroha_genesis::genesis_instructions_json::instruction_value)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let before_sns = legacy.transactions()[before_transactions - 2]
                .instructions()
                .iter()
                .map(iroha_genesis::genesis_instructions_json::instruction_value)
                .collect::<Vec<_>>();
            let before_tail =
                grant_permissions(legacy.transactions()[before_transactions - 1].instructions());
            let expected_client = fixture_account();
            let spec = profile.spec();
            let (expected_universal, expected_private) =
                expected_target_permissions(spec).expect("expected permissions");

            assert!(
                !before_tail
                    .iter()
                    .any(|permission| permission.name() == "CanReadRestrictedDataspace"),
                "exact legacy input must not already contain the new restricted-read grant"
            );

            let migrated = migrate(legacy, profile).expect("migrate exact legacy manifest");
            assert_eq!(migrated.transactions().len(), before_transactions + 1);
            assert!(
                migrated.transactions().len() <= PROTOCOL_MAX_GENESIS_TRANSACTIONS,
                "migrated private genesis must remain signable under the protocol transaction limit"
            );
            let universal = &migrated.transactions()[before_transactions - 2];
            let private_sns = &migrated.transactions()[before_transactions - 1];
            let private = &migrated.transactions()[before_transactions];
            assert_eq!(
                universal.instructions().len(),
                expected_universal.len() + 2,
                "universal setup must contain the reserve, grants, and reader role"
            );
            assert_eq!(private_sns.instructions().len(), spec.domains.len() + 1);
            assert!(
                universal.instructions()[0]
                    .as_any()
                    .downcast_ref::<MintBox>()
                    .is_some()
            );
            assert!(private_sns.instructions().iter().all(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterSnsName>()
                    .is_some()
            }));
            let after_sns = universal.instructions()[..1]
                .iter()
                .chain(private_sns.instructions())
                .map(iroha_genesis::genesis_instructions_json::instruction_value)
                .collect::<Vec<_>>();
            assert_eq!(
                after_sns, before_sns,
                "migration must preserve the exact SNS instruction inventory and order while splitting its transaction boundary"
            );
            assert_eq!(
                grant_permissions(&universal.instructions()[1..universal.instructions().len() - 1]),
                expected_universal
            );
            let RegisterBox::Role(reader_role) = universal
                .instructions()
                .last()
                .expect("universal batch has role registration")
                .as_any()
                .downcast_ref::<RegisterBox>()
                .expect("universal batch ends with registration")
            else {
                panic!("universal batch must end with role registration");
            };
            let expected_read = Permission::from(CanReadRestrictedDataspace {
                dataspace: DataSpaceId::new(spec.dataspace_id),
            });
            assert_eq!(
                reader_role.object().inner().id,
                crate::genesis::private_dataspace_reader_role_id(
                    spec.alias,
                    DataSpaceId::new(spec.dataspace_id)
                )
            );
            assert_eq!(reader_role.object().grant_to(), &expected_client);
            assert_eq!(
                reader_role
                    .object()
                    .inner()
                    .permissions()
                    .cloned()
                    .collect::<Vec<_>>(),
                vec![expected_read.clone()]
            );
            assert_eq!(private.instructions().len(), expected_private.len() + 1);
            let RegisterBox::Account(register) = private.instructions()[0]
                .as_any()
                .downcast_ref::<RegisterBox>()
                .expect("private batch begins with registration")
            else {
                panic!("private batch must begin with account registration");
            };
            assert_eq!(register.object().id, expected_client);
            assert_eq!(register.object().metadata, Metadata::default());
            assert_eq!(register.object().label, None);
            assert_eq!(register.object().uaid, None);
            assert!(register.object().opaque_ids.is_empty());
            assert_eq!(
                grant_permissions(&private.instructions()[1..]),
                expected_private
            );

            let after_prefix = migrated.transactions()[..before_transactions - 2]
                .iter()
                .map(|transaction| {
                    transaction
                        .instructions()
                        .iter()
                        .map(iroha_genesis::genesis_instructions_json::instruction_value)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                after_prefix, before_prefix,
                "transactions before the legacy SNS and permission tail must remain unchanged"
            );
            let split_tail =
                grant_permissions(&universal.instructions()[1..universal.instructions().len() - 1])
                    .into_iter()
                    .chain(grant_permissions(&private.instructions()[1..]))
                    .collect::<Vec<_>>();
            assert_eq!(
                split_tail,
                expected_universal
                    .iter()
                    .chain(&expected_private)
                    .cloned()
                    .collect::<Vec<_>>(),
                "migrated permission ordering must exactly match the target profile"
            );
            let mut expected_inventory = before_tail.into_iter().collect::<BTreeSet<_>>();
            assert!(expected_inventory.insert(expected_read.clone()));
            let migrated_inventory = expected_universal
                .iter()
                .chain(&expected_private)
                .cloned()
                .collect::<BTreeSet<_>>();
            assert_eq!(
                migrated_inventory, expected_inventory,
                "migration must preserve every legacy permission and add only the exact restricted-read grant"
            );
            assert_eq!(
                expected_universal
                    .iter()
                    .filter(|permission| permission.name() == "CanReadRestrictedDataspace")
                    .collect::<Vec<_>>(),
                Vec::<&Permission>::new(),
                "target universal account grants must not duplicate the role-based read capability"
            );
            assert_eq!(
                expected_private
                    .iter()
                    .filter(|permission| permission.name() == "CanReadRestrictedDataspace")
                    .collect::<Vec<_>>(),
                vec![&expected_read],
                "target private batch must contain the exact direct grant for proxy re-authorization"
            );
        }
    }

    #[test]
    fn migration_preserves_the_protocol_transaction_limit() {
        for profile in [Profile::PrivateSbp, Profile::PrivateCbuae] {
            let retained = pad_legacy_manifest_to_transaction_count(legacy_manifest(profile), 15);
            let migrated = migrate(retained, profile)
                .expect("a retained 15-transaction manifest must migrate to exactly the limit");
            assert_eq!(
                migrated.transactions().len(),
                PROTOCOL_MAX_GENESIS_TRANSACTIONS
            );

            let already_at_limit =
                pad_legacy_manifest_to_transaction_count(legacy_manifest(profile), 16);
            let error = migrate(already_at_limit, profile)
                .expect_err("migration must reject output that would exceed the protocol limit");
            assert!(
                error.to_string().contains("exceeding the protocol maximum"),
                "unexpected error: {error:?}"
            );
        }
    }

    #[test]
    fn rejects_already_migrated_and_mismatched_legacy_tails() {
        let migrated = migrate(legacy_manifest(Profile::PrivateSbp), Profile::PrivateSbp)
            .expect("first migration succeeds");
        let error = migrate(migrated, Profile::PrivateSbp)
            .expect_err("already migrated manifest must fail closed");
        assert!(
            error.to_string().contains("legacy mixed permission tail"),
            "unexpected error: {error:?}"
        );

        let error = migrate(legacy_manifest(Profile::PrivateSbp), Profile::PrivateCbuae)
            .expect_err("wrong profile must fail closed");
        assert!(
            error.to_string().contains("legacy mixed permission tail"),
            "unexpected error: {error:?}"
        );

        let mut unexpected = legacy_manifest(Profile::PrivateSbp);
        let tail_index = unexpected.transactions().len() - 1;
        let mut wrong_order = unexpected.transactions()[tail_index]
            .instructions()
            .to_vec();
        let register_domain = wrong_order
            .pop()
            .expect("retained SBP tail ends with CanRegisterDomain");
        wrong_order.insert(1, register_domain);
        unexpected
            .replace_instruction_only_transaction(tail_index, vec![wrong_order])
            .expect("construct wrong-order fixture");
        let error = migrate(unexpected, Profile::PrivateSbp)
            .expect_err("pre-audit SBP permission order must fail closed");
        assert!(
            error
                .to_string()
                .contains("exact destination and permission order"),
            "unexpected error: {error:?}"
        );

        let mut pregranted = legacy_manifest(Profile::PrivateSbp);
        let tail_index = pregranted.transactions().len() - 1;
        let mut legacy_tail = pregranted.transactions()[tail_index]
            .instructions()
            .to_vec();
        legacy_tail.push(
            Grant::account_permission(
                CanReadRestrictedDataspace {
                    dataspace: DataSpaceId::new(PRIVATE_SBP_DATASPACE_ID),
                },
                fixture_account(),
            )
            .into(),
        );
        pregranted
            .replace_instruction_only_transaction(tail_index, vec![legacy_tail])
            .expect("construct pregranted fixture");
        let error = migrate(pregranted, Profile::PrivateSbp)
            .expect_err("legacy input already containing the restricted-read grant must fail");
        assert!(
            error.to_string().contains("must contain exactly"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn exact_deployment_argv_parses() {
        use clap::Parser as _;

        for (profile_literal, expected_profile) in [
            ("private-sbp", Profile::PrivateSbp),
            ("private-cbuae", Profile::PrivateCbuae),
        ] {
            let cli = crate::Cli::try_parse_from([
                "kagami",
                "--ui-mode",
                "plain",
                "genesis",
                "migrate-private-dataspace-bootstrap",
                "/retained/genesis.json",
                "--profile",
                profile_literal,
                "--out-file",
                "/candidate/genesis.json.tmp",
            ])
            .expect("deployment migration argv must parse");
            let crate::Command::Genesis(crate::genesis::Args::MigratePrivateDataspaceBootstrap(
                args,
            )) = cli.command
            else {
                panic!("deployment argv must select the retained-genesis migration command");
            };
            assert_eq!(args.genesis_file, PathBuf::from("/retained/genesis.json"));
            assert_eq!(args.profile, expected_profile);
            assert_eq!(args.out_file, PathBuf::from("/candidate/genesis.json.tmp"));
        }
    }

    #[test]
    fn cli_writes_canonical_raw_manifest_and_preserves_relative_paths() {
        let temp = tempdir().expect("create temp directory");
        let input = temp.path().join("retained.json");
        let output = temp.path().join("migrated.json");
        let source = legacy_manifest(Profile::PrivateCbuae);
        fs::write(
            &input,
            norito::json::to_json_pretty(&source).expect("serialize retained fixture"),
        )
        .expect("write retained fixture");

        let mut writer = BufWriter::new(Vec::new());
        Args {
            genesis_file: input,
            profile: Profile::PrivateCbuae,
            out_file: output.clone(),
        }
        .run(&mut writer)
        .expect("run migration command");
        writer.flush().expect("flush command output");

        let rendered = fs::read_to_string(&output).expect("read migrated fixture");
        assert_eq!(
            rendered,
            norito::json::to_json_pretty(
                &norito::json::from_str::<RawGenesisTransaction>(&rendered)
                    .expect("parse canonical migrated output")
            )
            .expect("re-render canonical migrated output")
        );
        assert!(
            rendered.contains("relative-ivm"),
            "raw relative IVM path must not be resolved by migration"
        );

        let mut sink = BufWriter::new(Vec::new());
        let error = Args {
            genesis_file: output.clone(),
            profile: Profile::PrivateCbuae,
            out_file: output,
        }
        .run(&mut sink)
        .expect_err("in-place migration must fail before any write");
        assert!(
            error
                .to_string()
                .contains("refusing to overwrite the retained genesis in place"),
            "unexpected error: {error:?}"
        );
    }
}

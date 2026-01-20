//! Experimental explorer DTOs used for future Torii app API endpoints.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use hex;
use iroha_core::state::WorldReadOnly;
use iroha_data_model::{
    HasMetadata, Identifiable,
    account::{AccountAddress, AccountEntry, AccountId},
    asset::{AssetDefinition, AssetDefinitionId, AssetEntry, AssetId, Mintable},
    block::SignedBlock,
    domain::{Domain, DomainId},
    isi::{
        self, CustomInstruction, ExecuteTrigger, GrantBox, Instruction as IsiInstruction,
        InstructionBox, Log, MintBox, RegisterBox, RemoveAssetKeyValue, RemoveKeyValueBox,
        RevokeBox, SetAssetKeyValue, SetKeyValueBox, SetParameter, TransferAssetBatch, TransferBox,
        UnregisterBox, Upgrade,
        mint_burn::BurnBox,
        runtime_upgrade::{ActivateRuntimeUpgrade, CancelRuntimeUpgrade, ProposeRuntimeUpgrade},
    },
    metadata::Metadata,
    nft::{NftEntry, NftId},
    peer::Peer,
    transaction::{
        error::TransactionRejectionReason,
        executable::Executable,
        signed::{SignedTransaction, TransactionResult},
    },
};
use mv::storage::StorageReadOnly;
use norito::{
    codec::Encode,
    json::{self, Map, Value},
};
use qrcode::{
    EcLevel, QrCode,
    render::svg,
    types::{QrError, Version},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    address_format::AddressFormatPreference,
    json_macros::{JsonDeserialize, JsonSerialize},
};

const ACCOUNT_QR_DIMENSION_PX: u32 = 192;
const ACCOUNT_QR_ERROR_CORRECTION: EcLevel = EcLevel::M;
const ACCOUNT_QR_ERROR_CORRECTION_LABEL: &str = "M";

#[derive(Debug, Clone, Default)]
pub(crate) struct ExplorerAggregates {
    account_counters: BTreeMap<AccountId, AccountCounters>,
    domain_counters: BTreeMap<DomainId, DomainCounters>,
    definition_instances: BTreeMap<AssetDefinitionId, u32>,
    definition_holders: BTreeMap<AssetDefinitionId, BTreeSet<AccountId>>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct AccountCounters {
    domains: u32,
    assets: u32,
    nfts: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DomainCounters {
    accounts: u32,
    assets: u32,
    nfts: u32,
}

impl ExplorerAggregates {
    pub(crate) fn build(world: &impl WorldReadOnly) -> Self {
        let mut agg = Self::default();

        for account in world.accounts_iter() {
            let entry = agg
                .domain_counters
                .entry(account.id().domain().clone())
                .or_default();
            entry.accounts = entry.accounts.saturating_add(1);
            agg.account_counters
                .entry(account.id().clone())
                .or_default();
        }

        for domain in world.domains_iter() {
            let entry = agg
                .account_counters
                .entry(domain.owned_by().clone())
                .or_default();
            entry.domains = entry.domains.saturating_add(1);
            agg.domain_counters.entry(domain.id().clone()).or_default();
        }

        for asset in world.assets_iter() {
            let account_id = asset.id().account().clone();
            let definition_id = asset.id().definition().clone();
            let domain_id = asset.id().definition().domain().clone();

            let account_entry = agg.account_counters.entry(account_id.clone()).or_default();
            account_entry.assets = account_entry.assets.saturating_add(1);

            let domain_entry = agg.domain_counters.entry(domain_id).or_default();
            domain_entry.assets = domain_entry.assets.saturating_add(1);

            *agg.definition_instances
                .entry(definition_id.clone())
                .or_default() += 1;
            agg.definition_holders
                .entry(definition_id)
                .or_default()
                .insert(account_id);
        }

        for (nft_id, nft) in world.nfts().iter() {
            let owner_entry = agg
                .account_counters
                .entry(nft.owned_by.clone())
                .or_default();
            owner_entry.nfts = owner_entry.nfts.saturating_add(1);

            let domain_entry = agg
                .domain_counters
                .entry(nft_id.domain().clone())
                .or_default();
            domain_entry.nfts = domain_entry.nfts.saturating_add(1);
        }

        agg
    }

    pub(crate) fn account_counters(&self, id: &AccountId) -> AccountCounters {
        self.account_counters.get(id).copied().unwrap_or_default()
    }

    pub(crate) fn domain_counters(&self, id: &DomainId) -> DomainCounters {
        self.domain_counters.get(id).copied().unwrap_or_default()
    }

    pub(crate) fn definition_instance_count(&self, id: &AssetDefinitionId) -> u32 {
        self.definition_instances.get(id).copied().unwrap_or(0)
    }

    pub(crate) fn account_holds_definition(
        &self,
        definition: &AssetDefinitionId,
        account: &AccountId,
    ) -> bool {
        self.definition_holders
            .get(definition)
            .map_or(false, |holders| holders.contains(account))
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub(crate) struct ExplorerPaginationQuery {
    #[norito(default = "default_page")]
    pub page: u64,
    #[norito(default = "default_per_page")]
    pub per_page: u64,
    #[norito(default)]
    pub address_format: Option<String>,
}

#[derive(Clone, Debug, Default, JsonSerialize, JsonDeserialize)]
pub(crate) struct ExplorerAddressFormatQuery {
    #[norito(default)]
    pub address_format: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerPaginationMeta {
    pub page: u64,
    pub per_page: u64,
    pub total_pages: u64,
    pub total_items: u64,
}

impl ExplorerPaginationQuery {
    #[cfg(feature = "app_api")]
    pub fn address_format_pref(
        &self,
    ) -> crate::Result<crate::address_format::AddressFormatPreference> {
        crate::address_format::AddressFormatPreference::from_param(self.address_format.as_deref())
    }
}

impl ExplorerAddressFormatQuery {
    #[cfg(feature = "app_api")]
    pub fn address_format_pref(
        &self,
    ) -> crate::Result<crate::address_format::AddressFormatPreference> {
        crate::address_format::AddressFormatPreference::from_param(self.address_format.as_deref())
    }
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerAccountDto {
    pub id: String,
    pub ih58_address: String,
    pub compressed_address: String,
    pub network_prefix: u16,
    pub metadata: Value,
    pub owned_domains: u32,
    pub owned_assets: u32,
    pub owned_nfts: u32,
}

impl ExplorerAccountDto {
    pub(crate) fn from_entry(entry: AccountEntry<'_>, counts: AccountCounters) -> Self {
        let network_prefix = iroha_data_model::account::address::chain_discriminant();
        let address =
            AccountAddress::from_account_id(entry.id()).expect("account ids are always valid");
        Self {
            id: entry.id().to_string(),
            ih58_address: address
                .to_ih58(network_prefix)
                .unwrap_or_else(|_| entry.id().to_string()),
            compressed_address: address
                .to_compressed_sora()
                .unwrap_or_else(|_| entry.id().to_string()),
            network_prefix,
            metadata: metadata_to_json(entry.value().metadata()),
            owned_domains: counts.domains,
            owned_assets: counts.assets,
            owned_nfts: counts.nfts,
        }
    }
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerAccountsPage {
    pub pagination: ExplorerPaginationMeta,
    pub items: Vec<ExplorerAccountDto>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerAccountQrDto {
    pub canonical_id: String,
    pub literal: String,
    pub address_format: &'static str,
    pub network_prefix: u16,
    pub error_correction: &'static str,
    pub modules: u32,
    pub qr_version: u8,
    pub svg: String,
}

impl ExplorerAccountQrDto {
    pub(crate) fn build(
        account_id: &AccountId,
        preference: AddressFormatPreference,
    ) -> Result<Self, QrError> {
        let network_prefix = iroha_data_model::account::address::chain_discriminant();
        let literal = preference.display_literal(account_id);
        let (svg, qr_version) = render_account_qr_svg(&literal)?;
        Ok(Self {
            canonical_id: account_id.to_string(),
            literal,
            address_format: preference.metric_label(),
            network_prefix,
            error_correction: ACCOUNT_QR_ERROR_CORRECTION_LABEL,
            modules: ACCOUNT_QR_DIMENSION_PX,
            qr_version,
            svg,
        })
    }
}

fn render_account_qr_svg(input: &str) -> Result<(String, u8), QrError> {
    let code = QrCode::with_error_correction_level(input.as_bytes(), ACCOUNT_QR_ERROR_CORRECTION)?;
    let version = match code.version() {
        Version::Normal(n) | Version::Micro(n) => {
            u8::try_from(n).expect("QR versions fit in u8 range")
        }
    };
    let svg = code
        .render::<svg::Color>()
        .min_dimensions(ACCOUNT_QR_DIMENSION_PX, ACCOUNT_QR_DIMENSION_PX)
        .max_dimensions(ACCOUNT_QR_DIMENSION_PX, ACCOUNT_QR_DIMENSION_PX)
        .build();
    let svg = {
        let trimmed = svg.trim_start();
        if trimmed.starts_with("<?xml") {
            let after_decl = trimmed
                .find("?>")
                .map(|idx| &trimmed[idx + 2..])
                .unwrap_or(trimmed);
            after_decl.trim_start().to_owned()
        } else {
            trimmed.to_owned()
        }
    };
    Ok((svg, version))
}

pub(crate) fn paginate<T>(
    mut items: Vec<T>,
    page: u64,
    per_page: u64,
) -> (Vec<T>, ExplorerPaginationMeta) {
    let per_page = per_page.max(1);
    let total_items = items.len() as u64;
    let total_pages = total_items.div_ceil(per_page);
    let start = (page.saturating_sub(1))
        .saturating_mul(per_page)
        .min(total_items) as usize;
    if start > 0 {
        items.drain(0..start);
    }
    if items.len() > per_page as usize {
        items.truncate(per_page as usize);
    }
    (
        items,
        ExplorerPaginationMeta {
            page,
            per_page,
            total_pages,
            total_items,
        },
    )
}

pub(crate) fn metadata_to_json(metadata: &Metadata) -> Value {
    norito::json::to_value(metadata).unwrap_or_else(|_| Value::Object(Map::new()))
}

const fn default_page() -> u64 {
    1
}

const fn default_per_page() -> u64 {
    10
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerDomainDto {
    pub id: String,
    pub logo: Option<String>,
    pub metadata: Value,
    pub owned_by: String,
    pub accounts: u32,
    pub assets: u32,
    pub nfts: u32,
}

impl ExplorerDomainDto {
    pub(crate) fn from_domain(domain: &Domain, counts: DomainCounters) -> Self {
        Self {
            id: domain.id().to_string(),
            logo: domain.logo().as_ref().map(ToString::to_string),
            metadata: metadata_to_json(domain.metadata()),
            owned_by: domain.owned_by().to_string(),
            accounts: counts.accounts,
            assets: counts.assets,
            nfts: counts.nfts,
        }
    }
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerDomainsPage {
    pub pagination: ExplorerPaginationMeta,
    pub items: Vec<ExplorerDomainDto>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerAssetDefinitionDto {
    pub id: String,
    pub mintable: String,
    pub logo: Option<String>,
    pub metadata: Value,
    pub owned_by: String,
    pub assets: u32,
}

impl ExplorerAssetDefinitionDto {
    pub(crate) fn from_definition(
        definition: &AssetDefinition,
        aggregates: &ExplorerAggregates,
    ) -> Self {
        Self {
            id: definition.id().to_string(),
            mintable: mintable_label(definition.mintable()),
            logo: definition.logo().as_ref().map(ToString::to_string),
            metadata: metadata_to_json(definition.metadata()),
            owned_by: definition.owned_by().to_string(),
            assets: aggregates.definition_instance_count(definition.id()),
        }
    }
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerAssetDefinitionsPage {
    pub pagination: ExplorerPaginationMeta,
    pub items: Vec<ExplorerAssetDefinitionDto>,
}

fn mintable_label(mintable: Mintable) -> String {
    match mintable {
        Mintable::Infinitely => "Infinitely".to_string(),
        Mintable::Once => "Once".to_string(),
        Mintable::Not => "Not".to_string(),
        Mintable::Limited(tokens) => format!("Limited({})", tokens.value()),
    }
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerAssetDto {
    pub id: String,
    pub definition_id: String,
    pub account_id: String,
    pub value: String,
}

impl ExplorerAssetDto {
    pub(crate) fn from_entry(entry: AssetEntry<'_>) -> Self {
        Self {
            id: entry.id().to_string(),
            definition_id: entry.id().definition().to_string(),
            account_id: entry.id().account().to_string(),
            value: entry.value().as_ref().to_string(),
        }
    }
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerAssetsPage {
    pub pagination: ExplorerPaginationMeta,
    pub items: Vec<ExplorerAssetDto>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerNftDto {
    pub id: String,
    pub owned_by: String,
    pub metadata: Value,
}

impl ExplorerNftDto {
    pub(crate) fn from_entry(entry: NftEntry<'_>) -> Self {
        Self {
            id: entry.id().to_string(),
            owned_by: entry.value().owned_by.to_string(),
            metadata: metadata_to_json(&entry.value().content),
        }
    }
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerNftsPage {
    pub pagination: ExplorerPaginationMeta,
    pub items: Vec<ExplorerNftDto>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerBlockDto {
    pub hash: String,
    pub height: u64,
    pub created_at: String,
    pub prev_block_hash: Option<String>,
    pub transactions_hash: Option<String>,
    pub transactions_rejected: u32,
    pub transactions_total: u32,
}

impl ExplorerBlockDto {
    pub(crate) fn from_block(block: &SignedBlock) -> Self {
        let header = block.header();
        let external_total = block.external_transactions().len();
        Self {
            hash: block.hash().to_string(),
            height: header.height().get(),
            created_at: block_created_at(header.creation_time()),
            prev_block_hash: header.prev_block_hash().map(|hash| hash.to_string()),
            transactions_hash: header.merkle_root().map(|hash| hash.to_string()),
            transactions_rejected: count_rejected_transactions(block, external_total),
            transactions_total: saturating_usize_to_u32(external_total),
        }
    }
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerBlocksPage {
    pub pagination: ExplorerPaginationMeta,
    pub items: Vec<ExplorerBlockDto>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerNetworkMetricsDto {
    pub peers: u64,
    pub domains: u64,
    pub accounts: u64,
    pub assets: u64,
    pub transactions_accepted: u64,
    pub transactions_rejected: u64,
    pub block: u64,
    pub block_created_at: Option<String>,
    pub finalized_block: u64,
    pub avg_commit_time: Option<ExplorerDurationDto>,
    pub avg_block_time: Option<ExplorerDurationDto>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerTransactionDto {
    pub authority: String,
    pub hash: String,
    pub block: u64,
    pub created_at: String,
    pub executable: String,
    pub status: String,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerTransactionDetailDto {
    pub authority: String,
    pub hash: String,
    pub block: u64,
    pub created_at: String,
    pub executable: String,
    pub status: String,
    pub rejection_reason: Option<ExplorerTransactionRejectionDto>,
    pub metadata: Value,
    pub nonce: Option<u64>,
    pub signature: String,
    pub time_to_live: Option<ExplorerDurationDto>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerTransactionRejectionDto {
    pub scale: String,
    pub json: Value,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerDurationDto {
    pub ms: u64,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerTransactionsPage {
    pub pagination: ExplorerPaginationMeta,
    pub items: Vec<ExplorerTransactionDto>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExplorerInstructionKind {
    Register,
    Unregister,
    Mint,
    Burn,
    Transfer,
    SetKeyValue,
    RemoveKeyValue,
    Grant,
    Revoke,
    ExecuteTrigger,
    SetParameter,
    Upgrade,
    Log,
    Custom,
}

impl ExplorerInstructionKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Register => "Register",
            Self::Unregister => "Unregister",
            Self::Mint => "Mint",
            Self::Burn => "Burn",
            Self::Transfer => "Transfer",
            Self::SetKeyValue => "SetKeyValue",
            Self::RemoveKeyValue => "RemoveKeyValue",
            Self::Grant => "Grant",
            Self::Revoke => "Revoke",
            Self::ExecuteTrigger => "ExecuteTrigger",
            Self::SetParameter => "SetParameter",
            Self::Upgrade => "Upgrade",
            Self::Log => "Log",
            Self::Custom => "Custom",
        }
    }
}

impl std::str::FromStr for ExplorerInstructionKind {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "register" => Ok(Self::Register),
            "unregister" => Ok(Self::Unregister),
            "mint" => Ok(Self::Mint),
            "burn" => Ok(Self::Burn),
            "transfer" => Ok(Self::Transfer),
            "setkeyvalue" | "set_key_value" => Ok(Self::SetKeyValue),
            "removekeyvalue" | "remove_key_value" => Ok(Self::RemoveKeyValue),
            "grant" => Ok(Self::Grant),
            "revoke" => Ok(Self::Revoke),
            "executetrigger" | "execute_trigger" => Ok(Self::ExecuteTrigger),
            "setparameter" | "set_parameter" => Ok(Self::SetParameter),
            "upgrade" => Ok(Self::Upgrade),
            "log" => Ok(Self::Log),
            "custom" => Ok(Self::Custom),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerInstructionBoxDto {
    pub scale: String,
    pub json: Value,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerInstructionDto {
    pub authority: String,
    pub created_at: String,
    pub kind: String,
    pub r#box: ExplorerInstructionBoxDto,
    pub transaction_hash: String,
    pub transaction_status: String,
    pub block: u64,
    pub index: u32,
}

#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct ExplorerInstructionsPage {
    pub pagination: ExplorerPaginationMeta,
    pub items: Vec<ExplorerInstructionDto>,
}

pub(crate) fn instruction_kind(instruction: &InstructionBox) -> ExplorerInstructionKind {
    let wire_id = IsiInstruction::id(&**instruction);
    match wire_id {
        id if id == RegisterBox::WIRE_ID => ExplorerInstructionKind::Register,
        id if id == UnregisterBox::WIRE_ID => ExplorerInstructionKind::Unregister,
        id if id == MintBox::WIRE_ID => ExplorerInstructionKind::Mint,
        id if id == BurnBox::WIRE_ID => ExplorerInstructionKind::Burn,
        id if id == TransferBox::WIRE_ID || id == TransferAssetBatch::WIRE_ID => {
            ExplorerInstructionKind::Transfer
        }
        id if id == SetKeyValueBox::WIRE_ID => ExplorerInstructionKind::SetKeyValue,
        id if id == RemoveKeyValueBox::WIRE_ID => ExplorerInstructionKind::RemoveKeyValue,
        id if id == GrantBox::WIRE_ID => ExplorerInstructionKind::Grant,
        id if id == RevokeBox::WIRE_ID => ExplorerInstructionKind::Revoke,
        id if id == ExecuteTrigger::WIRE_ID => ExplorerInstructionKind::ExecuteTrigger,
        id if id == SetParameter::WIRE_ID => ExplorerInstructionKind::SetParameter,
        id if id == Upgrade::WIRE_ID
            || id == ProposeRuntimeUpgrade::WIRE_ID
            || id == ActivateRuntimeUpgrade::WIRE_ID
            || id == CancelRuntimeUpgrade::WIRE_ID =>
        {
            ExplorerInstructionKind::Upgrade
        }
        id if id == Log::WIRE_ID => ExplorerInstructionKind::Log,
        id if id == CustomInstruction::WIRE_ID => ExplorerInstructionKind::Custom,
        _ => {
            let any = (**instruction).as_any();
            if any.downcast_ref::<RegisterBox>().is_some() {
                ExplorerInstructionKind::Register
            } else if any.downcast_ref::<UnregisterBox>().is_some() {
                ExplorerInstructionKind::Unregister
            } else if any.downcast_ref::<MintBox>().is_some() {
                ExplorerInstructionKind::Mint
            } else if any.downcast_ref::<BurnBox>().is_some() {
                ExplorerInstructionKind::Burn
            } else if any.downcast_ref::<TransferBox>().is_some()
                || any.downcast_ref::<TransferAssetBatch>().is_some()
            {
                ExplorerInstructionKind::Transfer
            } else if any.downcast_ref::<SetKeyValueBox>().is_some()
                || any.downcast_ref::<SetAssetKeyValue>().is_some()
            {
                ExplorerInstructionKind::SetKeyValue
            } else if any.downcast_ref::<RemoveKeyValueBox>().is_some()
                || any.downcast_ref::<RemoveAssetKeyValue>().is_some()
            {
                ExplorerInstructionKind::RemoveKeyValue
            } else if any.downcast_ref::<GrantBox>().is_some() {
                ExplorerInstructionKind::Grant
            } else if any.downcast_ref::<RevokeBox>().is_some() {
                ExplorerInstructionKind::Revoke
            } else if any.downcast_ref::<ExecuteTrigger>().is_some() {
                ExplorerInstructionKind::ExecuteTrigger
            } else if any.downcast_ref::<SetParameter>().is_some() {
                ExplorerInstructionKind::SetParameter
            } else if any.downcast_ref::<Upgrade>().is_some()
                || any.downcast_ref::<ProposeRuntimeUpgrade>().is_some()
                || any.downcast_ref::<ActivateRuntimeUpgrade>().is_some()
                || any.downcast_ref::<CancelRuntimeUpgrade>().is_some()
            {
                ExplorerInstructionKind::Upgrade
            } else if any.downcast_ref::<Log>().is_some() {
                ExplorerInstructionKind::Log
            } else {
                ExplorerInstructionKind::Custom
            }
        }
    }
}

pub(crate) fn instruction_dto_with_kind(
    tx: &SignedTransaction,
    block_height: u64,
    result: &TransactionResult,
    instruction: &InstructionBox,
    kind: ExplorerInstructionKind,
    index: u32,
    address_format: AddressFormatPreference,
) -> ExplorerInstructionDto {
    ExplorerInstructionDto {
        authority: address_format.display_literal(tx.authority()),
        created_at: duration_to_rfc3339(tx.creation_time()),
        kind: kind.as_str().to_string(),
        r#box: instruction_box_dto(instruction, kind),
        transaction_hash: tx.hash_as_entrypoint().to_string(),
        transaction_status: transaction_status_label(result).to_string(),
        block: block_height,
        index,
    }
}

fn instruction_box_dto(
    instruction: &InstructionBox,
    kind: ExplorerInstructionKind,
) -> ExplorerInstructionBoxDto {
    ExplorerInstructionBoxDto {
        scale: instruction_scale_hex(instruction),
        json: instruction_json_payload(instruction, kind),
    }
}

fn instruction_scale_hex(instruction: &InstructionBox) -> String {
    let bytes = instruction.dyn_encode();
    format!("0x{}", hex::encode(bytes))
}

fn instruction_json_payload(instruction: &InstructionBox, kind: ExplorerInstructionKind) -> Value {
    let mut map = Map::new();
    map.insert("kind".to_string(), Value::String(kind.as_str().to_string()));
    map.insert(
        "payload".to_string(),
        structured_instruction_payload(instruction, kind),
    );
    map.insert(
        "wire_id".to_string(),
        Value::String(IsiInstruction::id(&**instruction).to_string()),
    );
    map.insert(
        "encoded".to_string(),
        Value::String(hex::encode(instruction.dyn_encode())),
    );
    Value::Object(map)
}

fn structured_instruction_payload(
    instruction: &InstructionBox,
    kind: ExplorerInstructionKind,
) -> Value {
    match kind {
        ExplorerInstructionKind::Register => register_payload(instruction),
        ExplorerInstructionKind::Unregister => unregister_payload(instruction),
        ExplorerInstructionKind::Mint => mint_payload(instruction),
        ExplorerInstructionKind::Burn => burn_payload(instruction),
        ExplorerInstructionKind::Transfer => transfer_payload(instruction),
        ExplorerInstructionKind::SetKeyValue => set_key_value_payload(instruction),
        ExplorerInstructionKind::RemoveKeyValue => remove_key_value_payload(instruction),
        ExplorerInstructionKind::Grant => grant_payload(instruction),
        ExplorerInstructionKind::Revoke => revoke_payload(instruction),
        ExplorerInstructionKind::ExecuteTrigger => execute_trigger_payload(instruction),
        ExplorerInstructionKind::SetParameter => set_parameter_payload(instruction),
        ExplorerInstructionKind::Upgrade => upgrade_payload(instruction),
        ExplorerInstructionKind::Log => log_payload(instruction),
        ExplorerInstructionKind::Custom => custom_payload(instruction),
    }
    .unwrap_or_else(|| fallback_structured_payload(instruction))
}

fn fallback_instruction_payload(instruction: &InstructionBox) -> Value {
    let mut object = Map::new();
    object.insert(
        "wire_id".to_string(),
        Value::String(IsiInstruction::id(&**instruction).to_string()),
    );
    object.insert(
        "encoded".to_string(),
        Value::String(hex::encode(instruction.dyn_encode())),
    );
    Value::Object(object)
}

fn fallback_structured_payload(instruction: &InstructionBox) -> Value {
    instruction_variant_value("Unknown", fallback_instruction_payload(instruction))
}

fn register_payload(instruction: &InstructionBox) -> Option<Value> {
    let register = instruction.as_any().downcast_ref::<RegisterBox>()?;
    let (variant, value) = match register {
        RegisterBox::Peer(inner) => ("Peer", json::to_value(inner).ok()?),
        RegisterBox::Domain(inner) => ("Domain", json::to_value(inner).ok()?),
        RegisterBox::Account(inner) => ("Account", json::to_value(inner).ok()?),
        RegisterBox::AssetDefinition(inner) => ("AssetDefinition", json::to_value(inner).ok()?),
        RegisterBox::Nft(inner) => ("Nft", json::to_value(inner).ok()?),
        RegisterBox::Role(inner) => ("Role", json::to_value(inner).ok()?),
        RegisterBox::Trigger(inner) => ("Trigger", json::to_value(inner).ok()?),
    };
    Some(instruction_variant_value(variant, value))
}

fn unregister_payload(instruction: &InstructionBox) -> Option<Value> {
    let unregister = instruction.as_any().downcast_ref::<UnregisterBox>()?;
    let (variant, value) = match unregister {
        UnregisterBox::Peer(inner) => ("Peer", json::to_value(inner).ok()?),
        UnregisterBox::Domain(inner) => ("Domain", json::to_value(inner).ok()?),
        UnregisterBox::Account(inner) => ("Account", json::to_value(inner).ok()?),
        UnregisterBox::AssetDefinition(inner) => ("AssetDefinition", json::to_value(inner).ok()?),
        UnregisterBox::Nft(inner) => ("Nft", json::to_value(inner).ok()?),
        UnregisterBox::Role(inner) => ("Role", json::to_value(inner).ok()?),
        UnregisterBox::Trigger(inner) => ("Trigger", json::to_value(inner).ok()?),
    };
    Some(instruction_variant_value(variant, value))
}

fn mint_payload(instruction: &InstructionBox) -> Option<Value> {
    let mint = instruction.as_any().downcast_ref::<MintBox>()?;
    let (variant, value) = match mint {
        MintBox::Asset(inner) => ("Asset", json::to_value(inner).ok()?),
        MintBox::TriggerRepetitions(inner) => ("TriggerRepetitions", json::to_value(inner).ok()?),
    };
    Some(instruction_variant_value(variant, value))
}

fn burn_payload(instruction: &InstructionBox) -> Option<Value> {
    let burn = instruction.as_any().downcast_ref::<BurnBox>()?;
    let (variant, value) = match burn {
        BurnBox::Asset(inner) => ("Asset", json::to_value(inner).ok()?),
        BurnBox::TriggerRepetitions(inner) => ("TriggerRepetitions", json::to_value(inner).ok()?),
    };
    Some(instruction_variant_value(variant, value))
}

fn transfer_payload(instruction: &InstructionBox) -> Option<Value> {
    if let Some(batch) = instruction.as_any().downcast_ref::<TransferAssetBatch>() {
        let value = json::to_value(batch).ok()?;
        return Some(instruction_variant_value("AssetBatch", value));
    }
    let transfer = instruction.as_any().downcast_ref::<TransferBox>()?;
    let (variant, value) = match transfer {
        TransferBox::Domain(inner) => ("Domain", json::to_value(inner).ok()?),
        TransferBox::AssetDefinition(inner) => ("AssetDefinition", json::to_value(inner).ok()?),
        TransferBox::Asset(inner) => ("Asset", json::to_value(inner).ok()?),
        TransferBox::Nft(inner) => ("Nft", json::to_value(inner).ok()?),
    };
    Some(instruction_variant_value(variant, value))
}

fn set_key_value_payload(instruction: &InstructionBox) -> Option<Value> {
    if let Some(asset) = instruction.as_any().downcast_ref::<SetAssetKeyValue>() {
        let value = json::to_value(asset).ok()?;
        return Some(instruction_variant_value("Asset", value));
    }
    let setter = instruction.as_any().downcast_ref::<SetKeyValueBox>()?;
    let (variant, value) = match setter {
        SetKeyValueBox::Domain(inner) => ("Domain", json::to_value(inner).ok()?),
        SetKeyValueBox::Account(inner) => ("Account", json::to_value(inner).ok()?),
        SetKeyValueBox::AssetDefinition(inner) => ("AssetDefinition", json::to_value(inner).ok()?),
        SetKeyValueBox::Nft(inner) => ("Nft", json::to_value(inner).ok()?),
        SetKeyValueBox::Trigger(inner) => ("Trigger", json::to_value(inner).ok()?),
    };
    Some(instruction_variant_value(variant, value))
}

fn remove_key_value_payload(instruction: &InstructionBox) -> Option<Value> {
    if let Some(asset) = instruction.as_any().downcast_ref::<RemoveAssetKeyValue>() {
        let value = json::to_value(asset).ok()?;
        return Some(instruction_variant_value("Asset", value));
    }
    let remover = instruction.as_any().downcast_ref::<RemoveKeyValueBox>()?;
    let (variant, value) = match remover {
        RemoveKeyValueBox::Domain(inner) => ("Domain", json::to_value(inner).ok()?),
        RemoveKeyValueBox::Account(inner) => ("Account", json::to_value(inner).ok()?),
        RemoveKeyValueBox::AssetDefinition(inner) => {
            ("AssetDefinition", json::to_value(inner).ok()?)
        }
        RemoveKeyValueBox::Nft(inner) => ("Nft", json::to_value(inner).ok()?),
        RemoveKeyValueBox::Trigger(inner) => ("Trigger", json::to_value(inner).ok()?),
    };
    Some(instruction_variant_value(variant, value))
}

fn grant_payload(instruction: &InstructionBox) -> Option<Value> {
    let grant = instruction.as_any().downcast_ref::<GrantBox>()?;
    let (variant, value) = match grant {
        GrantBox::Permission(inner) => ("PermissionToAccount", json::to_value(inner).ok()?),
        GrantBox::Role(inner) => ("RoleToAccount", json::to_value(inner).ok()?),
        GrantBox::RolePermission(inner) => ("PermissionToRole", json::to_value(inner).ok()?),
    };
    Some(instruction_variant_value(variant, value))
}

fn revoke_payload(instruction: &InstructionBox) -> Option<Value> {
    let revoke = instruction.as_any().downcast_ref::<RevokeBox>()?;
    let (variant, value) = match revoke {
        RevokeBox::Permission(inner) => ("PermissionFromAccount", json::to_value(inner).ok()?),
        RevokeBox::Role(inner) => ("RoleFromAccount", json::to_value(inner).ok()?),
        RevokeBox::RolePermission(inner) => ("PermissionFromRole", json::to_value(inner).ok()?),
    };
    Some(instruction_variant_value(variant, value))
}

fn execute_trigger_payload(instruction: &InstructionBox) -> Option<Value> {
    let exec = instruction.as_any().downcast_ref::<ExecuteTrigger>()?;
    let value = json::to_value(exec).ok()?;
    Some(instruction_variant_value("ExecuteTrigger", value))
}

fn set_parameter_payload(instruction: &InstructionBox) -> Option<Value> {
    let parameter = instruction.as_any().downcast_ref::<SetParameter>()?;
    let value = json::to_value(parameter).ok()?;
    Some(instruction_variant_value("SetParameter", value))
}

fn upgrade_payload(instruction: &InstructionBox) -> Option<Value> {
    if let Some(propose) = instruction.as_any().downcast_ref::<ProposeRuntimeUpgrade>() {
        let value = json::to_value(propose).ok()?;
        return Some(instruction_variant_value("ProposeRuntimeUpgrade", value));
    }
    if let Some(activate) = instruction
        .as_any()
        .downcast_ref::<ActivateRuntimeUpgrade>()
    {
        let value = json::to_value(activate).ok()?;
        return Some(instruction_variant_value("ActivateRuntimeUpgrade", value));
    }
    if let Some(cancel) = instruction.as_any().downcast_ref::<CancelRuntimeUpgrade>() {
        let value = json::to_value(cancel).ok()?;
        return Some(instruction_variant_value("CancelRuntimeUpgrade", value));
    }
    let upgrade = instruction.as_any().downcast_ref::<Upgrade>()?;
    let value = json::to_value(upgrade).ok()?;
    Some(instruction_variant_value("Upgrade", value))
}

fn log_payload(instruction: &InstructionBox) -> Option<Value> {
    let log = instruction.as_any().downcast_ref::<Log>()?;
    let value = json::to_value(log).ok()?;
    Some(instruction_variant_value("Log", value))
}

fn custom_payload(instruction: &InstructionBox) -> Option<Value> {
    let custom = instruction.as_any().downcast_ref::<CustomInstruction>()?;
    let parsed = json::parse_value(custom.payload.get())
        .unwrap_or_else(|_| Value::String(custom.payload.get().clone()));
    Some(instruction_variant_value("Custom", parsed))
}

fn instruction_variant_value(variant: &str, value: Value) -> Value {
    let mut map = Map::new();
    map.insert("variant".to_string(), Value::String(variant.to_string()));
    map.insert("value".to_string(), value);
    Value::Object(map)
}

fn encode_scale_hex<T: Encode>(value: &T) -> String {
    let bytes = Encode::encode(value);
    format!("0x{}", hex::encode(bytes))
}

fn executable_label(executable: &Executable) -> &'static str {
    match executable {
        Executable::Instructions(_) => "Instructions",
        Executable::Ivm(_) => "Wasm",
    }
}

fn transaction_status_label(result: &TransactionResult) -> &'static str {
    if result.as_ref().is_ok() {
        "Committed"
    } else {
        "Rejected"
    }
}

fn duration_to_rfc3339(duration: Duration) -> String {
    const FALLBACK: &str = "1970-01-01T00:00:00Z";
    let nanos = i128::from(duration.as_secs())
        .saturating_mul(1_000_000_000)
        .saturating_add(i128::from(duration.subsec_nanos()));
    OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .format(&Rfc3339)
        .unwrap_or_else(|_| FALLBACK.to_string())
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn ttl_to_dto(ttl: Option<Duration>) -> Option<ExplorerDurationDto> {
    ttl.map(|value| ExplorerDurationDto {
        ms: duration_ms(value),
    })
}

pub(crate) fn transaction_summary_dto(
    tx: &SignedTransaction,
    block_height: u64,
    result: &TransactionResult,
    address_format: AddressFormatPreference,
) -> ExplorerTransactionDto {
    ExplorerTransactionDto {
        authority: address_format.display_literal(tx.authority()),
        hash: tx.hash_as_entrypoint().to_string(),
        block: block_height,
        created_at: duration_to_rfc3339(tx.creation_time()),
        executable: executable_label(tx.instructions()).to_string(),
        status: transaction_status_label(result).to_string(),
    }
}

pub(crate) fn transaction_detail_dto(
    tx: &SignedTransaction,
    block_height: u64,
    result: &TransactionResult,
    address_format: AddressFormatPreference,
) -> ExplorerTransactionDetailDto {
    ExplorerTransactionDetailDto {
        authority: address_format.display_literal(tx.authority()),
        hash: tx.hash_as_entrypoint().to_string(),
        block: block_height,
        created_at: duration_to_rfc3339(tx.creation_time()),
        executable: executable_label(tx.instructions()).to_string(),
        status: transaction_status_label(result).to_string(),
        rejection_reason: result
            .as_ref()
            .err()
            .map(|reason| ExplorerTransactionRejectionDto {
                scale: encode_scale_hex(reason),
                json: norito::json::to_value(reason).unwrap_or(Value::Null),
            }),
        metadata: metadata_to_json(tx.metadata()),
        nonce: tx.nonce().map(|nonce| nonce.get().into()),
        signature: hex::encode(tx.signature().payload().payload()),
        time_to_live: ttl_to_dto(tx.time_to_live()),
    }
}

pub(crate) fn accounts_page<'world, I>(
    accounts: I,
    aggregates: &ExplorerAggregates,
    domain_filter: Option<&DomainId>,
    definition_filter: Option<&AssetDefinitionId>,
    page: u64,
    per_page: u64,
) -> ExplorerAccountsPage
where
    I: IntoIterator<Item = AccountEntry<'world>>,
{
    let mut items = Vec::new();
    for entry in accounts {
        if let Some(domain) = domain_filter {
            if entry.id().domain() != domain {
                continue;
            }
        }
        if let Some(definition) = definition_filter {
            if !aggregates.account_holds_definition(definition, entry.id()) {
                continue;
            }
        }
        let counts = aggregates.account_counters(entry.id());
        items.push(ExplorerAccountDto::from_entry(entry, counts));
    }
    items.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));
    let (items, pagination) = paginate(items, page, per_page);
    ExplorerAccountsPage { pagination, items }
}

pub(crate) fn domains_page<'world, I>(
    domains: I,
    aggregates: &ExplorerAggregates,
    owned_by: Option<&AccountId>,
    page: u64,
    per_page: u64,
) -> ExplorerDomainsPage
where
    I: IntoIterator<Item = &'world Domain>,
{
    let mut items = Vec::new();
    for domain in domains {
        if let Some(owner) = owned_by {
            if domain.owned_by() != owner {
                continue;
            }
        }
        let counts = aggregates.domain_counters(domain.id());
        items.push(ExplorerDomainDto::from_domain(domain, counts));
    }
    items.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));
    let (items, pagination) = paginate(items, page, per_page);
    ExplorerDomainsPage { pagination, items }
}

pub(crate) fn asset_definitions_page<'world, I>(
    definitions: I,
    aggregates: &ExplorerAggregates,
    domain_filter: Option<&DomainId>,
    owner_filter: Option<&AccountId>,
    page: u64,
    per_page: u64,
) -> ExplorerAssetDefinitionsPage
where
    I: IntoIterator<Item = &'world AssetDefinition>,
{
    let mut items = Vec::new();
    for definition in definitions {
        if let Some(domain) = domain_filter {
            if definition.id().domain() != domain {
                continue;
            }
        }
        if let Some(owner) = owner_filter {
            if definition.owned_by() != owner {
                continue;
            }
        }
        items.push(ExplorerAssetDefinitionDto::from_definition(
            definition, aggregates,
        ));
    }
    items.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));
    let (items, pagination) = paginate(items, page, per_page);
    ExplorerAssetDefinitionsPage { pagination, items }
}

pub(crate) fn assets_page<'world, I>(
    assets: I,
    owned_by: Option<&AccountId>,
    definition_filter: Option<&AssetDefinitionId>,
    page: u64,
    per_page: u64,
) -> ExplorerAssetsPage
where
    I: IntoIterator<Item = AssetEntry<'world>>,
{
    let mut items = Vec::new();
    for asset in assets {
        if let Some(account_id) = owned_by {
            if asset.id().account() != account_id {
                continue;
            }
        }
        if let Some(definition_id) = definition_filter {
            if asset.id().definition() != definition_id {
                continue;
            }
        }
        items.push(ExplorerAssetDto::from_entry(asset));
    }
    items.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));
    let (items, pagination) = paginate(items, page, per_page);
    ExplorerAssetsPage { pagination, items }
}

pub(crate) fn nfts_page<'world, I>(
    nfts: I,
    owned_by: Option<&AccountId>,
    domain_filter: Option<&DomainId>,
    page: u64,
    per_page: u64,
) -> ExplorerNftsPage
where
    I: IntoIterator<Item = NftEntry<'world>>,
{
    let mut items = Vec::new();
    for nft in nfts {
        if let Some(owner) = owned_by {
            if nft.value().owned_by != *owner {
                continue;
            }
        }
        if let Some(domain) = domain_filter {
            if nft.id().domain() != domain {
                continue;
            }
        }
        items.push(ExplorerNftDto::from_entry(nft));
    }
    items.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));
    let (items, pagination) = paginate(items, page, per_page);
    ExplorerNftsPage { pagination, items }
}

pub(crate) fn block_created_at(duration: Duration) -> String {
    duration_to_rfc3339(duration)
}

fn count_rejected_transactions(block: &SignedBlock, external_total: usize) -> u32 {
    if external_total == 0 || !block.has_results() {
        return 0;
    }
    let rejected = block
        .results()
        .take(external_total)
        .filter(|result| result.as_ref().is_err())
        .count();
    saturating_usize_to_u32(rejected)
}

fn saturating_usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet, iter, num::NonZeroU32, str::FromStr, time::Duration as StdDuration,
    };

    use iroha_data_model::{
        ChainId, Registrable, ValidationFail,
        account::AccountDetails,
        asset::{AssetDefinitionId, AssetId, definition::MintabilityTokens},
        block::{BlockHeader, builder::BlockBuilder},
        common::{Owned, Ref},
        domain::DomainId,
        isi::{Register, Transfer},
        metadata::Metadata,
        nft::{NftData, NftId},
        transaction::{
            error::TransactionRejectionReason,
            signed::{TransactionBuilder, TransactionResultInner},
        },
        trigger::DataTriggerSequence,
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
    use nonzero_ext::nonzero;

    use super::*;

    #[cfg(feature = "app_api")]
    #[test]
    fn address_format_query_preference_defaults() {
        let query = ExplorerAddressFormatQuery::default();
        assert_eq!(
            query.address_format_pref().expect("preference"),
            AddressFormatPreference::Ih58
        );
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn address_format_query_preference_parses_param() {
        let query = ExplorerAddressFormatQuery {
            address_format: Some("compressed".to_string()),
        };
        assert_eq!(
            query.address_format_pref().expect("preference"),
            AddressFormatPreference::Compressed
        );
    }

    #[test]
    fn paginate_truncates_correctly() {
        let items = vec![1, 2, 3, 4, 5];
        let (page, meta) = paginate(items, 2, 2);
        assert_eq!(page, vec![3, 4]);
        assert_eq!(meta.page, 2);
        assert_eq!(meta.per_page, 2);
        assert_eq!(meta.total_items, 5);
        assert_eq!(meta.total_pages, 3);
    }

    #[test]
    fn metadata_conversion_handles_entries() {
        let mut metadata = Metadata::default();
        let previous = metadata.insert("key".parse().unwrap(), json::Value::String("value".into()));
        assert!(previous.is_none(), "test metadata should start empty");
        let cloned = metadata_to_json(&metadata);
        match cloned {
            Value::Object(map) => {
                let value = map
                    .get("key")
                    .expect("metadata should contain inserted key");
                assert_eq!(value.as_str(), Some("value"));
            }
            _ => panic!("metadata should serialize into object"),
        }
    }

    #[test]
    fn mintable_label_matches_variants() {
        assert_eq!(mintable_label(Mintable::Infinitely), "Infinitely");
        assert_eq!(mintable_label(Mintable::Once), "Once");
        assert_eq!(mintable_label(Mintable::Not), "Not");
        let tokens = MintabilityTokens::try_new(3).expect("non-zero tokens");
        assert_eq!(mintable_label(Mintable::Limited(tokens)), "Limited(3)");
    }

    #[test]
    fn domain_dto_reflects_counts() {
        let mut domain =
            iroha_data_model::domain::Domain::new(DomainId::from_str("test").expect("domain name"))
                .build(&ALICE_ID);
        domain.metadata_mut().insert(
            "label".parse().unwrap(),
            json::Value::String("value".into()),
        );
        let counts = DomainCounters {
            accounts: 2,
            assets: 3,
            nfts: 4,
        };
        let dto = ExplorerDomainDto::from_domain(&domain, counts);
        assert_eq!(dto.accounts, 2);
        assert_eq!(dto.assets, 3);
        assert_eq!(dto.nfts, 4);
        assert_eq!(dto.owned_by, ALICE_ID.to_string());
    }

    #[test]
    fn asset_definition_dto_contains_metadata() {
        let def_id: AssetDefinitionId = "rose#wonderland".parse().expect("definition id");
        let mut definition =
            iroha_data_model::asset::definition::AssetDefinition::numeric(def_id.clone())
                .build(&ALICE_ID);
        definition.set_mintable(Mintable::Once);
        definition.metadata_mut().insert(
            "ticker".parse().unwrap(),
            json::Value::String("ROSE".into()),
        );
        let mut aggregates = ExplorerAggregates::default();
        aggregates.definition_instances.insert(def_id.clone(), 7);
        let dto = ExplorerAssetDefinitionDto::from_definition(&definition, &aggregates);
        assert_eq!(dto.mintable, "Once");
        assert_eq!(dto.assets, 7);
        assert_eq!(dto.owned_by, ALICE_ID.to_string());
    }

    #[test]
    fn asset_dto_formats_value() {
        let def_id: AssetDefinitionId = "rose#wonderland".parse().expect("definition id");
        let asset_id = AssetId::new(def_id, ALICE_ID.clone());
        let value = Owned::new(Numeric::from(42u32));
        let entry = Ref::new(&asset_id, &value);
        let dto = ExplorerAssetDto::from_entry(entry);
        assert_eq!(dto.id, asset_id.to_string());
        assert_eq!(dto.value, "42");
        assert_eq!(dto.account_id, ALICE_ID.to_string());
    }

    #[test]
    fn nft_dto_includes_metadata() {
        let nft_id: NftId = "rose$wonderland".parse().expect("nft id");
        let mut data = NftData {
            content: Metadata::default(),
            owned_by: ALICE_ID.clone(),
        };
        data.content.insert(
            "artist".parse().unwrap(),
            json::Value::String("Alice".into()),
        );
        let value = Owned::new(data);
        let entry = Ref::new(&nft_id, &value);
        let dto = ExplorerNftDto::from_entry(entry);
        assert_eq!(dto.id, nft_id.to_string());
        assert_eq!(dto.owned_by, ALICE_ID.to_string());
        match dto.metadata {
            Value::Object(map) => {
                assert_eq!(map.get("artist").and_then(Value::as_str), Some("Alice"));
            }
            _ => panic!("metadata should be object"),
        }
    }

    #[test]
    fn block_dto_counts_rejections() {
        let chain: ChainId = "test-chain".parse().expect("valid chain id");
        let tx = TransactionBuilder::new(chain, ALICE_ID.clone())
            .with_instructions(iter::empty::<iroha_data_model::isi::InstructionBox>())
            .sign(ALICE_KEYPAIR.private_key());
        let header = BlockHeader::new(nonzero!(3_u64), None, None, None, 1_700_000_000_000, 0);
        let mut builder = BlockBuilder::new(header);
        builder.push_transaction(tx);
        builder.push_result(TransactionResultInner::Err(
            TransactionRejectionReason::Validation(ValidationFail::InternalError(
                "boom".to_string(),
            )),
        ));
        let block = builder.build_with_signature(0, ALICE_KEYPAIR.private_key());

        let dto = ExplorerBlockDto::from_block(&block);
        assert_eq!(dto.height, 3);
        assert_eq!(dto.transactions_total, 1);
        assert_eq!(dto.transactions_rejected, 1);
        assert_eq!(dto.created_at, "2023-11-14T22:13:20Z");
        assert!(dto.transactions_hash.is_some());
    }

    #[test]
    fn timestamp_format_handles_epoch() {
        let formatted = block_created_at(Duration::from_millis(0));
        assert_eq!(formatted, "1970-01-01T00:00:00Z");
    }

    #[test]
    fn transaction_summary_reflects_status() {
        let chain: ChainId = "test-chain".parse().expect("valid chain id");
        let tx = TransactionBuilder::new(chain, ALICE_ID.clone())
            .with_instructions(iter::empty::<iroha_data_model::isi::InstructionBox>())
            .sign(ALICE_KEYPAIR.private_key());
        let result = TransactionResult(Ok(DataTriggerSequence::default()));
        let dto = transaction_summary_dto(&tx, 5, &result, AddressFormatPreference::Ih58);
        assert_eq!(dto.block, 5);
        assert_eq!(dto.authority, ALICE_ID.to_string());
        assert_eq!(dto.status, "Committed");
    }

    #[test]
    fn transaction_detail_includes_rejection_reason() {
        let chain: ChainId = "test-chain".parse().expect("valid chain id");
        let mut metadata = Metadata::default();
        metadata.insert(
            "purpose".parse().unwrap(),
            json::Value::String("test".into()),
        );
        let mut builder = TransactionBuilder::new(chain, ALICE_ID.clone())
            .with_instructions(iter::empty::<iroha_data_model::isi::InstructionBox>())
            .with_metadata(metadata);
        builder.set_creation_time(StdDuration::from_millis(1_700_000_000));
        builder
            .set_ttl(StdDuration::from_secs(30))
            .set_nonce(NonZeroU32::new(7).expect("nonce"));
        let tx = builder.sign(ALICE_KEYPAIR.private_key());
        let rejection = TransactionRejectionReason::Validation(ValidationFail::TooComplex);
        let result = TransactionResult(Err(rejection));
        let dto = transaction_detail_dto(&tx, 12, &result, AddressFormatPreference::Ih58);
        assert_eq!(dto.block, 12);
        assert_eq!(dto.status, "Rejected");
        assert_eq!(dto.authority, ALICE_ID.to_string());
        assert_eq!(dto.nonce, Some(7));
        assert!(dto.time_to_live.is_some());
        assert_eq!(
            dto.metadata.get("purpose").and_then(Value::as_str),
            Some("test")
        );
        assert!(dto.rejection_reason.is_some());
    }

    #[test]
    fn accounts_page_filters_by_domain_and_definition() {
        let def_id: AssetDefinitionId = "rose#wonderland".parse().expect("definition id");
        let mut aggregates = ExplorerAggregates::default();
        aggregates.account_counters.insert(
            ALICE_ID.clone(),
            AccountCounters {
                domains: 1,
                assets: 2,
                nfts: 0,
            },
        );
        aggregates.account_counters.insert(
            BOB_ID.clone(),
            AccountCounters {
                domains: 0,
                assets: 1,
                nfts: 0,
            },
        );
        let mut holders = BTreeSet::new();
        holders.insert(ALICE_ID.clone());
        aggregates
            .definition_holders
            .insert(def_id.clone(), holders);

        let alice_details = Owned::new(AccountDetails::new(
            Metadata::default(),
            None,
            None,
            Vec::new(),
        ));
        let bob_details = Owned::new(AccountDetails::new(
            Metadata::default(),
            None,
            None,
            Vec::new(),
        ));
        let alice_id = ALICE_ID.clone();
        let bob_id = BOB_ID.clone();
        let domain_filter = alice_id.domain().clone();
        let accounts = vec![
            Ref::new(&alice_id, &alice_details),
            Ref::new(&bob_id, &bob_details),
        ];

        let page = accounts_page(
            accounts,
            &aggregates,
            Some(&domain_filter),
            Some(&def_id),
            1,
            10,
        );
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, alice_id.to_string());
        assert_eq!(page.items[0].owned_assets, 2);
        assert_eq!(page.pagination.total_items, 1);
    }

    #[test]
    fn assets_page_filters_by_owner_and_definition() {
        let rose_def: AssetDefinitionId = "rose#wonderland".parse().expect("definition id");
        let lily_def: AssetDefinitionId = "lily#wonderland".parse().expect("definition id");
        let alice_asset_id = AssetId::new(rose_def.clone(), ALICE_ID.clone());
        let bob_asset_id = AssetId::new(lily_def.clone(), BOB_ID.clone());
        let alice_value = Owned::new(Numeric::from(10u32));
        let bob_value = Owned::new(Numeric::from(5u32));

        let owned_page = assets_page(
            vec![
                Ref::new(&alice_asset_id, &alice_value),
                Ref::new(&bob_asset_id, &bob_value),
            ],
            Some(&*ALICE_ID),
            None,
            1,
            10,
        );
        assert_eq!(owned_page.items.len(), 1);
        assert_eq!(owned_page.items[0].id, alice_asset_id.to_string());

        let definition_page = assets_page(
            vec![
                Ref::new(&alice_asset_id, &alice_value),
                Ref::new(&bob_asset_id, &bob_value),
            ],
            None,
            Some(&lily_def),
            1,
            10,
        );
        assert_eq!(definition_page.items.len(), 1);
        assert_eq!(definition_page.items[0].id, bob_asset_id.to_string());
    }

    #[test]
    fn nfts_page_filters_by_owner_and_domain() {
        let nft_alpha: NftId = "alpha$wonderland".parse().expect("nft id");
        let nft_beta: NftId = "beta$garden_of_live_flowers".parse().expect("nft id");
        let mut alpha_data = NftData {
            content: Metadata::default(),
            owned_by: ALICE_ID.clone(),
        };
        alpha_data.content.insert(
            "series".parse().unwrap(),
            json::Value::String("alpha".into()),
        );
        let alpha_value = Owned::new(alpha_data);
        let beta_value = Owned::new(NftData {
            content: Metadata::default(),
            owned_by: BOB_ID.clone(),
        });

        let owner_page = nfts_page(
            vec![
                Ref::new(&nft_alpha, &alpha_value),
                Ref::new(&nft_beta, &beta_value),
            ],
            Some(&*ALICE_ID),
            None,
            1,
            10,
        );
        assert_eq!(owner_page.items.len(), 1);
        assert_eq!(owner_page.items[0].id, nft_alpha.to_string());

        let domain_filter = nft_beta.domain().clone();
        let domain_page = nfts_page(
            vec![
                Ref::new(&nft_alpha, &alpha_value),
                Ref::new(&nft_beta, &beta_value),
            ],
            None,
            Some(&domain_filter),
            1,
            10,
        );
        assert_eq!(domain_page.items.len(), 1);
        assert_eq!(domain_page.items[0].id, nft_beta.to_string());
    }

    #[test]
    fn instruction_kind_classifies_register_and_transfer() {
        let register = Register::domain(iroha_data_model::domain::Domain::new(
            "test".parse().expect("domain id"),
        ));
        let register_box: InstructionBox = register.into();
        assert_eq!(
            instruction_kind(&register_box),
            ExplorerInstructionKind::Register
        );

        let asset_def: AssetDefinitionId = "rose#wonderland".parse().expect("definition id");
        let asset_id = AssetId::new(asset_def.clone(), ALICE_ID.clone());
        let transfer = Transfer::asset_numeric(asset_id, 1u32, BOB_ID.clone());
        let transfer_box: InstructionBox = transfer.into();
        assert_eq!(
            instruction_kind(&transfer_box),
            ExplorerInstructionKind::Transfer
        );
    }

    #[test]
    fn instruction_box_dto_wraps_payload_and_scale() {
        let register = Register::domain(iroha_data_model::domain::Domain::new(
            "payload".parse().expect("domain id"),
        ));
        let instruction: InstructionBox = register.into();
        let dto = instruction_box_dto(&instruction, ExplorerInstructionKind::Register);
        assert!(dto.scale.starts_with("0x"));
        match dto.json {
            Value::Object(map) => {
                assert_eq!(
                    map.get("kind")
                        .and_then(Value::as_str)
                        .expect("kind string"),
                    "Register"
                );
                assert!(map.contains_key("payload"));
                assert!(map.contains_key("wire_id"));
                assert!(map.contains_key("encoded"));
            }
            _ => panic!("instruction payload should serialize into object"),
        }
    }

    #[test]
    fn custom_instruction_payload_preserves_json_body() {
        let mut args = Map::new();
        args.insert("foo".to_string(), Value::from(1_u64));
        let mut root = Map::new();
        root.insert("kind".to_string(), Value::String("Demo".to_string()));
        root.insert("args".to_string(), Value::Object(args));
        let payload = iroha_primitives::json::Json::new(Value::Object(root));
        let custom = CustomInstruction::new(payload);
        let instruction: InstructionBox = custom.into();
        let dto = instruction_box_dto(&instruction, ExplorerInstructionKind::Custom);
        match dto.json {
            Value::Object(mut map) => {
                let kind_value = map.remove("kind").expect("kind string");
                assert_eq!(kind_value.as_str().expect("kind string"), "Custom");
                let payload = map
                    .remove("payload")
                    .expect("payload")
                    .get("value")
                    .cloned()
                    .expect("value key");
                assert!(payload.get("args").is_some());
            }
            _ => panic!("custom payload should be a structured object"),
        }
    }

    #[test]
    fn instruction_dto_carries_index() {
        let register = Register::domain(iroha_data_model::domain::Domain::new(
            "index_test".parse().expect("domain id"),
        ));
        let instruction = InstructionBox::from(register);
        let chain: ChainId = "test-chain".parse().expect("chain id");
        let tx = TransactionBuilder::new(chain, ALICE_ID.clone())
            .with_instructions(core::iter::once(instruction.clone()))
            .sign(ALICE_KEYPAIR.private_key());
        let result = TransactionResult(Ok(DataTriggerSequence::default()));
        let dto = instruction_dto_with_kind(
            &tx,
            5,
            &result,
            &instruction,
            ExplorerInstructionKind::Register,
            7,
            AddressFormatPreference::Ih58,
        );
        assert_eq!(dto.index, 7);
    }
}

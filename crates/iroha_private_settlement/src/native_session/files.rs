//! Closed file-source layout consumed only by the already admitted native command owner.

use super::*;
use crate::{
    OneCallPrivateSettlementRequest, PrivateSettlementBusinessTerms,
    PrivateSettlementRestrictedOpening,
};
use iroha::config::Config;
use iroha_config_base::file_source::{ConfigFileAccess, ConfigFileRequest, ConfigFileSource};
use iroha_core_privacy::privacy_engines::atomic_private_settlement::{
    ATOMIC_PRIVATE_SETTLEMENT_RESERVATION_MAX_BYTES_V1,
    decode_atomic_private_settlement_reservation_v1,
};
use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    nexus::{PrivateSettlementCapsulePaddingV1, PrivateSettlementProofIntentV1},
    parameter::system::ConsensusMode,
    sumeragi_finality::{MAX_SIGNED_GENESIS_BYTES_V1, SignedGenesisPinsV1},
};
use norito::derive::{JsonDeserialize, JsonSerialize};
use std::{
    cell::Cell,
    io,
    path::{Component, Path, PathBuf},
};
use zeroize::Zeroizing;

/// Maximum complete native file descriptor accepted before any copy or parse.
/// Paths and a fixed set of scalar controls are the entire descriptor; no witness belongs here.
pub const NATIVE_PRIVATE_SETTLEMENT_DESCRIPTOR_MAX_BYTES: usize = 16 * 1024;

#[derive(JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct OpenDescriptor {
    layout: String,
    client_file: PathBuf,
    request_file: PathBuf,
    pins_file: PathBuf,
    genesis_file: PathBuf,
    custody_key_file: PathBuf,
    custody_root: PathBuf,
    maximum_wallet_bytes: u64,
}
#[derive(JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct RequestFile {
    layout: String,
    intent: PrivateSettlementProofIntentV1,
    terms: Vec<TermsFile>,
    schema: Hash,
    minimum_context_time_ms: u64,
    endpoints: Vec<Vec<String>>,
    padding: PrivateSettlementCapsulePaddingV1,
    retention_height: u64,
    max_carrier_bytes: u64,
    max_artifact_bytes: u64,
    max_total_artifact_bytes: u64,
}
#[derive(JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct TermsFile {
    asset: AssetDefinitionId,
    payer: AccountId,
    recipient: AccountId,
    amount: u128,
    reimbursement: u128,
    memo: Vec<u8>,
}
#[derive(JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct PinsFile {
    layout: String,
    chain_id: String,
    network_id: NetworkId,
    genesis_hash: HashOf<BlockHeader>,
    signed_genesis_sha256: [u8; 32],
    genesis_public_key: PublicKey,
    roster: Vec<PublicKey>,
    mode: ConsensusMode,
    world_schema: Hash,
}
#[derive(JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct StageDescriptor {
    layout: String,
    ordinal: u8,
    reservation_file: PathBuf,
    opening_file: PathBuf,
}

pub(super) struct FileInput(Zeroizing<Vec<u8>>);
impl FileInput {
    pub(super) fn capture(
        bytes: &[u8],
        maximum: usize,
    ) -> Result<Self, NativePrivateSettlementSessionError> {
        if bytes.is_empty()
            || bytes.len() > NATIVE_PRIVATE_SETTLEMENT_DESCRIPTOR_MAX_BYTES
            || bytes.len() > maximum
        {
            return Err(NativePrivateSettlementSessionError::Capacity);
        }
        let mut owned = Zeroizing::new(Vec::new());
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| NativePrivateSettlementSessionError::Capacity)?;
        owned.extend_from_slice(bytes);
        Ok(Self(owned))
    }
    pub(super) fn open(
        self,
        limits: NativePrivateSettlementLimits,
    ) -> Result<opening::Opened, OneCallPrivateSettlementError> {
        let descriptor: OpenDescriptor = parse(&self.0)?;
        if descriptor.layout != "iroha.private-settlement.files.v1" {
            return Err(OneCallPrivateSettlementError::Binding);
        }
        for path in [
            &descriptor.client_file,
            &descriptor.request_file,
            &descriptor.pins_file,
            &descriptor.genesis_file,
            &descriptor.custody_key_file,
            &descriptor.custody_root,
        ] {
            absolute(path)?;
        }
        let reads = SourceReads::new(limits.maximum_original_bytes.get(), self.0.len())?;
        // All source bytes, including parser-owned references, debit the same cumulative budget.
        // Each retained no-follow native read sees the remaining bound before allocating.
        let config_bytes = reads.private(&descriptor.client_file, usize::MAX)?;
        let (config, _) = Config::load_bytes_with_musubi_publication_and_file_source(
            &descriptor.client_file,
            &config_bytes,
            &reads,
        )
        .map_err(|error| OneCallPrivateSettlementError::Provider(error.into()))?;
        drop(config_bytes);
        let account = iroha::client::Client::builder(config)
            .build()
            .map_err(|error| OneCallPrivateSettlementError::Provider(error.into()))?
            .account_client()
            .map_err(|_| OneCallPrivateSettlementError::Binding)?;
        let request_bytes = reads.private(&descriptor.request_file, usize::MAX)?;
        let request: RequestFile = parse(&request_bytes)?;
        drop(request_bytes);
        let request = request.construct()?;
        let pins_bytes = reads.private(&descriptor.pins_file, usize::MAX)?;
        let pins: PinsFile = parse(&pins_bytes)?;
        drop(pins_bytes);
        if pins.layout != "iroha.private-settlement.pins.v1" {
            return Err(OneCallPrivateSettlementError::Binding);
        }
        let genesis = reads.private(&descriptor.genesis_file, MAX_SIGNED_GENESIS_BYTES_V1)?;
        let key = reads.private(&descriptor.custody_key_file, 32)?;
        let key = Zeroizing::new(
            <[u8; 32]>::try_from(key.as_slice())
                .map_err(|_| OneCallPrivateSettlementError::Binding)?,
        );
        let stores = NativePrivateSettlementStores::new(
            descriptor.custody_root,
            key,
            std::num::NonZeroUsize::new(
                usize::try_from(descriptor.maximum_wallet_bytes)
                    .map_err(|_| OneCallPrivateSettlementError::Capacity)?,
            )
            .ok_or(OneCallPrivateSettlementError::Capacity)?,
        );
        let originals = NativePrivateSettlementOriginals::new(
            account,
            request,
            SignedGenesisPinsV1 {
                chain_id: pins.chain_id,
                network_id: pins.network_id,
                genesis_hash: pins.genesis_hash,
                signed_genesis_sha256: pins.signed_genesis_sha256,
                genesis_public_key: pins.genesis_public_key,
                roster: pins.roster.into_iter().map(Into::into).collect(),
                mode: pins.mode,
            },
            pins.world_schema,
            stores,
        );
        // Independently selected pins are authenticated by the sole original opener, before
        // creation of any custody directory. The source reader is not a trust authority.
        let mut opened = opening::OpenSource::from_owned(originals, genesis, limits)?
            .construct_with_staging(limits)?;
        let staging = opened
            .staging
            .as_mut()
            .and_then(Arc::get_mut)
            .ok_or(OneCallPrivateSettlementError::Binding)?;
        staging.original_bytes = staging
            .original_bytes
            .max(limits.maximum_original_bytes.get() - reads.remaining.get());
        Ok(opened)
    }
    pub(super) fn stage(
        self,
        staging: &opening::Staging,
        maximum: usize,
    ) -> Result<(), OneCallPrivateSettlementError> {
        let descriptor: StageDescriptor = parse(&self.0)?;
        if descriptor.layout != "iroha.private-settlement.stage.v1" {
            return Err(OneCallPrivateSettlementError::Binding);
        }
        absolute(&descriptor.reservation_file)?;
        absolute(&descriptor.opening_file)?;
        let terms = staging
            .request
            .0
            .terms
            .get(usize::from(descriptor.ordinal))
            .ok_or(OneCallPrivateSettlementError::Binding)?;
        let retained = staging
            .original_bytes
            .checked_add(self.0.len())
            .ok_or(OneCallPrivateSettlementError::Capacity)?;
        let reads = SourceReads::new(maximum, retained)?;
        let material = reads.private(
            &descriptor.reservation_file,
            ATOMIC_PRIVATE_SETTLEMENT_RESERVATION_MAX_BYTES_V1,
        )?;
        let (authorized, _) = decode_atomic_private_settlement_reservation_v1(
            material,
            staging.request.intent(),
            descriptor.ordinal,
        )?;
        let bytes = reads.private(
            &descriptor.opening_file,
            crate::wallet_store::OPENING_MAX_BYTES,
        )?;
        let (governance, policy) = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )?;
        staging.wallet.stage_authorized(
            staging.request.intent(),
            descriptor.ordinal,
            terms,
            PrivateSettlementRestrictedOpening { governance, policy },
            authorized,
        )?;
        Ok(())
    }
}
impl RequestFile {
    fn construct(self) -> Result<OneCallPrivateSettlementRequest, OneCallPrivateSettlementError> {
        if self.layout != "iroha.private-settlement.request.v1" {
            return Err(OneCallPrivateSettlementError::Binding);
        }
        let endpoints = self
            .endpoints
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|text| {
                        url::Url::parse(&text).map_err(|_| OneCallPrivateSettlementError::Binding)
                    })
                    .collect()
            })
            .collect::<Result<_, _>>()?;
        let terms = self
            .terms
            .into_iter()
            .map(|value| PrivateSettlementBusinessTerms {
                asset: value.asset,
                payer: value.payer,
                recipient: value.recipient,
                amount: value.amount,
                reimbursement: value.reimbursement,
                memo: value.memo,
            })
            .collect();
        OneCallPrivateSettlementRequest::new(
            self.intent,
            terms,
            self.schema,
            self.minimum_context_time_ms,
            endpoints,
            self.padding,
            self.retention_height,
            self.max_carrier_bytes,
            usize::try_from(self.max_artifact_bytes)
                .map_err(|_| OneCallPrivateSettlementError::Capacity)?,
            usize::try_from(self.max_total_artifact_bytes)
                .map_err(|_| OneCallPrivateSettlementError::Capacity)?,
        )
    }
}
fn parse<T: norito::json::JsonDeserialize>(
    bytes: &[u8],
) -> Result<T, OneCallPrivateSettlementError> {
    norito::json::from_slice(bytes).map_err(|_| OneCallPrivateSettlementError::Binding)
}
fn absolute(path: &Path) -> Result<(), OneCallPrivateSettlementError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(OneCallPrivateSettlementError::Binding);
    }
    Ok(())
}
struct SourceReads {
    remaining: Cell<usize>,
}
impl SourceReads {
    fn new(maximum: usize, retained: usize) -> Result<Self, OneCallPrivateSettlementError> {
        Ok(Self {
            remaining: Cell::new(
                maximum
                    .checked_sub(retained)
                    .ok_or(OneCallPrivateSettlementError::Capacity)?,
            ),
        })
    }
    fn private(
        &self,
        path: &Path,
        maximum: usize,
    ) -> Result<Zeroizing<Vec<u8>>, OneCallPrivateSettlementError> {
        self.read(
            path,
            ConfigFileRequest {
                access: ConfigFileAccess::Private,
                maximum,
            },
        )
        .map_err(OneCallPrivateSettlementError::Storage)
    }
}
impl ConfigFileSource for SourceReads {
    fn read(&self, path: &Path, request: ConfigFileRequest) -> io::Result<Zeroizing<Vec<u8>>> {
        let limit = request.maximum.min(self.remaining.get());
        if limit == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "native original source capacity unavailable",
            ));
        }
        let bytes = match request.access {
            ConfigFileAccess::Private => iroha_fs::read_private(path, limit)?,
            ConfigFileAccess::Public => iroha_fs::read_regular(path, limit)?,
        };
        self.remaining.set(
            self.remaining
                .get()
                .checked_sub(bytes.len())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "native original source capacity unavailable",
                    )
                })?,
        );
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests;

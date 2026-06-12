//! Global, per-block witness recorder for SBV‑AM (prototype).
//!
//! Records observed reads (pre-values) and writes (post-values) during execution.
//! Keys are encoded deterministically with a tag and ID strings. Values are the
//! canonical JSON strings from `iroha_primitives::json::Json` for metadata maps.
//!
//! This module is internal and accessed from execution/merge paths and the actor.

use core::str::FromStr as _;
use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard, OnceLock},
};

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    domain::DomainId,
    fastpq::{TransferTranscript, TransferTranscriptBundle},
    name::Name,
    nft::NftId,
};
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly;

use super::consensus::{ExecKv, ExecWitness};
use crate::state::{StateBlock, WorldReadOnly};

#[derive(Default)]
struct BlockWitness {
    active: bool,
    reads: BTreeMap<Vec<u8>, Vec<u8>>,  // key -> value (pre)
    writes: BTreeMap<Vec<u8>, Vec<u8>>, // key -> value (post; empty for delete)
    fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>,
}

/// Exclusive access guard for the global execution witness recorder.
///
/// Dropping the guard clears any unfinished capture so early validation returns
/// or panics cannot leak stale witness records into the next block.
pub struct ExecWitnessGuard {
    _guard: MutexGuard<'static, ()>,
}

impl Drop for ExecWitnessGuard {
    fn drop(&mut self) {
        clear_block();
    }
}

static SLOT: OnceLock<Mutex<BlockWitness>> = OnceLock::new();
static EXEC_WITNESS_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn slot() -> &'static Mutex<BlockWitness> {
    SLOT.get_or_init(|| Mutex::new(BlockWitness::default()))
}

fn exec_witness_lock() -> &'static Mutex<()> {
    EXEC_WITNESS_LOCK.get_or_init(|| Mutex::new(()))
}

fn lock_slot() -> MutexGuard<'static, BlockWitness> {
    match slot().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            iroha_logger::warn!(
                "execution witness recorder mutex was poisoned; recovering block witness state"
            );
            poisoned.into_inner()
        }
    }
}

fn lock_exec_witness_lock() -> MutexGuard<'static, ()> {
    match exec_witness_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            iroha_logger::warn!(
                "execution witness guard mutex was poisoned; recovering exclusive witness access"
            );
            poisoned.into_inner()
        }
    }
}

fn with_active_slot(f: impl FnOnce(&mut BlockWitness)) {
    let mut g = lock_slot();
    if g.active {
        f(&mut g);
    }
}

fn clear_block() {
    let mut g = lock_slot();
    g.active = false;
    g.reads.clear();
    g.writes.clear();
    g.fastpq_transcripts.clear();
}

/// Hold exclusive access to the global witness recorder for the duration of a block execution.
pub fn exec_witness_guard() -> ExecWitnessGuard {
    ExecWitnessGuard {
        _guard: lock_exec_witness_lock(),
    }
}

/// Start a new witness capture for the current block (clears previous data).
pub fn start_block() {
    let mut g = lock_slot();
    g.active = true;
    g.reads.clear();
    g.writes.clear();
    g.fastpq_transcripts.clear();
}

/// Drain the accumulated witness into an `ExecWitness` and clear the store.
pub fn drain_exec_witness() -> ExecWitness {
    let mut g = lock_slot();
    let mut reads: Vec<ExecKv> = Vec::with_capacity(g.reads.len());
    let mut writes: Vec<ExecKv> = Vec::with_capacity(g.writes.len());
    for (k, v) in &g.reads {
        reads.push(ExecKv {
            key: k.clone(),
            value: v.clone(),
        });
    }
    for (k, v) in &g.writes {
        writes.push(ExecKv {
            key: k.clone(),
            value: v.clone(),
        });
    }
    g.reads.clear();
    g.writes.clear();
    g.active = false;
    let mut fastpq_map = std::mem::take(&mut g.fastpq_transcripts);
    crate::fastpq::finalize_transfer_transcript_digests_in_map(&mut fastpq_map);
    let fastpq_transcripts = map_to_bundles(fastpq_map);
    ExecWitness {
        reads,
        writes,
        fastpq_transcripts,
        fastpq_batches: Vec::new(),
    }
}

fn map_to_bundles(map: BTreeMap<Hash, Vec<TransferTranscript>>) -> Vec<TransferTranscriptBundle> {
    map.into_iter()
        .map(|(entry_hash, transcripts)| TransferTranscriptBundle {
            entry_hash,
            transcripts,
        })
        .collect()
}

fn map_ref_to_bundles(
    map: &BTreeMap<Hash, Vec<TransferTranscript>>,
) -> Vec<TransferTranscriptBundle> {
    map.iter()
        .map(|(entry_hash, transcripts)| TransferTranscriptBundle {
            entry_hash: *entry_hash,
            transcripts: transcripts.clone(),
        })
        .collect()
}

fn key_sep() -> u8 {
    0x1F // Unit Separator
}

fn enc_key_prefix(tag: u8, a: &str, b: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + a.len() + 1 + b.len());
    out.push(tag);
    out.extend_from_slice(a.as_bytes());
    out.push(key_sep());
    out.extend_from_slice(b.as_bytes());
    out
}

fn key_account_kv(id: &AccountId, key: &Name) -> Vec<u8> {
    enc_key_prefix(0xA1, &id.to_string(), key.as_ref())
}
fn key_domain_kv(id: &DomainId, key: &Name) -> Vec<u8> {
    enc_key_prefix(0xA2, &id.to_string(), key.as_ref())
}
fn key_nft_kv(id: &NftId, key: &Name) -> Vec<u8> {
    enc_key_prefix(0xA3, &id.to_string(), key.as_ref())
}
fn key_asset_def_kv(id: &AssetDefinitionId, key: &Name) -> Vec<u8> {
    enc_key_prefix(0xA4, &id.to_string(), key.as_ref())
}

fn key_asset_balance(id: &AssetId) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + id.to_string().len());
    out.push(0xB1);
    out.extend_from_slice(id.to_string().as_bytes());
    out
}

fn key_asset_def_total(id: &AssetDefinitionId) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + id.to_string().len());
    out.push(0xB2);
    out.extend_from_slice(id.to_string().as_bytes());
    out
}

fn bytes_from_json(j: &iroha_primitives::json::Json) -> Vec<u8> {
    j.get().as_bytes().to_vec()
}

/// Record a read (pre-value) of account metadata.
pub fn record_read_account_kv(
    id: &AccountId,
    key: &Name,
    val: Option<&iroha_primitives::json::Json>,
) {
    let k = key_account_kv(id, key);
    let v = val.map(bytes_from_json).unwrap_or_default();
    with_active_slot(|g| {
        g.reads.entry(k).or_insert(v);
    });
}

/// Record a write (post-value) of account metadata.
pub fn record_write_account_kv(id: &AccountId, key: &Name, val: &iroha_primitives::json::Json) {
    let k = key_account_kv(id, key);
    let v = bytes_from_json(val);
    with_active_slot(|g| {
        g.writes.insert(k, v);
    });
}

/// Record a delete (post empty) of account metadata, with read pre-value supplied.
pub fn record_delete_account_kv(id: &AccountId, key: &Name, pre: &iroha_primitives::json::Json) {
    let k = key_account_kv(id, key);
    with_active_slot(|g| {
        g.reads
            .entry(k.clone())
            .or_insert_with(|| bytes_from_json(pre));
        g.writes.insert(k, Vec::new());
    });
}

/// Record a read (pre-value) of domain metadata.
pub fn record_read_domain_kv(
    id: &DomainId,
    key: &Name,
    val: Option<&iroha_primitives::json::Json>,
) {
    let k = key_domain_kv(id, key);
    let v = val.map(bytes_from_json).unwrap_or_default();
    with_active_slot(|g| {
        g.reads.entry(k).or_insert(v);
    });
}
/// Record a write (post-value) of domain metadata.
pub fn record_write_domain_kv(id: &DomainId, key: &Name, val: &iroha_primitives::json::Json) {
    let k = key_domain_kv(id, key);
    let v = bytes_from_json(val);
    with_active_slot(|g| {
        g.writes.insert(k, v);
    });
}
/// Record a delete (post empty) of domain metadata, with read pre-value supplied.
pub fn record_delete_domain_kv(id: &DomainId, key: &Name, pre: &iroha_primitives::json::Json) {
    let k = key_domain_kv(id, key);
    with_active_slot(|g| {
        g.reads
            .entry(k.clone())
            .or_insert_with(|| bytes_from_json(pre));
        g.writes.insert(k, Vec::new());
    });
}

/// Record a read (pre-value) of NFT metadata.
pub fn record_read_nft_kv(id: &NftId, key: &Name, val: Option<&iroha_primitives::json::Json>) {
    let k = key_nft_kv(id, key);
    let v = val.map(bytes_from_json).unwrap_or_default();
    with_active_slot(|g| {
        g.reads.entry(k).or_insert(v);
    });
}
/// Record a write (post-value) of NFT metadata.
pub fn record_write_nft_kv(id: &NftId, key: &Name, val: &iroha_primitives::json::Json) {
    let k = key_nft_kv(id, key);
    let v = bytes_from_json(val);
    with_active_slot(|g| {
        g.writes.insert(k, v);
    });
}
/// Record a delete (post empty) of NFT metadata, with read pre-value supplied.
pub fn record_delete_nft_kv(id: &NftId, key: &Name, pre: &iroha_primitives::json::Json) {
    let k = key_nft_kv(id, key);
    with_active_slot(|g| {
        g.reads
            .entry(k.clone())
            .or_insert_with(|| bytes_from_json(pre));
        g.writes.insert(k, Vec::new());
    });
}

/// Record asset balance read (pre-value) for an asset.
pub fn record_read_asset(id: &AssetId, val: Option<&iroha_primitives::numeric::Numeric>) {
    let k = key_asset_balance(id);
    let v = val
        .map(|n| Json::new(n.clone()).get().as_bytes().to_vec())
        .unwrap_or_default();
    with_active_slot(|g| {
        g.reads.entry(k).or_insert(v);
    });
}

/// Record asset balance write (post-value) for an asset.
pub fn record_write_asset(id: &AssetId, val: &iroha_primitives::numeric::Numeric) {
    let k = key_asset_balance(id);
    let v = Json::new(val.clone()).get().as_bytes().to_vec();
    with_active_slot(|g| {
        g.writes.insert(k, v);
    });
}

/// Record asset definition total supply read (pre-value).
pub fn record_read_asset_def_total(
    id: &AssetDefinitionId,
    val: Option<&iroha_primitives::numeric::Numeric>,
) {
    let k = key_asset_def_total(id);
    let v = val
        .map(|n| Json::new(n.clone()).get().as_bytes().to_vec())
        .unwrap_or_default();
    with_active_slot(|g| {
        g.reads.entry(k).or_insert(v);
    });
}

/// Record asset definition total supply write (post-value).
pub fn record_write_asset_def_total(
    id: &AssetDefinitionId,
    val: &iroha_primitives::numeric::Numeric,
) {
    let k = key_asset_def_total(id);
    let v = Json::new(val.clone()).get().as_bytes().to_vec();
    with_active_slot(|g| {
        g.writes.insert(k, v);
    });
}

/// Record a FASTPQ transfer transcript so `ExecWitness` consumers can replay transfers.
pub fn record_fastpq_transcript(transcript: &TransferTranscript) {
    with_active_slot(|g| {
        g.fastpq_transcripts
            .entry(transcript.batch_hash)
            .or_default()
            .push(transcript.clone());
    });
}

/// Copy finalized FASTPQ transcript digests from the block recorder into the execution witness.
pub(crate) fn apply_fastpq_transcript_digests(finalized: &BTreeMap<Hash, Vec<TransferTranscript>>) {
    let mut g = lock_slot();
    for (entry_hash, finalized_transcripts) in finalized {
        let Some(witness_transcripts) = g.fastpq_transcripts.get_mut(entry_hash) else {
            continue;
        };
        for (witness, finalized) in witness_transcripts
            .iter_mut()
            .zip(finalized_transcripts.iter())
        {
            if witness.poseidon_preimage_digest.is_none()
                && finalized.poseidon_preimage_digest.is_some()
                && same_transfer_transcript_without_digest(witness, finalized)
            {
                witness.poseidon_preimage_digest = finalized.poseidon_preimage_digest;
            }
        }
    }
}

fn same_transfer_transcript_without_digest(
    left: &TransferTranscript,
    right: &TransferTranscript,
) -> bool {
    left.batch_hash == right.batch_hash
        && left.deltas == right.deltas
        && left.authority_digest == right.authority_digest
}

/// Record a read (pre-value) of asset-definition metadata.
pub fn record_read_asset_def_kv(
    id: &AssetDefinitionId,
    key: &Name,
    val: Option<&iroha_primitives::json::Json>,
) {
    let k = key_asset_def_kv(id, key);
    let v = val.map(bytes_from_json).unwrap_or_default();
    with_active_slot(|g| {
        g.reads.entry(k).or_insert(v);
    });
}
/// Record a write (post-value) of asset-definition metadata.
pub fn record_write_asset_def_kv(
    id: &AssetDefinitionId,
    key: &Name,
    val: &iroha_primitives::json::Json,
) {
    let k = key_asset_def_kv(id, key);
    let v = bytes_from_json(val);
    with_active_slot(|g| {
        g.writes.insert(k, v);
    });
}
/// Record a delete (post empty) of asset-definition metadata, with read pre-value supplied.
pub fn record_delete_asset_def_kv(
    id: &AssetDefinitionId,
    key: &Name,
    pre: &iroha_primitives::json::Json,
) {
    let k = key_asset_def_kv(id, key);
    with_active_slot(|g| {
        g.reads
            .entry(k.clone())
            .or_insert_with(|| bytes_from_json(pre));
        g.writes.insert(k, Vec::new());
    });
}

/// Parse an access key string and record a pure read (if supported).
/// Currently supports only metadata detail keys:
/// - `account.detail:{account_id}:{key}`
/// - `domain.detail:{domain_id}:{key}`
/// - `asset_def.detail:{asset_def_id}:{key}`
/// - `nft.detail:{nft_id}:{key}`
///   Other keys are ignored.
#[allow(clippy::too_many_lines)]
pub fn record_read_from_access_key(state_block: &StateBlock<'_>, access_key: &str) {
    if let Some(rest) = access_key.strip_prefix("account.detail:") {
        let mut it = rest.splitn(2, ':');
        if let (Some(acc_s), Some(name_s)) = (it.next(), it.next()) {
            if let (Ok(acc), Ok(name)) = (
                iroha_data_model::account::AccountId::parse_encoded(acc_s)
                    .map(iroha_data_model::account::ParsedAccountId::into_account_id),
                Name::from_str(name_s),
            ) {
                if let Ok(acct) = state_block.world.account(&acc) {
                    let pre = acct.value().metadata().get(&name).cloned();
                    record_read_account_kv(&acc, &name, pre.as_ref());
                } else {
                    record_read_account_kv(&acc, &name, None);
                }
            }
        }
    }
    if let Some(rest) = access_key.strip_prefix("domain.detail:") {
        let mut it = rest.splitn(2, ':');
        if let (Some(dom_s), Some(name_s)) = (it.next(), it.next()) {
            if let (Ok(dom), Ok(name)) = (
                iroha_data_model::domain::DomainId::parse_fully_qualified(dom_s),
                Name::from_str(name_s),
            ) {
                if let Ok(domv) = state_block.world.domain(&dom) {
                    let pre = domv.metadata.get(&name).cloned();
                    record_read_domain_kv(&dom, &name, pre.as_ref());
                } else {
                    record_read_domain_kv(&dom, &name, None);
                }
            }
        }
    }
    if let Some(rest) = access_key.strip_prefix("asset_def.detail:") {
        let mut it = rest.splitn(2, ':');
        if let (Some(ad_s), Some(name_s)) = (it.next(), it.next()) {
            if let (Ok(ad), Ok(name)) = (
                iroha_data_model::asset::AssetDefinitionId::parse_address_literal(ad_s),
                Name::from_str(name_s),
            ) {
                if let Ok(def) = state_block.world.asset_definition(&ad) {
                    let pre = def.metadata.get(&name).cloned();
                    record_read_asset_def_kv(&ad, &name, pre.as_ref());
                } else {
                    record_read_asset_def_kv(&ad, &name, None);
                }
            }
        }
    }
    if let Some(rest) = access_key.strip_prefix("nft.detail:") {
        let mut it = rest.splitn(2, ':');
        if let (Some(nft_s), Some(name_s)) = (it.next(), it.next()) {
            if let (Ok(nft), Ok(name)) = (
                iroha_data_model::nft::NftId::from_str(nft_s),
                Name::from_str(name_s),
            ) {
                if let Ok(nftv) = state_block.world.nft(&nft) {
                    let pre = nftv.content.get(&name).cloned();
                    record_read_nft_kv(&nft, &name, pre.as_ref());
                } else {
                    record_read_nft_kv(&nft, &name, None);
                }
            }
        }
    }
    if let Some(rest) = access_key.strip_prefix("role.binding:") {
        // role.binding:{account}:{role}
        let mut it = rest.splitn(2, ':');
        if let (Some(acc_s), Some(role_s)) = (it.next(), it.next()) {
            if let (Ok(acc), Ok(role)) = (
                iroha_data_model::account::AccountId::parse_encoded(acc_s)
                    .map(iroha_data_model::account::ParsedAccountId::into_account_id),
                iroha_data_model::role::RoleId::from_str(role_s),
            ) {
                let present = state_block
                    .world
                    .account_roles_iter(&acc)
                    .any(|r| r == &role);
                let k = enc_key_prefix(0xC1, &acc.to_string(), &role.to_string());
                let v = Json::new(present).get().as_bytes().to_vec();
                with_active_slot(|g| {
                    g.reads.entry(k).or_insert(v);
                });
            }
        }
    }
    if let Some(rest) = access_key.strip_prefix("role:") {
        if let Ok(role) = iroha_data_model::role::RoleId::from_str(rest) {
            let present = state_block.world.roles().get(&role).is_some();
            let mut out = Vec::with_capacity(1 + rest.len());
            out.push(0xC2);
            out.extend_from_slice(rest.as_bytes());
            let v = Json::new(present).get().as_bytes().to_vec();
            with_active_slot(|g| {
                g.reads.entry(out).or_insert(v);
            });
        }
    }
    if let Some(rest) = access_key.strip_prefix("perm.account:") {
        // perm.account:{account}:{perm}
        let mut it = rest.splitn(2, ':');
        if let (Some(acc_s), Some(perm_s)) = (it.next(), it.next())
            && let Ok(acc) = iroha_data_model::account::AccountId::parse_encoded(acc_s)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        {
            let present = state_block
                .world
                .account_permissions_iter(&acc)
                .ok()
                .is_some_and(|mut it| it.any(|p| p.name() == perm_s));
            let canonical_account = acc.to_string();
            let k = enc_key_prefix(0xC3, &canonical_account, perm_s);
            let v = Json::new(present).get().as_bytes().to_vec();
            with_active_slot(|g| {
                g.reads.entry(k).or_insert(v);
            });
        }
    }
    if let Some(rest) = access_key.strip_prefix("perm.role:") {
        // perm.role:{role}:{perm}
        let mut it = rest.splitn(2, ':');
        if let (Some(role_s), Some(perm_s)) = (it.next(), it.next())
            && let Ok(role_id) = iroha_data_model::role::RoleId::from_str(role_s)
        {
            let present = state_block
                .world
                .role(&role_id)
                .ok()
                .is_some_and(|r| r.permissions().any(|p| p.name() == perm_s));
            let k = enc_key_prefix(0xC4, role_s, perm_s);
            let v = Json::new(present).get().as_bytes().to_vec();
            with_active_slot(|g| {
                g.reads.entry(k).or_insert(v);
            });
        }
    }
    if let Some(rest) = access_key.strip_prefix("asset:")
        && let Ok(id) = iroha_data_model::asset::AssetId::parse_literal(rest)
    {
        let pre = state_block.world.assets().get(&id).map(|v| &**v);
        record_read_asset(&id, pre);
        // no further processing needed for asset access
    }
    if let Some(rest) = access_key.strip_prefix("asset_def:")
        && let Ok(ad) = iroha_data_model::asset::AssetDefinitionId::parse_address_literal(rest)
    {
        if let Ok(def) = state_block.world.asset_definition(&ad) {
            record_read_asset_def_total(&ad, Some(def.total_quantity()));
        } else {
            record_read_asset_def_total(&ad, None);
        }
        // no further processing needed for asset_def access
    }
}

#[allow(dead_code)]
/// Snapshot the current witness without clearing it (for debugging/inspection).
pub fn snapshot_exec_witness() -> ExecWitness {
    let g = lock_slot();
    let mut reads: Vec<ExecKv> = Vec::with_capacity(g.reads.len());
    let mut writes: Vec<ExecKv> = Vec::with_capacity(g.writes.len());
    for (k, v) in &g.reads {
        reads.push(ExecKv {
            key: k.clone(),
            value: v.clone(),
        });
    }
    for (k, v) in &g.writes {
        writes.push(ExecKv {
            key: k.clone(),
            value: v.clone(),
        });
    }
    let mut fastpq_transcripts = map_ref_to_bundles(&g.fastpq_transcripts);
    crate::fastpq::finalize_transfer_transcript_bundle_digests_in_place(&mut fastpq_transcripts);
    ExecWitness {
        reads,
        writes,
        fastpq_transcripts,
        fastpq_batches: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, time::Duration};

    use iroha_data_model::{
        Registrable,
        account::Account,
        asset::{Asset, AssetDefinition},
        block::BlockHeader,
        domain::Domain,
        metadata::Metadata,
        nft::Nft,
        permission::{Permission, Permissions},
        role::{Role, RoleId},
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use nonzero_ext::nonzero;

    use super::*;
    // The SMT helpers live under the sumeragi module.
    use crate::sumeragi::smt::{KvPair, compute_post_state_root};
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    struct AccessKeyFixture {
        state: State,
        account: AccountId,
        missing_account: AccountId,
        domain: DomainId,
        missing_domain: DomainId,
        asset_definition: AssetDefinitionId,
        missing_asset_definition: AssetDefinitionId,
        asset: AssetId,
        missing_asset: AssetId,
        nft: NftId,
        missing_nft: NftId,
        role: RoleId,
        unicode_role_raw: String,
        unicode_role: RoleId,
        account_perm: &'static str,
        role_perm: &'static str,
    }

    fn metadata_entry(key: &str, value: &str) -> Metadata {
        let mut metadata = Metadata::default();
        metadata.insert(key.parse::<Name>().expect("metadata key"), value);
        metadata
    }

    #[allow(clippy::too_many_lines)]
    fn access_key_fixture() -> AccessKeyFixture {
        let account = (*ALICE_ID).clone();
        let missing_account = (*BOB_ID).clone();
        let domain = DomainId::try_new("wonderland", "universal").expect("domain");
        let missing_domain = DomainId::try_new("looking_glass", "universal").expect("domain");
        let asset_definition = AssetDefinitionId::new(
            domain.clone(),
            "rose".parse::<Name>().expect("asset definition name"),
        );
        let missing_asset_definition = AssetDefinitionId::new(
            domain.clone(),
            "missing_rose"
                .parse::<Name>()
                .expect("missing asset definition name"),
        );
        let asset = AssetId::new(asset_definition.clone(), account.clone());
        let missing_asset = AssetId::new(missing_asset_definition.clone(), account.clone());
        let nft: NftId = "ticket$wonderland.universal".parse().expect("nft id");
        let missing_nft: NftId = "missing_ticket$wonderland.universal"
            .parse()
            .expect("missing nft id");
        let role: RoleId = "auditor".parse().expect("role id");
        let unicode_role_raw = String::from("cafe\u{301}");
        let unicode_role: RoleId = unicode_role_raw.parse().expect("unicode role id");
        let account_perm = "can_account_read";
        let role_perm = "can_role_read";

        let mut account_metadata = metadata_entry("color", "red");
        account_metadata.insert(
            "color:shade".parse::<Name>().expect("colon metadata key"),
            "maroon",
        );
        let account_record = Account::new(account.clone())
            .with_metadata(account_metadata)
            .build(&account);
        let domain_record = Domain::new(domain.clone())
            .with_metadata(metadata_entry("region", "west"))
            .build(&account);
        let mut asset_definition_record = AssetDefinition::numeric(asset_definition.clone())
            .with_name(String::from("Rose"))
            .build(&account);
        asset_definition_record
            .metadata_mut()
            .insert("issuer".parse::<Name>().expect("metadata key"), "alice");
        asset_definition_record.total_quantity = Numeric::from(37u32);
        let asset_record = Asset::new(asset.clone(), Numeric::from(11u32));
        let nft_record = Nft::new(nft.clone(), metadata_entry("artist", "carroll")).build(&account);
        let role_record = Role::new(role.clone(), account.clone())
            .add_permission(Permission::new(role_perm.into(), Json::new(true)))
            .build(&account);
        let unicode_role_record = Role::new(unicode_role.clone(), account.clone())
            .add_permission(Permission::new(role_perm.into(), Json::new(true)))
            .build(&account);

        let mut world = World::with_assets_and_roles(
            [domain_record],
            [account_record],
            [asset_definition_record],
            [asset_record],
            [nft_record],
            [role_record, unicode_role_record],
        );
        world.grant_role_for_tests(account.clone(), role.clone());
        let mut account_permissions = Permissions::new();
        account_permissions.insert(Permission::new(account_perm.into(), Json::new(true)));
        world
            .account_permissions_mut_for_testing()
            .insert(account.clone(), account_permissions);

        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );

        AccessKeyFixture {
            state,
            account,
            missing_account,
            domain,
            missing_domain,
            asset_definition,
            missing_asset_definition,
            asset,
            missing_asset,
            nft,
            missing_nft,
            role,
            unicode_role_raw,
            unicode_role,
            account_perm,
            role_perm,
        }
    }

    fn access_key_header() -> BlockHeader {
        BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)
    }

    fn bool_json_bytes(value: bool) -> Vec<u8> {
        Json::new(value).get().as_bytes().to_vec()
    }

    fn numeric_json_bytes(value: u32) -> Vec<u8> {
        Json::new(Numeric::from(value)).get().as_bytes().to_vec()
    }

    fn assert_read_value(witness: &ExecWitness, key: Vec<u8>, expected: Vec<u8>) {
        let value = witness
            .reads
            .iter()
            .find(|kv| kv.key == key)
            .unwrap_or_else(|| panic!("expected read key {:?}", key))
            .value
            .clone();
        assert_eq!(value, expected);
    }

    fn assert_no_read_key(witness: &ExecWitness, key: Vec<u8>) {
        assert!(
            witness.reads.iter().all(|kv| kv.key != key),
            "unexpected read key {:?}",
            key
        );
    }

    fn sample_fastpq_transcript(seed: u8, batch_hash: Hash) -> (TransferTranscript, Hash) {
        use iroha_data_model::{
            asset::id::AssetDefinitionId,
            fastpq::{TransferDeltaTranscript, TransferTranscript},
        };

        let asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            format!("rose_{seed}").parse().unwrap(),
        );
        let delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: asset,
            amount: Numeric::from(u32::from(seed) + 1),
            from_balance_before: Numeric::from(100u32 + u32::from(seed)),
            from_balance_after: Numeric::from(95u32 + u32::from(seed)),
            to_balance_before: Numeric::from(u32::from(seed)),
            to_balance_after: Numeric::from(5u32 + u32::from(seed)),
            from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
            to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        };
        let digest = crate::fastpq::poseidon_preimage_digest(&delta, &batch_hash);
        (
            TransferTranscript {
                batch_hash,
                deltas: vec![delta],
                authority_digest: crate::fastpq::authority_digest(&ALICE_ID),
                poseidon_preimage_digest: None,
            },
            digest,
        )
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn access_key_supported_present_and_missing_reads_match_formal_gate() {
        let fixture = access_key_fixture();
        let state_block = fixture.state.block(access_key_header());
        let guard = exec_witness_guard();
        start_block();

        let color = "color".parse::<Name>().expect("metadata key");
        let region = "region".parse::<Name>().expect("metadata key");
        let issuer = "issuer".parse::<Name>().expect("metadata key");
        let artist = "artist".parse::<Name>().expect("metadata key");
        let missing_role: RoleId = "observer".parse().expect("role id");
        record_read_from_access_key(
            &state_block,
            &format!("account.detail:{}:color", fixture.account),
        );
        record_read_from_access_key(
            &state_block,
            &format!("account.detail:{}:color", fixture.missing_account),
        );
        record_read_from_access_key(
            &state_block,
            &format!("domain.detail:{}:region", fixture.domain),
        );
        record_read_from_access_key(
            &state_block,
            &format!("domain.detail:{}:region", fixture.missing_domain),
        );
        record_read_from_access_key(
            &state_block,
            &format!("asset_def.detail:{}:issuer", fixture.asset_definition),
        );
        record_read_from_access_key(
            &state_block,
            &format!(
                "asset_def.detail:{}:issuer",
                fixture.missing_asset_definition
            ),
        );
        record_read_from_access_key(&state_block, &format!("nft.detail:{}:artist", fixture.nft));
        record_read_from_access_key(
            &state_block,
            &format!("nft.detail:{}:artist", fixture.missing_nft),
        );
        record_read_from_access_key(
            &state_block,
            &format!("role.binding:{}:{}", fixture.account, fixture.role),
        );
        record_read_from_access_key(
            &state_block,
            &format!("role.binding:{}:{}", fixture.account, missing_role),
        );
        record_read_from_access_key(&state_block, &format!("role:{}", fixture.role));
        record_read_from_access_key(&state_block, &format!("role:{missing_role}"));
        record_read_from_access_key(
            &state_block,
            &format!("perm.account:{}:{}", fixture.account, fixture.account_perm),
        );
        record_read_from_access_key(
            &state_block,
            &format!("perm.account:{}:can_missing", fixture.account),
        );
        record_read_from_access_key(
            &state_block,
            &format!("perm.role:{}:{}", fixture.role, fixture.role_perm),
        );
        record_read_from_access_key(
            &state_block,
            &format!("perm.role:{}:can_missing", fixture.role),
        );
        record_read_from_access_key(&state_block, &format!("asset:{}", fixture.asset));
        record_read_from_access_key(&state_block, &format!("asset:{}", fixture.missing_asset));
        record_read_from_access_key(
            &state_block,
            &format!("asset_def:{}", fixture.asset_definition),
        );
        record_read_from_access_key(
            &state_block,
            &format!("asset_def:{}", fixture.missing_asset_definition),
        );

        let witness = drain_exec_witness();
        drop(guard);
        assert_eq!(witness.reads.len(), 20);
        assert!(witness.writes.is_empty());
        assert_read_value(
            &witness,
            key_account_kv(&fixture.account, &color),
            bytes_from_json(&Json::new("red")),
        );
        assert_read_value(
            &witness,
            key_account_kv(&fixture.missing_account, &color),
            Vec::new(),
        );
        assert_read_value(
            &witness,
            key_domain_kv(&fixture.domain, &region),
            bytes_from_json(&Json::new("west")),
        );
        assert_read_value(
            &witness,
            key_domain_kv(&fixture.missing_domain, &region),
            Vec::new(),
        );
        assert_read_value(
            &witness,
            key_asset_def_kv(&fixture.asset_definition, &issuer),
            bytes_from_json(&Json::new("alice")),
        );
        assert_read_value(
            &witness,
            key_asset_def_kv(&fixture.missing_asset_definition, &issuer),
            Vec::new(),
        );
        assert_read_value(
            &witness,
            key_nft_kv(&fixture.nft, &artist),
            bytes_from_json(&Json::new("carroll")),
        );
        assert_read_value(
            &witness,
            key_nft_kv(&fixture.missing_nft, &artist),
            Vec::new(),
        );
        assert_read_value(
            &witness,
            enc_key_prefix(
                0xC1,
                &fixture.account.to_string(),
                &fixture.role.to_string(),
            ),
            bool_json_bytes(true),
        );
        assert_read_value(
            &witness,
            enc_key_prefix(
                0xC1,
                &fixture.account.to_string(),
                &missing_role.to_string(),
            ),
            bool_json_bytes(false),
        );
        let mut present_role_key = vec![0xC2];
        present_role_key.extend_from_slice(fixture.role.to_string().as_bytes());
        assert_read_value(&witness, present_role_key, bool_json_bytes(true));
        let mut missing_role_key = vec![0xC2];
        missing_role_key.extend_from_slice(missing_role.to_string().as_bytes());
        assert_read_value(&witness, missing_role_key, bool_json_bytes(false));
        assert_read_value(
            &witness,
            enc_key_prefix(0xC3, &fixture.account.to_string(), fixture.account_perm),
            bool_json_bytes(true),
        );
        assert_read_value(
            &witness,
            enc_key_prefix(0xC3, &fixture.account.to_string(), "can_missing"),
            bool_json_bytes(false),
        );
        assert_read_value(
            &witness,
            enc_key_prefix(0xC4, &fixture.role.to_string(), fixture.role_perm),
            bool_json_bytes(true),
        );
        assert_read_value(
            &witness,
            enc_key_prefix(0xC4, &fixture.role.to_string(), "can_missing"),
            bool_json_bytes(false),
        );
        assert_read_value(
            &witness,
            key_asset_balance(&fixture.asset),
            numeric_json_bytes(11),
        );
        assert_read_value(
            &witness,
            key_asset_balance(&fixture.missing_asset),
            Vec::new(),
        );
        assert_read_value(
            &witness,
            key_asset_def_total(&fixture.asset_definition),
            numeric_json_bytes(37),
        );
        assert_read_value(
            &witness,
            key_asset_def_total(&fixture.missing_asset_definition),
            Vec::new(),
        );
    }

    #[test]
    fn access_key_rejects_unsupported_malformed_and_invalid_inputs_match_formal_gate() {
        let fixture = access_key_fixture();
        let state_block = fixture.state.block(access_key_header());
        let guard = exec_witness_guard();
        start_block();

        let invalid_keys = [
            format!("unsupported:{}:color", fixture.account),
            format!("account.detail:{}", fixture.account),
            format!("domain.detail:{}", fixture.domain),
            format!("asset_def.detail:{}", fixture.asset_definition),
            format!("nft.detail:{}", fixture.nft),
            format!("role.binding:{}", fixture.account),
            format!("perm.account:{}", fixture.account),
            format!("perm.role:{}", fixture.role),
            String::from("account.detail:not_an_account:color"),
            String::from("domain.detail:not_a_domain:region"),
            String::from("asset_def.detail:not_an_asset_definition:issuer"),
            String::from("nft.detail:not_an_nft:artist"),
            format!("role.binding:{}:bad role", fixture.account),
            String::from("role:bad role"),
            String::from("perm.account:not_an_account:can_account_read"),
            String::from("perm.role:bad role:can_role_read"),
            String::from("asset:not_an_asset"),
            String::from("asset_def:not_an_asset_definition"),
            format!("account.detail:{}:bad key", fixture.account),
        ];
        for key in invalid_keys {
            record_read_from_access_key(&state_block, &key);
        }

        let witness = drain_exec_witness();
        drop(guard);
        assert!(
            witness.reads.is_empty(),
            "unsupported, malformed, and invalid keys must be ignored"
        );
        assert!(witness.writes.is_empty());
    }

    #[test]
    fn access_key_canonicalization_tail_and_prefix_isolation_match_formal_gate() {
        let fixture = access_key_fixture();
        let state_block = fixture.state.block(access_key_header());
        let guard = exec_witness_guard();
        start_block();

        let color = "color".parse::<Name>().expect("metadata key");
        let shade = "color:shade".parse::<Name>().expect("metadata key");
        let issuer = "issuer".parse::<Name>().expect("metadata key");
        record_read_from_access_key(
            &state_block,
            &format!("account.detail: {} :color", fixture.account),
        );
        record_read_from_access_key(
            &state_block,
            &format!(
                "perm.account: {} :{}",
                fixture.account, fixture.account_perm
            ),
        );
        record_read_from_access_key(
            &state_block,
            &format!("account.detail:{}:color:shade", fixture.account),
        );
        record_read_from_access_key(
            &state_block,
            &format!("asset_def.detail:{}:issuer", fixture.asset_definition),
        );
        record_read_from_access_key(
            &state_block,
            &format!("role.binding:{}:{}", fixture.account, fixture.role),
        );
        record_read_from_access_key(
            &state_block,
            &format!(
                "perm.role:{}:{}",
                fixture.unicode_role_raw, fixture.role_perm
            ),
        );

        let witness = drain_exec_witness();
        drop(guard);
        assert_eq!(witness.reads.len(), 6);
        assert_read_value(
            &witness,
            key_account_kv(&fixture.account, &color),
            bytes_from_json(&Json::new("red")),
        );
        assert_no_read_key(
            &witness,
            enc_key_prefix(0xA1, &format!(" {} ", fixture.account), color.as_ref()),
        );
        assert_read_value(
            &witness,
            enc_key_prefix(0xC3, &fixture.account.to_string(), fixture.account_perm),
            bool_json_bytes(true),
        );
        assert_no_read_key(
            &witness,
            enc_key_prefix(
                0xC3,
                &format!(" {} ", fixture.account),
                fixture.account_perm,
            ),
        );
        assert_read_value(
            &witness,
            key_account_kv(&fixture.account, &shade),
            bytes_from_json(&Json::new("maroon")),
        );
        assert_read_value(
            &witness,
            key_asset_def_kv(&fixture.asset_definition, &issuer),
            bytes_from_json(&Json::new("alice")),
        );
        assert_no_read_key(&witness, key_asset_def_total(&fixture.asset_definition));
        assert_read_value(
            &witness,
            enc_key_prefix(
                0xC1,
                &fixture.account.to_string(),
                &fixture.role.to_string(),
            ),
            bool_json_bytes(true),
        );
        let mut role_fallthrough_key = vec![0xC2];
        role_fallthrough_key
            .extend_from_slice(format!("binding:{}:{}", fixture.account, fixture.role).as_bytes());
        assert_no_read_key(&witness, role_fallthrough_key);
        assert_read_value(
            &witness,
            enc_key_prefix(0xC4, &fixture.unicode_role_raw, fixture.role_perm),
            bool_json_bytes(true),
        );
        assert_no_read_key(
            &witness,
            enc_key_prefix(0xC4, &fixture.unicode_role.to_string(), fixture.role_perm),
        );
    }

    #[test]
    fn parity_same_witness_twice_same_root() {
        let _guard = exec_witness_guard();
        // First run
        start_block();
        // Simulate asset balance read+write
        let ad = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let aid = iroha_data_model::asset::AssetId::new(ad.clone(), (*ALICE_ID).clone());
        let pre = iroha_primitives::numeric::Numeric::from(10u32);
        let post = iroha_primitives::numeric::Numeric::from(15u32);
        record_read_asset(&aid, Some(&pre));
        record_write_asset(&aid, &post);
        // Simulate total supply
        let tot_pre = iroha_primitives::numeric::Numeric::from(100u32);
        let tot_post = iroha_primitives::numeric::Numeric::from(105u32);
        record_read_asset_def_total(&ad, Some(&tot_pre));
        record_write_asset_def_total(&ad, &tot_post);
        let w1 = drain_exec_witness();
        let r1 = compute_post_state_root(
            &w1.reads
                .iter()
                .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
                .collect::<Vec<_>>(),
            &w1.writes
                .iter()
                .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
                .collect::<Vec<_>>(),
        );

        // Second run (identical)
        start_block();
        record_read_asset(&aid, Some(&pre));
        record_write_asset(&aid, &post);
        record_read_asset_def_total(&ad, Some(&tot_pre));
        record_write_asset_def_total(&ad, &tot_post);
        let w2 = drain_exec_witness();
        let r2 = compute_post_state_root(
            &w2.reads
                .iter()
                .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
                .collect::<Vec<_>>(),
            &w2.writes
                .iter()
                .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
                .collect::<Vec<_>>(),
        );
        assert_eq!(r1, r2);
    }

    #[test]
    fn recorder_lifecycle_read_write_delete_and_drain_match_formal_gate() {
        let _guard = exec_witness_guard();
        start_block();

        let key: Name = "color".parse().expect("metadata key");
        let account = (*ALICE_ID).clone();
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset = AssetId::new(asset_definition, account.clone());
        record_read_asset(
            &asset,
            Some(&iroha_primitives::numeric::Numeric::from(1u32)),
        );
        record_write_asset(&asset, &iroha_primitives::numeric::Numeric::from(2u32));

        start_block();
        assert!(snapshot_exec_witness().reads.is_empty());
        assert!(snapshot_exec_witness().writes.is_empty());

        let first = Json::new("first");
        let second = Json::new("second");
        let post = Json::new("post");
        let delete_pre = Json::new("delete_pre");
        record_read_account_kv(&account, &key, Some(&first));
        record_read_account_kv(&account, &key, Some(&second));
        record_write_account_kv(&account, &key, &second);
        record_write_account_kv(&account, &key, &post);
        record_delete_account_kv(&account, &key, &delete_pre);
        record_read_asset(
            &asset,
            Some(&iroha_primitives::numeric::Numeric::from(7u32)),
        );
        record_write_asset(&asset, &iroha_primitives::numeric::Numeric::from(9u32));

        let snapshot = snapshot_exec_witness();
        assert_eq!(snapshot.reads.len(), 2);
        assert_eq!(snapshot.writes.len(), 2);

        let drained = drain_exec_witness();
        assert_eq!(drained.fastpq_batches, Vec::new());
        assert!(
            drained
                .reads
                .windows(2)
                .all(|pair| pair[0].key < pair[1].key)
        );
        assert!(
            drained
                .writes
                .windows(2)
                .all(|pair| pair[0].key < pair[1].key)
        );

        let account_key = key_account_kv(&account, &key);
        let read = drained
            .reads
            .iter()
            .find(|kv| kv.key == account_key)
            .expect("account read recorded");
        assert_eq!(read.value, bytes_from_json(&first));
        let write = drained
            .writes
            .iter()
            .find(|kv| kv.key == account_key)
            .expect("account write recorded");
        assert!(
            write.value.is_empty(),
            "delete should record an empty post value"
        );

        record_read_account_kv(&account, &key, Some(&second));
        assert!(
            drain_exec_witness().reads.is_empty(),
            "drain must deactivate capture so inactive records are ignored"
        );
    }

    #[test]
    fn recorder_recovers_poisoned_witness_locks() {
        let _ = std::panic::catch_unwind(|| {
            let _guard = exec_witness_lock()
                .lock()
                .expect("execution witness guard lock should be held");
            panic!("poison execution witness guard for recovery test");
        });
        let _ = std::panic::catch_unwind(|| {
            let mut guard = slot()
                .lock()
                .expect("execution witness slot lock should be held");
            guard.active = true;
            panic!("poison execution witness slot for recovery test");
        });

        let _guard = exec_witness_guard();
        start_block();

        let account = (*ALICE_ID).clone();
        let key: Name = "color".parse().expect("metadata key");
        let value = Json::new("red");
        record_read_account_kv(&account, &key, Some(&value));

        let drained = drain_exec_witness();
        assert_eq!(drained.reads.len(), 1);
        assert_eq!(drained.reads[0].key, key_account_kv(&account, &key));
        assert_eq!(drained.reads[0].value, bytes_from_json(&value));
    }

    #[test]
    fn recorder_key_namespaces_match_formal_tags_and_separators() {
        let account = (*ALICE_ID).clone();
        let domain = DomainId::try_new("wonderland", "universal").unwrap();
        let nft: NftId = "ticket$wonderland.universal".parse().expect("nft id");
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            domain.clone(),
            "rose".parse().unwrap(),
        );
        let asset = AssetId::new(asset_definition.clone(), account.clone());
        let key: Name = "color".parse().expect("metadata key");
        let sep = key_sep();

        let metadata_keys = [
            key_account_kv(&account, &key),
            key_domain_kv(&domain, &key),
            key_nft_kv(&nft, &key),
            key_asset_def_kv(&asset_definition, &key),
        ];
        let metadata_tags = metadata_keys
            .iter()
            .map(|key| key[0])
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(metadata_tags, [0xA1, 0xA2, 0xA3, 0xA4].into());
        assert!(
            metadata_keys.iter().all(|key| key.contains(&sep)),
            "metadata keys must include the unit separator between id and field"
        );

        let balance_key = key_asset_balance(&asset);
        assert_eq!(balance_key[0], 0xB1);
        assert!(!balance_key.contains(&sep));

        let total_key = key_asset_def_total(&asset_definition);
        assert_eq!(total_key[0], 0xB2);
        assert!(!total_key.contains(&sep));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fastpq_grouping_and_digest_copy_match_formal_gate() {
        let _guard = exec_witness_guard();
        start_block();

        let batch_hash = Hash::prehashed([0x44; Hash::LENGTH]);
        let other_hash = Hash::prehashed([0x45; Hash::LENGTH]);
        let missing_hash = Hash::prehashed([0x46; Hash::LENGTH]);
        let (first, first_digest) = sample_fastpq_transcript(1, batch_hash);
        let (second, second_digest) = sample_fastpq_transcript(2, batch_hash);
        let (other, other_digest) = sample_fastpq_transcript(3, other_hash);
        record_fastpq_transcript(&first);
        record_fastpq_transcript(&second);
        record_fastpq_transcript(&other);

        let snapshot = snapshot_exec_witness();
        let grouped = snapshot
            .fastpq_transcripts
            .iter()
            .find(|bundle| bundle.entry_hash == batch_hash)
            .expect("same-batch transcripts grouped");
        assert_eq!(grouped.transcripts.len(), 2);
        assert_eq!(grouped.transcripts[0].deltas, first.deltas);
        assert_eq!(grouped.transcripts[1].deltas, second.deltas);
        assert_eq!(
            grouped.transcripts[0].poseidon_preimage_digest,
            Some(first_digest)
        );
        assert_eq!(
            grouped.transcripts[1].poseidon_preimage_digest,
            Some(second_digest)
        );

        let mut reversed = BTreeMap::new();
        let mut finalized_second = second.clone();
        finalized_second.poseidon_preimage_digest = Some(second_digest);
        let mut finalized_first = first.clone();
        finalized_first.poseidon_preimage_digest = Some(first_digest);
        reversed.insert(
            batch_hash,
            vec![finalized_second.clone(), finalized_first.clone()],
        );
        apply_fastpq_transcript_digests(&reversed);
        {
            let g = lock_slot();
            let stored = g
                .fastpq_transcripts
                .get(&batch_hash)
                .expect("batch still recorded");
            assert_eq!(stored[0].poseidon_preimage_digest, None);
            assert_eq!(stored[1].poseidon_preimage_digest, None);
        }

        let mut finalized = BTreeMap::new();
        finalized.insert(batch_hash, vec![finalized_first, finalized_second]);
        let mut finalized_other = other.clone();
        finalized_other.poseidon_preimage_digest = Some(other_digest);
        finalized.insert(other_hash, vec![finalized_other]);
        finalized.insert(
            missing_hash,
            vec![sample_fastpq_transcript(4, missing_hash).0],
        );
        apply_fastpq_transcript_digests(&finalized);
        {
            let g = lock_slot();
            let stored = g
                .fastpq_transcripts
                .get(&batch_hash)
                .expect("batch still recorded");
            assert_eq!(stored[0].poseidon_preimage_digest, Some(first_digest));
            assert_eq!(stored[1].poseidon_preimage_digest, Some(second_digest));
            assert!(
                !g.fastpq_transcripts.contains_key(&missing_hash),
                "digest copy must not create missing batches"
            );
        }

        let replacement = Hash::prehashed([0xEE; Hash::LENGTH]);
        let mut overwrite = BTreeMap::new();
        let mut finalized_first = first;
        finalized_first.poseidon_preimage_digest = Some(replacement);
        overwrite.insert(batch_hash, vec![finalized_first]);
        apply_fastpq_transcript_digests(&overwrite);
        let drained = drain_exec_witness();
        let grouped = drained
            .fastpq_transcripts
            .iter()
            .find(|bundle| bundle.entry_hash == batch_hash)
            .expect("drained batch present");
        assert_eq!(
            grouped.transcripts[0].poseidon_preimage_digest,
            Some(first_digest),
            "existing digest must not be overwritten"
        );
        assert!(drained.fastpq_batches.is_empty());
    }

    #[test]
    fn records_fastpq_transcripts() {
        use iroha_data_model::{
            asset::id::AssetDefinitionId,
            fastpq::{TransferDeltaTranscript, TransferTranscript},
        };
        use iroha_primitives::numeric::Numeric;
        use iroha_test_samples::{ALICE_ID, BOB_ID};

        let _guard = exec_witness_guard();
        start_block();
        let asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: asset,
            amount: Numeric::from(5u32),
            from_balance_before: Numeric::from(100u32),
            from_balance_after: Numeric::from(95u32),
            to_balance_before: Numeric::from(0u32),
            to_balance_after: Numeric::from(5u32),
            from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
            to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        };
        let batch_hash = Hash::prehashed([0x11; Hash::LENGTH]);
        let transcript = TransferTranscript {
            batch_hash,
            deltas: vec![delta.clone()],
            authority_digest: crate::fastpq::authority_digest(&ALICE_ID),
            poseidon_preimage_digest: None,
        };
        let expected_digest = crate::fastpq::poseidon_preimage_digest(&delta, &batch_hash);
        record_fastpq_transcript(&transcript);

        let witness = drain_exec_witness();
        let stored = witness
            .fastpq_transcripts
            .iter()
            .find(|bundle| bundle.entry_hash == batch_hash)
            .expect("transcript recorded");
        assert_eq!(stored.transcripts.len(), 1);
        let mut expected = transcript;
        expected.poseidon_preimage_digest = Some(expected_digest);
        assert_eq!(stored.transcripts[0], expected);
        assert!(witness.fastpq_batches.is_empty());
        assert!(witness.reads.is_empty());
        assert!(witness.writes.is_empty());
    }

    #[test]
    fn apply_fastpq_transcript_digests_updates_recorded_witness_copy() {
        use iroha_data_model::{
            asset::id::AssetDefinitionId,
            fastpq::{TransferDeltaTranscript, TransferTranscript},
        };
        use iroha_primitives::numeric::Numeric;
        use iroha_test_samples::{ALICE_ID, BOB_ID};

        let _guard = exec_witness_guard();
        start_block();
        let asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: asset,
            amount: Numeric::from(5u32),
            from_balance_before: Numeric::from(100u32),
            from_balance_after: Numeric::from(95u32),
            to_balance_before: Numeric::from(0u32),
            to_balance_after: Numeric::from(5u32),
            from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
            to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        };
        let batch_hash = Hash::prehashed([0x33; Hash::LENGTH]);
        let transcript = TransferTranscript {
            batch_hash,
            deltas: vec![delta.clone()],
            authority_digest: crate::fastpq::authority_digest(&ALICE_ID),
            poseidon_preimage_digest: None,
        };
        let expected_digest = crate::fastpq::poseidon_preimage_digest(&delta, &batch_hash);
        record_fastpq_transcript(&transcript);

        let mut finalized = std::collections::BTreeMap::new();
        let mut finalized_transcript = transcript;
        finalized_transcript.poseidon_preimage_digest = Some(expected_digest);
        finalized.insert(batch_hash, vec![finalized_transcript]);
        apply_fastpq_transcript_digests(&finalized);

        let g = lock_slot();
        let stored = g
            .fastpq_transcripts
            .get(&batch_hash)
            .expect("transcript recorded");
        assert_eq!(stored[0].poseidon_preimage_digest, Some(expected_digest));
        drop(g);
        let _ = drain_exec_witness();
    }

    #[test]
    fn snapshot_finalizes_single_fastpq_transcript_without_clearing() {
        use iroha_data_model::{
            asset::id::AssetDefinitionId,
            fastpq::{TransferDeltaTranscript, TransferTranscript},
        };
        use iroha_primitives::numeric::Numeric;
        use iroha_test_samples::{ALICE_ID, BOB_ID};

        let _guard = exec_witness_guard();
        start_block();
        let asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: asset,
            amount: Numeric::from(5u32),
            from_balance_before: Numeric::from(100u32),
            from_balance_after: Numeric::from(95u32),
            to_balance_before: Numeric::from(0u32),
            to_balance_after: Numeric::from(5u32),
            from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
            to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        };
        let batch_hash = Hash::prehashed([0x22; Hash::LENGTH]);
        let transcript = TransferTranscript {
            batch_hash,
            deltas: vec![delta.clone()],
            authority_digest: crate::fastpq::authority_digest(&ALICE_ID),
            poseidon_preimage_digest: None,
        };
        let expected_digest = crate::fastpq::poseidon_preimage_digest(&delta, &batch_hash);
        record_fastpq_transcript(&transcript);

        let snapshot = snapshot_exec_witness();
        let stored = snapshot
            .fastpq_transcripts
            .iter()
            .find(|bundle| bundle.entry_hash == batch_hash)
            .expect("snapshot transcript recorded");
        assert_eq!(
            stored.transcripts[0].poseidon_preimage_digest,
            Some(expected_digest)
        );

        let drained = drain_exec_witness();
        assert_eq!(drained.fastpq_transcripts.len(), 1);
    }

    #[test]
    fn inactive_recorder_ignores_out_of_block_records() {
        use iroha_data_model::{asset::id::AssetDefinitionId, fastpq::TransferTranscript};
        use iroha_primitives::numeric::Numeric;

        let _guard = exec_witness_guard();
        let _ = drain_exec_witness();
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset = AssetId::new(asset_definition, (*ALICE_ID).clone());
        record_read_asset(&asset, Some(&Numeric::from(7u32)));
        record_fastpq_transcript(&TransferTranscript {
            batch_hash: Hash::prehashed([0x22; Hash::LENGTH]),
            deltas: Vec::new(),
            authority_digest: crate::fastpq::authority_digest(&ALICE_ID),
            poseidon_preimage_digest: None,
        });

        let witness = drain_exec_witness();
        assert!(witness.reads.is_empty());
        assert!(witness.writes.is_empty());
        assert!(witness.fastpq_transcripts.is_empty());
        assert!(witness.fastpq_batches.is_empty());
    }

    #[test]
    fn guard_drop_clears_unfinished_capture() {
        use iroha_primitives::numeric::Numeric;

        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset = AssetId::new(asset_definition, (*ALICE_ID).clone());
        let guard = exec_witness_guard();
        start_block();
        record_read_asset(&asset, Some(&Numeric::from(11u32)));
        drop(guard);

        let _guard = exec_witness_guard();
        let witness = drain_exec_witness();
        assert!(witness.reads.is_empty());
        assert!(witness.writes.is_empty());
        assert!(witness.fastpq_transcripts.is_empty());
        assert!(witness.fastpq_batches.is_empty());
    }

    #[test]
    fn exec_witness_guard_serializes_block_access() {
        let guard = exec_witness_guard();
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let _guard = exec_witness_guard();
            tx.send(()).expect("send guard signal");
        });

        assert!(
            rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "guard should prevent concurrent access"
        );
        drop(guard);
        rx.recv_timeout(Duration::from_secs(5))
            .expect("guard should release");
        handle.join().expect("thread joins");
    }
}

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
    reads: BTreeMap<Vec<u8>, Vec<u8>>,  // key -> value (pre)
    writes: BTreeMap<Vec<u8>, Vec<u8>>, // key -> value (post; empty for delete)
    fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>,
}

static SLOT: OnceLock<Mutex<BlockWitness>> = OnceLock::new();
static EXEC_WITNESS_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn slot() -> &'static Mutex<BlockWitness> {
    SLOT.get_or_init(|| Mutex::new(BlockWitness::default()))
}

fn exec_witness_lock() -> &'static Mutex<()> {
    EXEC_WITNESS_LOCK.get_or_init(|| Mutex::new(()))
}

/// Hold exclusive access to the global witness recorder for the duration of a block execution.
pub fn exec_witness_guard() -> MutexGuard<'static, ()> {
    exec_witness_lock()
        .lock()
        .expect("exec witness guard lock poisoned")
}

/// Start a new witness capture for the current block (clears previous data).
pub fn start_block() {
    let mut g = slot().lock().unwrap();
    g.reads.clear();
    g.writes.clear();
    g.fastpq_transcripts.clear();
}

/// Drain the accumulated witness into an `ExecWitness` and clear the store.
pub fn drain_exec_witness() -> ExecWitness {
    let mut g = slot().lock().unwrap();
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
    let fastpq_map = std::mem::take(&mut g.fastpq_transcripts);
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
    let mut g = slot().lock().unwrap();
    g.reads.entry(k).or_insert(v);
}

/// Record a write (post-value) of account metadata.
pub fn record_write_account_kv(id: &AccountId, key: &Name, val: &iroha_primitives::json::Json) {
    let k = key_account_kv(id, key);
    let v = bytes_from_json(val);
    let mut g = slot().lock().unwrap();
    g.writes.insert(k, v);
}

/// Record a delete (post empty) of account metadata, with read pre-value supplied.
pub fn record_delete_account_kv(id: &AccountId, key: &Name, pre: &iroha_primitives::json::Json) {
    let k = key_account_kv(id, key);
    let mut g = slot().lock().unwrap();
    g.reads
        .entry(k.clone())
        .or_insert_with(|| bytes_from_json(pre));
    g.writes.insert(k, Vec::new());
}

/// Record a read (pre-value) of domain metadata.
pub fn record_read_domain_kv(
    id: &DomainId,
    key: &Name,
    val: Option<&iroha_primitives::json::Json>,
) {
    let k = key_domain_kv(id, key);
    let v = val.map(bytes_from_json).unwrap_or_default();
    let mut g = slot().lock().unwrap();
    g.reads.entry(k).or_insert(v);
}
/// Record a write (post-value) of domain metadata.
pub fn record_write_domain_kv(id: &DomainId, key: &Name, val: &iroha_primitives::json::Json) {
    let k = key_domain_kv(id, key);
    let v = bytes_from_json(val);
    let mut g = slot().lock().unwrap();
    g.writes.insert(k, v);
}
/// Record a delete (post empty) of domain metadata, with read pre-value supplied.
pub fn record_delete_domain_kv(id: &DomainId, key: &Name, pre: &iroha_primitives::json::Json) {
    let k = key_domain_kv(id, key);
    let mut g = slot().lock().unwrap();
    g.reads
        .entry(k.clone())
        .or_insert_with(|| bytes_from_json(pre));
    g.writes.insert(k, Vec::new());
}

/// Record a read (pre-value) of NFT metadata.
pub fn record_read_nft_kv(id: &NftId, key: &Name, val: Option<&iroha_primitives::json::Json>) {
    let k = key_nft_kv(id, key);
    let v = val.map(bytes_from_json).unwrap_or_default();
    let mut g = slot().lock().unwrap();
    g.reads.entry(k).or_insert(v);
}
/// Record a write (post-value) of NFT metadata.
pub fn record_write_nft_kv(id: &NftId, key: &Name, val: &iroha_primitives::json::Json) {
    let k = key_nft_kv(id, key);
    let v = bytes_from_json(val);
    let mut g = slot().lock().unwrap();
    g.writes.insert(k, v);
}
/// Record a delete (post empty) of NFT metadata, with read pre-value supplied.
pub fn record_delete_nft_kv(id: &NftId, key: &Name, pre: &iroha_primitives::json::Json) {
    let k = key_nft_kv(id, key);
    let mut g = slot().lock().unwrap();
    g.reads
        .entry(k.clone())
        .or_insert_with(|| bytes_from_json(pre));
    g.writes.insert(k, Vec::new());
}

/// Record asset balance read (pre-value) for an asset.
pub fn record_read_asset(id: &AssetId, val: Option<&iroha_primitives::numeric::Numeric>) {
    let k = key_asset_balance(id);
    let v = val
        .map(|n| Json::new(n.clone()).get().as_bytes().to_vec())
        .unwrap_or_default();
    let mut g = slot().lock().unwrap();
    g.reads.entry(k).or_insert(v);
}

/// Record asset balance write (post-value) for an asset.
pub fn record_write_asset(id: &AssetId, val: &iroha_primitives::numeric::Numeric) {
    let k = key_asset_balance(id);
    let v = Json::new(val.clone()).get().as_bytes().to_vec();
    let mut g = slot().lock().unwrap();
    g.writes.insert(k, v);
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
    let mut g = slot().lock().unwrap();
    g.reads.entry(k).or_insert(v);
}

/// Record asset definition total supply write (post-value).
pub fn record_write_asset_def_total(
    id: &AssetDefinitionId,
    val: &iroha_primitives::numeric::Numeric,
) {
    let k = key_asset_def_total(id);
    let v = Json::new(val.clone()).get().as_bytes().to_vec();
    let mut g = slot().lock().unwrap();
    g.writes.insert(k, v);
}

/// Record a FASTPQ transfer transcript so `ExecWitness` consumers can replay transfers.
pub fn record_fastpq_transcript(transcript: &TransferTranscript) {
    let mut g = slot().lock().unwrap();
    g.fastpq_transcripts
        .entry(transcript.batch_hash)
        .or_default()
        .push(transcript.clone());
}

/// Record a read (pre-value) of asset-definition metadata.
pub fn record_read_asset_def_kv(
    id: &AssetDefinitionId,
    key: &Name,
    val: Option<&iroha_primitives::json::Json>,
) {
    let k = key_asset_def_kv(id, key);
    let v = val.map(bytes_from_json).unwrap_or_default();
    let mut g = slot().lock().unwrap();
    g.reads.entry(k).or_insert(v);
}
/// Record a write (post-value) of asset-definition metadata.
pub fn record_write_asset_def_kv(
    id: &AssetDefinitionId,
    key: &Name,
    val: &iroha_primitives::json::Json,
) {
    let k = key_asset_def_kv(id, key);
    let v = bytes_from_json(val);
    let mut g = slot().lock().unwrap();
    g.writes.insert(k, v);
}
/// Record a delete (post empty) of asset-definition metadata, with read pre-value supplied.
pub fn record_delete_asset_def_kv(
    id: &AssetDefinitionId,
    key: &Name,
    pre: &iroha_primitives::json::Json,
) {
    let k = key_asset_def_kv(id, key);
    let mut g = slot().lock().unwrap();
    g.reads
        .entry(k.clone())
        .or_insert_with(|| bytes_from_json(pre));
    g.writes.insert(k, Vec::new());
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
                iroha_data_model::domain::DomainId::from_str(dom_s),
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
                iroha_data_model::asset::AssetDefinitionId::from_str(ad_s),
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
                let mut g = slot().lock().unwrap();
                g.reads.entry(k).or_insert(v);
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
            let mut g = slot().lock().unwrap();
            g.reads.entry(out).or_insert(v);
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
            let mut g = slot().lock().unwrap();
            g.reads.entry(k).or_insert(v);
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
            let mut g = slot().lock().unwrap();
            g.reads.entry(k).or_insert(v);
        }
    }
    if let Some(rest) = access_key.strip_prefix("asset:")
        && let Ok(id) = iroha_data_model::asset::AssetId::parse_encoded(rest)
    {
        let pre = state_block.world.assets().get(&id).map(|v| &**v);
        record_read_asset(&id, pre);
        // no further processing needed for asset access
    }
    if let Some(rest) = access_key.strip_prefix("asset_def:")
        && let Ok(ad) = iroha_data_model::asset::AssetDefinitionId::from_str(rest)
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
    let g = slot().lock().unwrap();
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
    let fastpq_transcripts = map_ref_to_bundles(&g.fastpq_transcripts);
    ExecWitness {
        reads,
        writes,
        fastpq_transcripts,
        fastpq_batches: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iroha_test_samples::ALICE_ID;

    use super::*;
    // The SMT helpers live under the sumeragi module.
    use crate::sumeragi::smt::{KvPair, compute_post_state_root};

    #[test]
    fn parity_same_witness_twice_same_root() {
        let _guard = exec_witness_guard();
        // First run
        start_block();
        // Simulate asset balance read+write
        let ad = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
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
    fn records_fastpq_transcripts() {
        use std::str::FromStr;

        use iroha_data_model::{
            asset::id::AssetDefinitionId,
            fastpq::{TransferDeltaTranscript, TransferTranscript},
        };
        use iroha_primitives::numeric::Numeric;
        use iroha_test_samples::{ALICE_ID, BOB_ID};

        let _guard = exec_witness_guard();
        start_block();
        let asset = AssetDefinitionId::new("wonderland".parse().unwrap(), "rose".parse().unwrap());
        let delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: asset,
            amount: Numeric::from(5u32),
            from_balance_before: Numeric::from(100u32),
            from_balance_after: Numeric::from(95u32),
            to_balance_before: Numeric::from(0u32),
            to_balance_after: Numeric::from(5u32),
            from_merkle_proof: None,
            to_merkle_proof: None,
        };
        let batch_hash = Hash::prehashed([0x11; Hash::LENGTH]);
        let transcript = TransferTranscript {
            batch_hash,
            deltas: vec![delta.clone()],
            authority_digest: crate::fastpq::authority_digest(&ALICE_ID),
            poseidon_preimage_digest: None,
        };
        record_fastpq_transcript(&transcript);

        let witness = drain_exec_witness();
        let stored = witness
            .fastpq_transcripts
            .iter()
            .find(|bundle| bundle.entry_hash == batch_hash)
            .expect("transcript recorded");
        assert_eq!(stored.transcripts.len(), 1);
        assert_eq!(stored.transcripts[0], transcript);
        assert!(witness.fastpq_batches.is_empty());
        assert!(witness.reads.is_empty());
        assert!(witness.writes.is_empty());
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
        rx.recv_timeout(Duration::from_secs(1))
            .expect("guard should release");
        handle.join().expect("thread joins");
    }
}

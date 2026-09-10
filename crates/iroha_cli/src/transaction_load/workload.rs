//! Fixed routed account writes and bounded postcondition verification for scaling trials.

use super::*;
use iroha::data_model::{
    parameter::{CustomParameterId, Parameters},
    query::{QueryResponse, SingularQueryOutputBox, executor::FindParameters},
};

/// This is a collector workload bound, not an on-chain metadata limit.
pub(super) const MAX_EFFECTS_PER_ACCOUNT: usize = 1_024;
const MAX_BASELINE_FRAME_BYTES: usize = 16 * 1024;
const MAX_ACCOUNT_FRAME_BYTES: usize = 256 * 1024;
const MAX_QUERY_RESPONSE_BYTES: usize = 512 * 1024;
const WORKLOAD_LOGICAL_ID_BYTES: usize = 64;
const EFFECT_PREFIX: &str = "gscale_";
pub(super) const WORKLOAD_ID: &str = "self_owned_account_metadata_insert_v1";
pub(super) const ACCOUNT_SELECTION: &str = "(zero_based_cohort_sequence + first8le(sha256(gscale-account-offset-v1:seed))) modulo pool_length";

pub(super) fn account_offset(seed: &str, account_count: usize) -> Result<usize> {
    if account_count == 0 || account_count > MAX_ACCOUNTS {
        bail!("workload account pool must contain at most {MAX_ACCOUNTS} accounts");
    }
    let digest = Sha256::digest(format!("gscale-account-offset-v1:{seed}").as_bytes());
    let mut selector = [0_u8; 8];
    selector.copy_from_slice(&digest[..8]);
    Ok((u64::from_le_bytes(selector) % account_count as u64) as usize)
}

pub(super) fn validate_schedule(schedule: &Schedule, account_count: usize) -> Result<()> {
    if account_count == 0 || account_count > MAX_ACCOUNTS || account_count % 4 != 0 {
        bail!("routed workload requires a fixed pool of 4 through 64 accounts divisible by four");
    }
    let warmup = schedule.count(Cohort::Warmup)?;
    let measurement = schedule.count(Cohort::Measurement)?;
    if measurement == 0 || warmup % account_count != 0 || measurement % account_count != 0 {
        bail!("each nonzero workload cohort must contain a whole number of account-pool rounds");
    }
    let total = warmup
        .checked_add(measurement)
        .ok_or_else(|| eyre!("workload effect count overflow"))?;
    if total > MAX_ROWS || total / account_count > MAX_EFFECTS_PER_ACCOUNT {
        bail!(
            "complete warmup and measurement workload exceeds {MAX_EFFECTS_PER_ACCOUNT} insertions per account"
        );
    }
    Ok(())
}

fn effect(plan: &Planned) -> Result<(Name, Json)> {
    if plan.logical_id.len() != WORKLOAD_LOGICAL_ID_BYTES
        || !plan
            .logical_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("workload effect requires a canonical lowercase SHA-256 logical identity");
    }
    let key = format!("{EFFECT_PREFIX}{}", plan.logical_id).parse()?;
    let value = Json::try_new(plan.logical_id.as_str())?;
    Ok((key, value))
}

pub(super) fn executable(authority: &AccountId, plan: &Planned) -> Result<Executable> {
    let (key, value) = effect(plan)?;
    Ok(Executable::Instructions(
        vec![InstructionBox::from(SetKeyValue::account(
            authority.clone(),
            key,
            value,
        ))]
        .into(),
    ))
}

fn validate_value_limit(parameters: &Parameters) -> Result<()> {
    let key = CustomParameterId("max_metadata_value_bytes".parse()?);
    // Mirror the instruction's resolution: malformed/missing custom values use its default.
    // The instruction measures Json::as_ref(), which excludes the outer quotes of
    // string values. Derive the bound from a real value with the exact workload shape;
    // canonical JSON serialization includes two further bytes, but is not that limit.
    let representative = Json::try_new("0".repeat(WORKLOAD_LOGICAL_ID_BYTES))?;
    let required_bytes = u64::try_from(representative.as_ref().len())?;
    let limit = parameters
        .custom()
        .get(&key)
        .and_then(|parameter| parameter.payload().try_into_any_norito::<u64>().ok())
        .unwrap_or(1_048_576);
    if limit < required_bytes {
        bail!(
            "on-chain max_metadata_value_bytes cannot admit the {required_bytes}-byte workload value"
        );
    }
    Ok(())
}

/// Match the SDK's size-derived owned decoder allowance while retaining the
/// collector's frame cap and existing field, element and nesting bounds. This counts
/// cumulative allocations, including intermediate decoded representations.
fn workload_decode_limits(frame_bytes: usize) -> Result<norito::DecodeLimits> {
    if !(1..=MAX_QUERY_RESPONSE_BYTES).contains(&frame_bytes) {
        bail!("workload query frame is outside its fixed byte bound");
    }
    let canonical = norito::canonical_decode_limits(frame_bytes);
    Ok(norito::DecodeLimits::new(
        canonical.max_sequence_elements(),
        canonical.max_field_bytes(),
        canonical.max_total_elements(),
        canonical.max_total_allocated_bytes(),
        64,
    ))
}

fn checked_frame<T>(value: &T, frame_limit: usize) -> Result<Vec<u8>>
where
    T: norito::core::NoritoSerialize,
    for<'de> T: norito::core::NoritoDeserialize<'de>,
{
    if !(1..=MAX_QUERY_RESPONSE_BYTES).contains(&frame_limit) {
        bail!("workload serialization frame limit is outside its fixed byte bound");
    }
    let bytes = norito::core::to_bytes_bounded(value, frame_limit)?;
    let limits = workload_decode_limits(bytes.len())?;
    // Exercise the real canonical decoder and allocation budget, not an estimated JSON size.
    let decoded: T = norito::decode_from_bytes_with_limits(&bytes, limits)?;
    drop(decoded);
    Ok(bytes)
}

fn checked_account(account: &Account, frame_limit: usize) -> Result<Vec<u8>> {
    let bytes = checked_frame(account, frame_limit)?;
    let output = SingularQueryOutputBox::Account(account.clone());
    let _ = checked_frame(&output, MAX_ACCOUNT_FRAME_BYTES)?;
    let _ = checked_frame(&QueryResponse::Singular(output), MAX_QUERY_RESPONSE_BYTES)?;
    Ok(bytes)
}

fn require_baseline(account: &Account, authority: &AccountId) -> Result<()> {
    if account.id() != authority {
        bail!("workload account read returned a different canonical authority");
    }
    if account.label().is_some() || account.uaid().is_some() || !account.opaque_ids().is_empty() {
        bail!("workload accounts must be seeded as universal identities before alias state");
    }
    if account
        .metadata()
        .iter()
        .any(|(key, _)| key.as_ref().starts_with(EFFECT_PREFIX))
    {
        bail!("workload baseline already contains a reserved workload key");
    }
    let _ = checked_account(account, MAX_BASELINE_FRAME_BYTES)?;
    Ok(())
}

fn expected_account(
    baseline: &Account,
    records: &[Record],
    account_index: usize,
) -> Result<(Account, usize)> {
    let mut expected = baseline.clone();
    let mut count = 0_usize;
    for record in records
        .iter()
        .filter(|record| record.plan.account_index == account_index)
    {
        count = count
            .checked_add(1)
            .filter(|count| *count <= MAX_EFFECTS_PER_ACCOUNT)
            .ok_or_else(|| eyre!("workload account exceeds its complete-cohort insertion bound"))?;
        let (key, value) = effect(&record.plan)?;
        if expected.metadata.insert(key, value).is_some() {
            bail!("workload insertion would overwrite an existing logical effect");
        }
    }
    if count == 0 {
        bail!("workload account has no planned effects");
    }
    let _ = checked_account(&expected, MAX_ACCOUNT_FRAME_BYTES)?;
    Ok((expected, count))
}

fn verify_account(expected: &Account, observed: &Account) -> Result<String> {
    let expected_frame = checked_account(expected, MAX_ACCOUNT_FRAME_BYTES)?;
    let observed_frame = checked_account(observed, MAX_ACCOUNT_FRAME_BYTES)?;
    // Account's Eq implementation compares identity only. Compare the complete canonical
    // account representation so missing, overwritten, extra and changed baseline values fail.
    if expected_frame != observed_frame {
        bail!("canonical account does not contain the complete exact workload postcondition");
    }
    Ok(hex::encode(Sha256::digest(&observed_frame)))
}

/// A bounded baseline per signer; expected post-accounts are produced and dropped sequentially.
pub(super) struct Baselines(Vec<Account>);

fn validate_pool(authorities: &[&AccountId], records: &[Record]) -> Result<()> {
    if authorities.is_empty() || authorities.len() > MAX_ACCOUNTS || authorities.len() % 4 != 0 {
        bail!("workload verification account pool is outside its bound");
    }
    let mut identities = BTreeSet::new();
    if authorities
        .iter()
        .any(|authority| !identities.insert(*authority))
    {
        bail!("workload verification duplicates an account identity");
    }
    let maximum = authorities
        .len()
        .checked_mul(MAX_EFFECTS_PER_ACCOUNT)
        .ok_or_else(|| eyre!("workload verification row bound overflow"))?;
    if records.is_empty()
        || records.len() > maximum
        || records
            .iter()
            .any(|record| record.plan.account_index >= authorities.len())
    {
        bail!("workload verification rows do not fit the fixed account pool");
    }
    let mut logical = BTreeSet::new();
    if records
        .iter()
        .any(|record| !logical.insert(record.plan.logical_id.as_str()))
    {
        bail!("workload verification repeats a logical request identity");
    }
    Ok(())
}

fn preflight_accounts(
    authorities: &[&AccountId],
    records: &[Record],
    parameters: &Parameters,
    recorder: &dyn Recorder,
    mut read: impl FnMut(usize, &AccountId) -> Result<Account>,
) -> Result<Baselines> {
    validate_pool(authorities, records)?;
    validate_value_limit(parameters)?;
    let mut baselines = Vec::with_capacity(authorities.len());
    for (index, authority) in authorities.iter().enumerate() {
        let baseline = read(index, authority)?;
        require_baseline(&baseline, authority)?;
        let (expected, count) = expected_account(&baseline, records, index)?;
        let expected_frame = checked_account(&expected, MAX_ACCOUNT_FRAME_BYTES)?;
        recorder.record(norito::json!({"event": "workload_account_preflight",
            "authority": (authority.to_string()), "account_index": index,
            "expected_effects": count, "expected_account_sha256": (hex::encode(Sha256::digest(&expected_frame))),
            "expected_account_frame_bytes": (expected_frame.len())}))?;
        baselines.push(baseline);
    }
    Ok(Baselines(baselines))
}

fn verify_accounts(
    authorities: &[&AccountId],
    records: &[Record],
    baselines: &Baselines,
    recorder: &dyn Recorder,
    mut read: impl FnMut(usize, &AccountId) -> Result<Account>,
) -> Result<()> {
    validate_pool(authorities, records)?;
    if records
        .iter()
        .any(|record| !record.settled() || record.failure.is_some())
    {
        bail!(
            "workload postconditions require every scheduled request to reach global StateApplied"
        );
    }
    if baselines.0.len() != authorities.len() {
        bail!("workload baseline account pool changed before postcondition verification");
    }
    for (index, (authority, baseline)) in authorities.iter().zip(&baselines.0).enumerate() {
        if baseline.id() != *authority {
            bail!("workload baseline authority changed before verification");
        }
        let (expected, count) = expected_account(baseline, records, index)?;
        let observed = read(index, authority)?;
        let digest = verify_account(&expected, &observed)?;
        recorder.record(norito::json!({"event": "workload_account_postcondition",
            "authority": (authority.to_string()), "account_index": index,
            "verified_effects": count, "account_sha256": digest,
            "read_source": "signed_find_account_by_id_after_complete_drain"}))?;
    }
    Ok(())
}

pub(super) fn preflight(
    backend: &SdkBackend,
    records: &[Record],
    recorder: &dyn Recorder,
) -> Result<Baselines> {
    if backend.clients.len() != backend.accounts.len() || backend.clients.is_empty() {
        bail!("workload clients do not match the fixed account pool");
    }
    let parameters = backend.clients[0]
        .query_single(FindParameters)
        .map_err(|_| eyre!("workload parameter preflight failed; remote details omitted"))?;
    let authorities: Vec<_> = backend
        .accounts
        .iter()
        .map(AccountClient::authority)
        .collect();
    preflight_accounts(
        &authorities,
        records,
        &parameters,
        recorder,
        |index, authority| {
            backend.clients[index]
                .query_single(FindAccountById::new(authority.clone()))
                .map_err(|_| {
                    eyre!("workload account preflight read failed; remote details omitted")
                })
        },
    )
}

pub(super) fn verify(
    backend: &SdkBackend,
    records: &[Record],
    baselines: &Baselines,
    recorder: &dyn Recorder,
) -> Result<()> {
    if backend.clients.len() != backend.accounts.len() {
        bail!("workload clients changed before postcondition verification");
    }
    let authorities: Vec<_> = backend
        .accounts
        .iter()
        .map(AccountClient::authority)
        .collect();
    // Sequential real signed queries bound retained responses to one SDK body (64 MiB hard
    // transport ceiling) and one decoded account at a time. The smaller collector checks
    // reject before publication; unchanged Torii source/response admission applies first.
    verify_accounts(
        &authorities,
        records,
        baselines,
        recorder,
        |index, authority| {
            backend.clients[index]
                .query_single(FindAccountById::new(authority.clone()))
                .map_err(|_| {
                    eyre!("workload account postcondition read failed; remote details omitted")
                })
        },
    )
    // These reads prove useful effects. The deployment's separate canonical evidence owner
    // must still bind the observed application carrier, route, incarnation and committee.
}

#[cfg(test)]
#[path = "workload_tests.rs"]
mod tests;

use std::collections::HashMap;
use eyre::{Result, WrapErr, bail};
use hickory_proto::{
    op::{Message, Query, ResponseCode},
    rr::Record,
};
use norito_derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use tracing::warn;
use crate::{
    bundle::ProofBundleV1,
    config::{FreezeMetadata, FreezeState, StaticZone},
    dns,
    limits::{
        MAX_IDENTIFIER_BYTES, MAX_STATE_BUNDLES, MAX_STATE_RAD_ENTRIES, MAX_STATE_RETAINED_BYTES,
        MAX_STATIC_ZONES,
    },
    rad::{ResolverAttestation, rad_retained_bytes},
};
/// In-memory resolver state shared between tasks.
#[derive(Debug, Default)]
pub struct ResolverState {
    resolver_id: String,
    region: String,
    bundles: HashMap<String, ProofBundleV1>,
    resolver_adverts: HashMap<String, ResolverAttestation>,
    static_zones: HashMap<String, StaticZoneEntry>,
    bundle_retained_bytes: usize,
    rad_retained_bytes: usize,
    static_retained_bytes: usize,
}
impl ResolverState {
    #[must_use]
    pub fn new(resolver_id: String, region: String) -> Self {
        Self {
            resolver_id,
            region,
            bundles: HashMap::new(),
            resolver_adverts: HashMap::new(),
            static_zones: HashMap::new(),
            bundle_retained_bytes: 0,
            rad_retained_bytes: 0,
            static_retained_bytes: 0,
        }
    }
    #[must_use]
    pub fn resolver_id(&self) -> &str {
        &self.resolver_id
    }
    #[must_use]
    pub fn region(&self) -> &str {
        &self.region
    }
    pub fn update_bundles(
        &mut self,
        bundles: HashMap<String, ProofBundleV1>,
    ) -> Result<BundleDiff> {
        let retained_bytes = bundle_map_retained_bytes(&bundles)?;
        self.admit_projected_state(retained_bytes, self.rad_retained_bytes)?;
        let mut diff = BundleDiff::default();
        diff.reserve(bundles.len().saturating_add(self.bundles.len()))?;
        for (namehash, bundle) in &bundles {
            let snapshot = BundleSnapshot::from_bundle(bundle);
            if let Some(existing) = self.bundles.get(namehash) {
                let previous_snapshot = BundleSnapshot::from_bundle(existing);
                if existing.zone_version < bundle.zone_version {
                    diff.updated
                        .push((namehash.clone(), previous_snapshot, snapshot.clone()));
                } else if existing.zone_version > bundle.zone_version {
                    diff.reorged
                        .push((namehash.clone(), previous_snapshot, snapshot.clone()));
                }
            } else {
                diff.added.push((namehash.clone(), snapshot.clone()));
            }
        }
        for (namehash, bundle) in &self.bundles {
            if !bundles.contains_key(namehash) {
                let snapshot = BundleSnapshot::from_bundle(bundle);
                diff.removed.push((namehash.clone(), snapshot));
            }
        }
        self.bundles = bundles;
        self.bundle_retained_bytes = retained_bytes;
        Ok(diff)
    }
    pub fn update_resolver_adverts(
        &mut self,
        adverts: HashMap<String, ResolverAttestation>,
    ) -> Result<ResolverDiff> {
        let retained_bytes = rad_map_retained_bytes(&adverts)?;
        self.admit_projected_state(self.bundle_retained_bytes, retained_bytes)?;
        let mut diff = ResolverDiff::default();
        diff.reserve(adverts.len().saturating_add(self.resolver_adverts.len()))?;
        for (resolver_id, advert) in &adverts {
            match self.resolver_adverts.get(resolver_id) {
                Some(existing) if existing == advert => {}
                Some(_) => diff.updated.push(resolver_id.clone()),
                None => diff.added.push(resolver_id.clone()),
            }
        }
        for resolver_id in self.resolver_adverts.keys() {
            if !adverts.contains_key(resolver_id) {
                diff.removed.push(resolver_id.clone());
            }
        }
        self.resolver_adverts = adverts;
        self.rad_retained_bytes = retained_bytes;
        Ok(diff)
    }
    pub(crate) fn update_static_zones(&mut self, zones: &[StaticZone]) -> Result<()> {
        if zones.len() > MAX_STATIC_ZONES {
            bail!(
                "static-zone state contains {} entries; the limit is {MAX_STATIC_ZONES}",
                zones.len()
            );
        }
        let retained_bytes = zones.iter().try_fold(
            map_bucket_bytes::<String, StaticZoneEntry>(zones.len())?,
            |total, zone| {
                total
                    .checked_add(zone.retained_bytes)
                    .ok_or_else(|| eyre::eyre!("static-zone retained-byte accounting overflow"))
            },
        )?;
        admitted_state_retained_bytes(
            retained_bytes,
            self.bundle_retained_bytes,
            self.rad_retained_bytes,
        )?;
        let mut next = HashMap::new();
        next.try_reserve(zones.len())
            .wrap_err("failed to reserve bounded static-zone state map")?;
        for zone in zones {
            next.insert(
                zone.domain.clone(),
                StaticZoneEntry {
                    records: zone.records.clone(),
                    freeze: zone.freeze.clone(),
                },
            );
        }
        self.static_zones = next;
        self.static_retained_bytes = retained_bytes;
        Ok(())
    }
    fn admit_projected_state(&self, bundle_bytes: usize, rad_bytes: usize) -> Result<()> {
        admitted_state_retained_bytes(self.static_retained_bytes, bundle_bytes, rad_bytes)?;
        Ok(())
    }
    #[must_use]
    pub fn bundle_count(&self) -> usize {
        self.bundles.len()
    }
    #[must_use]
    pub fn zone_count(&self) -> usize {
        self.bundles.len()
    }
    #[must_use]
    pub fn resolver_advert_count(&self) -> usize {
        self.resolver_adverts.len()
    }
    #[must_use]
    pub fn static_zone_count(&self) -> usize {
        self.static_zones.len()
    }
    /// Remove proof bundles and resolver attestations that are outside their validity windows.
    pub fn prune_stale_entries(&mut self, now_unix: i64) -> Result<ExpiryDiff> {
        let mut diff = ExpiryDiff::default();
        diff.expired_bundles
            .try_reserve(self.bundles.len())
            .wrap_err("failed to reserve bundle-expiry diff")?;
        diff.expired_resolvers
            .try_reserve(self.resolver_adverts.len())
            .wrap_err("failed to reserve resolver-expiry diff")?;
        self.bundles.retain(|namehash, bundle| {
            let issued_at = bundle.freshness.issued_at as i64;
            let expires_at = bundle.freshness.expires_at as i64;
            let not_yet_valid = now_unix < issued_at;
            let expired = now_unix >= expires_at;
            if not_yet_valid || expired {
                let snapshot = BundleSnapshot::from_bundle(bundle);
                diff.expired_bundles.push((namehash.clone(), snapshot));
                return false;
            }
            true
        });
        self.resolver_adverts.retain(|resolver_id, advert| {
            match advert.valid_from_unix.cmp(&(now_unix as u64)) {
                std::cmp::Ordering::Greater => {
                    diff.expired_resolvers.push(ResolverInvalidation {
                        resolver_id: resolver_id.clone(),
                        reason: InvalidationReason::NotYetValid.as_str().to_string(),
                    });
                    false
                }
                std::cmp::Ordering::Less | std::cmp::Ordering::Equal
                    if advert.valid_until_unix <= now_unix as u64 =>
                {
                    diff.expired_resolvers.push(ResolverInvalidation {
                        resolver_id: resolver_id.clone(),
                        reason: InvalidationReason::Expired.as_str().to_string(),
                    });
                    false
                }
                _ => true,
            }
        });
        self.bundle_retained_bytes = bundle_map_retained_bytes(&self.bundles)?;
        self.rad_retained_bytes = rad_map_retained_bytes(&self.resolver_adverts)?;
        Ok(diff)
    }
    /// Resolve a DNS message using the in-memory state.
    #[must_use]
    pub fn resolve_message(&self, request: &Message) -> Message {
        if let Some(response) = self.resolve_static(request) {
            response
        } else if self.static_zones.is_empty() {
            dns::build_servfail_response(request)
        } else {
            dns::build_nxdomain_response(request)
        }
    }
    /// Produce a metrics snapshot derived from the in-memory view.
    #[must_use]
    pub fn metrics_snapshot(&self, now_unix: i64) -> ResolverStateMetrics {
        let mut proof_age_max_secs: Option<i64> = None;
        let mut proof_ttl_min_secs: Option<i64> = None;
        for bundle in self.bundles.values() {
            let issued_at = bundle.freshness.issued_at as i64;
            let expires_at = bundle.freshness.expires_at as i64;
            let age = now_unix.saturating_sub(issued_at);
            let ttl = expires_at.saturating_sub(now_unix);
            proof_age_max_secs = Some(match proof_age_max_secs {
                Some(current) => current.max(age),
                None => age,
            });
            proof_ttl_min_secs = Some(match proof_ttl_min_secs {
                Some(current) => current.min(ttl),
                None => ttl,
            });
        }
        ResolverStateMetrics {
            resolver_id: self.resolver_id.clone(),
            region: self.region.clone(),
            bundle_count: self.bundles.len(),
            resolver_advert_count: self.resolver_advert_count(),
            static_zone_count: self.static_zone_count(),
            proof_age_max_secs,
            proof_ttl_min_secs,
        }
    }
    fn resolve_static(&self, request: &Message) -> Option<Message> {
        let query = request.queries.first()?;
        let key = canonical_query_name(query);
        let entry = self.static_zones.get(&key)?;
        if let Some((meta, code)) = entry
            .freeze
            .as_ref()
            .and_then(|meta| freeze_response_code(meta).map(|code| (meta, code)))
        {
            warn!(
                target: "soradns::freeze",
                domain = %key,
                state = ?meta.state,
                ticket = meta.ticket.as_deref().unwrap_or("unknown"),
                expires_at = meta.expires_at.as_deref().unwrap_or(""),
                notes = ?meta.notes,
                "refusing query due to active SNS freeze"
            );
            return Some(build_authoritative_response(request, code, &[]));
        }
        Some(build_authoritative_response(
            request,
            ResponseCode::NoError,
            &entry.records,
        ))
    }
}
fn map_bucket_bytes<K, V>(capacity: usize) -> Result<usize> {
    capacity
        .checked_mul(std::mem::size_of::<(K, V)>())
        // Hash-table control bytes and allocator rounding are charged by a
        // second fixed-width unit per admitted bucket.
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or_else(|| eyre::eyre!("resolver state map capacity accounting overflow"))
}
fn admitted_state_retained_bytes(
    static_bytes: usize,
    bundle_bytes: usize,
    rad_bytes: usize,
) -> Result<usize> {
    static_bytes
        .checked_add(bundle_bytes)
        .and_then(|bytes| bytes.checked_add(rad_bytes))
        .filter(|bytes| *bytes <= MAX_STATE_RETAINED_BYTES)
        .ok_or_else(|| {
            eyre::eyre!(
                "resolver state exceeds the {MAX_STATE_RETAINED_BYTES}-byte retained-memory limit"
            )
        })
}
fn charge_key(total: &mut usize, key: &str, capacity: usize) -> Result<()> {
    if key.len() > MAX_IDENTIFIER_BYTES {
        bail!(
            "resolver state key contains {} bytes; the limit is {MAX_IDENTIFIER_BYTES}",
            key.len()
        );
    }
    *total = (*total)
        .checked_add(capacity)
        .ok_or_else(|| eyre::eyre!("resolver state key accounting overflow"))?;
    Ok(())
}
fn bundle_map_retained_bytes(bundles: &HashMap<String, ProofBundleV1>) -> Result<usize> {
    validate_state_count("proof-bundle state", bundles.len(), MAX_STATE_BUNDLES)?;
    let mut retained = map_bucket_bytes::<String, ProofBundleV1>(bundles.capacity())?;
    for (key, bundle) in bundles {
        charge_key(&mut retained, key, key.capacity())?;
        retained = retained
            .checked_add(bundle.retained_bytes()?)
            .ok_or_else(|| eyre::eyre!("proof-bundle state accounting overflow"))?;
    }
    Ok(retained)
}
fn rad_map_retained_bytes(adverts: &HashMap<String, ResolverAttestation>) -> Result<usize> {
    validate_state_count("RAD state", adverts.len(), MAX_STATE_RAD_ENTRIES)?;
    let mut retained = map_bucket_bytes::<String, ResolverAttestation>(adverts.capacity())?;
    for (key, advert) in adverts {
        charge_key(&mut retained, key, key.capacity())?;
        retained = retained
            .checked_add(rad_retained_bytes(advert)?)
            .ok_or_else(|| eyre::eyre!("RAD state accounting overflow"))?;
    }
    Ok(retained)
}
fn validate_state_count(label: &str, count: usize, maximum: usize) -> Result<()> {
    if count > maximum {
        bail!("{label} contains {count} entries; the limit is {maximum}");
    }
    Ok(())
}
fn canonical_query_name(query: &Query) -> String {
    query.name().to_ascii().trim_end_matches('.').to_lowercase()
}
#[derive(Debug, Clone)]
struct StaticZoneEntry {
    records: Vec<Record>,
    freeze: Option<FreezeMetadata>,
}
fn build_authoritative_response(
    request: &Message,
    code: ResponseCode,
    answers: &[Record],
) -> Message {
    let mut response = Message::response(request.metadata.id, request.metadata.op_code);
    response.metadata.recursion_desired = request.metadata.recursion_desired;
    response.metadata.recursion_available = true;
    response.metadata.authoritative = true;
    response.metadata.response_code = code;
    if let Some(query) = request.queries.first() {
        response.add_query(query.clone());
    }
    if code == ResponseCode::NoError {
        for record in answers {
            response.add_answer(record.clone());
        }
    }
    response
}
fn freeze_response_code(metadata: &FreezeMetadata) -> Option<ResponseCode> {
    match metadata.state {
        FreezeState::Soft => Some(ResponseCode::ServFail),
        FreezeState::Hard | FreezeState::Emergency => Some(ResponseCode::Refused),
        FreezeState::Thawing | FreezeState::Monitoring => None,
    }
}
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct BundleSnapshot {
    pub zone_version: u64,
    pub manifest_hash_hex: String,
    pub policy_hash_hex: String,
    pub car_root_cid: String,
    pub freshness_issued_at: u64,
    pub freshness_expires_at: u64,
    pub freshness_signer: String,
    pub freshness_signature_hex: String,
}
impl BundleSnapshot {
    fn from_bundle(bundle: &ProofBundleV1) -> Self {
        Self {
            zone_version: bundle.zone_version,
            manifest_hash_hex: hex::encode(bundle.manifest_hash),
            policy_hash_hex: hex::encode(bundle.policy_hash),
            car_root_cid: bundle.car_root_cid.clone(),
            freshness_issued_at: bundle.freshness.issued_at,
            freshness_expires_at: bundle.freshness.expires_at,
            freshness_signer: bundle.freshness.signer.clone(),
            freshness_signature_hex: hex::encode(&bundle.freshness.signature),
        }
    }
}
#[derive(Debug, Default, Clone)]
pub struct BundleDiff {
    pub added: Vec<(String, BundleSnapshot)>,
    pub updated: Vec<(String, BundleSnapshot, BundleSnapshot)>,
    pub reorged: Vec<(String, BundleSnapshot, BundleSnapshot)>,
    pub removed: Vec<(String, BundleSnapshot)>,
}
impl BundleDiff {
    fn reserve(&mut self, maximum_changes: usize) -> Result<()> {
        self.added
            .try_reserve(maximum_changes)
            .wrap_err("failed to reserve added-bundle diff")?;
        self.updated
            .try_reserve(maximum_changes)
            .wrap_err("failed to reserve updated-bundle diff")?;
        self.reorged
            .try_reserve(maximum_changes)
            .wrap_err("failed to reserve reorged-bundle diff")?;
        self.removed
            .try_reserve(maximum_changes)
            .wrap_err("failed to reserve removed-bundle diff")?;
        Ok(())
    }
}
#[derive(Debug, Default)]
pub struct ResolverDiff {
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
}
impl ResolverDiff {
    fn reserve(&mut self, maximum_changes: usize) -> Result<()> {
        self.added
            .try_reserve(maximum_changes)
            .wrap_err("failed to reserve added-resolver diff")?;
        self.updated
            .try_reserve(maximum_changes)
            .wrap_err("failed to reserve updated-resolver diff")?;
        self.removed
            .try_reserve(maximum_changes)
            .wrap_err("failed to reserve removed-resolver diff")?;
        Ok(())
    }
}
/// Differences surfaced when expiring stale bundles or resolver attestations.
#[derive(Debug, Default, Clone)]
pub struct ExpiryDiff {
    pub expired_bundles: Vec<(String, BundleSnapshot)>,
    pub expired_resolvers: Vec<ResolverInvalidation>,
}
/// Resolver invalidation event captured during pruning.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct ResolverInvalidation {
    pub resolver_id: String,
    pub reason: String,
}
/// Reason a resolver attestation was invalidated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidationReason {
    Expired,
    NotYetValid,
}
impl InvalidationReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Expired => "expired",
            Self::NotYetValid => "not_yet_valid",
        }
    }
}
/// Snapshot of resolver state metrics exposed via HTTP.
#[derive(Debug, Clone)]
pub struct ResolverStateMetrics {
    pub resolver_id: String,
    pub region: String,
    pub bundle_count: usize,
    pub resolver_advert_count: usize,
    pub static_zone_count: usize,
    pub proof_age_max_secs: Option<i64>,
    pub proof_ttl_min_secs: Option<i64>,
}
#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use hickory_proto::{
        op::{Message, MessageType, OpCode, Query},
        rr::{Name, RData, Record, RecordType, rdata::A},
    };
    use iroha_crypto::{PublicKey, Signature};
    use iroha_data_model::{
        account::AccountId,
        soradns::{
            GatewayHostSet, HttpTransportV1, PaddingPolicyV1, ResolverTlsBundle,
            ResolverTransportBundle, RotationPolicyV1, TlsProvisioningProfile, TlsTransportV1,
        },
    };
    use iroha_primitives::soradns::derive_gateway_hosts;
    use super::*;
    use crate::{
        bundle::{DelegationProofV1, FreshnessProofV1, KskEntryV1, ProofBundleV1, ZskSignatureV1},
        config::StaticZone,
    };
    fn sample_bundle(version: u64) -> ProofBundleV1 {
        ProofBundleV1 {
            namehash: [1; 32],
            zone_version: version,
            manifest_hash: [2; 32],
            car_root_cid: "cid".to_string(),
            ksk_set: vec![KskEntryV1 {
                public_key: vec![1],
                valid_from: 0,
                valid_until: 1,
                signature: vec![1],
            }],
            zsk_signatures: vec![ZskSignatureV1 {
                zsk_id: vec![1],
                signature: vec![1],
            }],
            delegation_chain: vec![DelegationProofV1 {
                parent_namehash: [4; 32],
                child_namehash: [5; 32],
                valid_from: 0,
                valid_until: 1,
                signature: vec![1],
            }],
            freshness: FreshnessProofV1 {
                issued_at: 0,
                expires_at: 1,
                signer: "signer".to_string(),
                signature: vec![1],
            },
            policy_hash: [3; 32],
        }
    }
    #[test]
    fn update_bundles_records_diffs() {
        let mut state = ResolverState::new("resolver".into(), "global".into());
        let mut first = HashMap::new();
        first.insert("hash".into(), sample_bundle(1));
        let diff_first = state.update_bundles(first).expect("bounded first update");
        assert_eq!(diff_first.added.len(), 1);
        assert_eq!(diff_first.added[0].0, "hash");
        assert_eq!(diff_first.added[0].1.zone_version, 1);
        let mut second = HashMap::new();
        second.insert("hash".into(), sample_bundle(2));
        let diff_second = state.update_bundles(second).expect("bounded second update");
        assert_eq!(diff_second.updated.len(), 1);
        assert_eq!(diff_second.updated[0].0, "hash");
        assert_eq!(diff_second.updated[0].1.zone_version, 1);
        assert_eq!(diff_second.updated[0].2.zone_version, 2);
        let diff_remove = state
            .update_bundles(HashMap::new())
            .expect("bounded removal update");
        assert_eq!(diff_remove.removed.len(), 1);
        assert_eq!(diff_remove.removed[0].0, "hash");
        assert_eq!(diff_remove.removed[0].1.zone_version, 2);
    }
    #[test]
    fn resolve_message_static_zone() {
        let mut state = ResolverState::new("resolver".into(), "global".into());
        let record = Record::from_rdata(
            Name::from_ascii("example.sora.").unwrap(),
            300,
            RData::A(A(Ipv4Addr::new(192, 0, 2, 1))),
        );
        state
            .update_static_zones(&[StaticZone {
                domain: "example.sora".into(),
                records: vec![record.clone()],
                freeze: None,
                retained_bytes: 1024,
            }])
            .expect("bounded static zones");
        let mut request = Message::new(0, MessageType::Query, OpCode::Query);
        request.add_query(Query::query(
            Name::from_ascii("example.sora").unwrap(),
            RecordType::A,
        ));
        let response = state.resolve_message(&request);
        assert_eq!(response.metadata.response_code, ResponseCode::NoError);
        assert_eq!(response.answers.len(), 1);
    }
    #[test]
    fn resolve_message_soft_freeze_returns_servfail() {
        let mut state = ResolverState::new("resolver".into(), "global".into());
        let record = Record::from_rdata(
            Name::from_ascii("freeze.sora.").unwrap(),
            300,
            RData::A(A(Ipv4Addr::new(192, 0, 2, 2))),
        );
        state
            .update_static_zones(&[StaticZone {
                domain: "freeze.sora".into(),
                records: vec![record],
                freeze: Some(FreezeMetadata {
                    state: FreezeState::Soft,
                    ticket: Some("SNS-DF-42".into()),
                    expires_at: None,
                    notes: vec![],
                }),
                retained_bytes: 1024,
            }])
            .expect("bounded static zones");
        let mut request = Message::new(0, MessageType::Query, OpCode::Query);
        request.add_query(Query::query(
            Name::from_ascii("freeze.sora").unwrap(),
            RecordType::A,
        ));
        let response = state.resolve_message(&request);
        assert_eq!(response.metadata.response_code, ResponseCode::ServFail);
        assert!(response.answers.is_empty());
    }
    #[test]
    fn resolve_message_hard_freeze_returns_refused() {
        let mut state = ResolverState::new("resolver".into(), "global".into());
        let record = Record::from_rdata(
            Name::from_ascii("blocked.sora.").unwrap(),
            300,
            RData::A(A(Ipv4Addr::new(192, 0, 2, 3))),
        );
        state
            .update_static_zones(&[StaticZone {
                domain: "blocked.sora".into(),
                records: vec![record],
                freeze: Some(FreezeMetadata {
                    state: FreezeState::Hard,
                    ticket: None,
                    expires_at: None,
                    notes: vec![],
                }),
                retained_bytes: 1024,
            }])
            .expect("bounded static zones");
        let mut request = Message::new(0, MessageType::Query, OpCode::Query);
        request.add_query(Query::query(
            Name::from_ascii("blocked.sora").unwrap(),
            RecordType::A,
        ));
        let response = state.resolve_message(&request);
        assert_eq!(response.metadata.response_code, ResponseCode::Refused);
        assert!(response.answers.is_empty());
    }
    #[test]
    fn resolve_message_emergency_freeze_returns_refused() {
        let mut state = ResolverState::new("resolver".into(), "global".into());
        let record = Record::from_rdata(
            Name::from_ascii("emergency.sora.").unwrap(),
            300,
            RData::A(A(Ipv4Addr::new(192, 0, 2, 44))),
        );
        state
            .update_static_zones(&[StaticZone {
                domain: "emergency.sora".into(),
                records: vec![record],
                freeze: Some(FreezeMetadata {
                    state: FreezeState::Emergency,
                    ticket: Some("SNS-DF-EMERGENCY".into()),
                    expires_at: Some("2026-03-10T00:00:00Z".into()),
                    notes: vec!["guardian hold".into()],
                }),
                retained_bytes: 1024,
            }])
            .expect("bounded static zones");
        let mut request = Message::new(0, MessageType::Query, OpCode::Query);
        request.add_query(Query::query(
            Name::from_ascii("emergency.sora").unwrap(),
            RecordType::A,
        ));
        let response = state.resolve_message(&request);
        assert_eq!(response.metadata.response_code, ResponseCode::Refused);
        assert!(
            response.answers.is_empty(),
            "emergency freeze must refuse DNS answers"
        );
    }
    #[test]
    fn resolve_message_thawing_freeze_serves_records() {
        let mut state = ResolverState::new("resolver".into(), "global".into());
        let record = Record::from_rdata(
            Name::from_ascii("thawing.sora.").unwrap(),
            300,
            RData::A(A(Ipv4Addr::new(192, 0, 2, 45))),
        );
        state
            .update_static_zones(&[StaticZone {
                domain: "thawing.sora".into(),
                records: vec![record.clone()],
                freeze: Some(FreezeMetadata {
                    state: FreezeState::Thawing,
                    ticket: None,
                    expires_at: Some("2026-03-11T00:00:00Z".into()),
                    notes: vec![],
                }),
                retained_bytes: 1024,
            }])
            .expect("bounded static zones");
        let mut request = Message::new(0, MessageType::Query, OpCode::Query);
        request.add_query(Query::query(
            Name::from_ascii("thawing.sora").unwrap(),
            RecordType::A,
        ));
        let response = state.resolve_message(&request);
        assert_eq!(response.metadata.response_code, ResponseCode::NoError);
        assert_eq!(response.answers.len(), 1);
        assert_eq!(response.answers[0], record);
    }
    #[test]
    fn resolve_message_monitoring_freeze_serves_records() {
        let mut state = ResolverState::new("resolver".into(), "global".into());
        let record = Record::from_rdata(
            Name::from_ascii("monitoring.sora.").unwrap(),
            300,
            RData::A(A(Ipv4Addr::new(192, 0, 2, 46))),
        );
        state
            .update_static_zones(&[StaticZone {
                domain: "monitoring.sora".into(),
                records: vec![record.clone()],
                freeze: Some(FreezeMetadata {
                    state: FreezeState::Monitoring,
                    ticket: Some("SNS-DF-MONITORING".into()),
                    expires_at: None,
                    notes: vec!["watch only".into()],
                }),
                retained_bytes: 1024,
            }])
            .expect("bounded static zones");
        let mut request = Message::new(0, MessageType::Query, OpCode::Query);
        request.add_query(Query::query(
            Name::from_ascii("monitoring.sora").unwrap(),
            RecordType::A,
        ));
        let response = state.resolve_message(&request);
        assert_eq!(response.metadata.response_code, ResponseCode::NoError);
        assert_eq!(response.answers.len(), 1);
        assert_eq!(response.answers[0], record);
    }
    #[test]
    fn resolve_message_nxdomain() {
        let mut state = ResolverState::new("resolver".into(), "global".into());
        let record = Record::from_rdata(
            Name::from_ascii("existing.sora.").unwrap(),
            60,
            RData::A(A(Ipv4Addr::new(203, 0, 113, 1))),
        );
        state
            .update_static_zones(&[StaticZone {
                domain: "existing.sora".into(),
                records: vec![record],
                freeze: None,
                retained_bytes: 1024,
            }])
            .expect("bounded static zones");
        let mut request = Message::new(0, MessageType::Query, OpCode::Query);
        request.add_query(Query::query(
            Name::from_ascii("missing.sora").unwrap(),
            RecordType::A,
        ));
        let response = state.resolve_message(&request);
        assert_eq!(response.metadata.response_code, ResponseCode::NXDomain);
    }
    #[test]
    fn resolve_message_servfail_without_static_zones() {
        let state = ResolverState::new("resolver".into(), "global".into());
        let mut request = Message::new(0, MessageType::Query, OpCode::Query);
        request.add_query(Query::query(
            Name::from_ascii("missing.sora").unwrap(),
            RecordType::A,
        ));
        let response = state.resolve_message(&request);
        assert_eq!(response.metadata.response_code, ResponseCode::ServFail);
    }
    #[test]
    fn prune_stale_entries_drops_expired_items() {
        let mut state = ResolverState::new("resolver".into(), "global".into());
        let mut bundles = HashMap::new();
        bundles.insert("hash".into(), sample_bundle(1));
        state.update_bundles(bundles).expect("bounded bundle state");
        let mut adverts = HashMap::new();
        adverts.insert("deadbeef".into(), sample_rad(0, 1));
        state
            .update_resolver_adverts(adverts)
            .expect("bounded RAD state");
        let diff = state.prune_stale_entries(2).expect("bounded expiry diff");
        assert_eq!(state.bundle_count(), 0);
        assert_eq!(state.resolver_advert_count(), 0);
        assert_eq!(diff.expired_bundles.len(), 1);
        assert_eq!(diff.expired_resolvers.len(), 1);
        assert_eq!(
            diff.expired_resolvers[0].reason,
            InvalidationReason::Expired.as_str()
        );
    }
    #[test]
    fn aggregate_state_budget_accepts_exact_and_rejects_plus_one() {
        assert_eq!(
            admitted_state_retained_bytes(
                MAX_STATE_RETAINED_BYTES / 2,
                MAX_STATE_RETAINED_BYTES / 4,
                MAX_STATE_RETAINED_BYTES / 4,
            )
            .expect("exact aggregate boundary"),
            MAX_STATE_RETAINED_BYTES
        );
        assert!(
            admitted_state_retained_bytes(MAX_STATE_RETAINED_BYTES, 1, 0).is_err(),
            "aggregate max + 1 must fail"
        );
    }
    #[test]
    fn state_count_budget_accepts_exact_and_rejects_plus_one() {
        validate_state_count("test map", MAX_STATE_BUNDLES, MAX_STATE_BUNDLES)
            .expect("exact map count");
        assert!(
            validate_state_count("test map", MAX_STATE_BUNDLES + 1, MAX_STATE_BUNDLES).is_err()
        );
    }
    fn sample_rad(valid_from_unix: u64, valid_until_unix: u64) -> ResolverAttestation {
        let bindings = derive_gateway_hosts("docs.sora").expect("derive hosts");
        let operator_account = {
            let public_key: PublicKey =
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                    .parse()
                    .expect("public key literal");
            AccountId::new(public_key)
        };
        ResolverAttestation {
            version: 1,
            resolver_id: [1; 32],
            fqdn: "docs.sora".into(),
            canonical_hosts: GatewayHostSet::from(&bindings),
            transport: ResolverTransportBundle {
                doh: Some(HttpTransportV1 {
                    endpoint: "https://docs.sora/dns-query".to_string(),
                    supports_get: true,
                    supports_post: true,
                    max_response_bytes: 2_048,
                }),
                dot: Some(TlsTransportV1 {
                    endpoint: "tls://docs.sora:853".to_string(),
                    alpn_protocols: vec!["dot".to_string()],
                    cipher_suites: vec!["TLS_AES_256_GCM_SHA384".to_string()],
                }),
                doq: None,
                odoh_relay: None,
                soranet_bridge: None,
                qname_minimisation: true,
                padding_policy: PaddingPolicyV1 {
                    min_bytes: 32,
                    max_bytes: 64,
                    pad_to_block: 16,
                },
            },
            tls: ResolverTlsBundle {
                provisioning_profiles: vec![TlsProvisioningProfile::Dns01],
                certificate_fingerprints: vec!["fp".to_string()],
                wildcard_hosts: vec!["*.gw.sora.id".to_string()],
                not_after_unix: 1_800_000_000,
            },
            resolver_manifest_hash: [2; 32],
            gar_manifest_hash: [3; 32],
            issued_at_unix: valid_from_unix,
            valid_from_unix,
            valid_until_unix,
            operator_account,
            operator_signature: Signature::try_from_bytes(&[0xA6; 64])
                .expect("RAD fixture operator signature is non-empty and nonzero"),
            governance_signature: Signature::try_from_bytes(&[1; 64])
                .expect("RAD fixture governance signature is non-empty and nonzero"),
            rotation_policy: RotationPolicyV1 {
                max_lifetime_days: 30,
                required_overlap_seconds: 86_400,
                require_dual_signatures: true,
            },
            telemetry_endpoint: None,
        }
    }
}

//! Governance and compliance denylist handling for the SoraFS gateway.

use std::{
    collections::hash_map::RandomState,
    time::{Duration, SystemTime},
};

use dashmap::DashMap;

/// Kind of resource blocked by the denylist.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DenylistKind {
    /// Provider identifier (32-byte canonical value).
    Provider([u8; 32]),
    /// Manifest digest (BLAKE3-256).
    ManifestDigest([u8; 32]),
    /// Content identifier (CID bytes).
    Cid(Vec<u8>),
    /// Canonical URL string.
    Url(String),
    /// Canonical account identifier (AccountAddress encoding).
    AccountId(String),
    /// Routing alias in `<alias>@<domain>` form.
    AccountAlias(String),
    /// Perceptual hash/embedding family identifier.
    PerceptualFamily {
        /// Canonical family identifier (UUID).
        family_id: [u8; 16],
        /// Optional variant identifier.
        variant_id: Option<[u8; 16]>,
    },
}

/// Policy tier applied to a denylist entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DenylistPolicyTier {
    /// Standard review cadence and TTL.
    Standard,
    /// Emergency canon with shortened TTL/review window.
    Emergency,
    /// Permanent entry backed by a governance supermajority.
    Permanent,
}

/// Policy limits applied when loading denylist entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DenylistPolicy {
    standard_ttl: Duration,
    emergency_ttl: Duration,
    emergency_review_window: Duration,
    require_governance_reference: bool,
}

impl DenylistPolicy {
    /// Construct a new policy configuration.
    #[must_use]
    pub const fn new(
        standard_ttl: Duration,
        emergency_ttl: Duration,
        emergency_review_window: Duration,
        require_governance_reference: bool,
    ) -> Self {
        Self {
            standard_ttl,
            emergency_ttl,
            emergency_review_window,
            require_governance_reference,
        }
    }

    /// Maximum TTL for standard entries.
    #[must_use]
    pub const fn standard_ttl(&self) -> Duration {
        self.standard_ttl
    }

    /// Maximum TTL for emergency entries.
    #[must_use]
    pub const fn emergency_ttl(&self) -> Duration {
        self.emergency_ttl
    }

    /// Required review window for emergency entries.
    #[must_use]
    pub const fn emergency_review_window(&self) -> Duration {
        self.emergency_review_window
    }

    /// Whether permanent entries must cite a governance reference.
    #[must_use]
    pub const fn require_governance_reference(&self) -> bool {
        self.require_governance_reference
    }
}

impl DenylistKind {
    /// Convenience helper that clones the key for inclusion in hits.
    #[must_use]
    pub fn clone_key(&self) -> Self {
        match self {
            Self::Provider(id) => Self::Provider(*id),
            Self::ManifestDigest(digest) => Self::ManifestDigest(*digest),
            Self::Cid(cid) => Self::Cid(cid.clone()),
            Self::Url(url) => Self::Url(url.clone()),
            Self::AccountId(account) => Self::AccountId(account.clone()),
            Self::AccountAlias(alias) => Self::AccountAlias(alias.clone()),
            Self::PerceptualFamily {
                family_id,
                variant_id,
            } => Self::PerceptualFamily {
                family_id: *family_id,
                variant_id: *variant_id,
            },
        }
    }
}

/// Metadata associated with a denylist entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DenylistEntry {
    jurisdiction: Option<String>,
    reason: Option<String>,
    issued_at: Option<SystemTime>,
    expires_at: Option<SystemTime>,
    alias: Option<String>,
    policy_tier: DenylistPolicyTier,
    canon: Option<String>,
    governance_reference: Option<String>,
    review_deadline: Option<SystemTime>,
}

impl DenylistEntry {
    /// Returns `true` if the entry has expired relative to `now`.
    #[must_use]
    pub fn is_expired(&self, now: SystemTime) -> bool {
        self.expires_at.map_or(false, |expiry| expiry <= now)
    }

    /// Jurisdiction associated with the entry, if any.
    #[must_use]
    pub fn jurisdiction(&self) -> Option<&str> {
        self.jurisdiction.as_deref()
    }

    /// Optional reason text.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    /// Returns the recorded expiry instant, if any.
    #[must_use]
    pub fn expires_at(&self) -> Option<SystemTime> {
        self.expires_at
    }

    /// Returns the issuance timestamp, if available.
    #[must_use]
    pub fn issued_at(&self) -> Option<SystemTime> {
        self.issued_at
    }

    /// Optional routing alias (if supplied alongside a canonical identifier).
    #[must_use]
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }

    /// Tier applied when enforcing TTL policy.
    #[must_use]
    pub fn policy_tier(&self) -> DenylistPolicyTier {
        self.policy_tier
    }

    /// Canon identifier recorded for emergency entries.
    #[must_use]
    pub fn canon(&self) -> Option<&str> {
        self.canon.as_deref()
    }

    /// Governance supermajority reference for permanent entries.
    #[must_use]
    pub fn governance_reference(&self) -> Option<&str> {
        self.governance_reference.as_deref()
    }

    /// Deadline by which emergency entries must be reviewed.
    #[must_use]
    pub fn review_deadline(&self) -> Option<SystemTime> {
        self.review_deadline
    }
}

/// Builder for [`DenylistEntry`] metadata.
#[derive(Debug)]
pub struct DenylistEntryBuilder {
    jurisdiction: Option<String>,
    reason: Option<String>,
    issued_at: Option<SystemTime>,
    expires_at: Option<SystemTime>,
    alias: Option<String>,
    policy_tier: DenylistPolicyTier,
    canon: Option<String>,
    governance_reference: Option<String>,
    review_deadline: Option<SystemTime>,
}

impl Default for DenylistEntryBuilder {
    fn default() -> Self {
        Self {
            jurisdiction: None,
            reason: None,
            issued_at: None,
            expires_at: None,
            alias: None,
            policy_tier: DenylistPolicyTier::Standard,
            canon: None,
            governance_reference: None,
            review_deadline: None,
        }
    }
}

impl DenylistEntryBuilder {
    /// Sets the jurisdiction code (ISO 3166-1 alpha-2).
    #[must_use]
    pub fn jurisdiction<S: Into<String>>(mut self, jurisdiction: S) -> Self {
        self.jurisdiction = Some(jurisdiction.into());
        self
    }

    /// Sets the human-readable reason attached to the entry.
    #[must_use]
    pub fn reason<S: Into<String>>(mut self, reason: S) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Sets the issuance time of the entry.
    #[must_use]
    pub fn issued_at(mut self, issued_at: SystemTime) -> Self {
        self.issued_at = Some(issued_at);
        self
    }

    /// Sets the expiry time for the entry.
    #[must_use]
    pub fn expires_at(mut self, expires_at: SystemTime) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Sets the expiry using a relative duration offset from `issued_at`.
    #[must_use]
    pub fn expires_in(mut self, offset: Duration) -> Self {
        let issued = self.issued_at.unwrap_or_else(SystemTime::now);
        self.issued_at = Some(issued);
        self.expires_at = Some(issued + offset);
        self
    }

    /// Attaches an optional alias (for example, `<alias>@<domain>`) to the entry.
    #[must_use]
    pub fn alias<S: Into<String>>(mut self, alias: S) -> Self {
        self.alias = Some(alias.into());
        self
    }

    /// Sets the policy tier for TTL enforcement.
    #[must_use]
    pub fn policy_tier(mut self, tier: DenylistPolicyTier) -> Self {
        self.policy_tier = tier;
        self
    }

    /// Records the canon identifier (used for emergency entries).
    #[must_use]
    pub fn canon<S: Into<String>>(mut self, canon: Option<S>) -> Self {
        self.canon = canon.map(Into::into);
        self
    }

    /// Records the governance reference for permanent entries.
    #[must_use]
    pub fn governance_reference<S: Into<String>>(mut self, reference: Option<S>) -> Self {
        self.governance_reference = reference.map(Into::into);
        self
    }

    /// Sets the review deadline for the entry.
    #[must_use]
    pub fn review_deadline(mut self, deadline: Option<SystemTime>) -> Self {
        self.review_deadline = deadline;
        self
    }

    /// Completes the builder.
    #[must_use]
    pub fn build(self) -> DenylistEntry {
        DenylistEntry {
            jurisdiction: self.jurisdiction,
            reason: self.reason,
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            alias: self.alias,
            policy_tier: self.policy_tier,
            canon: self.canon,
            governance_reference: self.governance_reference,
            review_deadline: self.review_deadline,
        }
    }

    /// Builds the entry while enforcing the provided policy.
    ///
    /// # Errors
    /// Returns an error if the builder does not satisfy the policy constraints.
    pub fn build_with_policy(mut self, policy: &DenylistPolicy) -> Result<DenylistEntry, String> {
        match self.policy_tier {
            DenylistPolicyTier::Standard => {
                let issued_at = self
                    .issued_at
                    .ok_or_else(|| "standard entries require `issued_at`".to_string())?;
                let max_expiry =
                    checked_add(issued_at, policy.standard_ttl()).ok_or_else(|| {
                        "standard entry TTL overflow when computing policy limits".to_string()
                    })?;
                let expires_at = if let Some(expiry) = self.expires_at {
                    if expiry <= issued_at {
                        return Err("`expires_at` must be later than `issued_at`".to_string());
                    }
                    if expiry > max_expiry {
                        return Err(format!(
                            "standard entries must expire within {:?}",
                            policy.standard_ttl()
                        ));
                    }
                    expiry
                } else {
                    max_expiry
                };
                self.expires_at = Some(expires_at);
            }
            DenylistPolicyTier::Emergency => {
                let issued_at = self
                    .issued_at
                    .ok_or_else(|| "emergency entries require `issued_at`".to_string())?;
                if self.canon.as_deref().map(str::is_empty).unwrap_or(true) {
                    return Err("emergency entries must include `emergency_canon`".to_string());
                }
                let max_expiry =
                    checked_add(issued_at, policy.emergency_ttl()).ok_or_else(|| {
                        "emergency entry TTL overflow when computing policy limits".to_string()
                    })?;
                let expires_at = if let Some(expiry) = self.expires_at {
                    if expiry <= issued_at {
                        return Err("`expires_at` must be later than `issued_at`".to_string());
                    }
                    if expiry > max_expiry {
                        return Err(format!(
                            "emergency entries must expire within {:?}",
                            policy.emergency_ttl()
                        ));
                    }
                    expiry
                } else {
                    max_expiry
                };
                self.expires_at = Some(expires_at);
                self.review_deadline = Some(
                    checked_add(issued_at, policy.emergency_review_window())
                        .ok_or_else(|| "emergency entry review window overflow".to_string())?,
                );
            }
            DenylistPolicyTier::Permanent => {
                if self.expires_at.is_some() {
                    return Err("permanent entries must omit `expires_at`".to_string());
                }
                if policy.require_governance_reference()
                    && self
                        .governance_reference
                        .as_deref()
                        .map(str::is_empty)
                        .unwrap_or(true)
                {
                    return Err("permanent entries require `governance_reference`".to_string());
                }
            }
        }

        Ok(self.build())
    }
}

/// Captured perceptual fingerprints for near-duplicate detection.
#[derive(Clone, Copy, Debug)]
pub struct PerceptualObservation<'a> {
    hash: Option<&'a [u8; 32]>,
    embedding_digest: Option<&'a [u8; 32]>,
}

impl<'a> PerceptualObservation<'a> {
    /// Construct a new observation.
    #[must_use]
    pub fn new(hash: Option<&'a [u8; 32]>, embedding_digest: Option<&'a [u8; 32]>) -> Self {
        Self {
            hash,
            embedding_digest,
        }
    }

    /// Returns `true` when no fingerprint data is available.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hash.is_none() && self.embedding_digest.is_none()
    }

    /// Borrow the perceptual hash.
    #[must_use]
    pub fn hash(&self) -> Option<&'a [u8; 32]> {
        self.hash
    }

    /// Borrow the embedding digest.
    #[must_use]
    pub fn embedding_digest(&self) -> Option<&'a [u8; 32]> {
        self.embedding_digest
    }
}

/// Metadata describing a perceptual family entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerceptualFamilyEntry {
    family_id: [u8; 16],
    variant_id: Option<[u8; 16]>,
    attack_vector: Option<String>,
    perceptual_hash: Option<[u8; 32]>,
    hamming_radius: u8,
    embedding_digest: Option<[u8; 32]>,
    metadata: DenylistEntry,
}

impl PerceptualFamilyEntry {
    /// Construct a new entry with the provided metadata.
    #[must_use]
    pub fn new(family_id: [u8; 16], metadata: DenylistEntry) -> Self {
        Self {
            family_id,
            variant_id: None,
            attack_vector: None,
            perceptual_hash: None,
            hamming_radius: 0,
            embedding_digest: None,
            metadata,
        }
    }

    /// Annotate with a specific variant identifier.
    #[must_use]
    pub fn with_variant_id(mut self, variant_id: Option<[u8; 16]>) -> Self {
        self.variant_id = variant_id;
        self
    }

    /// Set the attack vector description.
    #[must_use]
    pub fn with_attack_vector<S: Into<String>>(mut self, attack: Option<S>) -> Self {
        self.attack_vector = attack.map(Into::into);
        self
    }

    /// Attach a perceptual hash and its matching radius.
    #[must_use]
    pub fn with_perceptual_hash(mut self, hash: Option<[u8; 32]>, radius: u8) -> Self {
        self.perceptual_hash = hash;
        self.hamming_radius = radius;
        self
    }

    /// Attach an embedding digest fingerprint.
    #[must_use]
    pub fn with_embedding_digest(mut self, digest: Option<[u8; 32]>) -> Self {
        self.embedding_digest = digest;
        self
    }

    /// Borrow the associated metadata.
    #[must_use]
    pub fn metadata(&self) -> &DenylistEntry {
        &self.metadata
    }
}

/// Basis describing how a perceptual denylist match occurred.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PerceptualMatchBasis {
    /// Match triggered by a perceptual hash comparison.
    Hash {
        /// Observed perceptual hash.
        observed: [u8; 32],
        /// Canonical denylist hash.
        expected: [u8; 32],
        /// Observed Hamming distance.
        hamming_distance: u8,
        /// Configured matching radius.
        radius: u8,
    },
    /// Match triggered via embedding digest equivalence.
    Embedding {
        /// Observed embedding digest.
        observed: [u8; 32],
        /// Canonical digest recorded in the entry.
        expected: [u8; 32],
    },
}

/// Additional metadata returned when a perceptual match fires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerceptualMatch {
    basis: PerceptualMatchBasis,
    attack_vector: Option<String>,
}

impl PerceptualMatch {
    /// Construct hash-based match metadata.
    #[must_use]
    pub fn hash(
        observed: [u8; 32],
        expected: [u8; 32],
        hamming_distance: u8,
        radius: u8,
        attack_vector: Option<String>,
    ) -> Self {
        Self {
            basis: PerceptualMatchBasis::Hash {
                observed,
                expected,
                hamming_distance,
                radius,
            },
            attack_vector,
        }
    }

    /// Construct embedding-based match metadata.
    #[must_use]
    pub fn embedding(
        observed: [u8; 32],
        expected: [u8; 32],
        attack_vector: Option<String>,
    ) -> Self {
        Self {
            basis: PerceptualMatchBasis::Embedding { observed, expected },
            attack_vector,
        }
    }

    /// Inspect the match basis.
    #[must_use]
    pub fn basis(&self) -> &PerceptualMatchBasis {
        &self.basis
    }

    /// Optional attack vector annotation.
    #[must_use]
    pub fn attack_vector(&self) -> Option<&str> {
        self.attack_vector.as_deref()
    }
}

/// Hit returned when a request matches the denylist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DenylistHit {
    kind: DenylistKind,
    entry: DenylistEntry,
    perceptual_match: Option<PerceptualMatch>,
}

impl DenylistHit {
    /// Kind (resource identifier) that triggered the hit.
    #[must_use]
    pub fn kind(&self) -> &DenylistKind {
        &self.kind
    }

    /// Metadata describing the denylist decision.
    #[must_use]
    pub fn entry(&self) -> &DenylistEntry {
        &self.entry
    }

    /// Perceptual match metadata when the denial originated from a perceptual family.
    #[must_use]
    pub fn perceptual_match(&self) -> Option<&PerceptualMatch> {
        self.perceptual_match.as_ref()
    }

    /// Test-only constructor to build a hit with explicit components.
    #[cfg(test)]
    pub fn new_for_tests(
        kind: DenylistKind,
        entry: DenylistEntry,
        perceptual_match: Option<PerceptualMatch>,
    ) -> Self {
        Self {
            kind,
            entry,
            perceptual_match,
        }
    }
}

/// In-memory denylist store used by the gateway policy layer.
#[derive(Debug, Default)]
pub struct GatewayDenylist {
    entries: DashMap<DenylistKind, DenylistEntry, RandomState>,
    perceptual: DashMap<[u8; 16], Vec<PerceptualFamilyEntry>, RandomState>,
}

impl GatewayDenylist {
    /// Creates an empty denylist.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: DashMap::default(),
            perceptual: DashMap::default(),
        }
    }

    /// Constructs a denylist from the provided iterator of entries.
    #[must_use]
    pub fn from_entries<I>(entries: I) -> Self
    where
        I: IntoIterator<Item = (DenylistKind, DenylistEntry)>,
    {
        let map = DashMap::from_iter(entries);
        Self {
            entries: map,
            perceptual: DashMap::default(),
        }
    }

    /// Adds or updates a denylist entry.
    pub fn upsert(&self, kind: DenylistKind, entry: DenylistEntry) {
        self.entries.insert(kind, entry);
    }

    /// Adds or updates a perceptual family entry.
    pub fn upsert_perceptual(&self, entry: PerceptualFamilyEntry) {
        let family_id = entry.family_id;
        let mut guard = self.perceptual.entry(family_id).or_default();
        if let Some(existing) = guard
            .iter_mut()
            .find(|variant| variant.variant_id == entry.variant_id)
        {
            *existing = entry;
        } else {
            guard.push(entry);
        }
    }

    /// Removes a specific entry.
    pub fn remove(&self, kind: &DenylistKind) {
        self.entries.remove(kind);
    }

    /// Removes all perceptual entries associated with the family.
    pub fn remove_perceptual_family(&self, family_id: &[u8; 16]) {
        self.perceptual.remove(family_id);
    }

    /// Removes expired entries using `now` as the reference instant.
    pub fn prune_expired(&self, now: SystemTime) {
        let stale = self
            .entries
            .iter()
            .filter_map(|entry| {
                if entry.value().is_expired(now) {
                    Some(entry.key().clone_key())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for key in stale {
            self.entries.remove(&key);
        }

        let perceptual_keys: Vec<[u8; 16]> =
            self.perceptual.iter().map(|entry| *entry.key()).collect();
        for key in perceptual_keys {
            if let Some(mut bucket) = self.perceptual.get_mut(&key) {
                bucket.retain(|entry| !entry.metadata().is_expired(now));
                if bucket.is_empty() {
                    drop(bucket);
                    self.perceptual.remove(&key);
                }
            }
        }
    }

    /// Checks whether the given provider identifier is blocked.
    #[must_use]
    pub fn check_provider(&self, provider_id: &[u8; 32], now: SystemTime) -> Option<DenylistHit> {
        self.lookup(&DenylistKind::Provider(*provider_id), now)
    }

    /// Checks whether the given manifest digest is blocked.
    #[must_use]
    pub fn check_manifest_digest(&self, digest: &[u8; 32], now: SystemTime) -> Option<DenylistHit> {
        self.lookup(&DenylistKind::ManifestDigest(*digest), now)
    }

    /// Checks whether the given CID is blocked.
    #[must_use]
    pub fn check_cid(&self, cid: &[u8], now: SystemTime) -> Option<DenylistHit> {
        self.lookup(&DenylistKind::Cid(cid.to_vec()), now)
    }

    /// Checks whether the given URL is blocked.
    #[must_use]
    pub fn check_url<S: AsRef<str>>(&self, url: S, now: SystemTime) -> Option<DenylistHit> {
        self.lookup(&DenylistKind::Url(url.as_ref().to_owned()), now)
    }

    /// Checks whether the given canonical i105 account identifier is blocked.
    #[must_use]
    pub fn check_account_id<S: AsRef<str>>(
        &self,
        account_id: S,
        now: SystemTime,
    ) -> Option<DenylistHit> {
        self.lookup(
            &DenylistKind::AccountId(account_id.as_ref().to_owned()),
            now,
        )
    }

    /// Checks whether the given routing alias is blocked.
    #[must_use]
    pub fn check_account_alias<S: AsRef<str>>(
        &self,
        alias: S,
        now: SystemTime,
    ) -> Option<DenylistHit> {
        self.lookup(&DenylistKind::AccountAlias(alias.as_ref().to_owned()), now)
    }

    /// Checks whether the supplied perceptual observation matches a denylist entry.
    #[must_use]
    pub fn check_perceptual(
        &self,
        observation: &PerceptualObservation<'_>,
        now: SystemTime,
    ) -> Option<DenylistHit> {
        if observation.is_empty() {
            return None;
        }

        for mut bucket in self.perceptual.iter_mut() {
            bucket.retain(|entry| !entry.metadata().is_expired(now));
            if bucket.is_empty() {
                let key = *bucket.key();
                drop(bucket);
                self.perceptual.remove(&key);
                continue;
            }
            for entry in bucket.iter() {
                if let Some(hit) = match_perceptual_entry(entry, observation) {
                    return Some(DenylistHit {
                        kind: DenylistKind::PerceptualFamily {
                            family_id: entry.family_id,
                            variant_id: entry.variant_id,
                        },
                        entry: entry.metadata.clone(),
                        perceptual_match: Some(hit),
                    });
                }
            }
        }
        None
    }

    fn lookup(&self, key: &DenylistKind, now: SystemTime) -> Option<DenylistHit> {
        let entry = self.entries.get(key)?;
        if entry.is_expired(now) {
            // Drop stale entry on read to avoid repeated lookups.
            let key_clone = key.clone_key();
            drop(entry);
            self.entries.remove(&key_clone);
            return None;
        }

        Some(DenylistHit {
            kind: key.clone_key(),
            entry: entry.clone(),
            perceptual_match: None,
        })
    }
}

fn match_perceptual_entry(
    entry: &PerceptualFamilyEntry,
    observation: &PerceptualObservation<'_>,
) -> Option<PerceptualMatch> {
    if let (Some(expected), Some(observed)) = (entry.perceptual_hash, observation.hash()) {
        let distance = hamming_distance(&expected, observed);
        if distance <= entry.hamming_radius as u32 {
            return Some(PerceptualMatch::hash(
                *observed,
                expected,
                distance as u8,
                entry.hamming_radius,
                entry.attack_vector.clone(),
            ));
        }
    }

    if let (Some(expected), Some(observed)) =
        (entry.embedding_digest, observation.embedding_digest())
    {
        if expected == *observed {
            return Some(PerceptualMatch::embedding(
                *observed,
                expected,
                entry.attack_vector.clone(),
            ));
        }
    }

    None
}

fn hamming_distance(lhs: &[u8; 32], rhs: &[u8; 32]) -> u32 {
    lhs.iter().zip(rhs).map(|(a, b)| (a ^ b).count_ones()).sum()
}

fn checked_add(base: SystemTime, delta: Duration) -> Option<SystemTime> {
    base.checked_add(delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denylist_prune_expired() {
        let now = SystemTime::now();
        let mut builder = DenylistEntryBuilder::default();
        builder = builder
            .issued_at(now)
            .expires_at(now + Duration::from_secs(1));
        let denylist =
            GatewayDenylist::from_entries([(DenylistKind::Provider([0x11; 32]), builder.build())]);

        assert!(denylist.check_provider(&[0x11; 32], now).is_some());
        denylist.prune_expired(now + Duration::from_secs(2));
        assert!(
            denylist
                .check_provider(&[0x11; 32], now + Duration::from_secs(2))
                .is_none()
        );
    }

    #[test]
    fn perceptual_entry_matches_by_hamming_distance() {
        let now = SystemTime::now();
        let metadata = DenylistEntryBuilder::default().build();
        let entry = PerceptualFamilyEntry::new([0xEF; 16], metadata)
            .with_variant_id(Some([0xAA; 16]))
            .with_perceptual_hash(Some([0xFF; 32]), 4);
        let denylist = GatewayDenylist::new();
        denylist.upsert_perceptual(entry);
        let mut observation_hash = [0xFF; 32];
        observation_hash[0] = 0xFE;
        let observation = PerceptualObservation::new(Some(&observation_hash), None);

        let hit = denylist.check_perceptual(&observation, now).expect("match");
        assert!(matches!(hit.kind(), DenylistKind::PerceptualFamily { .. }));
        let match_meta = hit.perceptual_match().expect("perceptual metadata");
        assert!(matches!(
            match_meta.basis(),
            PerceptualMatchBasis::Hash { .. }
        ));
    }

    #[test]
    fn perceptual_prune_removes_expired_variant() {
        let now = SystemTime::now();
        let metadata = DenylistEntryBuilder::default()
            .issued_at(now)
            .expires_at(now + Duration::from_secs(1))
            .build();
        let entry = PerceptualFamilyEntry::new([0xEF; 16], metadata)
            .with_perceptual_hash(Some([0xAA; 32]), 1);
        let denylist = GatewayDenylist::new();
        denylist.upsert_perceptual(entry);
        denylist.prune_expired(now + Duration::from_secs(5));
        let observation_hash = [0xAA; 32];
        let observation = PerceptualObservation::new(Some(&observation_hash), None);
        assert!(denylist.check_perceptual(&observation, now).is_none());
    }

    #[test]
    fn standard_policy_assigns_default_ttl() {
        let policy = DenylistPolicy::new(
            Duration::from_secs(600),
            Duration::from_secs(60),
            Duration::from_secs(30),
            true,
        );
        let issued_at = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let entry = DenylistEntryBuilder::default()
            .issued_at(issued_at)
            .build_with_policy(&policy)
            .expect("standard entry builds");
        assert_eq!(
            entry.expires_at(),
            Some(issued_at + Duration::from_secs(600))
        );
        assert_eq!(entry.policy_tier(), DenylistPolicyTier::Standard);
    }

    #[test]
    fn emergency_policy_enforces_canon_and_review_deadline() {
        let policy = DenylistPolicy::new(
            Duration::from_secs(600),
            Duration::from_secs(90),
            Duration::from_secs(30),
            true,
        );
        let issued_at = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let entry = DenylistEntryBuilder::default()
            .policy_tier(DenylistPolicyTier::Emergency)
            .issued_at(issued_at)
            .canon(Some("csam-hotline"))
            .build_with_policy(&policy)
            .expect("emergency entry");
        assert_eq!(
            entry.expires_at(),
            Some(issued_at + Duration::from_secs(90))
        );
        assert_eq!(
            entry.review_deadline(),
            Some(issued_at + Duration::from_secs(30))
        );
        assert_eq!(entry.canon(), Some("csam-hotline"));
        assert_eq!(entry.policy_tier(), DenylistPolicyTier::Emergency);
    }

    #[test]
    fn permanent_policy_requires_governance_reference() {
        let policy = DenylistPolicy::new(
            Duration::from_secs(600),
            Duration::from_secs(90),
            Duration::from_secs(30),
            true,
        );
        let issued_at = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let err = DenylistEntryBuilder::default()
            .policy_tier(DenylistPolicyTier::Permanent)
            .issued_at(issued_at)
            .build_with_policy(&policy)
            .expect_err("missing governance reference should fail");
        assert!(err.contains("governance_reference"));

        let entry = DenylistEntryBuilder::default()
            .policy_tier(DenylistPolicyTier::Permanent)
            .issued_at(issued_at)
            .governance_reference(Some("council-resolution-2025-014"))
            .build_with_policy(&policy)
            .expect("permanent entry");
        assert_eq!(entry.expires_at(), None);
        assert_eq!(
            entry.governance_reference(),
            Some("council-resolution-2025-014")
        );
        assert_eq!(entry.policy_tier(), DenylistPolicyTier::Permanent);
    }
}

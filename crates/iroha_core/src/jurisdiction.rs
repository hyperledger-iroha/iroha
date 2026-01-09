//! JDG attestations: SDN enforcement, committee scheduling, attestation guards, and retention.
//!
//! This module provides:
//! - A registry-backed SDN enforcer that validates [`JdgAttestation`] SDN commitments.
//! - Committee rotation helpers that load Norito manifests and select the active committee.
//! - An attestation guard that enforces dataspace/committee/signer/expiry rules and verifies signatures.
//! - A small in-memory retention store for attestation history (indexed per dataspace and epoch) to
//!   support audit replay and evidence retrieval.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use iroha_crypto::{Algorithm, HashOf, Signature};
use iroha_data_model::{
    jurisdiction::{
        JdgAttestation, JdgCommitteeId, JdgSdnKeyRecord, JdgSdnPolicy, JdgSdnRegistry,
        JdgSdnRegistryError, JdgSdnValidationError, JdgSignatureScheme,
    },
    nexus::DataSpaceId,
};
use iroha_schema::IntoSchema;
use norito::{codec::Encode, decode_from_reader};
use thiserror::Error;

/// Enforces JDG SDN commitments with a registry and policy.
#[derive(Debug, Clone)]
pub struct JdgSdnEnforcer {
    policy: JdgSdnPolicy,
    registry: JdgSdnRegistry,
}

impl JdgSdnEnforcer {
    /// Build an enforcer from explicit SDN key records.
    ///
    /// The records are inserted in the provided order, honouring the rotation
    /// policy for parent retirement and overlap guardrails.
    ///
    /// # Errors
    ///
    /// Returns [`JdgSdnRegistryError`] if a record fails registration (duplicate
    /// keys, missing parents, overlap violations, etc.).
    pub fn from_records(
        policy: JdgSdnPolicy,
        records: impl IntoIterator<Item = JdgSdnKeyRecord>,
    ) -> Result<Self, JdgSdnRegistryError> {
        let mut registry = JdgSdnRegistry::default();
        for record in records {
            registry.register_key(record, &policy.rotation)?;
        }
        Ok(Self { policy, registry })
    }

    /// Load an enforcer from Norito-encoded SDN key records.
    ///
    /// # Errors
    ///
    /// Returns [`JdgSdnLoadError`] when the file cannot be read, the registry
    /// payload fails Norito decoding, or registration violates the rotation
    /// policy.
    pub fn from_path<P: AsRef<Path>>(
        path: P,
        policy: JdgSdnPolicy,
    ) -> Result<Self, JdgSdnLoadError> {
        let path_ref = path.as_ref();
        let file = File::open(path_ref).map_err(|source| JdgSdnLoadError::Io {
            path: path_ref.to_path_buf(),
            source,
        })?;
        Self::from_reader(file, policy).map_err(|error| match error {
            JdgSdnLoadError::Decode { source, .. } => JdgSdnLoadError::Decode {
                source,
                path: Some(path_ref.to_path_buf()),
            },
            JdgSdnLoadError::Registry { index, source, .. } => JdgSdnLoadError::Registry {
                index,
                source,
                path: Some(path_ref.to_path_buf()),
            },
            other => other,
        })
    }

    /// Load an enforcer from any reader containing Norito-encoded key records.
    ///
    /// # Errors
    ///
    /// Returns [`JdgSdnLoadError`] when decoding fails or registration violates
    /// the rotation policy.
    pub fn from_reader(
        mut reader: impl Read,
        policy: JdgSdnPolicy,
    ) -> Result<Self, JdgSdnLoadError> {
        let records: Vec<JdgSdnKeyRecord> = decode_from_reader(&mut reader)
            .map_err(|source| JdgSdnLoadError::Decode { source, path: None })?;
        let mut registry = JdgSdnRegistry::default();
        for (index, record) in records.into_iter().enumerate() {
            registry
                .register_key(record, &policy.rotation)
                .map_err(|source| JdgSdnLoadError::Registry {
                    index,
                    source,
                    path: None,
                })?;
        }
        Ok(Self { policy, registry })
    }

    /// Validate a JDG attestation against the registry and policy.
    ///
    /// # Errors
    ///
    /// Returns [`JdgSdnValidationError`] when structural attestation checks,
    /// registry lookups, activation windows, or seals fail.
    pub fn validate(&self, attestation: &JdgAttestation) -> Result<(), JdgSdnValidationError> {
        attestation.validate_with_sdn_registry(&self.registry, &self.policy)
    }

    /// Active policy in use.
    #[must_use]
    pub fn policy(&self) -> &JdgSdnPolicy {
        &self.policy
    }

    /// Registered SDN keys.
    #[must_use]
    pub fn registry(&self) -> &JdgSdnRegistry {
        &self.registry
    }

    /// Snapshot of the registered SDN key records.
    #[must_use]
    pub fn registry_snapshot(&self) -> Vec<JdgSdnKeyRecord> {
        self.registry.keys().cloned().collect()
    }
}

static ENFORCER: OnceLock<JdgSdnEnforcer> = OnceLock::new();

/// Initialise the global JDG SDN enforcer from a registry file and policy.
///
/// Use this during node bootstrap to make the SDN registry available to
/// admission/consensus components that need to validate JDG attestations.
///
/// # Errors
///
/// Returns [`JdgSdnLoadError`] when loading or registration fails, or an
/// [`JdgSdnLoadError::AlreadyInitialised`] if called more than once.
pub fn init_enforcer_from_path(
    path: impl AsRef<Path>,
    policy: JdgSdnPolicy,
) -> Result<(), JdgSdnLoadError> {
    let enforcer = JdgSdnEnforcer::from_path(path, policy)?;
    ENFORCER
        .set(enforcer)
        .map_err(|_| JdgSdnLoadError::AlreadyInitialised)?;
    Ok(())
}

/// Access the globally initialised JDG SDN enforcer, if present.
#[must_use]
pub fn enforcer() -> Option<&'static JdgSdnEnforcer> {
    ENFORCER.get()
}

/// Snapshot the active SDN registry and policy if the global enforcer exists.
#[must_use]
pub fn sdn_registry_status() -> Option<(JdgSdnPolicy, Vec<JdgSdnKeyRecord>)> {
    enforcer().map(|enforcer| {
        let policy = *enforcer.policy();
        let registry = enforcer.registry_snapshot();
        (policy, registry)
    })
}

/// Errors surfaced while loading an SDN registry.
#[derive(Debug, Error)]
pub enum JdgSdnLoadError {
    /// Failed to read the registry payload.
    #[error("failed to read SDN registry from {path}")]
    Io {
        /// Path used for the read attempt.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to decode the Norito payload.
    #[error("failed to decode SDN registry")]
    Decode {
        /// Optional source path (if known).
        path: Option<PathBuf>,
        /// Underlying decode error.
        #[source]
        source: norito::Error,
    },
    /// SDN key registration violated the rotation policy.
    #[error("invalid SDN key at index {index}")]
    Registry {
        /// Position of the offending record.
        index: usize,
        /// Optional source path (if known).
        path: Option<PathBuf>,
        /// Underlying registration error.
        #[source]
        source: JdgSdnRegistryError,
    },
    /// Attempted to initialise the global enforcer more than once.
    #[error("JDG SDN enforcer already initialised")]
    AlreadyInitialised,
}

/// Manifest entry describing a committee schedule for a single dataspace.
#[derive(Debug, Clone, PartialEq, Eq, Encode, norito::codec::Decode, IntoSchema)]
pub struct JdgCommitteeManifest {
    /// Dataspace covered by the manifest.
    pub dataspace: DataSpaceId,
    /// Ordered committee records for the dataspace.
    pub committees: Vec<JdgCommitteeRecord>,
}

/// Committee membership/rotation record.
#[derive(Debug, Clone, PartialEq, Eq, Encode, norito::codec::Decode, IntoSchema)]
pub struct JdgCommitteeRecord {
    /// Committee identifier bound into attestations.
    pub committee_id: JdgCommitteeId,
    /// Ordered public keys that are eligible to sign.
    pub members: Vec<JdgCommitteeMember>,
    /// Threshold required for acceptance.
    pub threshold: u16,
    /// Height at which this committee activates (inclusive).
    pub activation_height: u64,
    /// Height at which this committee retires (inclusive). After this height
    /// only the grace window is allowed.
    pub retire_height: u64,
}

/// Committee member with optional proof-of-possession.
#[derive(Debug, Clone, PartialEq, Eq, Encode, norito::codec::Decode, IntoSchema)]
pub struct JdgCommitteeMember {
    /// Member public key.
    pub public_key: iroha_crypto::PublicKey,
    /// Proof-of-possession for BLS keys.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub pop: Option<Vec<u8>>,
}

impl JdgCommitteeRecord {
    fn validate(&self) -> Result<(), JdgCommitteeError> {
        if self.threshold == 0 {
            return Err(JdgCommitteeError::ZeroThreshold {
                committee_id: self.committee_id,
            });
        }
        if self.members.is_empty() {
            return Err(JdgCommitteeError::EmptyCommittee {
                committee_id: self.committee_id,
            });
        }
        if usize::from(self.threshold) > self.members.len() {
            return Err(JdgCommitteeError::ThresholdExceedsMembers {
                committee_id: self.committee_id,
                members: self.members.len(),
                threshold: self.threshold,
            });
        }
        let dedup: BTreeSet<_> = self
            .members
            .iter()
            .map(|member| {
                let (alg, bytes) = member.public_key.to_bytes();
                (alg, bytes)
            })
            .collect();
        if dedup.len() != self.members.len() {
            return Err(JdgCommitteeError::DuplicateMember {
                committee_id: self.committee_id,
            });
        }
        for (index, member) in self.members.iter().enumerate() {
            let (algorithm, _) = member.public_key.to_bytes();
            if matches!(algorithm, Algorithm::BlsNormal | Algorithm::BlsSmall) {
                let Some(pop) = member.pop.as_ref() else {
                    return Err(JdgCommitteeError::MissingMemberPop {
                        committee_id: self.committee_id,
                        index,
                        algorithm,
                    });
                };
                #[cfg(feature = "bls")]
                {
                    let res = match algorithm {
                        Algorithm::BlsNormal => {
                            iroha_crypto::bls_normal_pop_verify(&member.public_key, pop)
                        }
                        Algorithm::BlsSmall => {
                            iroha_crypto::bls_small_pop_verify(&member.public_key, pop)
                        }
                        _ => Ok(()),
                    };
                    if res.is_err() {
                        return Err(JdgCommitteeError::InvalidMemberPop {
                            committee_id: self.committee_id,
                            index,
                            algorithm,
                        });
                    }
                }
            }
        }
        if self.retire_height < self.activation_height {
            return Err(JdgCommitteeError::InvalidWindow {
                committee_id: self.committee_id,
                activation_height: self.activation_height,
                retire_height: self.retire_height,
            });
        }
        Ok(())
    }

    fn contains_height(&self, height: u64, grace_blocks: u64) -> bool {
        height >= self.activation_height
            && height <= self.retire_height.saturating_add(grace_blocks)
    }
}

/// Committee schedule keyed by dataspace with overlap guardrails.
#[derive(Debug, Clone)]
pub struct JdgCommitteeSchedule {
    entries: BTreeMap<DataSpaceId, Vec<JdgCommitteeRecord>>,
    grace_blocks: u64,
}

impl JdgCommitteeSchedule {
    /// Build a schedule from the supplied manifests.
    ///
    /// `grace_blocks` controls the allowed overlap between a retiring and activating committee.
    ///
    /// # Errors
    ///
    /// Returns [`JdgCommitteeError`] when validation fails or windows overlap beyond the grace
    /// window.
    pub fn from_manifests(
        manifests: impl IntoIterator<Item = JdgCommitteeManifest>,
        grace_blocks: u64,
    ) -> Result<Self, JdgCommitteeError> {
        let mut schedule = Self {
            entries: BTreeMap::new(),
            grace_blocks,
        };
        for manifest in manifests {
            schedule.insert_manifest(manifest)?;
        }
        Ok(schedule)
    }

    /// Load a committee manifest bundle from a Norito-encoded file.
    ///
    /// # Errors
    ///
    /// Returns [`JdgCommitteeLoadError`] when the manifest cannot be read, decoded, or validated.
    pub fn from_path(
        path: impl AsRef<Path>,
        grace_blocks: u64,
    ) -> Result<Self, JdgCommitteeLoadError> {
        let path_ref = path.as_ref();
        let file = File::open(path_ref).map_err(|source| JdgCommitteeLoadError::Io {
            path: path_ref.to_path_buf(),
            source,
        })?;
        let manifests: Vec<JdgCommitteeManifest> =
            decode_from_reader(file).map_err(|source| JdgCommitteeLoadError::Decode {
                path: path_ref.to_path_buf(),
                source,
            })?;
        Self::from_manifests(manifests, grace_blocks).map_err(JdgCommitteeLoadError::Validation)
    }

    /// Active committee for the given dataspace and block height.
    ///
    /// # Errors
    ///
    /// Returns [`JdgCommitteeError`] when no committee is configured or height falls outside of
    /// the active/grace windows.
    pub fn active_committee(
        &self,
        dataspace: &DataSpaceId,
        block_height: u64,
    ) -> Result<&JdgCommitteeRecord, JdgCommitteeError> {
        let entries = self
            .entries
            .get(dataspace)
            .ok_or(JdgCommitteeError::MissingDataspace {
                dataspace: *dataspace,
            })?;
        entries
            .iter()
            .rev()
            .find(|record| record.contains_height(block_height, self.grace_blocks))
            .ok_or(JdgCommitteeError::MissingCommitteeForHeight {
                dataspace: *dataspace,
                block_height,
            })
    }

    fn insert_manifest(&mut self, manifest: JdgCommitteeManifest) -> Result<(), JdgCommitteeError> {
        let entry = self.entries.entry(manifest.dataspace).or_default();
        for record in manifest.committees {
            record.validate()?;
            if let Some(prev) = entry.last() {
                if record.activation_height <= prev.activation_height {
                    return Err(JdgCommitteeError::NonMonotonicActivation {
                        committee_id: record.committee_id,
                    });
                }
                if record.activation_height <= prev.retire_height.saturating_add(self.grace_blocks)
                {
                    // Allowed overlap, nothing to do.
                } else {
                    return Err(JdgCommitteeError::OverlapExceedsGrace {
                        committee_id: record.committee_id,
                        grace_blocks: self.grace_blocks,
                    });
                }
            }
            entry.push(record);
        }
        Ok(())
    }
}

/// Guards applied to JDG attestations before admission/consensus.
#[derive(Debug, Clone)]
pub struct JdgAttestationGuard {
    schedule: JdgCommitteeSchedule,
    sdn_enforcer: Option<JdgSdnEnforcer>,
    max_attestation_bytes: usize,
    max_attestation_lag: u64,
    allowed_signature_schemes: BTreeSet<JdgSignatureScheme>,
}

impl JdgAttestationGuard {
    /// Construct an attestation guard.
    pub fn new(
        schedule: JdgCommitteeSchedule,
        sdn_enforcer: Option<JdgSdnEnforcer>,
        max_attestation_bytes: usize,
        max_attestation_lag: u64,
        allowed_signature_schemes: BTreeSet<JdgSignatureScheme>,
    ) -> Self {
        assert!(
            !allowed_signature_schemes.is_empty(),
            "jdg_signature_schemes must not be empty"
        );
        Self {
            schedule,
            sdn_enforcer,
            max_attestation_bytes,
            max_attestation_lag,
            allowed_signature_schemes,
        }
    }

    /// Validate the attestation structure, SDN commitments, committee membership, and signatures.
    ///
    /// # Errors
    ///
    /// Returns [`JdgAttestationGuardError`] when validation fails, the attestation is stale/expired,
    /// the committee does not match, or signatures are invalid.
    pub fn validate(
        &self,
        attestation: &JdgAttestation,
        dataspace: DataSpaceId,
        current_block_height: u64,
    ) -> Result<&JdgCommitteeRecord, JdgAttestationGuardError> {
        let size = norito::to_bytes(attestation)
            .map(|bytes| bytes.len())
            .map_err(JdgAttestationGuardError::Encode)?;
        if size > self.max_attestation_bytes {
            return Err(JdgAttestationGuardError::Oversized {
                bytes: size,
                limit: self.max_attestation_bytes,
            });
        }

        if attestation.scope.dataspace != dataspace {
            return Err(JdgAttestationGuardError::DataspaceMismatch {
                expected: dataspace,
                actual: attestation.scope.dataspace,
            });
        }

        if attestation.expiry_height <= current_block_height {
            return Err(JdgAttestationGuardError::Expired {
                expiry_height: attestation.expiry_height,
                current_block_height,
            });
        }

        if attestation.block_height > current_block_height {
            return Err(JdgAttestationGuardError::FutureDated {
                block_height: attestation.block_height,
                current_block_height,
            });
        }

        if current_block_height
            > attestation
                .block_height
                .saturating_add(self.max_attestation_lag)
        {
            return Err(JdgAttestationGuardError::Stale {
                block_height: attestation.block_height,
                current_block_height,
                max_attestation_lag: self.max_attestation_lag,
            });
        }

        if let Some(enforcer) = &self.sdn_enforcer {
            enforcer
                .validate(attestation)
                .map_err(JdgAttestationGuardError::Sdn)?;
        } else {
            attestation
                .validate()
                .map_err(JdgAttestationGuardError::Attestation)?;
        }

        let committee = self
            .schedule
            .active_committee(&dataspace, attestation.block_height)
            .map_err(JdgAttestationGuardError::Committee)?;
        if attestation.committee_id != committee.committee_id {
            return Err(JdgAttestationGuardError::CommitteeIdMismatch {
                expected: committee.committee_id,
                actual: attestation.committee_id,
            });
        }
        if attestation.committee_threshold != committee.threshold {
            return Err(JdgAttestationGuardError::ThresholdMismatch {
                expected: committee.threshold,
                actual: attestation.committee_threshold,
            });
        }
        let scheme =
            JdgSignatureScheme::try_from(attestation.signature.scheme_id).map_err(|_| {
                JdgAttestationGuardError::UnsupportedSignatureScheme {
                    scheme_id: attestation.signature.scheme_id,
                }
            })?;
        if !self.allowed_signature_schemes.contains(&scheme) {
            return Err(JdgAttestationGuardError::UnsupportedSignatureScheme {
                scheme_id: attestation.signature.scheme_id,
            });
        }
        Self::validate_signers(attestation, committee)?;
        Self::verify_signatures(attestation, committee, scheme)?;

        Ok(committee)
    }

    fn validate_signers(
        attestation: &JdgAttestation,
        committee: &JdgCommitteeRecord,
    ) -> Result<(), JdgAttestationGuardError> {
        let committee_members: BTreeSet<_> = committee
            .members
            .iter()
            .map(|member| {
                let (alg, bytes) = member.public_key.to_bytes();
                (alg, bytes)
            })
            .collect();
        for signer in &attestation.signer_set {
            let (alg, bytes) = signer.to_bytes();
            if !committee_members.contains(&(alg, bytes)) {
                return Err(JdgAttestationGuardError::UnknownSigner {
                    committee_id: committee.committee_id,
                });
            }
        }
        Ok(())
    }

    fn verify_signatures(
        attestation: &JdgAttestation,
        committee: &JdgCommitteeRecord,
        scheme: JdgSignatureScheme,
    ) -> Result<(), JdgAttestationGuardError> {
        let bitmap = attestation
            .signature
            .signer_bitmap
            .clone()
            .unwrap_or_default();

        let signable_hash: HashOf<_> = attestation.signing_hash();
        let hash_bytes: &[u8] = signable_hash.as_ref();

        let signer_indexes = if bitmap.is_empty() {
            (0..attestation.signer_set.len()).collect::<Vec<_>>()
        } else {
            collect_bits(bitmap, attestation.signer_set.len())
        };

        if signer_indexes.len() < usize::from(committee.threshold) {
            return Err(JdgAttestationGuardError::ThresholdNotMet {
                valid: signer_indexes.len(),
                threshold: committee.threshold,
            });
        }

        match scheme {
            JdgSignatureScheme::SimpleThreshold => {
                if attestation.signature.signatures.len() != signer_indexes.len() {
                    return Err(JdgAttestationGuardError::SignatureCountMismatch {
                        expected: signer_indexes.len(),
                        actual: attestation.signature.signatures.len(),
                    });
                }

                let mut valid = 0usize;
                for (sig_idx, signer_index) in signer_indexes.iter().enumerate() {
                    let signer_index = *signer_index;
                    let signer = attestation.signer_set.get(signer_index).ok_or(
                        JdgAttestationGuardError::SignatureCountMismatch {
                            expected: signer_index + 1,
                            actual: attestation.signer_set.len(),
                        },
                    )?;
                    let raw_sig = attestation.signature.signatures.get(sig_idx).ok_or(
                        JdgAttestationGuardError::SignatureCountMismatch {
                            expected: sig_idx + 1,
                            actual: attestation.signature.signatures.len(),
                        },
                    )?;
                    let signature = Signature::from_bytes(raw_sig);
                    if signature.verify(signer, hash_bytes).is_ok() {
                        valid += 1;
                    } else {
                        return Err(JdgAttestationGuardError::SignatureInvalid { index: sig_idx });
                    }
                }

                if valid < usize::from(committee.threshold) {
                    return Err(JdgAttestationGuardError::ThresholdNotMet {
                        valid,
                        threshold: committee.threshold,
                    });
                }
            }
            JdgSignatureScheme::BlsNormalAggregate => {
                #[cfg(feature = "bls")]
                {
                    if attestation.signature.signatures.len() != 1 {
                        return Err(JdgAttestationGuardError::SignatureCountMismatch {
                            expected: 1,
                            actual: attestation.signature.signatures.len(),
                        });
                    }
                    let mut pop_map: BTreeMap<(Algorithm, Vec<u8>), Vec<u8>> = BTreeMap::new();
                    for member in &committee.members {
                        if let Some(pop) = member.pop.as_ref() {
                            let (alg, bytes) = member.public_key.to_bytes();
                            pop_map.insert((alg, bytes.to_vec()), pop.clone());
                        }
                    }
                    let mut public_keys = Vec::with_capacity(signer_indexes.len());
                    let mut pops = Vec::with_capacity(signer_indexes.len());
                    for signer_index in &signer_indexes {
                        let signer = attestation.signer_set.get(*signer_index).ok_or(
                            JdgAttestationGuardError::SignatureCountMismatch {
                                expected: *signer_index + 1,
                                actual: attestation.signer_set.len(),
                            },
                        )?;
                        let (algorithm, bytes) = signer.to_bytes();
                        if algorithm != Algorithm::BlsNormal {
                            return Err(JdgAttestationGuardError::SignatureInvalid { index: 0 });
                        }
                        let pop = pop_map
                            .get(&(algorithm, bytes.to_vec()))
                            .ok_or(JdgAttestationGuardError::SignatureInvalid { index: 0 })?;
                        public_keys.push(signer);
                        pops.push(pop.as_slice());
                    }

                    let signature = &attestation.signature.signatures[0];
                    iroha_crypto::bls_normal_verify_preaggregated_same_message(
                        hash_bytes,
                        signature.as_slice(),
                        &public_keys,
                        &pops,
                    )
                    .map_err(|_| JdgAttestationGuardError::SignatureInvalid { index: 0 })?;
                }
                #[cfg(not(feature = "bls"))]
                {
                    return Err(JdgAttestationGuardError::UnsupportedSignatureScheme {
                        scheme_id: scheme.scheme_id(),
                    });
                }
            }
        }
        Ok(())
    }
}

fn collect_bits(bitmap: Vec<u8>, max_len: usize) -> Vec<usize> {
    let mut indexes = Vec::new();
    for (byte_idx, byte) in bitmap.into_iter().enumerate() {
        for bit in 0..8 {
            if (byte & (1 << bit)) != 0 {
                let idx = byte_idx * 8 + bit;
                if idx < max_len {
                    indexes.push(idx);
                }
            }
        }
    }
    indexes
}

/// In-memory retention store for JDG attestations, indexed per dataspace and epoch.
#[derive(Debug, Clone)]
pub struct JdgAttestationStore {
    max_per_dataspace: usize,
    records: BTreeMap<DataSpaceId, VecDeque<JdgAttestationRecord>>,
}

impl JdgAttestationStore {
    /// Construct a new store with the specified per-dataspace cap.
    pub fn new(max_per_dataspace: usize) -> Self {
        Self {
            max_per_dataspace,
            records: BTreeMap::new(),
        }
    }

    /// Insert an attestation, pruning oldest entries for the dataspace when needed.
    pub fn insert(
        &mut self,
        dataspace: DataSpaceId,
        attestation: JdgAttestation,
        recorded_height: u64,
    ) -> JdgAttestationRecord {
        let record = JdgAttestationRecord {
            dataspace,
            epoch: attestation.epoch,
            recorded_height,
            attestation,
        };
        let slot = self.records.entry(dataspace).or_default();
        slot.push_back(record.clone());
        while slot.len() > self.max_per_dataspace {
            slot.pop_front();
        }
        record
    }

    /// Fetch attestations for a dataspace ordered from oldest to newest.
    #[must_use]
    pub fn for_dataspace(&self, dataspace: &DataSpaceId) -> Vec<JdgAttestationRecord> {
        self.records
            .get(dataspace)
            .map(|records| records.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Fetch attestations for a dataspace filtered by epoch.
    #[must_use]
    pub fn for_dataspace_and_epoch(
        &self,
        dataspace: &DataSpaceId,
        epoch: u64,
    ) -> Vec<JdgAttestationRecord> {
        self.records
            .get(dataspace)
            .map(|records| {
                records
                    .iter()
                    .filter(|record| record.epoch == epoch)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Stored attestation snapshot with indexing metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JdgAttestationRecord {
    /// Dataspace identifier for indexing.
    pub dataspace: DataSpaceId,
    /// Epoch carried by the attestation.
    pub epoch: u64,
    /// Block height at which the node recorded the attestation.
    pub recorded_height: u64,
    /// Underlying attestation payload.
    pub attestation: JdgAttestation,
}

/// Errors surfaced while loading or validating committee manifests.
#[derive(Debug, Error)]
pub enum JdgCommitteeLoadError {
    /// IO failure.
    #[error("failed to read committee manifest {path}")]
    Io {
        /// Path used for the read attempt.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },
    /// Norito decode failure.
    #[error("failed to decode committee manifest {path}")]
    Decode {
        /// Path used for the read attempt.
        path: PathBuf,
        /// Underlying decode error.
        #[source]
        source: norito::Error,
    },
    /// Validation error.
    #[error("invalid committee manifest")]
    Validation(#[source] JdgCommitteeError),
}

/// Errors surfaced by committee schedule validation.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum JdgCommitteeError {
    /// Committee threshold is zero.
    #[error("committee {committee_id:?} threshold must be > 0")]
    ZeroThreshold {
        /// Committee identifier.
        committee_id: JdgCommitteeId,
    },
    /// Committee is empty.
    #[error("committee {committee_id:?} has no members")]
    EmptyCommittee {
        /// Committee identifier.
        committee_id: JdgCommitteeId,
    },
    /// Threshold exceeds committee size.
    #[error("committee {committee_id:?} threshold {threshold} exceeds member count {members}")]
    ThresholdExceedsMembers {
        /// Committee identifier.
        committee_id: JdgCommitteeId,
        /// Member count.
        members: usize,
        /// Threshold.
        threshold: u16,
    },
    /// Duplicate member detected.
    #[error("committee {committee_id:?} contains duplicate members")]
    DuplicateMember {
        /// Committee identifier.
        committee_id: JdgCommitteeId,
    },
    /// BLS member missing proof-of-possession.
    #[error("committee {committee_id:?} member {index} missing PoP for {algorithm:?}")]
    MissingMemberPop {
        /// Committee identifier.
        committee_id: JdgCommitteeId,
        /// Member index.
        index: usize,
        /// Algorithm requiring PoP.
        algorithm: Algorithm,
    },
    /// BLS member PoP failed verification.
    #[error("committee {committee_id:?} member {index} invalid PoP for {algorithm:?}")]
    InvalidMemberPop {
        /// Committee identifier.
        committee_id: JdgCommitteeId,
        /// Member index.
        index: usize,
        /// Algorithm requiring PoP.
        algorithm: Algorithm,
    },
    /// Activation/retire window is invalid.
    #[error(
        "committee {committee_id:?} retire height {retire_height} precedes activation {activation_height}"
    )]
    InvalidWindow {
        /// Committee identifier.
        committee_id: JdgCommitteeId,
        /// Activation height.
        activation_height: u64,
        /// Retire height.
        retire_height: u64,
    },
    /// Committee activation not strictly increasing.
    #[error("committee {committee_id:?} activation is not monotonic")]
    NonMonotonicActivation {
        /// Committee identifier.
        committee_id: JdgCommitteeId,
    },
    /// Overlap exceeds the configured grace window.
    #[error("committee {committee_id:?} rotation exceeds grace window of {grace_blocks} blocks")]
    OverlapExceedsGrace {
        /// Committee identifier.
        committee_id: JdgCommitteeId,
        /// Configured grace blocks.
        grace_blocks: u64,
    },
    /// Dataspace not configured.
    #[error("no JDG committee configured for dataspace {dataspace:?}")]
    MissingDataspace {
        /// Dataspace identifier.
        dataspace: DataSpaceId,
    },
    /// No committee for the given height.
    #[error("no JDG committee active for dataspace {dataspace:?} at height {block_height}")]
    MissingCommitteeForHeight {
        /// Dataspace identifier.
        dataspace: DataSpaceId,
        /// Block height requested.
        block_height: u64,
    },
}

/// Errors surfaced while validating JDG attestations at admission/consensus boundaries.
#[derive(Debug, Error)]
pub enum JdgAttestationGuardError {
    /// Norito encoding failed during size calculation.
    #[error("failed to encode attestation for sizing")]
    Encode(#[source] norito::Error),
    /// Attestation exceeds configured size cap.
    #[error("attestation size {bytes} bytes exceeds cap {limit}")]
    Oversized {
        /// Calculated attestation size.
        bytes: usize,
        /// Size cap.
        limit: usize,
    },
    /// Dataspace does not match expected context.
    #[error("attestation dataspace {actual:?} does not match expected {expected:?}")]
    DataspaceMismatch {
        /// Expected dataspace.
        expected: DataSpaceId,
        /// Dataspace carried by the attestation.
        actual: DataSpaceId,
    },
    /// Attestation expired.
    #[error("attestation expired at {expiry_height}, current block {current_block_height}")]
    Expired {
        /// Expiry height.
        expiry_height: u64,
        /// Current block height.
        current_block_height: u64,
    },
    /// Attestation block height lies in the future relative to validation.
    #[error("attestation block {block_height} is ahead of current block {current_block_height}")]
    FutureDated {
        /// Attested block height.
        block_height: u64,
        /// Current block height.
        current_block_height: u64,
    },
    /// Attestation too far in the past relative to the configured lag.
    #[error(
        "attestation block {block_height} is stale at {current_block_height} with max lag {max_attestation_lag}"
    )]
    Stale {
        /// Attested block height.
        block_height: u64,
        /// Current block height.
        current_block_height: u64,
        /// Maximum allowed lag.
        max_attestation_lag: u64,
    },
    /// Attestation structural validation failed.
    #[error("attestation failed validation")]
    Attestation(#[source] iroha_data_model::jurisdiction::JdgAttestationError),
    /// SDN validation failed.
    #[error("attestation SDN validation failed")]
    Sdn(#[source] JdgSdnValidationError),
    /// Committee selection failed.
    #[error("committee error")]
    Committee(#[source] JdgCommitteeError),
    /// Committee identifier mismatch.
    #[error("committee id mismatch: expected {expected:?}, got {actual:?}")]
    CommitteeIdMismatch {
        /// Expected committee id.
        expected: JdgCommitteeId,
        /// Actual committee id.
        actual: JdgCommitteeId,
    },
    /// Threshold mismatch relative to the configured committee.
    #[error("committee threshold mismatch: expected {expected}, got {actual}")]
    ThresholdMismatch {
        /// Expected threshold.
        expected: u16,
        /// Actual threshold.
        actual: u16,
    },
    /// Signer not present in the configured committee.
    #[error("attestation signer not part of committee {committee_id:?}")]
    UnknownSigner {
        /// Committee identifier.
        committee_id: JdgCommitteeId,
    },
    /// Signature count does not match signer bitmap/cardinality.
    #[error("signature count mismatch: expected {expected}, got {actual}")]
    SignatureCountMismatch {
        /// Expected signature count.
        expected: usize,
        /// Actual signature count.
        actual: usize,
    },
    /// Individual signature failed verification.
    #[error("attestation signature {index} failed verification")]
    SignatureInvalid {
        /// Index of the invalid signature.
        index: usize,
    },
    /// Threshold not met after verification.
    #[error("attestation signatures valid {valid} below threshold {threshold}")]
    ThresholdNotMet {
        /// Valid signature count.
        valid: usize,
        /// Required threshold.
        threshold: u16,
    },
    /// Unsupported signature scheme identifier.
    #[error("unsupported JDG signature scheme id {scheme_id}")]
    UnsupportedSignatureScheme {
        /// Provided scheme id.
        scheme_id: u16,
    },
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[cfg(feature = "bls")]
    use iroha_crypto::Algorithm;
    use iroha_crypto::{Hash, Signature, SignatureOf};
    use iroha_data_model::{
        jurisdiction::{
            JdgAttestationScope, JdgBlockRange, JdgSignatureScheme, JdgStateAccessSet, JdgVerdict,
        },
        nexus::DataSpaceId,
    };
    use tempfile::tempdir;

    use super::*;

    fn simple_signature_schemes() -> BTreeSet<JdgSignatureScheme> {
        BTreeSet::from([JdgSignatureScheme::SimpleThreshold])
    }

    fn sample_scope() -> JdgAttestationScope {
        JdgAttestationScope {
            jurisdiction_id: iroha_data_model::jurisdiction::JurisdictionId::new(b"JUR1".to_vec())
                .expect("valid id"),
            dataspace: DataSpaceId::new(7),
            block_range: JdgBlockRange::new(10, 12).expect("valid range"),
        }
    }

    fn sample_attestation(
        scope: &JdgAttestationScope,
        sdn_keypair: &iroha_crypto::KeyPair,
    ) -> JdgAttestation {
        let mut commitment = iroha_data_model::jurisdiction::JdgSdnCommitment {
            version: iroha_data_model::jurisdiction::JDG_SDN_COMMITMENT_VERSION_V1,
            scope: scope.clone(),
            encrypted_payload_hash: Hash::prehashed([0x77; 32]),
            seal: SignatureOf::from_signature(Signature::from_bytes(&[0u8])),
            sdn_public_key: sdn_keypair.public_key().clone(),
        };
        let seal = SignatureOf::from_hash(sdn_keypair.private_key(), commitment.signing_hash());
        commitment.seal = seal;

        let signer = iroha_crypto::KeyPair::random().public_key().clone();
        JdgAttestation {
            version: iroha_data_model::jurisdiction::JDG_ATTESTATION_VERSION_V1,
            scope: scope.clone(),
            pre_state_version: Hash::prehashed([0x11; 32]),
            access_set: JdgStateAccessSet::normalized(vec![b"a".to_vec()], vec![b"b".to_vec()]),
            verdict: JdgVerdict::Accept,
            post_state_delta: vec![0xAA],
            jurisdiction_root: Hash::prehashed([0x22; 32]),
            sdn_commitments: vec![commitment],
            da_ack: None,
            expiry_height: 99,
            committee_id: iroha_data_model::jurisdiction::JdgCommitteeId::new([0x33; 32]),
            committee_threshold: 1,
            statement_hash: Hash::prehashed([0x44; 32]),
            proof: None,
            signer_set: vec![signer],
            epoch: 0,
            block_height: 11,
            signature: iroha_data_model::jurisdiction::JdgThresholdSignature {
                scheme_id: iroha_data_model::jurisdiction::JDG_SIGNATURE_SCHEME_SIMPLE_THRESHOLD,
                signer_bitmap: Some(vec![0b0000_0001]),
                signatures: vec![vec![0x55]],
            },
        }
    }

    fn policy_require_commitments() -> JdgSdnPolicy {
        JdgSdnPolicy {
            require_commitments: true,
            rotation: iroha_data_model::jurisdiction::JdgSdnRotationPolicy {
                dual_publish_blocks: 2,
            },
        }
    }

    #[test]
    fn enforcer_loads_registry_from_reader() {
        let policy = policy_require_commitments();
        let scope = sample_scope();
        let sdn_keypair = iroha_crypto::KeyPair::random();
        let records = vec![JdgSdnKeyRecord {
            public_key: sdn_keypair.public_key().clone(),
            activated_at: 0,
            retired_at: None,
            rotation_parent: None,
        }];
        let bytes = norito::to_bytes(&records).expect("encode registry");

        let enforcer =
            JdgSdnEnforcer::from_reader(Cursor::new(bytes), policy).expect("load registry");
        let attestation = sample_attestation(&scope, &sdn_keypair);

        enforcer
            .validate(&attestation)
            .expect("attestation must verify");
    }

    #[test]
    fn enforcer_rejects_missing_commitments_when_required() {
        let policy = policy_require_commitments();
        let enforcer =
            JdgSdnEnforcer::from_records(policy, Vec::new()).expect("empty registry allowed");
        let mut attestation = sample_attestation(&sample_scope(), &iroha_crypto::KeyPair::random());
        attestation.sdn_commitments.clear();

        let err = enforcer
            .validate(&attestation)
            .expect_err("missing commitments should be rejected");
        match err {
            JdgSdnValidationError::Attestation { source } => {
                assert_eq!(
                    source,
                    iroha_data_model::jurisdiction::JdgAttestationError::MissingSdnCommitments
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn enforcer_rejects_scope_mismatch() {
        let policy = policy_require_commitments();
        let scope = sample_scope();
        let sdn_keypair = iroha_crypto::KeyPair::random();
        let enforcer = JdgSdnEnforcer::from_records(
            policy,
            vec![JdgSdnKeyRecord {
                public_key: sdn_keypair.public_key().clone(),
                activated_at: 0,
                retired_at: None,
                rotation_parent: None,
            }],
        )
        .expect("build registry");

        let mut attestation = sample_attestation(&scope, &sdn_keypair);
        attestation.sdn_commitments[0].scope.dataspace = DataSpaceId::new(99);

        let err = enforcer
            .validate(&attestation)
            .expect_err("scope mismatch expected");
        assert!(matches!(
            err,
            JdgSdnValidationError::ScopeMismatch { index: 0 }
                | JdgSdnValidationError::Attestation { .. }
        ));
    }

    #[test]
    fn enforcer_rejects_inactive_key() {
        let policy = policy_require_commitments();
        let scope = sample_scope();
        let sdn_keypair = iroha_crypto::KeyPair::random();
        let enforcer = JdgSdnEnforcer::from_records(
            policy,
            vec![JdgSdnKeyRecord {
                public_key: sdn_keypair.public_key().clone(),
                activated_at: 99,
                retired_at: None,
                rotation_parent: None,
            }],
        )
        .expect("registry built");
        let attestation = sample_attestation(&scope, &sdn_keypair);

        let err = enforcer
            .validate(&attestation)
            .expect_err("inactive key should be rejected");
        assert!(matches!(
            err,
            JdgSdnValidationError::InactiveSdnKey { index: 0, .. }
        ));
    }

    #[test]
    fn load_registry_rejects_overlap_beyond_policy() {
        let policy = JdgSdnPolicy {
            require_commitments: true,
            rotation: iroha_data_model::jurisdiction::JdgSdnRotationPolicy {
                dual_publish_blocks: 1,
            },
        };
        let parent = iroha_crypto::KeyPair::random();
        let child = iroha_crypto::KeyPair::random();
        let records = vec![
            JdgSdnKeyRecord {
                public_key: parent.public_key().clone(),
                activated_at: 1,
                retired_at: Some(50),
                rotation_parent: None,
            },
            JdgSdnKeyRecord {
                public_key: child.public_key().clone(),
                activated_at: 10,
                retired_at: None,
                rotation_parent: Some(parent.public_key().clone()),
            },
        ];
        let bytes = norito::to_bytes(&records).expect("encode registry");
        let err =
            JdgSdnEnforcer::from_reader(Cursor::new(bytes), policy).expect_err("overlap must fail");
        assert!(matches!(
            err,
            JdgSdnLoadError::Registry {
                source: JdgSdnRegistryError::OverlapExceedsPolicy { .. },
                ..
            }
        ));
    }

    #[test]
    fn global_enforcer_initialises_once() {
        let dir = tempdir().expect("tmp dir");
        let registry_path = dir.path().join("sdn_registry.norito");

        let policy = policy_require_commitments();
        let records = vec![JdgSdnKeyRecord {
            public_key: iroha_crypto::KeyPair::random().public_key().clone(),
            activated_at: 0,
            retired_at: None,
            rotation_parent: None,
        }];
        let bytes = norito::to_bytes(&records).expect("encode registry");
        std::fs::write(&registry_path, bytes).expect("write registry");

        init_enforcer_from_path(&registry_path, policy).expect("init enforcer");
        let err = init_enforcer_from_path(&registry_path, policy_require_commitments())
            .expect_err("second init must fail");
        matches!(err, JdgSdnLoadError::AlreadyInitialised)
            .then_some(())
            .expect("expected AlreadyInitialised");

        let enforcer = enforcer().expect("enforcer should be available");
        assert_eq!(enforcer.policy(), &policy_require_commitments());
        assert_eq!(enforcer.registry_snapshot().len(), 1);
    }

    fn committee_with_members(
        dataspace: DataSpaceId,
        activation: u64,
        retire: u64,
        threshold: u16,
        member_count: usize,
    ) -> (JdgCommitteeRecord, Vec<iroha_crypto::KeyPair>) {
        assert!(
            member_count >= usize::from(threshold),
            "member count must cover threshold"
        );
        let signers: Vec<_> = (0..member_count)
            .map(|_| iroha_crypto::KeyPair::random())
            .collect();
        let mut committee_id_bytes = [0u8; 32];
        committee_id_bytes[..8].copy_from_slice(&dataspace.as_u64().to_le_bytes());
        let committee = JdgCommitteeRecord {
            committee_id: iroha_data_model::jurisdiction::JdgCommitteeId::new(committee_id_bytes),
            members: signers
                .iter()
                .map(|kp| JdgCommitteeMember {
                    public_key: kp.public_key().clone(),
                    pop: None,
                })
                .collect(),
            threshold,
            activation_height: activation,
            retire_height: retire,
        };
        (committee, signers)
    }

    #[cfg(feature = "bls")]
    fn committee_with_bls_members(
        dataspace: DataSpaceId,
        activation: u64,
        retire: u64,
        threshold: u16,
        member_count: usize,
    ) -> (JdgCommitteeRecord, Vec<iroha_crypto::KeyPair>) {
        assert!(
            member_count >= usize::from(threshold),
            "member count must cover threshold"
        );
        let signers: Vec<_> = (0..member_count)
            .map(|_| iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal))
            .collect();
        let mut committee_id_bytes = [0u8; 32];
        committee_id_bytes[..8].copy_from_slice(&dataspace.as_u64().to_le_bytes());
        let committee = JdgCommitteeRecord {
            committee_id: iroha_data_model::jurisdiction::JdgCommitteeId::new(committee_id_bytes),
            members: signers
                .iter()
                .map(|kp| JdgCommitteeMember {
                    public_key: kp.public_key().clone(),
                    pop: Some(
                        iroha_crypto::bls_normal_pop_prove(kp.private_key())
                            .expect("pop for committee member"),
                    ),
                })
                .collect(),
            threshold,
            activation_height: activation,
            retire_height: retire,
        };
        (committee, signers)
    }

    fn signed_attestation_for_committee(
        committee: &JdgCommitteeRecord,
        signers: &[iroha_crypto::KeyPair],
        signer_indexes: &[usize],
        dataspace: DataSpaceId,
        block_height: u64,
        expiry_height: u64,
        epoch: u64,
    ) -> JdgAttestation {
        let scope = JdgAttestationScope {
            jurisdiction_id: iroha_data_model::jurisdiction::JurisdictionId::new(
                b"JUR-TEST".into(),
            )
            .expect("jurisdiction id"),
            dataspace,
            block_range: JdgBlockRange::new(block_height, block_height).expect("range"),
        };
        let signer_set: Vec<_> = signer_indexes
            .iter()
            .map(|idx| signers[*idx].public_key().clone())
            .collect();
        let mut attestation = JdgAttestation {
            version: iroha_data_model::jurisdiction::JDG_ATTESTATION_VERSION_V1,
            scope,
            pre_state_version: Hash::prehashed([0x10; 32]),
            access_set: JdgStateAccessSet::normalized(vec![b"r".to_vec()], vec![b"w".to_vec()]),
            verdict: JdgVerdict::Accept,
            post_state_delta: vec![0xAB],
            jurisdiction_root: Hash::prehashed([0x22; 32]),
            sdn_commitments: Vec::new(),
            da_ack: None,
            expiry_height,
            committee_id: committee.committee_id,
            committee_threshold: committee.threshold,
            statement_hash: Hash::prehashed([0x44; 32]),
            proof: None,
            signer_set,
            epoch,
            block_height,
            signature: iroha_data_model::jurisdiction::JdgThresholdSignature {
                scheme_id: JdgSignatureScheme::SimpleThreshold.scheme_id(),
                signer_bitmap: None,
                signatures: Vec::new(),
            },
        };
        let signing_hash = attestation.signing_hash();
        let signatures = signer_indexes
            .iter()
            .map(|idx| {
                Signature::new(signers[*idx].private_key(), signing_hash.as_ref())
                    .payload()
                    .to_vec()
            })
            .collect();
        attestation.signature.signatures = signatures;
        attestation
    }

    #[cfg(feature = "bls")]
    fn bls_aggregated_attestation_for_committee(
        committee: &JdgCommitteeRecord,
        signers: &[iroha_crypto::KeyPair],
        signer_indexes: &[usize],
        dataspace: DataSpaceId,
        block_height: u64,
        expiry_height: u64,
        epoch: u64,
    ) -> JdgAttestation {
        let scope = JdgAttestationScope {
            jurisdiction_id: iroha_data_model::jurisdiction::JurisdictionId::new(
                b"JUR-TEST".into(),
            )
            .expect("jurisdiction id"),
            dataspace,
            block_range: JdgBlockRange::new(block_height, block_height).expect("range"),
        };
        let signer_set: Vec<_> = signer_indexes
            .iter()
            .map(|idx| signers[*idx].public_key().clone())
            .collect();
        let mut attestation = JdgAttestation {
            version: iroha_data_model::jurisdiction::JDG_ATTESTATION_VERSION_V1,
            scope,
            pre_state_version: Hash::prehashed([0x10; 32]),
            access_set: JdgStateAccessSet::normalized(vec![b"r".to_vec()], vec![b"w".to_vec()]),
            verdict: JdgVerdict::Accept,
            post_state_delta: vec![0xAB],
            jurisdiction_root: Hash::prehashed([0x22; 32]),
            sdn_commitments: Vec::new(),
            da_ack: None,
            expiry_height,
            committee_id: committee.committee_id,
            committee_threshold: committee.threshold,
            statement_hash: Hash::prehashed([0x44; 32]),
            proof: None,
            signer_set,
            epoch,
            block_height,
            signature: iroha_data_model::jurisdiction::JdgThresholdSignature {
                scheme_id: JdgSignatureScheme::BlsNormalAggregate.scheme_id(),
                signer_bitmap: None,
                signatures: Vec::new(),
            },
        };
        let signing_hash = attestation.signing_hash();
        let signatures: Vec<Vec<u8>> = signer_indexes
            .iter()
            .map(|idx| {
                Signature::new(signers[*idx].private_key(), signing_hash.as_ref())
                    .payload()
                    .to_vec()
            })
            .collect();
        let signature_refs: Vec<&[u8]> = signatures.iter().map(Vec::as_slice).collect();
        let aggregated =
            iroha_crypto::bls_normal_aggregate_signatures(&signature_refs).expect("aggregate");
        attestation.signature.signatures = vec![aggregated];
        attestation
    }

    #[test]
    fn attestation_guard_accepts_valid_committee() {
        let dataspace = DataSpaceId::new(9);
        let (committee, signers) = committee_with_members(dataspace, 10, 20, 2, 3);
        let manifest = JdgCommitteeManifest {
            dataspace,
            committees: vec![committee.clone()],
        };
        let schedule =
            JdgCommitteeSchedule::from_manifests(vec![manifest], 1).expect("schedule builds");
        let guard = JdgAttestationGuard::new(schedule, None, 4096, 8, simple_signature_schemes());

        let attestation =
            signed_attestation_for_committee(&committee, &signers, &[0, 1], dataspace, 12, 30, 1);
        let committee_used = guard
            .validate(&attestation, dataspace, 15)
            .expect("attestation should pass");
        assert_eq!(committee_used.committee_id, committee.committee_id);
    }

    #[test]
    fn attestation_guard_rejects_unknown_signer() {
        let dataspace = DataSpaceId::new(10);
        let (committee, signers) = committee_with_members(dataspace, 1, 5, 1, 2);
        let manifest = JdgCommitteeManifest {
            dataspace,
            committees: vec![committee.clone()],
        };
        let schedule =
            JdgCommitteeSchedule::from_manifests(vec![manifest], 0).expect("schedule builds");
        let guard = JdgAttestationGuard::new(schedule, None, 4096, 4, simple_signature_schemes());

        let mut attestation =
            signed_attestation_for_committee(&committee, &signers, &[0], dataspace, 2, 8, 0);
        attestation.signer_set[0] = iroha_crypto::KeyPair::random().public_key().clone();
        let err = guard
            .validate(&attestation, dataspace, 3)
            .expect_err("signer must be rejected");
        assert!(matches!(
            err,
            JdgAttestationGuardError::UnknownSigner { .. }
        ));
    }

    #[test]
    fn attestation_guard_rejects_stale_attestation() {
        let dataspace = DataSpaceId::new(11);
        let (committee, signers) = committee_with_members(dataspace, 5, 50, 1, 2);
        let manifest = JdgCommitteeManifest {
            dataspace,
            committees: vec![committee.clone()],
        };
        let schedule =
            JdgCommitteeSchedule::from_manifests(vec![manifest], 0).expect("schedule builds");
        let guard = JdgAttestationGuard::new(schedule, None, 4096, 2, simple_signature_schemes());

        let attestation =
            signed_attestation_for_committee(&committee, &signers, &[0], dataspace, 6, 20, 0);
        let err = guard
            .validate(&attestation, dataspace, 10)
            .expect_err("attestation should be stale");
        assert!(matches!(err, JdgAttestationGuardError::Stale { .. }));
    }

    #[test]
    fn attestation_guard_rejects_bad_scheme() {
        let dataspace = DataSpaceId::new(12);
        let (committee, signers) = committee_with_members(dataspace, 0, 10, 1, 1);
        let manifest = JdgCommitteeManifest {
            dataspace,
            committees: vec![committee.clone()],
        };
        let schedule =
            JdgCommitteeSchedule::from_manifests(vec![manifest], 0).expect("schedule builds");
        let guard = JdgAttestationGuard::new(schedule, None, 4096, 10, simple_signature_schemes());

        let mut attestation =
            signed_attestation_for_committee(&committee, &signers, &[0], dataspace, 3, 30, 0);
        attestation.signature.scheme_id = 99;
        let err = guard
            .validate(&attestation, dataspace, 4)
            .expect_err("bad scheme rejected");
        assert!(matches!(
            err,
            JdgAttestationGuardError::UnsupportedSignatureScheme { scheme_id: 99 }
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn attestation_guard_accepts_bls_aggregate_scheme() {
        let dataspace = DataSpaceId::new(14);
        let (committee, signers) = committee_with_bls_members(dataspace, 1, 10, 2, 3);
        let manifest = JdgCommitteeManifest {
            dataspace,
            committees: vec![committee.clone()],
        };
        let schedule =
            JdgCommitteeSchedule::from_manifests(vec![manifest], 0).expect("schedule builds");
        let guard = JdgAttestationGuard::new(
            schedule,
            None,
            4096,
            5,
            BTreeSet::from([JdgSignatureScheme::BlsNormalAggregate]),
        );

        let attestation = bls_aggregated_attestation_for_committee(
            &committee,
            &signers,
            &[0, 2],
            dataspace,
            2,
            20,
            0,
        );
        guard
            .validate(&attestation, dataspace, 3)
            .expect("aggregate signature should pass");
    }

    #[test]
    fn attestation_store_prunes_per_dataspace() {
        let dataspace = DataSpaceId::new(13);
        let (committee, signers) = committee_with_members(dataspace, 0, 10, 1, 2);
        let mut store = JdgAttestationStore::new(2);

        let a1 = signed_attestation_for_committee(&committee, &signers, &[0], dataspace, 1, 20, 0);
        let a2 = signed_attestation_for_committee(&committee, &signers, &[0], dataspace, 2, 20, 0);
        let a3 = signed_attestation_for_committee(&committee, &signers, &[0], dataspace, 3, 20, 1);

        store.insert(dataspace, a1.clone(), 5);
        store.insert(dataspace, a2.clone(), 6);
        store.insert(dataspace, a3.clone(), 7);

        let records = store.for_dataspace(&dataspace);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].attestation.block_height, 2);
        assert_eq!(records[1].attestation.block_height, 3);

        let epoch_records = store.for_dataspace_and_epoch(&dataspace, 1);
        assert_eq!(epoch_records.len(), 1);
        assert_eq!(epoch_records[0].attestation.block_height, 3);
    }
}

//! One signed producer inventory with complete logical dispatch and bounded original reads.
//!
//! Authentication establishes which originals the installation owner selected. It
//! does not establish that those originals implement their declared sources. The
//! native per-stage importers and complete finality graph must reconstruct and
//! qualify every source before a wallet producer can be installed.

use std::io::Read;

use iroha_kagemusha_proof::{
    a_relation::schedule::compiled::{OperationSchedule, compiled_routes},
    finality::catalog::ArtifactRecord,
};
use iroha_plonk_recursion::obligation::ledger::Variant;

use super::*;

#[path = "producer_inventory/recipe.rs"]
mod recipe;
pub use recipe::{ReceiptSourceRecipeV1, SourceScopeV1};

#[path = "producer_inventory/compiler.rs"]
mod compiler;
pub use compiler::{CompilationErrorV1, CompilationPhaseV1, CompiledKeyV1, CompiledOperationV1, CompiledOmegaV1,
    CompiledQV1, CompiledSigmasV1, OfflineCompilerV1, OriginalSinkV1, QClassesV1, q_classes};

#[path = "producer_inventory/finality.rs"]
mod finality;
pub use finality::{FinalityQualificationErrorV1, QualifiedReceiptSourceV1};

#[path = "producer_inventory/sigma.rs"]
mod sigma;
pub(super) use sigma::compiled_sigma_policy;

#[path = "producer_inventory/q.rs"]
mod q;
pub use q::{QProgramRecipeV1, QQualificationErrorV1, QualifiedQProgramV1, q_source_recipe};

#[path = "producer_inventory/bootstrap.rs"]
mod bootstrap;
pub use bootstrap::{BootstrapQualificationErrorV1, QualifiedBootstrapProgramV1};
pub use sigma::{QualifiedSigmasV1, SigmaQualificationErrorV1};

#[path = "producer_inventory/operation.rs"]
mod operation;
pub use operation::{
    OperationQualificationErrorV1, QualifiedOperationOwnerV1, QualifiedOperationRouteV1,
};

#[path = "producer_inventory/omega.rs"]
mod omega;
pub use omega::{OmegaQualificationErrorV1, QualifiedOmegaProgramV1};

/// Canonical inventory metadata cap; original proving tables are stored separately.
pub const CATALOG_MAX_BYTES_V1: usize = 16 << 20;
/// Maximum distinct non-finality originals in one complete native inventory.
pub const ARTIFACT_MAX_COUNT_V1: usize = 4_096;
/// Absolute per-original PK envelope bound, independent of a smaller local reader limit.
pub const PROVING_KEY_MAX_BYTES_V1: usize = 1 << 30;

/// Exact content address and byte length, with no caller-selected storage path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.original_blob.v1")]
pub struct BlobV1 {
    /// Exact original byte count, checked before allocation.
    pub bytes: u64,
    /// SHA-256 of those exact bytes.
    pub sha256: [u8; 32],
}
impl BlobV1 {
    /// Describe bytes for offline packaging; this grants no authentication.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.len() as u64,
            sha256: Sha256::digest(bytes).into(),
        }
    }
    fn length(self, cap: usize) -> Result<usize, Error> {
        let length = usize::try_from(self.bytes).map_err(|_| Error::Inventory)?;
        if length == 0 || length > cap || self.sha256 == [0; 32] {
            return Err(Error::Inventory);
        }
        Ok(length)
    }
}

/// Unmodified descriptor, VK and original PK content addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.producer_original.v1")]
pub struct OriginalV1 {
    /// Canonical native descriptor bytes.
    pub descriptor: BlobV1,
    /// Canonical native verifying-key bytes.
    pub verifying_key: BlobV1,
    /// Original `PIPAPK01` proving tables; never runtime-generated replacements.
    pub proving_key: BlobV1,
}
impl OriginalV1 {
    fn validate(self) -> Result<(), Error> {
        self.descriptor.length(DESCRIPTOR_MAX_BYTES_V1)?;
        self.verifying_key.length(VERIFYING_KEY_MAX_BYTES_V1)?;
        self.proving_key.length(PROVING_KEY_MAX_BYTES_V1)?;
        Ok(())
    }
    fn matches(self, original: &ArtifactOriginalV1) -> bool {
        self.descriptor == BlobV1::of(&original.descriptor)
            && self.verifying_key == BlobV1::of(&original.verifying_key)
    }
}

/// Exact native context and installed source references for one compiled operation.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.operation_originals.v1")]
pub struct OperationV1 {
    /// One-based index in the compiled fourteen-variant order.
    pub variant: u8,
    /// Ordered global selectors admitted by the own fixed Q-sigma class.
    pub own_class: Vec<u8>,
    /// Ordered incoming class selectors, empty exactly when no incoming sigma exists.
    pub incoming_class: Vec<u8>,
    /// Exact canonical Fp words of the reconstructed native ContextPlan schema.
    /// The source importer must compare the whole schema, not only its length.
    pub context: Vec<[u8; 32]>,
    /// Q originals, in the operation's exact Q order.
    pub q: Vec<u32>,
    /// A originals, in the compiled stage order.
    pub a: Vec<u32>,
    /// W originals between consecutive A stages.
    pub w: Vec<u32>,
}

/// Native signed-genesis policy and complete finality integrity inventory.
/// Independent native genesis authentication remains mandatory at graph mount.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.finality_originals.v1")]
pub struct FinalityV1 {
    /// Native network identity.
    pub network: [u8; 32],
    /// Global consensus instance identity.
    pub instance: [u8; 32],
    /// Exact initial epoch-context identity.
    pub initial_context: [u8; 32],
    /// Initial native scheduling epoch.
    pub initial_epoch: u64,
    /// Exact six native consensus parameters, in HistoryAnchor order.
    pub parameters: [u64; 6],
    /// Canonically ordered original source/wrapper integrity records.
    /// Full graph mounting checks every exact compiled node/name and child key.
    pub originals: Vec<ArtifactRecord>,
}

/// Complete canonical metadata preimage committed by the one signed artifact identity.
/// Index references select only members of `originals`; none are storage paths.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.producer_inventory.v1")]
pub struct ProducerInventoryV1 {
    /// Exactly version one, with no retired profile decoder.
    pub version: u16,
    /// Exact compiled native profile, including operation and finality leaf schedules.
    pub native_profile: [u8; 32],
    /// Unique original references, all consumed by this inventory.
    pub originals: Vec<OriginalV1>,
    /// All sixteen sigma originals in global selector order.
    pub sigma: [u32; 16],
    /// Compiled operation programs, each referenced by at least one logical route.
    pub operations: Vec<OperationV1>,
    /// Program index for every compiled logical selector route, in that exact order.
    pub routes: Vec<u32>,
    /// Exact distinct terminal A verifier identities, in first occurrence among programs.
    pub terminals: Vec<u32>,
    /// Sole final native Omega original.
    pub omega: u32,
    /// Ordinary transaction/finality source originals; no publisher-key substitution.
    pub finality: FinalityV1,
}

fn variant(tag: u8) -> Result<Variant, Error> {
    tag.checked_sub(1)
        .and_then(|i| Variant::ALL.get(usize::from(i)))
        .copied()
        .ok_or(Error::Inventory)
}
fn selectors(values: &[u8], empty: bool) -> Result<(), Error> {
    if (!empty && values.is_empty())
        || values.len() > 16
        || values.iter().any(|i| *i >= 16)
        || values.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(Error::Inventory);
    }
    Ok(())
}

impl ProducerInventoryV1 {
    fn member(&self, index: u32) -> Result<&OriginalV1, Error> {
        self.originals
            .get(usize::try_from(index).map_err(|_| Error::Inventory)?)
            .ok_or(Error::Inventory)
    }
    fn validate(&self) -> Result<(), Error> {
        let routes = compiled_routes();
        if self.version != 1
            || self.native_profile
                != artifact_digest(b"native-profile", &native_profile_transcript_v1()?)
            || self.originals.is_empty()
            || self.originals.len() > ARTIFACT_MAX_COUNT_V1
            || self.operations.is_empty()
            || self.operations.len() > routes.len()
            || self.routes.len() != routes.len()
            || self.terminals.is_empty()
            || self.terminals.len() > 32
        {
            return Err(Error::Inventory);
        }
        for (i, original) in self.originals.iter().enumerate() {
            original.validate()?;
            if self.originals[..i].contains(original) {
                return Err(Error::Inventory);
            }
        }
        let mut used = vec![false; self.originals.len()];
        for index in self.sigma.into_iter().chain([self.omega]) {
            self.member(index)?;
            used[usize::try_from(index).map_err(|_| Error::Inventory)?] = true;
        }
        let mut expected_terminals: Vec<u32> = Vec::new();
        let mut terminal_keys = Vec::new();
        for operation in &self.operations {
            let variant = variant(operation.variant)?;
            let schedule = OperationSchedule::for_variant(variant);
            let q_count = schedule.q_partitions().iter().flatten().count();
            selectors(&operation.own_class, false)?;
            selectors(&operation.incoming_class, true)?;
            let allowed: Vec<_> = routes
                .iter()
                .filter(|route| route.variant == variant)
                .collect();
            if operation
                .own_class
                .iter()
                .any(|selector| !allowed.iter().any(|route| route.own == *selector))
                || operation.incoming_class.iter().any(|selector| {
                    !allowed
                        .iter()
                        .any(|route| route.incoming == Some(*selector))
                })
            {
                return Err(Error::Inventory);
            }
            let incoming = matches!(
                variant,
                Variant::Receive | Variant::ReceiveRenewed | Variant::ArchiveReceive
            );
            if operation.incoming_class.is_empty() == incoming
                || operation.q.len() != q_count
                || operation.a.len() != schedule.stage_count()
                || operation.w.len() + 1 != operation.a.len()
                || operation.context.is_empty()
                || operation.context.len() > 4_096
                || operation
                    .context
                    .iter()
                    .any(|word| Option::<Fp>::from(Fp::from_repr(*word)).is_none())
            {
                return Err(Error::Inventory);
            }
            for index in operation
                .q
                .iter()
                .chain(&operation.a)
                .chain(&operation.w)
                .copied()
            {
                self.member(index)?;
                used[usize::try_from(index).map_err(|_| Error::Inventory)?] = true;
            }
            let index = *operation.a.last().ok_or(Error::Inventory)?;
            let terminal = self.member(index)?;
            let identity = (terminal.descriptor, terminal.verifying_key);
            if !terminal_keys.contains(&identity) {
                expected_terminals.push(index);
                terminal_keys.push(identity);
            }
        }
        if used.contains(&false) || self.terminals != expected_terminals {
            return Err(Error::Inventory);
        }
        let mut programs = vec![false; self.operations.len()];
        for (route, index) in routes.iter().zip(&self.routes) {
            let index = usize::try_from(*index).map_err(|_| Error::Inventory)?;
            let operation = self.operations.get(index).ok_or(Error::Inventory)?;
            if variant(operation.variant)? != route.variant
                || !operation.own_class.contains(&route.own)
                || route
                    .incoming
                    .is_some_and(|i| !operation.incoming_class.contains(&i))
            {
                return Err(Error::Inventory);
            }
            programs[index] = true;
        }
        if programs.contains(&false) {
            return Err(Error::Inventory);
        }
        let finality = &self.finality;
        if [
            finality.network,
            finality.instance,
            finality.initial_context,
        ]
        .contains(&[0; 32])
            || finality.originals.is_empty()
            || finality.originals.len() > 65_536
        {
            return Err(Error::Inventory);
        }
        let mut previous: Option<&[u8]> = None;
        for record in &finality.originals {
            record.validate_identity().map_err(|_| Error::Inventory)?;
            if record.name.is_empty()
                || record.name.len() > 512
                || previous.is_some_and(|p| p >= record.name.as_slice())
            {
                return Err(Error::Inventory);
            }
            for ((length, hash), cap) in record.lengths.iter().zip(&record.sha256).zip([
                DESCRIPTOR_MAX_BYTES_V1,
                VERIFYING_KEY_MAX_BYTES_V1,
                PROVING_KEY_MAX_BYTES_V1,
            ]) {
                BlobV1 {
                    bytes: *length,
                    sha256: *hash,
                }
                .length(cap)?;
            }
            previous = Some(&record.name);
        }
        Ok(())
    }
    /// Canonical bounded packaging bytes; structural validation is not source admission.
    /// # Errors
    /// Missing/reordered dispatch, invalid references/caps, malformed fields or encoding failure.
    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let bytes = norito::encode_canonical(self).map_err(|_| Error::Inventory)?;
        bounded(&bytes, CATALOG_MAX_BYTES_V1)?;
        Ok(bytes)
    }
    /// Exact domain-separated commitment included in the signed seventeen-verifier inventory.
    /// # Errors
    /// The complete bounded canonical inventory cannot be encoded.
    pub fn digest(&self) -> Result<[u8; 32], Error> {
        Ok(artifact_digest(
            b"producer-catalog",
            &self.to_canonical_bytes()?,
        ))
    }
}

/// Content-addressed read boundary. Implementations return an original byte stream,
/// never allocate from an unchecked declared size or resolve a witness-selected path.
pub trait OriginalSourceV1 {
    /// Open one selected original by its authenticated SHA-256 content address.
    /// # Errors
    /// Unavailable content or storage failure; neither permits replacement material.
    fn open(&mut self, sha256: [u8; 32]) -> Result<Box<dyn Read + '_>, Error>;
}

/// One active original for strict source import; drop it after the importer returns.
pub struct OriginalBytesV1 {
    /// Original descriptor.
    pub descriptor: Vec<u8>,
    /// Original verifier.
    pub verifying_key: Vec<u8>,
    /// Original proving tables; no other PK is retained by the inventory view.
    pub proving_key: Vec<u8>,
}

/// Immutable signed inventory view. This has no `NativeProofs` implementation,
/// wallet-open capability or conversion to one based on a caller-provided verdict.
pub struct AuthenticatedProducerInventoryV1 {
    inventory: ProducerInventoryV1,
    scheme_id: [u8; 32],
    manifest_digest: [u8; 32],
}
impl InstalledVerifierPackV1 {
    /// Authenticate the exact canonical producer preimage selected by this same manifest.
    /// This checks complete logical dispatch and shared verifier originals; every
    /// native source, child key, context schema and original PK still requires import.
    /// # Errors
    /// Wrong commitment, incomplete metadata, malformed/capped frame or replaced verifier.
    pub fn authenticate_producer_inventory(
        &self,
        bytes: &[u8],
    ) -> Result<AuthenticatedProducerInventoryV1, Error> {
        bounded(bytes, CATALOG_MAX_BYTES_V1)?;
        if artifact_digest(b"producer-catalog", bytes) != self.pack.producer_catalog_digest {
            return Err(Error::Inventory);
        }
        let inventory: ProducerInventoryV1 = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| Error::Inventory)?;
        inventory.validate()?;
        if inventory.finality.network != self.verifier.scheme().network_id {
            return Err(Error::Inventory);
        }
        for (index, original) in inventory.sigma.into_iter().zip(&self.pack.steps) {
            if !inventory.member(index)?.matches(&original.artifact) {
                return Err(Error::Inventory);
            }
        }
        if !inventory
            .member(inventory.omega)?
            .matches(&self.pack.lineage)
        {
            return Err(Error::Inventory);
        }
        Ok(AuthenticatedProducerInventoryV1 {
            inventory,
            scheme_id: self.verifier.scheme().scheme_id(),
            manifest_digest: self.verifier.manifest_digest(),
        })
    }
}

fn read(source: &mut dyn OriginalSourceV1, blob: BlobV1, cap: usize) -> Result<Vec<u8>, Error> {
    let length = blob.length(cap)?;
    let maximum = blob.bytes.checked_add(1).ok_or(Error::Inventory)?;
    let mut bytes = Vec::with_capacity(length);
    source
        .open(blob.sha256)?
        .take(maximum)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Inventory)?;
    if bytes.len() != length || BlobV1::of(&bytes) != blob {
        return Err(Error::Inventory);
    }
    Ok(bytes)
}
impl AuthenticatedProducerInventoryV1 {
    /// Signed immutable inventory metadata; reading it grants no source capability.
    #[must_use]
    pub const fn inventory(&self) -> &ProducerInventoryV1 {
        &self.inventory
    }
    /// Exact independently installed scheme and manifest identities, in that order.
    #[must_use]
    pub const fn installation(&self) -> ([u8; 32], [u8; 32]) {
        (self.scheme_id, self.manifest_digest)
    }
    /// Read one authenticated descriptor/VK pair, retaining no PK bytes.
    /// The concrete native owner still parses and qualifies the exact source identity.
    /// # Errors
    /// Unknown member or absent, truncated, extended or changed metadata originals.
    pub fn read_verifier_original(
        &self,
        index: u32,
        source: &mut dyn OriginalSourceV1,
    ) -> Result<ArtifactOriginalV1, Error> {
        let original = *self.inventory.member(index)?;
        Ok(ArtifactOriginalV1 {
            descriptor: read(source, original.descriptor, DESCRIPTOR_MAX_BYTES_V1)?,
            verifying_key: read(source, original.verifying_key, VERIFYING_KEY_MAX_BYTES_V1)?,
        })
    }
    /// Read and hash-check one original, with a finite local PK limit before allocation.
    /// The caller must pass these bytes to the exact source-specific native importer.
    /// # Errors
    /// Unknown member, invalid local limit, absent/truncated/extended/mutated original.
    pub fn read_original(
        &self,
        index: u32,
        source: &mut dyn OriginalSourceV1,
        maximum_pk_bytes: usize,
    ) -> Result<OriginalBytesV1, Error> {
        if maximum_pk_bytes == 0 || maximum_pk_bytes > PROVING_KEY_MAX_BYTES_V1 {
            return Err(Error::Inventory);
        }
        let original = *self.inventory.member(index)?;
        original.proving_key.length(maximum_pk_bytes)?;
        Ok(OriginalBytesV1 {
            descriptor: read(source, original.descriptor, DESCRIPTOR_MAX_BYTES_V1)?,
            verifying_key: read(source, original.verifying_key, VERIFYING_KEY_MAX_BYTES_V1)?,
            proving_key: read(source, original.proving_key, maximum_pk_bytes)?,
        })
    }
}

#[cfg(test)]
#[path = "producer_inventory/tests.rs"]
mod tests;

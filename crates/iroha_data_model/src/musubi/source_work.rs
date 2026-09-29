//! Allocation-free public shape admission before Musubi source validation.
//!
//! These local limits bound borrowed input elements and byte lengths, not Norito
//! output, CPU instructions, transaction validity or gas. No serialization or
//! semantic validation runs here. Every container is admitted before traversal.
//! Signature backend and validator allocation ownership are separate obligations.

use super::*;

/// Required local ceilings for one complete borrowed source-shape traversal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceGeometryLimits {
    /// Containers, variants and nested entries admitted before visiting them.
    pub elements: u64,
    /// Borrowed variable text, public-key and signature payload byte lengths.
    pub variable_bytes: u64,
}

/// Resource dimension whose public input demand cannot be admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceGeometryDimension {
    /// Containers, variants and nested entries.
    Elements,
    /// Variable byte payloads; not encoded or emitted bytes.
    VariableBytes,
}

/// Typed local failure; it has no wire codec or fabricated pool-release signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    variant_size_differences,
    reason = "the bounded inline refusal must report its exact demand without allocating"
)]
pub enum SourceGeometryError {
    /// The next complete input demand exceeds its local ceiling.
    Limit {
        /// Resource whose traversal was refused.
        dimension: SourceGeometryDimension,
        /// Checked cumulative demand, before processing the next input.
        requested: u64,
        /// Caller-supplied ceiling.
        limit: u64,
    },
    /// A complete public demand is not representable.
    Overflow(SourceGeometryDimension),
}

impl fmt::Display for SourceGeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit {
                dimension,
                requested,
                limit,
            } => write!(
                f,
                "Musubi source geometry {dimension:?} demand {requested} exceeds local limit {limit}"
            ),
            Self::Overflow(dimension) => {
                write!(f, "Musubi source geometry {dimension:?} demand overflow")
            }
        }
    }
}
impl std::error::Error for SourceGeometryError {}

/// One public source shape reached by live or universal projection validation.
///
/// Fields not read by these predicates, such as a pin's metadata or an order's
/// retained canonical payload, are intentionally outside these scoped shapes.
#[derive(Clone, Copy, Debug)]
pub enum SourceShape<'a> {
    /// Canonical domainless account/controller.
    Account(&'a AccountId),
    /// Structural package key, independent of the payload's duplicated identity.
    PackageId(&'a MusubiPackageIdV1),
    /// Complete release key including prerelease data.
    ReleaseId(&'a MusubiReleaseIdV1),
    /// Directory selector key.
    Selector(&'a MusubiPackageSelectorV1),
    /// Archive registration, receipt and current location directory.
    Archive(&'a MusubiArchiveRecordV1),
    /// Full retained provider attestation and registering account.
    Attestation(&'a MusubiProviderBundleAttestationRecordV1),
    /// Current or historical location and its provider list.
    Location(&'a MusubiArchiveLocationV1),
    /// Permanent replication-order archive binding and lifecycle.
    OrderBinding(&'a MusubiReplicationOrderLocationReferenceV1),
    /// Current pin's compared chunker fields.
    Pin(&'a crate::sorafs::pin_registry::PinManifestRecord),
    /// Current order's compared completion evidence.
    Order(&'a crate::sorafs::pin_registry::ReplicationOrderRecord),
    /// Package identity and owner/member sets.
    Package(&'a MusubiPackageRecordV1),
    /// Complete release record consumed by the universal predicate.
    Release(&'a MusubiReleaseRecordV1),
    /// Complete resolver projection, including duplicated source values.
    Resolver(&'a MusubiResolverReleaseRowV1),
    /// Complete directory projection.
    Directory(&'a MusubiOrderedPackageEntryV1),
}

/// Monotonic, allocation-free counter for one attempted public shape admission.
///
/// There is no default or unlimited construction. A refusal exposes no partial
/// validation evidence; abandon this attempt before invoking semantic validators.
/// Copying this local counter grants no resource custody or validation authority.
#[derive(Clone, Copy, Debug)]
pub struct SourceGeometry {
    limits: SourceGeometryLimits,
    used: SourceGeometryLimits,
}

impl SourceGeometry {
    /// Start one traversal with explicit finite caller-selected ceilings.
    pub const fn new(limits: SourceGeometryLimits) -> Self {
        Self {
            limits,
            used: SourceGeometryLimits {
                elements: 0,
                variable_bytes: 0,
            },
        }
    }
    /// Input geometry successfully admitted so far, including repeated visits.
    pub const fn used(&self) -> SourceGeometryLimits {
        self.used
    }
    /// Admit the complete borrowed input before any semantic or codec processing.
    ///
    /// # Errors
    /// Returns a typed local refusal before an excessive nested traversal.
    pub fn admit(&mut self, shape: SourceShape<'_>) -> Result<(), SourceGeometryError> {
        match shape {
            SourceShape::Account(value) => self.account(value),
            SourceShape::PackageId(value) => self.package_id(value),
            SourceShape::ReleaseId(value) => self.release_id(value),
            SourceShape::Selector(value) => self.selector(value),
            SourceShape::Archive(value) => self.archive(value),
            SourceShape::Attestation(value) => self.attestation_record(value),
            SourceShape::Location(value) => self.location(value),
            SourceShape::OrderBinding(value) => self.order_binding(value),
            SourceShape::Pin(value) => self.pin(value),
            SourceShape::Order(value) => self.order(value),
            SourceShape::Package(value) => self.package(value),
            SourceShape::Release(value) => self.release(value),
            SourceShape::Resolver(value) => self.resolver(value),
            SourceShape::Directory(value) => self.directory(value),
        }
    }
    fn add(
        &mut self,
        dimension: SourceGeometryDimension,
        count: usize,
    ) -> Result<(), SourceGeometryError> {
        let count = u64::try_from(count).map_err(|_| SourceGeometryError::Overflow(dimension))?;
        let (used, limit) = match dimension {
            SourceGeometryDimension::Elements => (&mut self.used.elements, self.limits.elements),
            SourceGeometryDimension::VariableBytes => {
                (&mut self.used.variable_bytes, self.limits.variable_bytes)
            }
        };
        let requested = used
            .checked_add(count)
            .ok_or(SourceGeometryError::Overflow(dimension))?;
        if requested > limit {
            return Err(SourceGeometryError::Limit {
                dimension,
                requested,
                limit,
            });
        }
        *used = requested;
        Ok(())
    }
    fn node(&mut self) -> Result<(), SourceGeometryError> {
        self.items(1)
    }
    fn items(&mut self, count: usize) -> Result<(), SourceGeometryError> {
        self.add(SourceGeometryDimension::Elements, count)
    }
    fn bytes(&mut self, value: &[u8]) -> Result<(), SourceGeometryError> {
        self.node()?;
        self.add(SourceGeometryDimension::VariableBytes, value.len())
    }
    fn text(&mut self, value: &str) -> Result<(), SourceGeometryError> {
        self.bytes(value.as_bytes())
    }
    fn key(&mut self, value: &PublicKey) -> Result<(), SourceGeometryError> {
        self.node()?;
        // Even invalid/discarded keys have measurable retained input geometry.
        // Validation remains with the semantic owner after complete admission.
        self.node()?;
        self.add(
            SourceGeometryDimension::VariableBytes,
            value.input_payload_len(),
        )
    }
    fn account(&mut self, value: &AccountId) -> Result<(), SourceGeometryError> {
        self.node()?;
        match value.controller() {
            AccountController::Single(key) => self.key(key),
            AccountController::Multisig(policy) => {
                self.items(policy.members().len())?;
                for member in policy.members() {
                    self.key(member.public_key())?;
                }
                Ok(())
            }
        }
    }
    fn package_id(&mut self, value: &MusubiPackageIdV1) -> Result<(), SourceGeometryError> {
        let MusubiPackageIdV1 {
            home_dataspace: _,
            scope,
            name,
        } = value;
        self.node()?;
        match scope {
            MusubiPackageScopeV1::DataspaceRoot => {}
            MusubiPackageScopeV1::Domain(domain) => self.text(domain.as_ref())?,
        }
        self.text(name.as_str())
    }
    fn release_id(&mut self, value: &MusubiReleaseIdV1) -> Result<(), SourceGeometryError> {
        let MusubiReleaseIdV1 { package, version } = value;
        self.node()?;
        self.package_id(package)?;
        self.version(version)
    }
    fn selector(&mut self, value: &MusubiPackageSelectorV1) -> Result<(), SourceGeometryError> {
        let MusubiPackageSelectorV1 { namespace, name } = value;
        self.node()?;
        self.text(namespace.as_str())?;
        self.text(name.as_str())
    }
    fn version(&mut self, value: &MusubiVersionV1) -> Result<(), SourceGeometryError> {
        let MusubiVersionV1 {
            major: _,
            minor: _,
            patch: _,
            prerelease,
        } = value;
        self.node()?;
        self.items(prerelease.len())?;
        for identifier in prerelease {
            match identifier {
                MusubiPrereleaseIdentifierV1::Numeric(_) => {}
                MusubiPrereleaseIdentifierV1::AlphaNumeric(text) => self.text(text)?,
            }
        }
        Ok(())
    }
    fn requirement(&mut self, value: &MusubiVersionReqV1) -> Result<(), SourceGeometryError> {
        self.node()?;
        match value {
            MusubiVersionReqV1::Any
            | MusubiVersionReqV1::MajorWildcard(_)
            | MusubiVersionReqV1::MinorWildcard(_) => Ok(()),
            MusubiVersionReqV1::Caret(version)
            | MusubiVersionReqV1::Tilde(version)
            | MusubiVersionReqV1::Exact(version) => self.version(version),
            MusubiVersionReqV1::Comparators(values) => {
                self.items(values.len())?;
                for value in values {
                    let MusubiVersionComparatorV1 { op: _, version } = value;
                    self.version(version)?;
                }
                Ok(())
            }
        }
    }
    fn dependencies(
        &mut self,
        values: &[MusubiDependencyReqV1],
    ) -> Result<(), SourceGeometryError> {
        self.items(values.len())?;
        for value in values {
            let MusubiDependencyReqV1 {
                alias,
                package,
                requirement,
            } = value;
            self.text(alias.as_ref())?;
            self.package_id(package)?;
            self.requirement(requirement)?;
        }
        Ok(())
    }
}

mod archive;
mod publication;
#[cfg(test)]
mod tests;

//! Chunker profile registry helpers used by SoraFS manifests.

#![allow(unexpected_cfgs)]

use std::{collections::HashSet, sync::OnceLock};

use sorafs_chunker::ChunkProfile;

/// Multihash code used for CAR payload digests in the default profile.
pub const DEFAULT_MULTIHASH_CODE: u64 = 0x1f;

/// Canonical dag-cbor codec identifier for manifest roots.
pub const MANIFEST_DAG_CODEC: u64 = 0x71;

#[derive(Debug, Clone, Copy)]
struct RawChunkerDescriptor {
    id: u32,
    namespace: &'static str,
    name: &'static str,
    semver: &'static str,
    profile: ChunkProfile,
    multihash_code: u64,
    aliases: &'static [&'static str],
}

const RAW_REGISTRY: &[RawChunkerDescriptor] = &[
    RawChunkerDescriptor {
        id: 1,
        namespace: "sorafs",
        name: "sf1",
        semver: "1.0.0",
        profile: ChunkProfile::DEFAULT,
        multihash_code: DEFAULT_MULTIHASH_CODE,
        aliases: &["sorafs.sf1@1.0.0", "sorafs-sf1"],
    },
    RawChunkerDescriptor {
        id: 2,
        namespace: "sorafs",
        name: "sf2",
        semver: "1.0.0",
        profile: ChunkProfile::SF2,
        multihash_code: DEFAULT_MULTIHASH_CODE,
        aliases: &["sorafs.sf2@1.0.0", "sorafs-sf2"],
    },
];

/// Descriptor describing a registered chunker profile.
#[derive(Debug, Clone, Copy)]
pub struct ChunkerProfileDescriptor {
    /// Numeric profile identifier advertised in manifests/adverts.
    pub id: crate::ProfileId,
    /// Namespace that scopes the profile registry (`sorafs`).
    pub namespace: &'static str,
    /// Human-readable profile name (e.g., `sf1`).
    pub name: &'static str,
    /// Semantic version string for the profile parameters.
    pub semver: &'static str,
    /// Chunker parameters.
    pub profile: ChunkProfile,
    /// Multihash code used when deriving chunk digests.
    pub multihash_code: u64,
    /// Additional handles recognised as aliases for negotiation.
    pub aliases: &'static [&'static str],
}

impl ChunkerProfileDescriptor {
    #[must_use]
    pub fn matches(&self, profile: ChunkProfile, multihash_code: u64) -> bool {
        self.profile == profile && self.multihash_code == multihash_code
    }

    const fn from_raw(raw: &RawChunkerDescriptor) -> Self {
        Self {
            id: crate::ProfileId(raw.id),
            namespace: raw.namespace,
            name: raw.name,
            semver: raw.semver,
            profile: raw.profile,
            multihash_code: raw.multihash_code,
            aliases: raw.aliases,
        }
    }
}

fn registry_storage() -> &'static Vec<ChunkerProfileDescriptor> {
    static REGISTRY: OnceLock<Vec<ChunkerProfileDescriptor>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        RAW_REGISTRY
            .iter()
            .map(ChunkerProfileDescriptor::from_raw)
            .collect()
    })
}

/// Returns the canonical registry entries.
#[must_use]
pub fn registry() -> &'static [ChunkerProfileDescriptor] {
    registry_storage().as_slice()
}

/// Returns the descriptor matching `profile_id`, if any.
#[must_use]
pub fn lookup(id: crate::ProfileId) -> Option<&'static ChunkerProfileDescriptor> {
    registry().iter().find(|entry| entry.id == id)
}

/// Returns the descriptor matching `namespace.name@semver` or `namespace/name@semver`.
#[must_use]
pub fn lookup_by_handle(handle: &str) -> Option<&'static ChunkerProfileDescriptor> {
    let (namespace, name, semver) = parse_handle(handle)?;
    lookup_by_identity(namespace, name, semver)
}

/// Returns the descriptor that matches the provided parameters.
#[must_use]
pub fn lookup_by_profile(
    profile: ChunkProfile,
    multihash_code: u64,
) -> Option<&'static ChunkerProfileDescriptor> {
    registry()
        .iter()
        .find(|entry| entry.matches(profile, multihash_code))
}

/// Returns the descriptor identified by namespace/name/semver triple.
#[must_use]
pub fn lookup_by_identity(
    namespace: &str,
    name: &str,
    semver: &str,
) -> Option<&'static ChunkerProfileDescriptor> {
    registry()
        .iter()
        .find(|entry| entry.namespace == namespace && entry.name == name && entry.semver == semver)
}

/// Returns the default `sorafs-sf1` descriptor.
#[must_use]
pub fn default_descriptor() -> &'static ChunkerProfileDescriptor {
    registry()
        .first()
        .expect("registry must contain at least one descriptor")
}

/// Ensures the registry satisfies governance charter rules.
#[must_use = "Charter compliance must be checked during startup"]
pub fn ensure_charter_compliance() -> Result<(), CharterViolation> {
    validate_entries(registry())
}

fn validate_entries(entries: &[ChunkerProfileDescriptor]) -> Result<(), CharterViolation> {
    let mut prev_id: Option<u32> = None;
    let mut canonical_handles = HashSet::new();
    let mut alias_handles = HashSet::new();

    for descriptor in entries {
        let id = descriptor.id.0;
        if id == 0 {
            return Err(CharterViolation::InvalidId(id));
        }
        if let Some(prev) = prev_id
            && id <= prev
        {
            return Err(CharterViolation::NonMonotonicIds {
                previous: prev,
                current: id,
            });
        }
        prev_id = Some(id);

        let canonical = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        if descriptor.aliases.is_empty() {
            return Err(CharterViolation::AliasListEmpty {
                handle: canonical.clone(),
            });
        }
        if descriptor.aliases[0] != canonical {
            return Err(CharterViolation::CanonicalNotFirst {
                handle: canonical.clone(),
            });
        }
        if !canonical_handles.insert(canonical.clone()) {
            return Err(CharterViolation::DuplicateCanonical { handle: canonical });
        }

        if descriptor
            .aliases
            .iter()
            .any(|alias| alias.trim().is_empty())
        {
            return Err(CharterViolation::EmptyAlias { handle: canonical });
        }
        if descriptor
            .aliases
            .iter()
            .any(|alias| alias.trim() != *alias)
        {
            return Err(CharterViolation::AliasContainsWhitespace {
                handle: canonical.clone(),
            });
        }

        for &alias in descriptor.aliases {
            if alias == canonical {
                continue;
            }
            if !alias_handles.insert(alias.to_string()) {
                return Err(CharterViolation::DuplicateAlias {
                    alias: alias.to_string(),
                });
            }
            if canonical_handles.contains(alias) {
                return Err(CharterViolation::AliasConflictsWithCanonical {
                    alias: alias.to_string(),
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sf2_profile_is_registered_with_aliases() {
        let descriptor = lookup_by_handle("sorafs.sf2@1.0.0")
            .expect("sf2 profile should be present in the registry");
        assert_eq!(descriptor.id, crate::ProfileId(2));
        assert_eq!(descriptor.namespace, "sorafs");
        assert_eq!(descriptor.name, "sf2");
        assert_eq!(descriptor.semver, "1.0.0");
        assert_eq!(descriptor.profile, ChunkProfile::SF2);
        assert_eq!(descriptor.multihash_code, DEFAULT_MULTIHASH_CODE);
        assert_eq!(
            descriptor.aliases.first(),
            Some(&"sorafs.sf2@1.0.0"),
            "canonical handle must be first alias"
        );
        assert!(
            descriptor.aliases.contains(&"sorafs-sf2"),
            "expected `sorafs-sf2` alias to be registered"
        );
    }

    #[test]
    fn charter_compliance_covers_sf2() {
        ensure_charter_compliance().expect("registry must satisfy charter invariants");
    }
}

/// Violations detected while validating the charter.
#[derive(Debug, thiserror::Error)]
pub enum CharterViolation {
    #[error("chunker registry ids must start at 1 and be monotonically increasing; found {0}")]
    InvalidId(u32),
    #[error(
        "chunker registry ids must be monotonically increasing; found previous={previous}, current={current}"
    )]
    NonMonotonicIds { previous: u32, current: u32 },
    #[error("chunker profile `{handle}` must declare at least one alias")]
    AliasListEmpty { handle: String },
    #[error("chunker profile `{handle}` must list its canonical handle first")]
    CanonicalNotFirst { handle: String },
    #[error("chunker canonical handle `{handle}` appears more than once")]
    DuplicateCanonical { handle: String },
    #[error("chunker alias `{alias}` duplicates another alias")]
    DuplicateAlias { alias: String },
    #[error("chunker alias `{alias}` conflicts with a canonical handle")]
    AliasConflictsWithCanonical { alias: String },
    #[error("chunker profile `{handle}` contains an empty alias")]
    EmptyAlias { handle: String },
    #[error("chunker profile `{handle}` contains an alias with surrounding whitespace")]
    AliasContainsWhitespace { handle: String },
}

fn parse_handle(handle: &str) -> Option<(&str, &str, &str)> {
    if let Some((ns_name, semver)) = handle.split_once('@') {
        if let Some((namespace, name)) = ns_name.split_once('.') {
            return Some((namespace, name, semver));
        }
        if let Some((namespace, name)) = ns_name.split_once('/') {
            return Some((namespace, name, semver));
        }
    }
    None
}

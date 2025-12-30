//! Registry of SoraFS chunker profiles and their negotiation metadata.

use std::{collections::HashSet, sync::OnceLock};

use sorafs_chunker::ChunkProfile;

use crate::chunker_registry_data::{RAW_REGISTRY, RawChunkerDescriptor};

/// Multihash code used for CAR payload digests in the default profile.
pub const DEFAULT_MULTIHASH_CODE: u64 = 0x1f;

/// Canonical dag-cbor codec identifier for manifest roots.
pub const MANIFEST_DAG_CODEC: u64 = 0x71;

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
    /// Returns true if this descriptor matches the provided chunk profile.
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

/// Violations detected while validating the charter.
#[derive(Debug, thiserror::Error)]
pub enum CharterViolation {
    #[error("chunker registry ids must start at 1 and be monotonically increasing; found {0}")]
    InvalidId(u32),
    #[error("chunker registry ids must increase monotonically (saw {previous} then {current})")]
    NonMonotonicIds { previous: u32, current: u32 },
    #[error("duplicate canonical chunker handle encountered: {handle}")]
    DuplicateCanonical { handle: String },
    #[error("profile alias list may not be empty for {handle}")]
    AliasListEmpty { handle: String },
    #[error("profile alias list must start with the canonical handle ({handle})")]
    CanonicalNotFirst { handle: String },
    #[error("profile aliases must include the canonical handle ({handle})")]
    CanonicalMissingFromAliases { handle: String },
    #[error("profile alias cannot be empty for {handle}")]
    EmptyAlias { handle: String },
    #[error("profile alias for {handle} must not contain leading/trailing whitespace")]
    AliasContainsWhitespace { handle: String },
    #[error("duplicate profile alias encountered: {alias}")]
    DuplicateAlias { alias: String },
    #[error("profile alias {alias} conflicts with another canonical handle")]
    AliasConflictsWithCanonical { alias: String },
}

fn parse_handle(handle: &str) -> Option<(&str, &str, &str)> {
    let trimmed = handle.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (prefix, semver) = trimmed.rsplit_once('@')?;
    if semver.is_empty() {
        return None;
    }
    let (namespace, name) = prefix.split_once('.').or_else(|| prefix.split_once('/'))?;
    if namespace.is_empty() || name.is_empty() {
        return None;
    }
    Some((namespace, name, semver))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_descriptor_matches_chunker_defaults() {
        let descriptor = default_descriptor();
        assert_eq!(descriptor.id, crate::ProfileId(1));
        assert_eq!(descriptor.namespace, "sorafs");
        assert_eq!(descriptor.name, "sf1");
        assert_eq!(descriptor.semver, "1.0.0");
        assert_eq!(descriptor.profile, ChunkProfile::DEFAULT);
        assert_eq!(descriptor.multihash_code, DEFAULT_MULTIHASH_CODE);
        assert_eq!(descriptor.aliases, &["sorafs.sf1@1.0.0", "sorafs-sf1"]);
        assert!(lookup(descriptor.id).is_some());
        assert!(lookup_by_profile(ChunkProfile::DEFAULT, DEFAULT_MULTIHASH_CODE).is_some());
    }

    #[test]
    fn lookup_by_handle_accepts_dot_or_slash_separator() {
        let descriptor = default_descriptor();
        let handle_dot = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let handle_slash = format!(
            "{}/{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let resolved_dot = lookup_by_handle(&handle_dot).expect("dot handle resolves");
        let resolved_slash = lookup_by_handle(&handle_slash).expect("slash handle resolves");
        assert!(std::ptr::eq(descriptor, resolved_dot));
        assert!(std::ptr::eq(descriptor, resolved_slash));
    }

    #[test]
    fn lookup_by_handle_rejects_malformed_values() {
        assert!(lookup_by_handle("").is_none());
        assert!(lookup_by_handle("sorafs").is_none());
        assert!(lookup_by_handle("sorafs.sf1").is_none());
        assert!(lookup_by_handle("sorafs.sf1@").is_none());
        assert!(lookup_by_handle(".sf1@1.0.0").is_none());
    }

    #[test]
    fn charter_compliance_holds_for_builtins() {
        ensure_charter_compliance().expect("registry charter compliance");
    }

    #[test]
    fn charter_rejects_missing_alias_list() {
        let descriptor = ChunkerProfileDescriptor {
            id: crate::ProfileId(2),
            namespace: "sorafs",
            name: "sf2",
            semver: "1.0.0",
            profile: ChunkProfile::DEFAULT,
            multihash_code: DEFAULT_MULTIHASH_CODE,
            aliases: &[],
        };
        let err = super::validate_entries(&[descriptor]).unwrap_err();
        assert!(matches!(err, CharterViolation::AliasListEmpty { .. }));
    }

    #[test]
    fn charter_rejects_when_canonical_not_first() {
        static ALIASES: &[&str] = &["sorafs-sf1", "sorafs.sf1@1.0.0"];
        let descriptor = ChunkerProfileDescriptor {
            id: crate::ProfileId(1),
            namespace: "sorafs",
            name: "sf1",
            semver: "1.0.0",
            profile: ChunkProfile::DEFAULT,
            multihash_code: DEFAULT_MULTIHASH_CODE,
            aliases: ALIASES,
        };
        let err = super::validate_entries(&[descriptor]).unwrap_err();
        assert!(matches!(err, CharterViolation::CanonicalNotFirst { .. }));
    }

    #[test]
    fn charter_rejects_alias_with_whitespace() {
        static ALIASES: &[&str] = &["sorafs.sf1@1.0.0", " sorafs-sf1 "];
        let descriptor = ChunkerProfileDescriptor {
            id: crate::ProfileId(1),
            namespace: "sorafs",
            name: "sf1",
            semver: "1.0.0",
            profile: ChunkProfile::DEFAULT,
            multihash_code: DEFAULT_MULTIHASH_CODE,
            aliases: ALIASES,
        };
        let err = super::validate_entries(&[descriptor]).unwrap_err();
        assert!(matches!(
            err,
            CharterViolation::AliasContainsWhitespace { .. }
        ));
    }
}

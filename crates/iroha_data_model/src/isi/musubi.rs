//! Musubi package registry instructions.

use super::*;
use crate::musubi::{
    MusubiPackageId, MusubiPackageRef, MusubiRelease, MusubiShortAlias, MusubiVersion,
};

isi! {
    /// Publish an immutable Musubi package release into the registry.
    pub struct PublishMusubiRelease {
        /// Complete release record requested by the publisher.
        pub release: MusubiRelease,
    }
}

impl PublishMusubiRelease {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.musubi.release.publish";

    /// Construct a publish instruction.
    #[must_use]
    pub const fn new(release: MusubiRelease) -> Self {
        Self { release }
    }
}

impl crate::seal::Instruction for PublishMusubiRelease {}

isi! {
    /// Yank an existing Musubi release without deleting or replacing it.
    pub struct YankMusubiRelease {
        /// Exact release to yank.
        pub package: MusubiPackageRef,
        /// Human-readable reason recorded with the yank.
        pub reason: String,
    }
}

impl YankMusubiRelease {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.musubi.release.yank";

    /// Construct a yank instruction.
    #[must_use]
    pub fn new(package: MusubiPackageRef, reason: impl Into<String>) -> Self {
        Self {
            package,
            reason: reason.into(),
        }
    }
}

impl crate::seal::Instruction for YankMusubiRelease {}

isi! {
    /// Bind or update a curated global short alias for a Musubi package id.
    pub struct SetMusubiShortAlias {
        /// Alias binding selected by governance or an elevated registry authority.
        pub alias: MusubiShortAlias,
    }
}

impl SetMusubiShortAlias {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.musubi.short_alias.set";

    /// Construct a short-alias instruction.
    #[must_use]
    pub const fn new(alias: MusubiShortAlias) -> Self {
        Self { alias }
    }
}

impl crate::seal::Instruction for SetMusubiShortAlias {}

isi! {
    /// Assert that a package id has a concrete released version.
    pub struct AssertMusubiReleaseExists {
        /// Canonical package id.
        pub package: MusubiPackageId,
        /// Exact version that must exist.
        pub version: MusubiVersion,
    }
}

impl AssertMusubiReleaseExists {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.musubi.release.assert_exists";

    /// Construct an existence assertion.
    #[must_use]
    pub const fn new(package: MusubiPackageId, version: MusubiVersion) -> Self {
        Self { package, version }
    }
}

impl crate::seal::Instruction for AssertMusubiReleaseExists {}

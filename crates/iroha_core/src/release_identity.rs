//! Source-bound identity shared by running validators and local release preparation.

use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::parameter::system::ConsensusHandshakeMetadata;

/// Immutable build metadata supplied by the executable that owns a runtime.
///
/// Shared libraries never inspect environment variables or Git to construct this
/// value. It has no runtime configuration, deserialization, mutation, or default
/// path. An explicit development label is never valid release provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildIdentity {
    version: &'static str,
    source_commit: &'static str,
    dpn_validator_release_commit: Option<&'static str>,
    cargo_features: Option<&'static str>,
    target_triple: Option<&'static str>,
}

/// Invalid metadata compiled into an executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BuildIdentityError {
    /// The executable lacks a canonical source revision or package version.
    #[error(
        "executable identity requires a version and an exact lowercase 40-digit source commit or local-fast-build"
    )]
    InvalidSource,
    /// The sealed source marker contradicts the canonical executable revision.
    #[error("sealed source metadata must equal the executable's exact source commit")]
    ConflictingSealedSource,
    /// Development metadata cannot authorize a release operation.
    #[error("release admission requires an exact lowercase 40-digit source commit")]
    DevelopmentSource,
}

impl BuildIdentity {
    /// Validate immutable metadata compiled into the owning executable.
    ///
    /// The canonical source is `VERGEN_GIT_SHA`; `IROHA_GIT_COMMIT_HASH` is
    /// an optional matching sealed artifact marker, never a fallback. Local
    /// builds explicitly select `local-fast-build` without a sealed marker.
    ///
    /// # Errors
    /// Returns an error for absent, malformed or contradictory source metadata.
    /// Valid syntax alone does not establish authenticated release provenance.
    pub fn from_compiled_parts(
        version: &'static str,
        source_commit: Option<&'static str>,
        sealed_source_commit: Option<&'static str>,
        dpn_validator_release_commit: Option<&'static str>,
        cargo_features: Option<&'static str>,
        target_triple: Option<&'static str>,
    ) -> Result<Self, BuildIdentityError> {
        let source_commit = source_commit.ok_or(BuildIdentityError::InvalidSource)?;
        let exact_commit = is_exact_commit(source_commit);
        if version.is_empty()
            || version.trim() != version
            || (!exact_commit && source_commit != "local-fast-build")
        {
            return Err(BuildIdentityError::InvalidSource);
        }
        if sealed_source_commit.is_some_and(|sealed| !exact_commit || sealed != source_commit) {
            return Err(BuildIdentityError::ConflictingSealedSource);
        }
        Ok(Self {
            version,
            source_commit,
            dpn_validator_release_commit,
            cargo_features,
            target_triple,
        })
    }

    /// Package version compiled into the executable.
    #[must_use]
    pub const fn version(self) -> &'static str {
        self.version
    }

    /// Canonical source revision compiled into the executable.
    #[must_use]
    pub const fn source_commit(self) -> &'static str {
        self.source_commit
    }

    /// Require the exact source commit needed by release admission.
    ///
    /// # Errors
    /// Rejects explicit development identity. Callers must additionally verify
    /// the signed source, signer and immutable artifact closure.
    pub fn release_source_commit(self) -> Result<&'static str, BuildIdentityError> {
        if is_exact_commit(self.source_commit) {
            Ok(self.source_commit)
        } else {
            Err(BuildIdentityError::DevelopmentSource)
        }
    }

    /// Source-bound fingerprint published by the Sumeragi adapter.
    ///
    /// Its preimage remains the package version immediately followed by the
    /// exact source revision. Features and target do not change this identity.
    #[must_use]
    pub fn build_fingerprint(self) -> Hash {
        let mut bytes = self.version.as_bytes().to_vec();
        bytes.extend_from_slice(self.source_commit.as_bytes());
        Hash::new(bytes)
    }

    /// Public executable metadata in the canonical Torii status schema.
    #[must_use]
    pub fn status(self) -> iroha_torii_shared::status::BuildStatus {
        iroha_torii_shared::status::BuildStatus {
            version: self.version.to_owned(),
            git_commit_sha: self.source_commit.to_owned(),
            dpn_validator_release_commit: self
                .dpn_validator_release_commit
                .unwrap_or("unknown")
                .to_owned(),
            cargo_features: self.cargo_features.unwrap_or("unknown").to_owned(),
            target_triple: self.target_triple.unwrap_or("unknown").to_owned(),
        }
    }
}

/// Capture the canonical build metadata in the executable invoking this macro.
///
/// Invoke at the executable startup boundary, then pass the returned immutable
/// identity to runtime constructors. Expansion occurs in the caller, so the
/// shared Core library does not embed or depend on these environment values.
#[macro_export]
macro_rules! compiled_build_identity {
    () => {
        $crate::release_identity::BuildIdentity::from_compiled_parts(
            env!("CARGO_PKG_VERSION"),
            option_env!("VERGEN_GIT_SHA"),
            option_env!("IROHA_GIT_COMMIT_HASH"),
            option_env!("IROHA_DPN_VALIDATOR_RELEASE_COMMIT"),
            option_env!("VERGEN_CARGO_FEATURES"),
            option_env!("VERGEN_CARGO_TARGET_TRIPLE"),
        )
    };
}

fn is_exact_commit(source: &str) -> bool {
    source.len() == 40
        && source
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Verify canonical signed genesis bytes against the explicitly selected genesis key.
/// Returns the actual block hash and its validated signed consensus metadata.
pub fn genesis_identity(
    bytes: &[u8],
    public_key: &PublicKey,
) -> eyre::Result<(Hash, ConsensusHandshakeMetadata)> {
    let block = iroha_genesis::decode_signed_genesis(bytes)?;
    if !block.header().is_genesis() || block.encode_wire()?.as_slice() != bytes {
        eyre::bail!("release genesis is not canonical framed Norito");
    }
    crate::validate_genesis_block(
        &block,
        &iroha_data_model::account::AccountId::new(public_key.clone()),
    )?;
    let metadata = iroha_genesis::signed_genesis_consensus_metadata(&block)?;
    Ok((Hash::from(block.hash()), metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = "1234567890abcdef1234567890abcdef12345678";
    const OTHER: &str = "2234567890abcdef1234567890abcdef12345678";

    #[test]
    fn executable_identity_rejects_missing_malformed_and_conflicting_source() {
        for source in [
            None,
            Some(""),
            Some("unknown"),
            Some(" HEAD "),
            Some("test-build"),
            Some("1234567890abcdef1234567890abcdef1234567"),
            Some("1234567890ABCDEF1234567890abcdef12345678"),
            Some("1234567890abcdef1234567890abcdef12345678\n"),
        ] {
            assert_eq!(
                BuildIdentity::from_compiled_parts("3.0.0", source, None, None, None, None),
                Err(BuildIdentityError::InvalidSource)
            );
        }
        for version in ["", " 3.0.0", "3.0.0\n"] {
            assert_eq!(
                BuildIdentity::from_compiled_parts(version, Some(SOURCE), None, None, None, None),
                Err(BuildIdentityError::InvalidSource)
            );
        }
        assert_eq!(
            BuildIdentity::from_compiled_parts("3.0.0", None, Some(SOURCE), None, None, None),
            Err(BuildIdentityError::InvalidSource)
        );
        for (source, sealed) in [
            (SOURCE, OTHER),
            (SOURCE, ""),
            ("local-fast-build", SOURCE),
            ("local-fast-build", "local-fast-build"),
        ] {
            assert_eq!(
                BuildIdentity::from_compiled_parts(
                    "3.0.0",
                    Some(source),
                    Some(sealed),
                    None,
                    None,
                    None
                ),
                Err(BuildIdentityError::ConflictingSealedSource)
            );
        }
    }

    #[test]
    fn development_identity_cannot_admit_a_release() {
        let development = BuildIdentity::from_compiled_parts(
            "3.0.0",
            Some("local-fast-build"),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            development.release_source_commit(),
            Err(BuildIdentityError::DevelopmentSource)
        );
        let release = BuildIdentity::from_compiled_parts(
            "3.0.0",
            Some(SOURCE),
            Some(SOURCE),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(release.release_source_commit(), Ok(SOURCE));
        assert_ne!(release.build_fingerprint(), development.build_fingerprint());
    }

    #[test]
    fn executable_profiles_share_exact_source_fingerprint_and_report_owned_metadata() {
        let daemon = BuildIdentity::from_compiled_parts(
            "3.0.0",
            Some(SOURCE),
            Some(SOURCE),
            Some(OTHER),
            Some("telemetry"),
            Some("aarch64-apple-darwin"),
        )
        .unwrap();
        let cli = BuildIdentity::from_compiled_parts(
            "3.0.0",
            Some(SOURCE),
            Some(SOURCE),
            None,
            Some("cli"),
            Some("aarch64-apple-darwin"),
        )
        .unwrap();
        let pk2 = BuildIdentity::from_compiled_parts(
            "3.0.0",
            Some(SOURCE),
            Some(SOURCE),
            None,
            Some("dev-tools"),
            Some("aarch64-unknown-linux-gnu"),
        )
        .unwrap();
        assert_eq!(daemon.build_fingerprint(), cli.build_fingerprint());
        assert_eq!(daemon.build_fingerprint(), pk2.build_fingerprint());
        assert_eq!(
            daemon.build_fingerprint(),
            Hash::new(b"3.0.01234567890abcdef1234567890abcdef12345678")
        );
        for changed in [
            BuildIdentity::from_compiled_parts("3.0.1", Some(SOURCE), None, None, None, None)
                .unwrap(),
            BuildIdentity::from_compiled_parts("3.0.0", Some(OTHER), None, None, None, None)
                .unwrap(),
        ] {
            assert_ne!(daemon.build_fingerprint(), changed.build_fingerprint());
        }
        let status = daemon.status();
        assert_eq!(status.version, daemon.version());
        assert_eq!(status.git_commit_sha, SOURCE);
        assert_eq!(status.dpn_validator_release_commit, OTHER);
        assert_eq!(status.cargo_features, "telemetry");
        assert_eq!(status.target_triple, "aarch64-apple-darwin");
    }
}

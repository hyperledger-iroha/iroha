//! Unauthenticated source recipes shared by offline construction and strict intake.
//!
//! These values select compiled sources only. They cannot construct an installed
//! capability, substitute a signed manifest or bypass original-key qualification.

use iroha_kagemusha_proof::{
    a_relation::{bootstrap::BootstrapPolicy, own::OwnPolicy},
    finality::{continuity::SourceVerifier, history::HistoryAnchor},
};
use iroha_plonk_gadgets::p256::native::{Affine, words_from_be};

use super::*;

/// Raw provider/root source policy, independent of the eventual scheme/manifest ID.
/// Offline tooling supplies its intended policy; installation derives the same
/// values from its authenticated scheme. Construction grants no authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceScopeV1 {
    provider: [u128; 2],
    root: Affine,
}
impl SourceScopeV1 {
    /// Validate only the fixed provider and canonical finite root point.
    /// # Errors
    /// Zero provider or invalid P-256 point.
    pub fn new(provider: [u128; 2], root: Affine) -> Result<Self, Error> {
        OwnPolicy::new(provider, root).map_err(|_| Error::Authority)?;
        Ok(Self { provider, root })
    }

    pub(super) fn from_scheme(scheme: &KagemushaWalletSchemeV1) -> Result<Self, Error> {
        scheme
            .scheme_root_key
            .validate()
            .map_err(|_| Error::Authority)?;
        let key = scheme.scheme_root_key.as_sec1_bytes();
        let root = Affine {
            x: words_from_be(key[1..33].try_into().map_err(|_| Error::Authority)?),
            y: words_from_be(key[33..65].try_into().map_err(|_| Error::Authority)?),
        };
        let provider = [
            u128::from_le_bytes(
                scheme.provider_contract[..16]
                    .try_into()
                    .map_err(|_| Error::Authority)?,
            ),
            u128::from_le_bytes(
                scheme.provider_contract[16..]
                    .try_into()
                    .map_err(|_| Error::Authority)?,
            ),
        ];
        Self::new(provider, root)
    }
    pub(super) fn own(self) -> Result<OwnPolicy, Error> {
        OwnPolicy::new(self.provider, self.root).map_err(|_| Error::Authority)
    }
    pub(super) fn bootstrap(self) -> Result<BootstrapPolicy, Error> {
        BootstrapPolicy::new(self.provider, self.root).map_err(|_| Error::Authority)
    }
    pub(super) const fn root(self) -> Affine {
        self.root
    }
}

/// Exact receipt source and intended genesis anchor before wallet installation.
/// The source is sealed to its compiled owner, but this raw pair is not an
/// authenticated wallet installation or approval of the caller's chosen genesis.
#[derive(Clone, Copy)]
pub struct ReceiptSourceRecipeV1<'a> {
    pub(super) source: &'a SourceVerifier,
    pub(super) anchor: &'a HistoryAnchor,
}
impl<'a> ReceiptSourceRecipeV1<'a> {
    /// Assemble a source recipe without claiming installation authority.
    #[must_use]
    pub const fn new(source: &'a SourceVerifier, anchor: &'a HistoryAnchor) -> Self {
        Self { source, anchor }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_source_scope_checks_both_fixed_inputs_without_installation_identity() {
        assert!(SourceScopeV1::new([0; 2], Affine::GENERATOR).is_err());
        let invalid = Affine {
            x: [0; 4],
            y: [0; 4],
        };
        assert!(SourceScopeV1::new([1, 2], invalid).is_err());
        let scope = SourceScopeV1::new([1, 2], Affine::GENERATOR).unwrap();
        scope.own().unwrap();
        scope.bootstrap().unwrap();
        assert_eq!(scope.root(), Affine::GENERATOR);
    }
}

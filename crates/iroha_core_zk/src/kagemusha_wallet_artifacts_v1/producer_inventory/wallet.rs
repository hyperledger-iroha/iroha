//! One complete authenticated source grant for native wallet proving.
//!
//! Metadata remains resident; each active import reads one exact signed original.
//! There is no constructor from an Omega-only grant, readiness flag or subset.

use iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier;
use iroha_kagemusha_proof::{
    a_relation::schedule::compiled::OperationRoute,
    finality::{catalog::VerifierLimits, native::Parameters},
};
use iroha_plonk::{ProvingKey, keys::pk::artifact::ReadConfig, pcs::ipa::PinnedParams};

use super::*;

/// A complete wallet source graph or one bounded active import was rejected.
#[derive(Debug, thiserror::Error)]
pub enum WalletSourcesErrorV1 {
    /// Wrong installation, route, member or original content address.
    #[error(transparent)]
    Original(#[from] Error),
    /// One of the sixteen compiled sigma sources did not qualify.
    #[error(transparent)]
    Sigma(#[from] SigmaQualificationErrorV1),
    /// The complete ordinary receipt graph differs from native genesis or its sources.
    #[error(transparent)]
    Finality(#[from] FinalityQualificationErrorV1),
    /// A Q source or exact signature policy did not qualify.
    #[error(transparent)]
    Q(#[from] QQualificationErrorV1),
    /// One logical operation route or stage did not qualify.
    #[error(transparent)]
    Operation(#[from] OperationQualificationErrorV1),
    /// The sole final Omega source failed complete terminal-catalog closure.
    #[error(transparent)]
    Omega(#[from] OmegaQualificationErrorV1),
}

impl WalletSourcesErrorV1 {
    /// Whether caller cancellation interrupted source work without a proof verdict.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        match self {
            Self::Original(error) => error.is_cancelled(),
            Self::Sigma(error) => match error {
                SigmaQualificationErrorV1::Original(error) => error.is_cancelled(),
                SigmaQualificationErrorV1::Monetary(error) => error.is_cancelled(),
                SigmaQualificationErrorV1::Administrative(error) => error.is_cancelled(),
                SigmaQualificationErrorV1::Parameters | SigmaQualificationErrorV1::Metadata(_) => {
                    false
                }
            },
            Self::Finality(error) => match error {
                FinalityQualificationErrorV1::Original(error) => error.is_cancelled(),
                FinalityQualificationErrorV1::Source(error) => error.is_cancelled(),
                FinalityQualificationErrorV1::Anchor(_)
                | FinalityQualificationErrorV1::AnchorMismatch => false,
            },
            Self::Q(error) => match error {
                QQualificationErrorV1::Original(error) => error.is_cancelled(),
                QQualificationErrorV1::Sigma(error) => error.is_cancelled(),
                QQualificationErrorV1::Signature(error) => error.is_cancelled(),
                QQualificationErrorV1::Source | QQualificationErrorV1::Metadata(_) => false,
            },
            Self::Operation(error) => match error {
                OperationQualificationErrorV1::Cancelled => true,
                OperationQualificationErrorV1::Original(error) => error.is_cancelled(),
                OperationQualificationErrorV1::Bootstrap(error) => match error {
                    BootstrapQualificationErrorV1::Original(error) => error.is_cancelled(),
                    BootstrapQualificationErrorV1::Native(error) => error.is_cancelled(),
                    BootstrapQualificationErrorV1::Source => false,
                },
                OperationQualificationErrorV1::Source
                | OperationQualificationErrorV1::A(_)
                | OperationQualificationErrorV1::W(_) => false,
            },
            Self::Omega(error) => match error {
                OmegaQualificationErrorV1::Original(error) => error.is_cancelled(),
                OmegaQualificationErrorV1::Native(error) => error.is_cancelled(),
                OmegaQualificationErrorV1::Routes | OmegaQualificationErrorV1::Catalog => false,
            },
        }
    }

    /// Whether the failure is missing or unreadable reinstallable source material.
    /// Cryptographic, length, hash, installation and source mismatches remain false.
    pub const fn is_unavailable(&self) -> bool {
        matches!(
            self,
            Self::Original(Error::Unavailable)
                | Self::Sigma(SigmaQualificationErrorV1::Original(Error::Unavailable))
                | Self::Finality(FinalityQualificationErrorV1::Original(Error::Unavailable))
                | Self::Q(QQualificationErrorV1::Original(Error::Unavailable))
                | Self::Operation(OperationQualificationErrorV1::Original(Error::Unavailable))
                | Self::Operation(OperationQualificationErrorV1::Bootstrap(
                    BootstrapQualificationErrorV1::Original(Error::Unavailable)
                ))
                | Self::Omega(OmegaQualificationErrorV1::Original(Error::Unavailable))
        )
    }
}

/// Complete source-qualified wallet graph under one authenticated installation.
/// This owns every exact sigma, receipt, Q, logical operation and final Omega
/// source. No original PK or proving polynomial is retained between imports.
/// Custody admission and durable wallet orchestration remain separate owners.
pub struct QualifiedWalletSourcesV1 {
    authenticated: AuthenticatedProducerInventoryV1,
    scope: SourceScopeV1,
    sigmas: QualifiedSigmasV1,
    finality: QualifiedReceiptSourceV1,
    q: Vec<QualifiedQProgramV1>,
    routes: Vec<QualifiedOperationRouteV1>,
    omega: QualifiedOmegaProgramV1,
}

fn route_index(route: OperationRoute) -> Result<usize, Error> {
    compiled_routes()
        .iter()
        .position(|candidate| *candidate == route)
        .ok_or(Error::Inventory)
}

fn require_installation(
    selected: ([u8; 32], [u8; 32]),
    installed: ([u8; 32], [u8; 32]),
) -> Result<(), Error> {
    if selected != installed {
        return Err(Error::Authority);
    }
    Ok(())
}

impl AuthenticatedProducerInventoryV1 {
    /// Consume this authenticated inventory only after every compiled source has
    /// been reconstructed and every selected original PK strictly imported.
    /// The native finality verifier must be independently selected from signed genesis.
    /// All imports are sequential; only exact descriptor/VK metadata survives each.
    /// # Errors
    /// Another installation/native genesis, any incomplete route or source graph,
    /// changed source/context/key, unavailable original, or a resource bound failure.
    pub fn qualify_wallet(
        self,
        installed: &InstalledVerifierPackV1,
        native_finality: &SumeragiFinalityVerifier,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
        finality_params: Parameters,
        finality_limits: VerifierLimits,
    ) -> Result<QualifiedWalletSourcesV1, WalletSourcesErrorV1> {
        require_installation(
            self.installation(),
            (
                installed.verifier().scheme().scheme_id(),
                installed.verifier().manifest_digest(),
            ),
        )?;
        let scope = SourceScopeV1::from_scheme(installed.verifier().scheme())?;
        // Native genesis binding happens before any source-original read.
        let finality =
            self.qualify_finality(native_finality, originals, finality_params, finality_limits)?;
        let sigmas = self.qualify_sigmas(originals, config)?;
        let mut q = Vec::with_capacity(self.inventory.operations.len());
        for program in 0..self.inventory.operations.len() {
            q.push(self.qualify_q_program(
                installed,
                &sigmas,
                u32::try_from(program).map_err(|_| Error::Inventory)?,
                originals,
                config,
            )?);
        }
        let mut routes = Vec::with_capacity(self.inventory.routes.len());
        for (index, program) in self.inventory.routes.iter().enumerate() {
            routes.push(
                self.qualify_operation_route(
                    installed,
                    q.get(usize::try_from(*program).map_err(|_| Error::Inventory)?)
                        .ok_or(Error::Inventory)?,
                    u32::try_from(index).map_err(|_| Error::Inventory)?,
                    Some(&finality),
                    originals,
                    config,
                )?,
            );
        }
        let omega = self.qualify_omega(installed, &routes, originals, config)?;
        Ok(QualifiedWalletSourcesV1 {
            authenticated: self,
            scope,
            sigmas,
            finality,
            q,
            routes,
            omega,
        })
    }
}

impl QualifiedWalletSourcesV1 {
    /// Exact scheme and manifest identities of this complete graph.
    pub const fn installation(&self) -> ([u8; 32], [u8; 32]) {
        self.authenticated.installation()
    }
    /// Compiled provider and scheme-root source policy.
    pub const fn scope(&self) -> SourceScopeV1 {
        self.scope
    }
    /// Signed immutable references for this fully qualified graph.
    pub const fn inventory(&self) -> &ProducerInventoryV1 {
        self.authenticated.inventory()
    }
    /// Every exact sigma verifier in the global selector order.
    pub const fn sigmas(&self) -> &QualifiedSigmasV1 {
        &self.sigmas
    }
    /// Complete receipt source bound to independently authenticated native genesis.
    pub const fn finality(&self) -> &QualifiedReceiptSourceV1 {
        &self.finality
    }
    /// Complete sole Omega source; partial owners cannot construct this grant.
    pub const fn omega(&self) -> &QualifiedOmegaProgramV1 {
        &self.omega
    }
    /// The exact qualified owner selected by operation and both sigma selectors.
    /// # Errors
    /// Undefined or incompatible own/incoming selector for the requested operation.
    pub fn route(&self, route: OperationRoute) -> Result<&QualifiedOperationRouteV1, Error> {
        self.routes.get(route_index(route)?).ok_or(Error::Inventory)
    }
    /// Ordered Q sources for the exact selected logical route.
    /// # Errors
    /// Undefined route or an unavailable program index.
    pub fn q(&self, route: OperationRoute) -> Result<&QualifiedQProgramV1, Error> {
        let (_, _, program) = self.route(route)?.identity();
        self.q
            .get(usize::try_from(program).map_err(|_| Error::Inventory)?)
            .ok_or(Error::Inventory)
    }
    fn record(&self, route: OperationRoute) -> Result<&OperationV1, Error> {
        let (_, _, program) = self.route(route)?.identity();
        self.inventory()
            .operations
            .get(usize::try_from(program).map_err(|_| Error::Inventory)?)
            .ok_or(Error::Inventory)
    }
    /// Import exactly one selected sigma's original tables into its compiled native owner.
    /// Drop the returned prover before importing the next stage to bound resident tables.
    /// # Errors
    /// Undefined selector, bounded read or strict original-source/key mismatch.
    pub fn import_sigma(
        &self,
        selector: u8,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
    ) -> Result<ImportedSigmaV1, WalletSourcesErrorV1> {
        self.import_sigma_cancellable(selector, originals, config, None)
    }

    /// Import the same exact source with a caller-owned cancellation signal.
    /// # Errors
    /// The ordinary source errors, or cancellation without an imported key.
    pub fn import_sigma_cancellable(
        &self,
        selector: u8,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<ImportedSigmaV1, WalletSourcesErrorV1> {
        iroha_pasta::CancellationToken::checkpoint(cancellation).map_err(|_| Error::Cancelled)?;
        let index = *self
            .inventory()
            .sigma
            .get(usize::from(selector))
            .ok_or(Error::Inventory)?;
        let original = self
            .authenticated
            .read_original(index, originals, config.maximum_bytes)?;
        let k12 = PinnedParams::derive(12).map_err(|_| SigmaQualificationErrorV1::Parameters)?;
        let k14 = PinnedParams::derive(14).map_err(|_| SigmaQualificationErrorV1::Parameters)?;
        let owner = sigma::import_prover(
            usize::from(selector),
            &original,
            &k12,
            &k14,
            config,
            cancellation,
        )?;
        let actual = owner.metadata()?;
        let expected = self.sigmas.key(selector).ok_or(Error::Inventory)?;
        if actual.binding() != expected.binding()
            || actual.key().to_bytes() != expected.key().to_bytes()
        {
            return Err(Error::Inventory.into());
        }
        Ok(owner)
    }
    /// Import one exact Q source for a selected route, preserving its fixed signature policy.
    /// # Errors
    /// Undefined route/stage, bounded original failure or a changed source/key identity.
    pub fn import_q(
        &self,
        route: OperationRoute,
        stage: usize,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
    ) -> Result<ImportedQV1, WalletSourcesErrorV1> {
        self.import_q_cancellable(route, stage, originals, config, None)
    }

    /// Import the same exact source with a caller-owned cancellation signal.
    /// # Errors
    /// The ordinary source errors, or cancellation without an imported key.
    pub fn import_q_cancellable(
        &self,
        route: OperationRoute,
        stage: usize,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<ImportedQV1, WalletSourcesErrorV1> {
        iroha_pasta::CancellationToken::checkpoint(cancellation).map_err(|_| Error::Cancelled)?;
        let record = self.record(route)?;
        let index = *record.q.get(stage).ok_or(Error::Inventory)?;
        let original = self
            .authenticated
            .read_original(index, originals, config.maximum_bytes)?;
        let (source, signatures) = q_source_recipe(
            self.scope,
            route.variant,
            &record.own_class,
            &record.incoming_class,
            self.sigmas.metadata(),
        )?;
        let pallas = PinnedParams::derive(16).map_err(|_| QQualificationErrorV1::Source)?;
        let owner = q::import(
            stage,
            &source,
            &signatures,
            &original,
            &pallas,
            config,
            cancellation,
        )?;
        let actual = owner.metadata()?;
        let expected = self.q(route)?.keys().get(stage).ok_or(Error::Inventory)?;
        if actual.binding() != expected.binding()
            || actual.key().to_bytes() != expected.key().to_bytes()
        {
            return Err(Error::Inventory.into());
        }
        Ok(owner)
    }
    /// Import the original key for one selected native A stage.
    /// # Errors
    /// Undefined route/stage, bounded read or any strict source/context/key mismatch.
    pub fn import_a(
        &self,
        route: OperationRoute,
        stage: usize,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
    ) -> Result<ProvingKey<Eq>, WalletSourcesErrorV1> {
        self.import_a_cancellable(route, stage, originals, config, None)
    }

    /// Import the same exact source with a caller-owned cancellation signal.
    /// # Errors
    /// The ordinary source errors, or cancellation without an imported key.
    pub fn import_a_cancellable(
        &self,
        route: OperationRoute,
        stage: usize,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<ProvingKey<Eq>, WalletSourcesErrorV1> {
        iroha_pasta::CancellationToken::checkpoint(cancellation).map_err(|_| Error::Cancelled)?;
        let index = *self.record(route)?.a.get(stage).ok_or(Error::Inventory)?;
        let original = self
            .authenticated
            .read_original(index, originals, config.maximum_bytes)?;
        Ok(self.route(route)?.owner().import_a(
            stage,
            &original.proving_key,
            config,
            cancellation,
        )?)
    }
    /// Import the original key for one selected native W continuation stage.
    /// # Errors
    /// Undefined route/stage, bounded read or any strict source/context/key mismatch.
    pub fn import_w(
        &self,
        route: OperationRoute,
        stage: usize,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
    ) -> Result<ProvingKey<Ep>, WalletSourcesErrorV1> {
        self.import_w_cancellable(route, stage, originals, config, None)
    }

    /// Import the same exact source with a caller-owned cancellation signal.
    /// # Errors
    /// The ordinary source errors, or cancellation without an imported key.
    pub fn import_w_cancellable(
        &self,
        route: OperationRoute,
        stage: usize,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<ProvingKey<Ep>, WalletSourcesErrorV1> {
        iroha_pasta::CancellationToken::checkpoint(cancellation).map_err(|_| Error::Cancelled)?;
        let index = *self.record(route)?.w.get(stage).ok_or(Error::Inventory)?;
        let original = self
            .authenticated
            .read_original(index, originals, config.maximum_bytes)?;
        Ok(self.route(route)?.owner().import_w(
            stage,
            &original.proving_key,
            config,
            cancellation,
        )?)
    }
    /// Import the sole complete final Omega original for one active terminal proof.
    /// # Errors
    /// Bounded read failure or changed original/source/verifier identity.
    pub fn import_omega(
        &self,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
    ) -> Result<iroha_kagemusha_proof::omega::native::Prover, WalletSourcesErrorV1> {
        self.import_omega_cancellable(originals, config, None)
    }

    /// Import the same exact source with a caller-owned cancellation signal.
    /// # Errors
    /// The ordinary source errors, or cancellation without an imported key.
    pub fn import_omega_cancellable(
        &self,
        originals: &mut dyn OriginalSourceV1,
        config: ReadConfig,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<iroha_kagemusha_proof::omega::native::Prover, WalletSourcesErrorV1> {
        iroha_pasta::CancellationToken::checkpoint(cancellation).map_err(|_| Error::Cancelled)?;
        Ok(self
            .omega
            .import_prover_cancellable(originals, config, cancellation)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_typed_original_outages_are_reinstallable() {
        for error in [
            Error::Unavailable,
            Error::Inventory,
            Error::Profile,
            Error::Authority,
            Error::Proof,
        ] {
            let expected = error == Error::Unavailable;
            for nested in [
                WalletSourcesErrorV1::Original(error),
                WalletSourcesErrorV1::Sigma(SigmaQualificationErrorV1::Original(error)),
                WalletSourcesErrorV1::Finality(FinalityQualificationErrorV1::Original(error)),
                WalletSourcesErrorV1::Q(QQualificationErrorV1::Original(error)),
                WalletSourcesErrorV1::Operation(OperationQualificationErrorV1::Original(error)),
                WalletSourcesErrorV1::Operation(OperationQualificationErrorV1::Bootstrap(
                    BootstrapQualificationErrorV1::Original(error),
                )),
                WalletSourcesErrorV1::Omega(OmegaQualificationErrorV1::Original(error)),
            ] {
                assert_eq!(nested.is_unavailable(), expected);
            }
        }
    }

    #[test]
    fn dispatch_uses_all_three_fields_and_rejects_every_uncompiled_combination() {
        let routes = compiled_routes();
        for (index, route) in routes.iter().enumerate() {
            assert_eq!(route_index(*route), Ok(index));
        }
        for variant in Variant::ALL {
            for own in 0..=16 {
                for incoming in core::iter::once(None).chain((0..=16).map(Some)) {
                    let route = OperationRoute {
                        variant,
                        own,
                        incoming,
                    };
                    assert_eq!(
                        route_index(route).ok(),
                        routes.iter().position(|r| *r == route)
                    );
                }
            }
        }
    }

    #[test]
    fn complete_grant_requires_both_exact_installation_identities() {
        let installation = ([1; 32], [2; 32]);
        require_installation(installation, installation).unwrap();
        for field in 0..2 {
            for byte in 0..32 {
                let mut changed = installation;
                if field == 0 {
                    changed.0[byte] ^= 1;
                } else {
                    changed.1[byte] ^= 1;
                }
                assert_eq!(
                    require_installation(installation, changed),
                    Err(Error::Authority)
                );
            }
        }
    }
}

impl crate::kagemusha_wallet_proofs_v1::NativeProofError for WalletSourcesErrorV1 {
    fn is_cancelled(&self) -> bool {
        WalletSourcesErrorV1::is_cancelled(self)
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    #[test]
    fn source_cancellation_never_becomes_invalid_or_unavailable_material() {
        let cases = [
            WalletSourcesErrorV1::Original(Error::Cancelled),
            WalletSourcesErrorV1::Operation(OperationQualificationErrorV1::Cancelled),
            WalletSourcesErrorV1::Q(QQualificationErrorV1::Original(Error::Cancelled)),
            WalletSourcesErrorV1::Omega(OmegaQualificationErrorV1::Original(Error::Cancelled)),
            WalletSourcesErrorV1::Finality(FinalityQualificationErrorV1::Original(
                Error::Cancelled,
            )),
            WalletSourcesErrorV1::Sigma(SigmaQualificationErrorV1::Original(Error::Cancelled)),
        ];
        for error in cases {
            assert!(error.is_cancelled());
            assert!(!error.is_unavailable());
        }
        assert!(!WalletSourcesErrorV1::Original(Error::Unavailable).is_cancelled());
        assert!(!WalletSourcesErrorV1::Original(Error::Proof).is_cancelled());
    }
}

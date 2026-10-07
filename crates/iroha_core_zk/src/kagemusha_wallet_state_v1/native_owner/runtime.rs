//! Move-only native installation and account-admission ownership.

use super::*;
use crate::kagemusha_wallet_intake_v1::{self as intake, PendingWalletOpenV1};

/// Authenticated installation and sole original store retained across account retries.
/// Only native provisioning can construct its installed-source capabilities.
pub struct NativeWalletRuntimeV1<F: KagemushaWalletFsV1, P, S> {
    custody: RuntimeCustodyV1<F, P>,
    installed: Arc<InstalledVerifierPackV1>,
    sources: Arc<QualifiedWalletSourcesV1>,
    genesis: Arc<SumeragiFinalityVerifier>,
    originals: S,
    read: ReadConfig,
    budget: MemoryBudget,
}

pub(super) enum RuntimeCustodyV1<F: KagemushaWalletFsV1, P> {
    Exclusive(crate::kagemusha_wallet_advance_v1::KagemushaWalletProviderV1<F, P>),
    // Unexpected native references retain custody and cannot confer admission.
    Shared(AdvanceHandle<F, P>),
}

/// Failure of original admission or native state recovery.
#[derive(Debug, thiserror::Error)]
pub enum NativeOpenErrorV1 {
    /// Original/account/source admission did not complete.
    #[error(transparent)]
    Intake(#[from] intake::Error),
    /// Native state/proof initialization did not complete.
    #[error(transparent)]
    State(#[from] Error),
}

/// Failed admission retaining the exclusive runtime for a fresh begin attempt.
pub struct NativeOpenFailureV1<F: KagemushaWalletFsV1, P, S> {
    runtime: NativeWalletRuntimeV1<F, P, S>,
    error: NativeOpenErrorV1,
}
impl<F: KagemushaWalletFsV1, P, S> std::fmt::Debug for NativeOpenFailureV1<F, P, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeOpenFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl<F: KagemushaWalletFsV1, P, S> NativeOpenFailureV1<F, P, S> {
    /// Recover the same unadmitted runtime and its precise failure.
    #[must_use]
    pub fn into_parts(self) -> (NativeWalletRuntimeV1<F, P, S>, NativeOpenErrorV1) {
        (self.runtime, self.error)
    }
}

/// One native account challenge; failed finish or explicit abandonment consumes it.
pub struct PendingNativeWalletOpenV1<F: KagemushaWalletFsV1, P, S> {
    pending: PendingWalletOpenV1<F, P>,
    installed: Arc<InstalledVerifierPackV1>,
    sources: Arc<QualifiedWalletSourcesV1>,
    genesis: Arc<SumeragiFinalityVerifier>,
    originals: S,
    read: ReadConfig,
    budget: MemoryBudget,
}

/// Concrete coordinator admitted from actual originals and an existing account signature.
pub type NativeWalletCoordinatorV1<F, P, S> =
    Coordinator<AdvanceHandle<F, P>, ProviderArchive<F, P>, NativeWalletProofsV1<F, P, S>>;

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    NativeWalletRuntimeV1<F, P, S>
{
    /// Retain native-provisioned capabilities and the sole original store before account intake.
    /// This is not account admission and performs no payment-key or monetary operation.
    #[must_use]
    pub fn new(
        provider: crate::kagemusha_wallet_advance_v1::KagemushaWalletProviderV1<F, P>,
        installed: Arc<InstalledVerifierPackV1>,
        sources: Arc<QualifiedWalletSourcesV1>,
        genesis: Arc<SumeragiFinalityVerifier>,
        originals: S,
        read: ReadConfig,
        budget: MemoryBudget,
    ) -> Self {
        Self {
            custody: RuntimeCustodyV1::Exclusive(provider),
            installed,
            sources,
            genesis,
            originals,
            read,
            budget,
        }
    }

    /// Reconcile custody and sample a fresh account challenge bound to original frames.
    /// # Errors
    /// Returns the same unadmitted runtime on every failure, including unavailable storage.
    pub fn begin(
        self,
        credential: &[u8],
        certificates: &[u8],
        account: &[u8],
        asset: &[u8],
    ) -> Result<PendingNativeWalletOpenV1<F, P, S>, NativeOpenFailureV1<F, P, S>> {
        let Self {
            custody,
            installed,
            sources,
            genesis,
            originals,
            read,
            budget,
        } = self;
        let provider =
            match custody {
                RuntimeCustodyV1::Exclusive(provider) => provider,
                RuntimeCustodyV1::Shared(handle) => match handle.try_into_provider() {
                    Ok(provider) => provider,
                    Err(handle) => return Err(NativeOpenFailureV1 {
                        runtime: Self {
                            custody: RuntimeCustodyV1::Shared(handle),
                            installed,
                            sources,
                            genesis,
                            originals,
                            read,
                            budget,
                        },
                        error: Error::Provider(ProviderError::Unavailable(
                            crate::kagemusha_wallet_advance_v1::KagemushaWalletUnavailableV1::Busy,
                        ))
                        .into(),
                    }),
                },
            };
        let pending = match PendingWalletOpenV1::begin(
            provider,
            Arc::clone(&installed),
            Arc::clone(&sources),
            credential,
            certificates,
            account,
            asset,
        ) {
            Ok(pending) => pending,
            Err(failure) => {
                let (provider, error) = failure.into_parts();
                return Err(NativeOpenFailureV1 {
                    runtime: Self {
                        custody: RuntimeCustodyV1::Exclusive(provider),
                        installed,
                        sources,
                        genesis,
                        originals,
                        read,
                        budget,
                    },
                    error: error.into(),
                });
            }
        };
        Ok(PendingNativeWalletOpenV1 {
            pending,
            installed,
            sources,
            genesis,
            originals,
            read,
            budget,
        })
    }
}
impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    PendingNativeWalletOpenV1<F, P, S>
{
    /// Exact fresh native message for the existing Ed25519 account to sign.
    #[must_use]
    pub fn challenge(&self) -> &[u8] {
        self.pending.challenge()
    }

    /// Abandon this challenge and preserve custody for a fresh begin.
    #[must_use]
    pub fn abandon(self) -> NativeWalletRuntimeV1<F, P, S> {
        NativeWalletRuntimeV1 {
            custody: RuntimeCustodyV1::Exclusive(self.pending.abandon()),
            installed: self.installed,
            sources: self.sources,
            genesis: self.genesis,
            originals: self.originals,
            read: self.read,
            budget: self.budget,
        }
    }

    /// Consume the account challenge and initialize the actual native operation owner.
    /// # Errors
    /// The challenge is consumed; all native custody and original artifacts are returned for retry.
    pub fn finish(
        self,
        signature: &[u8],
    ) -> Result<NativeWalletCoordinatorV1<F, P, S>, NativeOpenFailureV1<F, P, S>> {
        let Self {
            pending,
            installed,
            sources,
            genesis,
            originals,
            read,
            budget,
        } = self;
        let admitted = match pending.finish(signature) {
            Ok(admitted) => admitted,
            Err(failure) => {
                let (provider, error) = failure.into_parts();
                return Err(NativeOpenFailureV1 {
                    runtime: NativeWalletRuntimeV1 {
                        custody: RuntimeCustodyV1::Exclusive(provider),
                        installed,
                        sources,
                        genesis,
                        originals,
                        read,
                        budget,
                    },
                    error: error.into(),
                });
            }
        };
        match admitted.into_coordinator(&genesis, originals, read, budget) {
            Ok(wallet) => Ok(wallet),
            Err((custody, originals, error)) => Err(NativeOpenFailureV1 {
                runtime: NativeWalletRuntimeV1 {
                    custody,
                    installed,
                    sources,
                    genesis,
                    originals,
                    read,
                    budget,
                },
                error: error.into(),
            }),
        }
    }
}

/// Independently provisioned native deployment authority and fixed resource limits.
/// No foreign open request or received Payment may supply these trust selections.
pub struct NativeInstallationConfigV1 {
    /// Exact installed scheme and signed manifest selected by native deployment.
    pub installation: crate::kagemusha_wallet_artifacts_v1::InstallationV1,
    /// Already authenticated native genesis and its exact finality committee.
    pub genesis: Arc<SumeragiFinalityVerifier>,
    /// Bounded strict original-key importer policy.
    pub read: ReadConfig,
    /// Native process-wide proving memory budget.
    pub budget: MemoryBudget,
    /// Pinned ordinary finality source parameters.
    pub finality_parameters: iroha_kagemusha_proof::finality::native::Parameters,
    /// Exact ordinary finality graph bounds.
    pub finality_limits: iroha_kagemusha_proof::finality::catalog::VerifierLimits,
}

/// Failed native startup retains the actual provider and sole original source for retry.
pub struct NativeStartupFailureV1<F: KagemushaWalletFsV1, P, S> {
    provider: crate::kagemusha_wallet_advance_v1::KagemushaWalletProviderV1<F, P>,
    originals: S,
    error: Error,
}
impl<F: KagemushaWalletFsV1, P, S> std::fmt::Debug for NativeStartupFailureV1<F, P, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeStartupFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl<F: KagemushaWalletFsV1, P, S> NativeStartupFailureV1<F, P, S> {
    /// Recover exact exclusive custody, source ownership and failure without any admission grant.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        crate::kagemusha_wallet_advance_v1::KagemushaWalletProviderV1<F, P>,
        S,
        Error,
    ) {
        (self.provider, self.originals, self.error)
    }
}
impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    NativeWalletRuntimeV1<F, P, S>
{
    /// Native startup loader for the signed complete wallet graph and real custody provider.
    ///
    /// The embedding app calls this from trusted native initialization, using independently
    /// deployed pins and authenticated genesis. It authenticates the complete signed verifier
    /// pack, inventory, all 52 source routes and every original proving key before retaining
    /// a runtime. Missing deployment material is unavailable, never a default trust root.
    /// # Errors
    /// Invalid signatures, mismatched native configuration, incomplete source graphs or I/O.
    /// The exclusive provider and original source are returned on every failure.
    pub fn load(
        config: &NativeInstallationConfigV1,
        provider: crate::kagemusha_wallet_advance_v1::KagemushaWalletProviderV1<F, P>,
        verifier_pack: &[u8],
        producer_inventory: &[u8],
        mut originals: S,
    ) -> Result<Self, NativeStartupFailureV1<F, P, S>> {
        let qualified = (|| -> Result<_, Error> {
            if provider.scheme_id() != &config.installation.scheme_id {
                return Err(Error::Proof("native configured custody scheme"));
            }
            let installed = proof(InstalledVerifierPackV1::load(
                verifier_pack,
                config.installation,
            ))?;
            let authenticated =
                proof(installed.authenticate_producer_inventory(producer_inventory))?;
            let sources = authenticated
                .qualify_wallet(
                    &installed,
                    &config.genesis,
                    &mut originals,
                    config.read,
                    config.finality_parameters.clone(),
                    config.finality_limits,
                )
                .map_err(|error| {
                    if error.is_unavailable() {
                        Error::ArtifactsUnavailable("native startup original source")
                    } else {
                        Error::Proof("native startup complete source graph")
                    }
                })?;
            Ok((Arc::new(installed), Arc::new(sources)))
        })();
        match qualified {
            Ok((installed, sources)) => Ok(Self::new(
                provider,
                installed,
                sources,
                Arc::clone(&config.genesis),
                originals,
                config.read,
                config.budget,
            )),
            Err(error) => Err(NativeStartupFailureV1 {
                provider,
                originals,
                error,
            }),
        }
    }
}

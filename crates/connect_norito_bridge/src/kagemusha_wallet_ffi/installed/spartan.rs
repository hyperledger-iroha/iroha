//! Signed five-entry Spartan carrier intake for the existing move-only runtime owner.
//!
//! The signed application release names this profile explicitly and pins the four carrier
//! metadata identities, the registered asset and the manifest. The scheme root that certified
//! the manifest's Artifact signer and this build's own compiled program digests are
//! independent of that release and of the carrier. Programs are read only from the installed
//! carrier root: no caller buffer, and no verifier pack or proof engine of the retained wallet
//! graph, selects or authenticates them. The result is the existing registry owner holding
//! the existing Spartan session. It never holds or produces a retained-graph coordinator.

use super::*;
use crate::kagemusha_wallet_ffi::spartan::{
    Action, FutureSession, SelectedEnrollment, Session as ProgramSession,
};
use iroha_core_zk::{
    kagemusha_spartan_native::{FutureRuntime, native},
    kagemusha_wallet_artifacts_v1::producer_inventory::spartan as carrier,
    kagemusha_wallet_enrollment_v1::{EnrollmentOwnerV1, SpartanCredentialProfileV1},
};
use iroha_data_model::{
    isi::kagemusha_wallet::spartan::InstallKagemushaSpartanProgramsV1 as Install,
    sumeragi_finality::SumeragiCommitVerifierV1,
};
use iroha_fs::PrivateDirectory;

type Provider<P> = state::NativeProviderV1<advance::KagemushaWalletStdFsV1, P>;
type Enrolling<P> = EnrollmentOwnerV1<advance::KagemushaWalletStdFsV1, P>;

/// Retained custody of the installed carrier root these programs were read from. It routes
/// no original and grants nothing; the runtime keeps it for the provider's whole lifetime.
/// Until then the buffer-free open rechecks this same root and never opens another one.
struct InstalledCarrier {
    root: PrivateDirectory,
}

/// Parameters authenticated from the installed carrier, with the exact governed originals
/// the session fingerprints for delivery retries. Only [`Installation::admit`] constructs it.
/// The one admitted set is shared, never rebuilt: enrollment borrows it as its credential
/// profile and the session then takes the same set.
struct Installation {
    parameters: Arc<native::AuthenticatedParameters>,
    install: Install,
    carrier: InstalledCarrier,
}

fn reply(kind: i32, bytes: Vec<u8>) -> Response {
    Response {
        kind,
        bytes,
        ..Response::default()
    }
}

/// Changed, foreign or malformed carrier DATA and refused authority are invalid. A failed
/// allocation is a local resource refusal, and a storage failure stays unavailable.
fn carrier_error(error: std::io::Error) -> Failure {
    use std::io::ErrorKind as Kind;
    if error
        .get_ref()
        .is_some_and(|inner| inner.is::<carrier::CompilerUnavailable>())
    {
        return Failure::code(ARTIFACTS_UNAVAILABLE);
    }
    if error
        .get_ref()
        .is_some_and(|inner| inner.is::<std::collections::TryReserveError>())
    {
        return Failure::code(RESOURCE);
    }
    match error.kind() {
        Kind::InvalidData | Kind::InvalidInput | Kind::NotFound | Kind::Other => {
            Failure::code(INVALID)
        }
        _ => Failure::unavailable(
            UNAVAILABLE,
            advance::KagemushaWalletUnavailableV1::from_io(&error),
        ),
    }
}

/// Open the exact installed carrier root. The path is bounded DATA naming one existing
/// private directory; nothing is created, and no role path inside it is caller-selected.
fn carrier_root(root: &[u8]) -> Result<PrivateDirectory> {
    if root.is_empty() || root.len() > ROOT_MAX || root.contains(&0) {
        return Err(Failure::code(INVALID));
    }
    let root = std::path::Path::new(std::str::from_utf8(root).map_err(|_| Failure::code(INVALID))?);
    if !root.is_absolute() {
        return Err(Failure::code(INVALID));
    }
    PrivateDirectory::open_exact(root).map_err(carrier_error)
}

/// The signed profile requires its complete metadata offer, byte-identical to the signed
/// pins. These two small frames are compared, never decoded or trusted here: the carrier
/// reader authenticates the installed files against the same pins.
fn require_offer(input: &RuntimeOriginals<'_>, release: &carrier::ReleasePins) -> Result<()> {
    if input.verifier_pack.is_empty()
        || input.producer_inventory.is_empty()
        || input.originals_root.is_empty()
        || BlobV1::of(input.verifier_pack) != release.pack
        || BlobV1::of(input.producer_inventory) != release.inventory
    {
        return Err(Failure::code(INVALID));
    }
    Ok(())
}

/// Exact equality with the installed originals. This compares; it never installs.
fn same_originals(install: &Install, bundle: &[u8], manifest: &[u8], signature: &[u8; 64]) -> bool {
    bundle == install.programs.as_slice()
        && manifest == install.manifest.as_slice()
        && *signature == install.signature
}

impl Installation {
    /// Authenticate the installed carrier under the signed profile, the registered asset and
    /// manifest pins, the scheme root, this build's compiled programs and the authenticated
    /// genesis. No platform, custody root, provider or owner exists yet.
    fn admit(
        input: &RuntimeOriginals<'_>,
        selected: &Selection,
        opening: &mut OpeningMetricsV1,
    ) -> Result<Self> {
        let profile = selected.spartan.ok_or(Failure::code(INVALID))?;
        // One signed release names exactly one financial profile.
        if selected.financial.is_some() {
            return Err(Failure::code(INVALID));
        }
        require_offer(input, &profile.release)?;
        let root = carrier_root(input.originals_root)?;
        let (install, producer) = carrier::read_from_build(
            &root,
            profile.release,
            &selected.scheme,
            profile.asset,
            profile.manifest_digest,
        )
        .map_err(carrier_error)?;
        if install.scheme != selected.scheme.scheme_id()
            || install.asset != profile.asset
            || install.manifest_digest != profile.manifest_digest
        {
            return Err(Failure::code(INVALID));
        }
        opening.lap("spartan_carrier");
        let finality =
            SumeragiCommitVerifierV1::new(&selected.genesis).map_err(|_| Failure::code(INVALID))?;
        let parameters = match producer {
            carrier::CompiledPrograms::Four(_) => native::AuthenticatedParameters::install(
                &install.programs,
                &install.manifest,
                install.signature,
                &selected.scheme,
                finality,
            ),
            carrier::CompiledPrograms::Five(programs) => {
                native::AuthenticatedParameters::install_five(
                    &install.programs,
                    &install.manifest,
                    install.signature,
                    &profile.manifest_digest,
                    &programs,
                    &selected.scheme,
                    finality,
                )
            }
        }?;
        if parameters.manifest_digest() != &profile.manifest_digest
            || parameters.id() != &selected.scheme.relation_id
        {
            return Err(Failure::code(INVALID));
        }
        // Close carrier custody over the whole program import before any owner can exist.
        root.revalidate().map_err(carrier_error)?;
        opening.lap("spartan_programs");
        Ok(Self {
            parameters: Arc::new(parameters),
            install,
            carrier: InstalledCarrier { root },
        })
    }
}

/// The custody phase the buffer-free open requires: the exclusive provider, before any
/// enrollment or account session holds it. A missing owner is closed; every other phase is
/// a conflict and stays exactly as it was found.
fn require_unopened<P: advance::KagemushaWalletPlatformV1>(
    custody: Option<&Custody<P>>,
) -> Result<()> {
    match custody.ok_or(Failure::code(CLOSED))? {
        Custody::Exclusive(_) => Ok(()),
        Custody::Enrolling(_) | Custody::Programs(_) => Err(Failure::code(CONFLICT)),
    }
}

/// The one custody owner of this attempt, in exactly one phase. Every refusal leaves it in
/// the phase it was found in; no phase is rebuilt from DATA.
enum Custody<P: advance::KagemushaWalletPlatformV1> {
    /// Exclusive provider before enrollment or account admission.
    Exclusive(Box<Provider<P>>),
    /// Enrollment on the same provider; completion returns it to `Exclusive`.
    Enrolling(Box<Enrolling<P>>),
    /// The single monetary engine of this owner, over the same provider.
    Programs(Box<ProgramSession<P, InstalledCarrier>>),
}

struct ProgramAdmission<P: advance::KagemushaWalletPlatformV1> {
    custody: Option<Custody<P>>,
    // Taken exactly once, by the session, together with the exclusive provider.
    installation: Option<Box<Installation>>,
    binding: BoundOriginals,
    enrollment_session: Option<Session>,
    enrolled: bool,
}

impl<P: advance::KagemushaWalletPlatformV1 + 'static> ProgramAdmission<P> {
    fn start_enrollment(&mut self, originals: [&[u8]; 3]) -> Result<Response> {
        if let Some(session) = &self.enrollment_session {
            if self.enrolled {
                return Err(Failure::code(CONFLICT));
            }
            if !session.matches(originals) {
                // A prior successful Start reply may have been lost. Fresh genuine session
                // originals reauthenticate that same retained phase, never another one.
                let renewed = self.binding.enrollment_session(originals)?;
                session.require_renewal_identity(&renewed)?;
                let Some(Custody::Enrolling(owner)) = self.custody.as_mut() else {
                    return Err(Failure::code(CONFLICT));
                };
                owner.renew_session(&renewed.config)?;
                self.enrollment_session = Some(renewed);
            }
            return Ok(reply(37, self.binding.enrollment_projection()));
        }
        let session = self.binding.enrollment_session(originals)?;
        let custody = self.custody.take().ok_or(Failure::code(CLOSED))?;
        let Custody::Exclusive(provider) = custody else {
            self.custody = Some(custody);
            return Err(Failure::code(CONFLICT));
        };
        // This owner admitted the parameters, so it alone selects the credential profile:
        // every E6 of this enrollment is the exact Spartan original under that same set.
        // No generator is derived here and no credential byte chooses a verifier.
        let Some(installation) = self.installation.as_deref() else {
            self.custody = Some(Custody::Exclusive(provider));
            return Err(Failure::code(INTERNAL));
        };
        let profile = SpartanCredentialProfileV1::new(Arc::clone(&installation.parameters));
        match EnrollmentOwnerV1::new_spartan(*provider, session.config.clone(), profile) {
            Ok(owner) => {
                self.custody = Some(Custody::Enrolling(Box::new(owner)));
                self.enrollment_session = Some(session);
                Ok(reply(37, self.binding.enrollment_projection()))
            }
            Err((provider, error)) => {
                self.custody = Some(Custody::Exclusive(Box::new(provider)));
                Err(error.into())
            }
        }
    }

    fn finish_enrollment(&mut self) -> Result<Response> {
        if !self.enrolled {
            let Some(Custody::Enrolling(owner)) = self.custody.as_mut() else {
                return Err(Failure::code(CONFLICT));
            };
            // Verifies the retained E6 result and request under this owner's Spartan
            // credential profile; a refusal keeps the whole enrollment phase for an exact
            // retry. The Spartan account intake receives the complete E6 (or its split
            // originals) from its caller and authenticates it again itself.
            let _retained = owner.open_originals()?;
            match self.custody.take() {
                Some(Custody::Enrolling(owner)) => {
                    self.custody = Some(Custody::Exclusive(Box::new((*owner).into_provider())));
                }
                other => {
                    self.custody = other;
                    return Err(Failure::code(INTERNAL));
                }
            }
            self.enrolled = true;
        }
        Ok(reply(26, Vec::new()))
    }

    /// Answer the buffer-free open from the installation this owner authenticated itself.
    /// It reads no caller original, moves no custody and creates no session: the exclusive
    /// provider, the authenticated parameters and the retained carrier root stay as they
    /// are for the account admission that follows, so a lost reply is retried unchanged.
    /// A closed owner, an owner in enrollment and a carrier root whose custody changed
    /// since admission all refuse.
    fn open_installed(&self) -> Result<Response> {
        require_unopened(self.custody.as_ref())?;
        let installation = self
            .installation
            .as_deref()
            .ok_or(Failure::code(INTERNAL))?;
        installation
            .carrier
            .root
            .revalidate()
            .map_err(carrier_error)?;
        Ok(reply(81, installation.parameters.id().to_vec()))
    }
}

impl<P: advance::KagemushaWalletPlatformV1 + 'static> open::Admission for ProgramAdmission<P> {
    fn spartan(&mut self, action: Action<'_>) -> Result<Response> {
        // An account admission from a complete E6 names no asset and no issuer. This owner's
        // signed selection supplies both, replacing anything an inner caller attached.
        let action = match action {
            Action::BeginAccountResult {
                result, account, ..
            } => Action::BeginAccountResult {
                result,
                account,
                selected: Some(SelectedEnrollment {
                    asset: self.binding.asset_original(),
                    issuer: self.binding.selected.enrollment_certificate(),
                }),
            },
            other => other,
        };
        if let Some(Custody::Programs(session)) = self.custody.as_mut() {
            // An existing session refuses the buffer-free open itself.
            return session.call(action);
        }
        if matches!(action, Action::OpenInstalled) {
            return self.open_installed();
        }
        if let Action::Install {
            bundle,
            manifest,
            signature,
        } = action
        {
            // Offered buffers are only compared with the programs this owner already
            // authenticated from its carrier. They select and install nothing.
            let installation = self
                .installation
                .as_deref()
                .ok_or(Failure::code(INTERNAL))?;
            return if same_originals(&installation.install, bundle, manifest, signature) {
                Ok(reply(81, installation.parameters.id().to_vec()))
            } else {
                Err(Failure::code(CONFLICT))
            };
        }
        let custody = self.custody.take().ok_or(Failure::code(CLOSED))?;
        let Custody::Exclusive(provider) = custody else {
            self.custody = Some(custody);
            return Err(Failure::code(CONFLICT));
        };
        let Some(installation) = self.installation.take() else {
            self.custody = Some(Custody::Exclusive(provider));
            return Err(Failure::code(INTERNAL));
        };
        let Installation {
            parameters,
            install,
            carrier,
        } = *installation;
        // The same exclusive journal moves to the authenticated parameters. This transfer
        // cannot fail, and a funded head of the retained graph never decodes as their head.
        let runtime = FutureRuntime::from_installed(
            *provider,
            parameters,
            self.binding.selected.scheme,
            carrier,
        );
        let mut session = Box::new(ProgramSession::new(
            runtime,
            &install.programs,
            &install.manifest,
            &install.signature,
        ));
        // The session is retained whatever this first call answers.
        let answer = session.call(action);
        self.custody = Some(Custody::Programs(session));
        answer
    }

    fn enrollment(&mut self, action: enrollment::Action<'_>) -> Result<Response> {
        use enrollment::Action as Step;
        match action {
            Step::RenewSession(originals) => {
                if self.enrolled {
                    return Err(Failure::code(CONFLICT));
                }
                let session = self.binding.enrollment_session(originals)?;
                self.enrollment_session
                    .as_ref()
                    .ok_or(Failure::code(CONFLICT))?
                    .require_renewal_identity(&session)?;
                let Some(Custody::Enrolling(owner)) = self.custody.as_mut() else {
                    return Err(Failure::code(CONFLICT));
                };
                // Authenticate before touching the retained owner. No provider, platform,
                // generation reply or outstanding vendor dispatch moves or is reconstructed.
                owner.renew_session(&session.config)?;
                self.enrollment_session = Some(session);
                Ok(reply(39, Vec::new()))
            }
            Step::Start(originals) => self.start_enrollment(originals),
            Step::Load => self.finish_enrollment(),
            // Account admission belongs to the Spartan session alone. The retained graph's
            // account open would need its coordinator, which this owner never has.
            Step::BeginOpen | Step::BeginResult(_) => Err(Failure::code(CONFLICT)),
            step => {
                if let Step::Begin([_, _, asset]) = &step
                    && self.binding.asset_original() != *asset
                {
                    return Err(Failure::code(INVALID));
                }
                let Some(Custody::Enrolling(owner)) = self.custody.as_mut() else {
                    return Err(Failure::code(CONFLICT));
                };
                enrollment::perform(&mut **owner, step)
            }
        }
    }

    fn prepare_close(&mut self) -> Result<()> {
        if let Some(Custody::Enrolling(owner)) = self.custody.as_mut() {
            owner.flush_apple_returned()?;
        }
        Ok(())
    }

    fn begin(&mut self, _: [&[u8]; 4]) -> Result<Vec<u8>> {
        // No retained-graph artifacts are installed on this owner.
        Err(Failure::code(ARTIFACTS_UNAVAILABLE))
    }

    fn finish(&mut self, _: &[u8]) -> Result<(Box<dyn Wallet>, state::Scheduler)> {
        Err(Failure::code(INVALID))
    }

    fn cancel(&mut self) -> Result<()> {
        Err(Failure::code(INVALID))
    }
}

/// One installation attempt of the signed Spartan profile: complete carrier and program
/// authentication, then a fresh platform, custody root, provider and owner. It creates no
/// registry ID, key, enrollment or account authority, and it neither reads nor changes the
/// retained wallet graph or its memo.
pub(super) fn install<P: advance::KagemushaWalletPlatformV1 + 'static>(
    input: &RuntimeOriginals<'_>,
    selected: Arc<Selection>,
    mut opening: OpeningMetricsV1,
    android: bool,
    platform: impl FnOnce() -> Result<(P, std::path::PathBuf)>,
) -> Result<Box<WalletInstallationAttempt>> {
    let installation = Installation::admit(input, &selected, &mut opening)?;
    let (platform, custody_root) = platform()?;
    let provider = open_custody_provider(platform, custody_root, selected.scheme.scheme_id())?;
    let owner = open::admission_owner(Box::new(ProgramAdmission {
        custody: Some(Custody::Exclusive(Box::new(provider))),
        installation: Some(Box::new(installation)),
        binding: BoundOriginals { selected, android },
        enrollment_session: None,
        enrolled: false,
    }));
    opening.lap("runtime");
    opening.finish();
    Ok(Box::new(WalletInstallationAttempt::new(owner, opening)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pins(pack: &[u8], inventory: &[u8]) -> carrier::ReleasePins {
        carrier::ReleasePins {
            pack: BlobV1::of(pack),
            inventory: BlobV1::of(inventory),
            catalog_digest: [3; 32],
            transport: BlobV1::of(b"transport"),
        }
    }

    fn offer<'a>(pack: &'a [u8], inventory: &'a [u8], root: &'a [u8]) -> RuntimeOriginals<'a> {
        RuntimeOriginals {
            app_manifest: b"app",
            envelope: b"envelope",
            wallet_runtime: b"runtime",
            verifier_pack: pack,
            producer_inventory: inventory,
            signed_genesis: b"genesis",
            originals_root: root,
            registration_source: b"",
        }
    }

    #[test]
    fn signed_profile_requires_its_complete_byte_identical_metadata_offer() {
        let release = pins(b"pack", b"inventory");
        require_offer(&offer(b"pack", b"inventory", b"/carrier"), &release).unwrap();
        let refused: [(&[u8], &[u8], &[u8]); 7] = [
            (b"", b"inventory", b"/carrier"),
            (b"pack", b"", b"/carrier"),
            (b"pack", b"inventory", b""),
            (b"", b"", b""),
            (b"pack ", b"inventory", b"/carrier"),
            (b"pack", b"Inventory", b"/carrier"),
            (b"inventory", b"pack", b"/carrier"),
        ];
        for (pack, inventory, root) in refused {
            assert_eq!(
                require_offer(&offer(pack, inventory, root), &release),
                Err(Failure::code(INVALID))
            );
        }
    }

    #[test]
    fn carrier_root_is_bounded_absolute_utf8_data_and_never_created() {
        let oversized = vec![b'/'; ROOT_MAX + 1];
        let refused: [&[u8]; 5] = [
            b"",
            b"relative/carrier",
            b"/carrier\0suffix",
            &[b'/', 0xff, 0xfe],
            &oversized,
        ];
        for root in refused {
            assert_eq!(carrier_root(root).err(), Some(Failure::code(INVALID)));
        }
        let absent = std::env::temp_dir().join("iroha-absent-spartan-carrier-root");
        assert!(!absent.exists());
        assert!(carrier_root(absent.to_str().unwrap().as_bytes()).is_err());
        assert!(!absent.exists());
    }

    #[test]
    fn carrier_refusals_keep_invalid_resource_and_unavailable_distinct() {
        use std::io::{Error, ErrorKind};
        for error in [
            Error::new(ErrorKind::InvalidData, "changed carrier"),
            Error::from(ErrorKind::NotFound),
            Error::other("refused authority"),
        ] {
            assert_eq!(carrier_error(error), Failure::code(INVALID));
        }
        let exhausted = Vec::<u8>::new().try_reserve_exact(usize::MAX).unwrap_err();
        assert_eq!(
            carrier_error(Error::new(
                ErrorKind::Unsupported,
                carrier::CompilerUnavailable
            )),
            Failure::code(ARTIFACTS_UNAVAILABLE)
        );
        assert_eq!(
            carrier_error(Error::from(ErrorKind::Unsupported)).status,
            UNAVAILABLE
        );
        assert_eq!(
            carrier_error(Error::other(exhausted)),
            Failure::code(RESOURCE)
        );
        assert_eq!(
            carrier_error(Error::from(ErrorKind::PermissionDenied)).status,
            UNAVAILABLE
        );
    }

    #[test]
    fn offered_install_buffers_only_compare_with_the_installed_originals() {
        const PROGRAMS: &[u8] = &[6, 0, 7, 255];
        const MANIFEST: &[u8] = &[4, 0, 255];
        const SIGNATURE: [u8; 64] = [5; 64];
        let install = Install {
            scheme: [1; 32],
            asset: [2; 32],
            manifest_digest: [3; 32],
            manifest: MANIFEST.to_vec(),
            signature: SIGNATURE,
            programs: PROGRAMS.to_vec(),
        };
        assert!(same_originals(&install, PROGRAMS, MANIFEST, &SIGNATURE));
        let mut changed = SIGNATURE;
        changed[63] ^= 1;
        let empty: &[u8] = &[];
        let refused: [(&[u8], &[u8], [u8; 64]); 4] = [
            (&PROGRAMS[..3], MANIFEST, SIGNATURE),
            (PROGRAMS, &MANIFEST[..2], SIGNATURE),
            (PROGRAMS, MANIFEST, changed),
            (empty, empty, [0; 64]),
        ];
        for (bundle, manifest, signature) in refused {
            assert!(!same_originals(&install, bundle, manifest, &signature));
        }
    }

    #[test]
    fn buffer_free_open_refuses_an_owner_whose_custody_is_gone() {
        // Only the absent phase exists without a provider: it is a closed owner, never an
        // unopened one. The other phases need actual exclusive custody to construct.
        assert_eq!(
            require_unopened::<CallbackPlatform>(None),
            Err(Failure::code(CLOSED))
        );
    }
}

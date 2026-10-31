//! Native release environment admission with one canonical bundle decoder.
//!
//! The producer and every release process consumer share this admission contract.
//! Retained ownership does not itself establish release qualification.

use std::{
    env, io,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use color_eyre::{
    Result,
    eyre::{WrapErr as _, ensure, eyre},
};
use iroha_crypto::sha256_reader_bounded;
use iroha_fs::{FileSnapshot, PrivateDirectory, ReaderDirectory, RetainedFile};
use iroha_release_bundle::{
    bundle::{AdmissionCustody, Bundle, BundleAnchors},
    manifest::{BinaryKind, is_digest},
};

use super::{lowercase_hex, resolved_program::ResolvedProgram};

const SOURCE: &str = "IROHA_RELEASE_SOURCE_MANIFEST_SHA256";
const MANIFEST: &str = "IROHA_RELEASE_PREBUILT_MANIFEST_SHA256";
const LOCK: &str = "IROHA_RELEASE_CARGO_LOCK_SHA256";
const ARTIFACT: &str = "IROHA_RELEASE_ARTIFACT_ROOT";
const TARGET: &str = "IROHA_TEST_TARGET_DIR";
const SKIP: &str = "IROHA_TEST_SKIP_BUILD";
const MAX_LOCK: u64 = 16 * 1024 * 1024;

#[derive(Debug)]
struct ReleaseInputs {
    source: String,
    manifest: String,
    lock: Option<String>,
    artifact: PathBuf,
    target: PathBuf,
}

fn exact_env(name: &str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(eyre!("{name} must contain valid Unicode")),
    }
}

fn exact_path(value: &str, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    let normalized: PathBuf = path.components().collect();
    ensure!(
        path.is_absolute()
            && path.as_os_str() == normalized.as_os_str()
            && !path
                .components()
                .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
            && !value.chars().any(char::is_control),
        "{label} must have normalized absolute spelling"
    );
    Ok(path)
}

impl ReleaseInputs {
    fn environment() -> Result<Option<Self>> {
        Self::from_values(exact_env)
    }

    fn from_values(mut value: impl FnMut(&str) -> Result<Option<String>>) -> Result<Option<Self>> {
        let source = value(SOURCE)?;
        let manifest = value(MANIFEST)?;
        let lock = value(LOCK)?;
        let Some(source) = source else {
            ensure!(
                manifest.is_none() && lock.is_none(),
                "{MANIFEST} and {LOCK} require {SOURCE}"
            );
            return Ok(None);
        };
        ensure!(is_digest(&source), "{SOURCE} must be lowercase SHA-256");
        let manifest =
            manifest.ok_or_else(|| eyre!("release resolution requires inherited {MANIFEST}"))?;
        ensure!(is_digest(&manifest), "{MANIFEST} must be lowercase SHA-256");
        if let Some(lock) = &lock {
            ensure!(is_digest(lock), "{LOCK} must be lowercase SHA-256");
        }
        ensure!(
            value(SKIP)?.as_deref() == Some("1"),
            "release resolution requires {SKIP}=1 after top-level prebuild"
        );
        let artifact = exact_path(
            &value(ARTIFACT)?
                .ok_or_else(|| eyre!("release resolution requires inherited {ARTIFACT}"))?,
            ARTIFACT,
        )?;
        let target = exact_path(
            &value(TARGET)?
                .ok_or_else(|| eyre!("release resolution requires inherited {TARGET}"))?,
            TARGET,
        )?;
        let expected = artifact
            .join("sumeragi-release")
            .join(&source)
            .join("programs");
        ensure!(
            target.parent().map(Path::as_os_str) == Some(expected.as_os_str()),
            "{TARGET} must name an immediate private invocation bundle under {}",
            expected.display()
        );
        let valid_name = target
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("invocation."))
            .is_some_and(|suffix| {
                suffix.len() == 6 && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
            });
        ensure!(
            valid_name,
            "{TARGET} invocation name requires exactly six ASCII alphanumeric characters"
        );
        Ok(Some(Self {
            source,
            manifest,
            lock,
            artifact,
            target,
        }))
    }
}

/// Original roots and independent lockfile held through full bundle admission and selection.
#[derive(Debug)]
pub struct ReleaseProgramContract {
    bundle: Arc<Bundle>,
    source: ReaderDirectory,
    artifact: PrivateDirectory,
    parents: Vec<ReaderDirectory>,
    private_parents: Vec<PrivateDirectory>,
    lock: RetainedFile,
    lock_snapshot: FileSnapshot,
}

impl ReleaseProgramContract {
    fn open(repo: &Path, inputs: ReleaseInputs) -> Result<Arc<Self>> {
        let source = ReaderDirectory::open(repo).wrap_err("admit original release source root")?;
        let artifact = PrivateDirectory::open_exact(&inputs.artifact)
            .wrap_err("admit exact private release artifact root")?;
        let artifact_reader = ReaderDirectory::open(&inputs.artifact)?;
        ensure!(
            artifact.identity()? == artifact_reader.identity()?,
            "release artifact root original changed"
        );
        let mut parents = Vec::new();
        let mut source_ids = Vec::new();
        let mut artifact_ids = Vec::new();
        for (path, identities) in [
            (source.path(), &mut source_ids),
            (artifact.path(), &mut artifact_ids),
        ] {
            for ancestor in path.ancestors() {
                let original = ReaderDirectory::open(ancestor)?;
                identities.push(original.identity()?);
                parents.push(original);
            }
        }
        ensure!(
            !artifact_ids.contains(&source.identity()?)
                && !source_ids.contains(&artifact.identity()?),
            "{ARTIFACT} must not overlap repository source through native directory identities"
        );
        let release = artifact_reader.open_child("sumeragi-release")?;
        let source_programs = release.open_child(&inputs.source)?;
        let programs = source_programs.open_child("programs")?;
        let mut private_parents = Vec::new();
        for parent in [&release, &source_programs, &programs] {
            let private = PrivateDirectory::open_exact(parent.path())?;
            ensure!(
                private.identity()? == parent.identity()?,
                "source-bound release parent original changed"
            );
            private_parents.push(private);
        }
        let mut lock = source.open_retained_regular("Cargo.lock")?;
        let lock_snapshot = lock.snapshot()?;
        let (digest, _) = sha256_reader_bounded(lock.file_mut(), MAX_LOCK)?;
        ensure!(
            lock.snapshot()? == lock_snapshot,
            "workspace Cargo.lock changed while independently hashed"
        );
        let lock_digest = lowercase_hex(&digest);
        if let Some(expected) = inputs.lock {
            ensure!(
                expected == lock_digest,
                "workspace Cargo.lock differs from inherited {LOCK}"
            );
        }
        let bundle = Bundle::open(
            &programs,
            inputs
                .target
                .file_name()
                .ok_or_else(|| eyre!("release target lacks invocation name"))?,
            BundleAnchors {
                source: &inputs.source,
                cargo_lock: &lock_digest,
                manifest: &inputs.manifest,
            },
        )?;
        ensure!(
            bundle.path().as_os_str() == inputs.target.as_os_str(),
            "release bundle path differs from inherited {TARGET}"
        );
        parents.extend([artifact_reader, release, source_programs, programs]);
        let contract = Self {
            bundle,
            source,
            artifact,
            parents,
            private_parents,
            lock,
            lock_snapshot,
        };
        contract.revalidate()?;
        Ok(Arc::new(contract))
    }

    fn revalidate(&self) -> Result<()> {
        self.source.revalidate()?;
        self.artifact.revalidate()?;
        for parent in &self.parents {
            parent.revalidate()?;
        }
        for parent in &self.private_parents {
            parent.revalidate()?;
        }
        self.lock.revalidate()?;
        ensure!(
            self.lock.snapshot()? == self.lock_snapshot,
            "release source Cargo.lock original changed"
        );
        self.bundle.revalidate()?;
        Ok(())
    }

    pub(super) fn select(
        self: &Arc<Self>,
        kind: BinaryKind,
        candidate: Option<&Path>,
    ) -> Result<ResolvedProgram> {
        self.revalidate()?;
        let custody: Arc<dyn AdmissionCustody> = self.clone();
        let program = self.bundle.program(kind).with_admission_custody(custody)?;
        if let Some(candidate) = candidate {
            ensure!(
                candidate.as_os_str() == program.diagnostic_path()?.as_os_str(),
                "release binary must select its exact manifest-attested native name"
            );
        }
        self.revalidate()?;
        Ok(ResolvedProgram::retained(program))
    }
}

pub fn release_program_contract(repo: &Path) -> Result<Option<Arc<ReleaseProgramContract>>> {
    ReleaseInputs::environment()?
        .map(|inputs| ReleaseProgramContract::open(repo, inputs))
        .transpose()
}

impl AdmissionCustody for ReleaseProgramContract {
    fn revalidate(&self) -> io::Result<()> {
        ReleaseProgramContract::revalidate(self).map_err(|error| {
            io::Error::other(format!("release resolution custody changed: {error:#}"))
        })
    }
}

#[cfg(test)]
#[path = "release_contract_tests.rs"]
mod tests;

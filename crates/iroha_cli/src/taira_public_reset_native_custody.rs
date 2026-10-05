//! Explicit OS-operator initialization of the independently fixed native Mac custodian.
//!
//! The actual account and running image select custody. Caller-selected public
//! digests establish the bootstrap trust decision; no incoming reset inventory,
//! private signing key, service configuration or network response can seed it.

use super::super::super::{TRUSTED_KEY_SCHEMA_V1, TrustedKeyV1};
use super::super::{HOST_GUARD_SCHEMA_V1, HostGuardV1, upload_parent};
use super::*;
use crate::taira_public_reset::validate_absolute_normal_path;
use iroha_crypto::{Algorithm, PublicKey};
use std::{ffi::OsStr, io, str::FromStr as _};

const MAX_IMAGE: u64 = 512 * 1024 * 1024;
const MAX_KEY: usize = 16 * 1024;
const ROOT_NAME: &str = "public-reset-v1";
const STAGE_PREFIX: &str = ".native-edge-custody-stage-";
const TRUSTED_NAME: &str = "trusted-public-key.json";

/// Bootstrap fixed host trust from the actual running native executable and an explicit public key.
#[derive(clap::Args, Debug)]
pub(in crate::taira_public_reset) struct InitializeNativeEdgeCustody {
    /// Independently selected SHA-256 of the actual currently invoked Darwin image.
    #[arg(long, value_name = "SHA256")]
    expected_executable_sha256: String,
    /// Independently selected public TrustedKeyV1 record, retained in owner-private custody.
    #[arg(long, value_name = "PATH")]
    trusted_public_key: PathBuf,
    /// Independent SHA-256 of the exact public authority record bytes.
    #[arg(long, value_name = "SHA256")]
    expected_trusted_public_key_sha256: String,
}

#[derive(Debug, JsonSerialize)]
struct InitializationReceiptV1 {
    schema: String,
    outcome: String,
    qualified: bool,
    compiled_source_commit: String,
    owner_uid: u32,
    owner_gid: u32,
    owner_user: String,
    owner_home: String,
    custody_root: String,
    dispatcher: NativePublicFileV1,
    guard: NativePublicFileV1,
    trusted_public_key: NativePublicFileV1,
}

#[cfg(unix)]
struct NativeAccount {
    uid: u32,
    gid: u32,
    user: String,
    home: PathBuf,
}

#[cfg(unix)]
impl NativeAccount {
    fn custody(&self) -> PathBuf {
        self.home.join(".local/share/iroha/taira").join(ROOT_NAME)
    }

    fn guard(&self, image_sha256: &str, key_sha256: &str) -> HostGuardV1 {
        let service = self.home.join(".local/share/iroha/taira/edge");
        let service_root = service.to_string_lossy().into_owned();
        HostGuardV1 {
            schema: HOST_GUARD_SCHEMA_V1.into(),
            host_slug: "taira-edge".into(),
            state_root: service.join("state").to_string_lossy().into_owned(),
            upload_parent: upload_parent(&service_root),
            service_root,
            trusted_key_sha256: key_sha256.into(),
            dispatcher_path: self
                .custody()
                .join("dispatcher/iroha")
                .to_string_lossy()
                .into_owned(),
            dispatcher_sha256: image_sha256.into(),
        }
    }
}

#[cfg(unix)]
fn require_directory(directory: &PrivateDirectory, account: &NativeAccount) -> Result<()> {
    directory.revalidate()?;
    let metadata = fs::symlink_metadata(directory.path())?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != account.uid
        || metadata.gid() != account.gid
        || metadata.mode() & 0o7777 != 0o700
    {
        return Err(eyre!(
            "native custodian directory differs from exact private account custody"
        ));
    }
    directory.revalidate()?;
    Ok(())
}

#[cfg(unix)]
fn require_names(directory: &PrivateDirectory, names: &[&str]) -> Result<()> {
    let mut expected: Vec<_> = names
        .iter()
        .map(|name| OsStr::new(name).to_owned())
        .collect();
    expected.sort();
    if directory.entries(names.len() + 1)? != expected {
        return Err(eyre!(
            "native custodian contains unknown or incomplete initialization state"
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn require_file(pin: &PublicPin, account: &NativeAccount, mode: u16) -> Result<()> {
    let identity = &pin.reference.file.identity;
    if identity.uid != account.uid
        || identity.gid != account.gid
        || identity.mode != mode
        || identity.links != 1
    {
        return Err(eyre!(
            "native custodian file differs from exact account custody"
        ));
    }
    pin.revalidate()
}

#[cfg(unix)]
fn rehash(pin: &mut PublicPin) -> Result<()> {
    pin.revalidate()?;
    pin.retained.file_mut().rewind()?;
    let digest = hash_reader(
        &mut pin
            .retained
            .file_mut()
            .take(pin.reference.file.identity.size + 1),
    )?;
    pin.retained.file_mut().rewind()?;
    if digest != pin.reference.sha256 {
        return Err(eyre!("native bootstrap public input content changed"));
    }
    pin.revalidate()
}

#[cfg(unix)]
fn copy_dispatcher_new(
    directory: &PrivateDirectory,
    account: &NativeAccount,
    image: &mut PublicPin,
) -> Result<(File, NativePublicFileV1)> {
    require_directory(directory, account)?;
    let path = directory.path().join("iroha");
    let mut copied = RetainedFile::create_new_private(&path)?;
    image.retained.file_mut().rewind()?;
    let copied_len = io::copy(
        &mut image
            .retained
            .file_mut()
            .take(image.reference.file.identity.size + 1),
        copied.file_mut(),
    )?;
    if copied_len != image.reference.file.identity.size {
        return Err(eyre!("native bootstrap image extent changed during copy"));
    }
    copied.file().sync_all()?;
    copied.revalidate()?;
    image.revalidate()?;
    // Keep the create-only inode handle through permission finalization and root publication.
    // The private creator is released only after its original descriptor is duplicated.
    let created = copied.file().try_clone()?;
    created.set_permissions(fs::Permissions::from_mode(0o755))?;
    created.sync_all()?;
    let identity = protocol::native_file_identity(&created.metadata()?)?;
    drop(copied);
    let mut observed = PublicPin::open(&path, MAX_IMAGE, false)?;
    require_file(&observed, account, 0o755)?;
    rehash(&mut observed)?;
    if observed.reference.file.identity != identity
        || observed.reference.sha256 != image.reference.sha256
    {
        return Err(eyre!("native bootstrap created dispatcher lineage changed"));
    }
    require_directory(directory, account)?;
    Ok((created, observed.reference))
}

#[cfg(unix)]
struct ObservedCustody {
    dispatcher_directory: PrivateDirectory,
    edge_directory: PrivateDirectory,
    dispatcher: PublicPin,
    guard: PublicPin,
    key: PublicPin,
}

#[cfg(unix)]
impl ObservedCustody {
    fn revalidate(&mut self, root: &PrivateDirectory, account: &NativeAccount) -> Result<()> {
        require_directory(root, account)?;
        require_directory(&self.dispatcher_directory, account)?;
        require_directory(&self.edge_directory, account)?;
        require_file(&self.dispatcher, account, 0o755)?;
        require_file(&self.guard, account, 0o600)?;
        require_file(&self.key, account, 0o600)?;
        for pin in [&mut self.dispatcher, &mut self.guard, &mut self.key] {
            rehash(pin)?;
        }
        Ok(())
    }

    fn require_fresh_layout(&self, root: &PrivateDirectory) -> Result<()> {
        require_names(root, &["dispatcher", "taira-edge"])?;
        require_names(&self.dispatcher_directory, &["iroha"])?;
        require_names(&self.edge_directory, &["guard.json", TRUSTED_NAME])
    }

    fn receipt(
        &self,
        account: &NativeAccount,
        commit: &str,
        outcome: &str,
    ) -> InitializationReceiptV1 {
        InitializationReceiptV1 {
            schema: "iroha.taira.public-reset.native-edge-custody-initialized.v1".into(),
            outcome: outcome.into(),
            qualified: false,
            compiled_source_commit: commit.into(),
            owner_uid: account.uid,
            owner_gid: account.gid,
            owner_user: account.user.clone(),
            owner_home: account.home.to_string_lossy().into_owned(),
            custody_root: account.custody().to_string_lossy().into_owned(),
            dispatcher: self.dispatcher.reference.clone(),
            guard: self.guard.reference.clone(),
            trusted_public_key: self.key.reference.clone(),
        }
    }
}

#[cfg(unix)]
fn observe_custody(
    root: &PrivateDirectory,
    account: &NativeAccount,
    image_sha256: &str,
    key_sha256: &str,
    guard_body: &[u8],
    key_body: &[u8],
) -> Result<ObservedCustody> {
    require_directory(root, account)?;
    let dispatcher_directory = root.open_child("dispatcher")?;
    let edge_directory = root.open_child("taira-edge")?;
    require_directory(&dispatcher_directory, account)?;
    require_directory(&edge_directory, account)?;
    let dispatcher = PublicPin::open(&dispatcher_directory.path().join("iroha"), MAX_IMAGE, false)?;
    let mut guard = PublicPin::open(
        &edge_directory.path().join("guard.json"),
        MAX_KEY as u64,
        true,
    )?;
    let mut key = PublicPin::open(
        &edge_directory.path().join(TRUSTED_NAME),
        MAX_KEY as u64,
        true,
    )?;
    if dispatcher.reference.sha256 != image_sha256 || key.reference.sha256 != key_sha256 {
        return Err(eyre!(
            "initialized native custodian differs from the independent bootstrap digests"
        ));
    }
    let guard_bytes = guard.bytes(MAX_KEY)?;
    let _: HostGuardV1 = json::from_slice(&guard_bytes)
        .map_err(|_| eyre!("initialized native custodian guard is invalid"))?;
    if guard_bytes != guard_body || key.bytes(MAX_KEY)? != key_body {
        return Err(eyre!(
            "initialized native custodian differs from the exact canonical authority"
        ));
    }
    let mut observed = ObservedCustody {
        dispatcher_directory,
        edge_directory,
        dispatcher,
        guard,
        key,
    };
    observed.revalidate(root, account)?;
    Ok(observed)
}

/// Initialize only from already admitted native account, executable and public authority.
/// Test fixtures may exercise this publication owner without claiming actual dispatcher authority.
#[cfg(unix)]
fn initialize_bound(
    account: &NativeAccount,
    image_path: &Path,
    commit: &str,
    args: &InitializeNativeEdgeCustody,
    before_publish: &mut dyn FnMut(&Path) -> Result<()>,
) -> Result<InitializationReceiptV1> {
    validate_lower_hex(
        "native bootstrap executable digest",
        &args.expected_executable_sha256,
        64,
    )?;
    validate_lower_hex(
        "native bootstrap public key digest",
        &args.expected_trusted_public_key_sha256,
        64,
    )?;
    validate_absolute_normal_path(
        &args.trusted_public_key,
        "native bootstrap public authority path",
    )?;
    let mut image = PublicPin::open(image_path, MAX_IMAGE, false)?;
    let source_identity = &image.reference.file.identity;
    if source_identity.uid != account.uid
        || source_identity.gid != account.gid
        || source_identity.links != 1
        || source_identity.mode & 0o111 == 0
        || source_identity.mode & 0o022 != 0
        || image.reference.sha256 != args.expected_executable_sha256
    {
        return Err(eyre!(
            "actual native image differs from independently selected executable custody or digest"
        ));
    }
    iroha_deploy::managed::admit_native_program(&mut image.retained)?;
    image.revalidate()?;
    let mut key = PublicPin::open(&args.trusted_public_key, MAX_KEY as u64, true)?;
    require_file(&key, account, 0o600)?;
    if key.reference.sha256 != args.expected_trusted_public_key_sha256 {
        return Err(eyre!(
            "native bootstrap public key differs from its independent digest"
        ));
    }
    let key_body = key.bytes(MAX_KEY)?;
    let trusted: TrustedKeyV1 = json::from_slice(&key_body)
        .map_err(|_| eyre!("native bootstrap public authority record is invalid"))?;
    let public = PublicKey::from_str(&trusted.public_key)
        .map_err(|_| eyre!("native bootstrap public authority key is invalid"))?;
    if trusted.schema != TRUSTED_KEY_SCHEMA_V1
        || trusted.algorithm != "ed25519"
        || public.try_algorithm()? != Algorithm::Ed25519
        || public.to_string() != trusted.public_key
    {
        return Err(eyre!(
            "native bootstrap requires an independently selected canonical Ed25519 authority"
        ));
    }
    let guard_body =
        json::to_json(&account.guard(&image.reference.sha256, &key.reference.sha256))?.into_bytes();
    let custody_path = account.custody();
    let parent_path = custody_path
        .parent()
        .ok_or_else(|| eyre!("fixed native custody has no parent"))?;
    rehash(&mut image)?;
    rehash(&mut key)?;
    let parent = PrivateDirectory::open_or_create(parent_path)?;
    require_directory(&parent, account)?;
    match fs::symlink_metadata(&custody_path) {
        Ok(_) => {
            let root = parent.open_child(ROOT_NAME)?;
            let mut observed = observe_custody(
                &root,
                account,
                &image.reference.sha256,
                &key.reference.sha256,
                &guard_body,
                &key_body,
            )?;
            rehash(&mut image)?;
            rehash(&mut key)?;
            // Replay inspects only the three independent bootstrap anchors.
            // Extra operational children receive no interpretation or authority.
            observed.revalidate(&root, account)?;
            parent.revalidate()?;
            return Ok(observed.receipt(account, commit, "already_initialized"));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if parent
        .entries(4096)?
        .iter()
        .any(|name| name.to_string_lossy().starts_with(STAGE_PREFIX))
    {
        return Err(eyre!(
            "native bootstrap has unpublished custody requiring explicit reconciliation"
        ));
    }
    let stage = parent.create_child(format!(
        "{STAGE_PREFIX}{}",
        hex::encode(rand::random::<[u8; 16]>())
    ))?;
    let dispatcher_directory = stage.create_child("dispatcher")?;
    let edge_directory = stage.create_child("taira-edge")?;
    let (created_dispatcher, dispatcher_reference) =
        copy_dispatcher_new(&dispatcher_directory, account, &mut image)?;
    edge_directory.write_atomic("guard.json", &guard_body, PublishMode::CreateNew)?;
    edge_directory.write_atomic(TRUSTED_NAME, &key_body, PublishMode::CreateNew)?;
    dispatcher_directory.sync()?;
    edge_directory.sync()?;
    drop(dispatcher_directory);
    drop(edge_directory);
    let mut observed = observe_custody(
        &stage,
        account,
        &image.reference.sha256,
        &key.reference.sha256,
        &guard_body,
        &key_body,
    )?;
    let dispatcher_identity = observed.dispatcher_directory.identity()?;
    let edge_identity = observed.edge_directory.identity()?;
    if observed.dispatcher.reference != dispatcher_reference {
        return Err(eyre!("native bootstrap staged dispatcher lineage changed"));
    }
    observed.require_fresh_layout(&stage)?;
    before_publish(stage.path())?;
    observed.revalidate(&stage, account)?;
    observed.require_fresh_layout(&stage)?;
    if observed.dispatcher_directory.identity()? != dispatcher_identity
        || observed.edge_directory.identity()? != edge_identity
    {
        return Err(eyre!("native bootstrap staged directory lineage changed"));
    }
    // The created public file FDs remain open across the root rename. Their full
    // snapshots must match the new canonical paths, not merely equivalent bytes.
    let mut created = vec![(created_dispatcher, dispatcher_reference)];
    created.extend(
        [&observed.guard, &observed.key]
            .into_iter()
            .map(|pin| Ok((pin.retained.file().try_clone()?, pin.reference.clone())))
            .collect::<Result<Vec<_>>>()?,
    );
    drop(observed);
    stage.sync()?;
    rehash(&mut image)?;
    rehash(&mut key)?;
    parent.revalidate()?;
    let published = stage.rename_to_sibling(ROOT_NAME, PublishMode::CreateNew)?;
    let mut observed = observe_custody(
        &published,
        account,
        &image.reference.sha256,
        &key.reference.sha256,
        &guard_body,
        &key_body,
    )?;
    observed.require_fresh_layout(&published)?;
    if observed.dispatcher_directory.identity()? != dispatcher_identity
        || observed.edge_directory.identity()? != edge_identity
    {
        return Err(eyre!(
            "native bootstrap publication changed its created directory lineage"
        ));
    }
    for ((opened, expected), actual) in
        created
            .iter()
            .zip([&observed.dispatcher, &observed.guard, &observed.key])
    {
        if protocol::native_file_identity(&opened.metadata()?)? != expected.file.identity
            || actual.reference.file.identity != expected.file.identity
            || actual.reference.sha256 != expected.sha256
        {
            return Err(eyre!(
                "native bootstrap publication changed its created file lineage"
            ));
        }
    }
    rehash(&mut image)?;
    rehash(&mut key)?;
    observed.revalidate(&published, account)?;
    parent.revalidate()?;
    Ok(observed.receipt(account, commit, "initialized"))
}

pub(in crate::taira_public_reset) fn initialize_native_edge_custody(
    args: &InitializeNativeEdgeCustody,
    output: &mut impl Write,
) -> Result<()> {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Err(eyre!(
            "native custodian initialization requires Darwin/AArch64"
        ));
    }
    #[cfg(unix)]
    {
        // Resolve independent OS authority before any caller-selected input path.
        let (uid, gid, user, home) = native_account()?;
        let commit = crate::compiled_build_identity()?.release_source_commit()?;
        let image = std::env::current_exe()?;
        let account = NativeAccount {
            uid,
            gid,
            user,
            home: home.into(),
        };
        let receipt = initialize_bound(&account, &image, commit, args, &mut |_| Ok(()))?;
        writeln!(output, "{}", json::to_json(&receipt)?)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (args, output);
        Err(eyre!(
            "native custodian initialization requires Unix file custody"
        ))
    }
}

#[cfg(test)]
#[path = "taira_public_reset_native_custody_tests.rs"]
mod tests;

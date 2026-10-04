//! Fixed existing Android encrypted-wallet intake. This imports no runtime root or current grant.
//! The same existing S key is retained only in the actual immutable AccountClient; no new vault,
//! key generation, account registration or activation/recovery mutation occurs.
use super::*;

#[cfg(any(target_os = "android", all(test, unix)))]
const RESOURCE_FILE: &str = "existing-account-resource-allocation.norito";
#[cfg(any(target_os = "android", all(test, unix)))]
const RESOURCE_MAX: usize = 4096;
#[cfg(any(target_os = "android", all(test, unix)))]
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::ExistingAndroidAccountResourcesV1")]
struct Resources {
    version: u16,
    signatory_original: Vec<u8>,
    network: [u8; 32],
    capacity: KagemushaDurableCapacityV1,
}
fn resource_capacity(available: u64) -> Result<KagemushaDurableCapacityV1, Error> {
    // Assign half the currently available filesystem to finite byte-accounting pools; leave
    // half unassigned for other journals/app data. This is a local resource budget, not a
    // monetary grant or a promise that the filesystem cannot run out between future fsyncs.
    let capacity = KagemushaDurableCapacityV1 {
        inbox_bytes: available / 4,
        outbox_bytes: available / 4,
    };
    capacity.validate().map_err(|_| Error::Rejected)?;
    Ok(capacity)
}

/// Claim the sole actual original before receiver authentication can enter JNI. This private
/// ordering helper grants no account/source authority; a refused receiver still consumes it.
#[cfg(any(target_os = "android", test))]
fn claim_existing_account_before_receiver(
    claim: impl FnOnce() -> Result<NativeCompositionOriginal, Error>,
    authenticate_receiver: impl FnOnce() -> Result<(), Error>,
) -> Result<NativeCompositionOriginal, Error> {
    let composition = claim()?;
    composition.require_current()?;
    authenticate_receiver()?;
    composition.require_current()?;
    Ok(composition)
}

/// Retire every incomplete actual loan before its thread-local result is drained. The fixed
/// production closure denies only the same registered original and performs no source/key I/O.
#[cfg(any(target_os = "android", test))]
fn retire_incomplete_existing_account_loan(
    complete: bool,
    retire_original: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    if complete { Ok(()) } else { retire_original() }
}

#[cfg(any(target_os = "android", all(test, unix)))]
mod android {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
    use iroha_data_model::account::AccountAddress;
    use jni::{
        JNIEnv,
        objects::{GlobalRef, JByteArray, JObject, JString, JValue},
    };
    use std::{
        fs::{File, OpenOptions},
        os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt},
    };
    use zeroize::Zeroizing;

    fn directory(path: &Path, private: bool) -> Result<File, Error> {
        if !path.is_absolute() || path.canonicalize().map_err(|_| Error::Rejected)? != path {
            return Err(Error::Rejected);
        }
        let mut prefix = PathBuf::new();
        for part in path.components() {
            prefix.push(part);
            let meta = std::fs::symlink_metadata(&prefix).map_err(|_| Error::Rejected)?;
            if meta.file_type().is_symlink() {
                return Err(Error::Rejected);
            }
        }
        let fd = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_DIRECTORY)
            .open(path)
            .map_err(|_| Error::Rejected)?;
        let meta = fd.metadata().map_err(|_| Error::Rejected)?;
        if !meta.is_dir()
            || meta.uid() != unsafe { libc::geteuid() }
            || meta.mode() & (if private { 0o077 } else { 0o022 }) != 0
        {
            return Err(Error::Rejected);
        }
        Ok(fd)
    }
    fn named_same(directory: &File, path: &Path) -> Result<(), Error> {
        let held = directory.metadata().map_err(|_| Error::Rejected)?;
        let named = std::fs::symlink_metadata(path).map_err(|_| Error::Rejected)?;
        if !named.is_dir()
            || named.file_type().is_symlink()
            || held.dev() != named.dev()
            || held.ino() != named.ino()
            || held.uid() != named.uid()
            || held.mode() != named.mode()
        {
            return Err(Error::Rejected);
        }
        Ok(())
    }
    fn local_storage(
        env: &mut JNIEnv<'_>,
        application: &JObject<'_>,
    ) -> Result<(PathBuf, File), Error> {
        let file = env
            .call_method(application, "getNoBackupFilesDir", "()Ljava/io/File;", &[])
            .and_then(|v| v.l())
            .map_err(|_| Error::Rejected)?;
        let raw = env
            .call_method(&file, "getCanonicalPath", "()Ljava/lang/String;", &[])
            .and_then(|v| v.l())
            .map_err(|_| Error::Rejected)?;
        let raw = JString::from(raw);
        let root = PathBuf::from(String::from(
            env.get_string(&raw).map_err(|_| Error::Rejected)?,
        ));
        open_local_storage(&root)
    }

    /// Open the single protected KAGEMUSHA core directory under Android's no-backup root.
    fn open_local_storage(root: &Path) -> Result<(PathBuf, File), Error> {
        let parent = directory(root, false)?;
        let path = root.join("kagemusha-core-v1");
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                use std::os::unix::fs::DirBuilderExt;
                std::fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&path)
                    .map_err(|_| Error::Rejected)?;
                parent.sync_all().map_err(|_| Error::Rejected)?;
            }
            Ok(_) => (),
            Err(_) => return Err(Error::Rejected),
        }
        named_same(&parent, root)?;
        let held = directory(&path, true)?;
        Ok((path, held))
    }
    fn available(directory: &File) -> Result<u64, Error> {
        use std::os::fd::AsRawFd;
        let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        if unsafe { libc::fstatvfs(directory.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
            return Err(Error::Rejected);
        }
        let stat = unsafe { stat.assume_init() };
        u64::try_from(stat.f_bavail)
            .ok()
            .and_then(|n| n.checked_mul(u64::try_from(stat.f_frsize).ok()?))
            .ok_or(Error::Rejected)
    }
    fn resources(
        path: &Path,
        held_directory: &File,
        signatory: &AccountId,
        network: [u8; 32],
    ) -> Result<KagemushaDurableCapacityV1, Error> {
        named_same(held_directory, path)?;
        let original = path.join(RESOURCE_FILE);
        let wanted = norito::encode_canonical(signatory).map_err(|_| Error::Rejected)?;
        let flags = libc::O_NOFOLLOW | libc::O_CLOEXEC;
        let fd = match OpenOptions::new()
            .read(true)
            .custom_flags(flags)
            .open(&original)
        {
            Ok(fd) => fd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // There is no compatibility repair: missing allocation beside any existing
                // Native material is uncertain custody, never permission to allocate afresh.
                if std::fs::read_dir(path)
                    .map_err(|_| Error::Rejected)?
                    .next()
                    .is_some()
                {
                    return Err(Error::Rejected);
                }
                let data = Resources {
                    version: 1,
                    signatory_original: wanted.clone(),
                    network,
                    capacity: resource_capacity(available(held_directory)?)?,
                };
                let bytes = norito::encode_canonical(&data).map_err(|_| Error::Rejected)?;
                if bytes.len() > RESOURCE_MAX {
                    return Err(Error::Rejected);
                }
                // create_new is the only writer. An uncertain create/write never truncates or
                // repairs a retained allocation. All account/journal originals remain intact.
                let fd = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .custom_flags(flags)
                    .open(&original)
                    .map_err(|_| Error::Rejected)?;
                fd.write_all_at(&bytes, 0).map_err(|_| Error::Rejected)?;
                fd.sync_all().map_err(|_| Error::Rejected)?;
                held_directory.sync_all().map_err(|_| Error::Rejected)?;
                fd
            }
            Err(_) => return Err(Error::Rejected),
        };
        let meta = fd.metadata().map_err(|_| Error::Rejected)?;
        let named = std::fs::symlink_metadata(&original).map_err(|_| Error::Rejected)?;
        if !meta.is_file()
            || named.file_type().is_symlink()
            || meta.nlink() != 1
            || meta.uid() != unsafe { libc::geteuid() }
            || meta.mode() & 0o077 != 0
            || meta.len() == 0
            || meta.len() > RESOURCE_MAX as u64
            || meta.dev() != named.dev()
            || meta.ino() != named.ino()
            || meta.len() != named.len()
        {
            return Err(Error::Rejected);
        }
        let mut bytes = vec![0; meta.len() as usize];
        fd.read_exact_at(&mut bytes, 0)
            .map_err(|_| Error::Rejected)?;
        let data: Resources = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(RESOURCE_MAX),
        )
        .map_err(|_| Error::Rejected)?;
        if norito::encode_canonical(&data).map_err(|_| Error::Rejected)? != bytes
            || data.version != 1
            || data.signatory_original != wanted
            || data.network != network
        {
            return Err(Error::Rejected);
        }
        data.capacity.validate().map_err(|_| Error::Rejected)?;
        let after = fd.metadata().map_err(|_| Error::Rejected)?;
        let after_named = std::fs::symlink_metadata(&original).map_err(|_| Error::Rejected)?;
        if after.dev() != meta.dev()
            || after.ino() != meta.ino()
            || after.len() != meta.len()
            || after.mtime() != meta.mtime()
            || after.mtime_nsec() != meta.mtime_nsec()
            || after.ctime() != meta.ctime()
            || after.ctime_nsec() != meta.ctime_nsec()
            || after_named.dev() != meta.dev()
            || after_named.ino() != meta.ino()
        {
            return Err(Error::Rejected);
        }
        named_same(held_directory, path)?;
        Ok(data.capacity)
    }

    pub(super) fn intake<'local>(
        composition: NativeCompositionOriginal,
        env: &mut JNIEnv<'local>,
        application: &JObject<'local>,
        class: &jni::objects::JClass<'local>,
        original_signatory: &str,
        seed: &[u8],
        context: Arc<InstalledContext>,
    ) -> Result<
        (
            [Vec<u8>; 3],
            Arc<KagemushaNativeOrdinaryRuntimeStartupV1>,
            u64,
        ),
        Error,
    > {
        composition.require_current()?;
        if seed.len() != 32
            || original_signatory.len() > 512
            || original_signatory.is_empty()
            || original_signatory.trim() != original_signatory
        {
            return Err(Error::Rejected);
        }
        let discriminant =
            AccountAddress::i105_discriminant(original_signatory).map_err(|_| Error::Rejected)?;
        let address =
            AccountAddress::from_i105_for_discriminant(original_signatory, Some(discriminant))
                .map_err(|_| Error::Rejected)?;
        let key = KeyPair::from_private_key(
            PrivateKey::from_bytes(Algorithm::Ed25519, seed).map_err(|_| Error::Rejected)?,
        )
        .map_err(|_| Error::Rejected)?;
        let signatory = AccountId::new(key.public_key().clone());
        if signatory
            .to_account_address()
            .map_err(|_| Error::Rejected)?
            != address
        {
            return Err(Error::Rejected);
        }
        // Root/package/source/measurement, network, endpoints and released key profile originate
        // solely in this existing measured Native loader. No selector is supplied by managed code.
        context.require_jni_owner(env, class)?;
        composition.require_current()?;
        let inventory = context.inventory()?;
        composition.require_current()?;
        let account = inventory
            .account_client_for_existing_signatory(&signatory, key, discriminant)
            .map_err(|_| Error::Rejected)?;
        let wallet = account
            .authority()
            .to_account_address()
            .map_err(|_| Error::Rejected)?
            .to_i105_for_discriminant(discriminant)
            .map_err(|_| Error::Rejected)?;
        composition.require_current()?;
        context.require_jni_owner(env, class)?;
        let (storage, directory) = local_storage(env, application)?;
        composition.require_current()?;
        let capacity = resources(
            &storage,
            &directory,
            &signatory,
            *account.network_id().as_bytes(),
        )?;
        composition.require_current()?;
        context.require_jni_owner(env, class)?;
        let clock_disposition =
            crate::kagemusha_core_coordinator_v1::ordinary_app_identity::native_existing_account_journal_disposition(
                &storage.join("native-clock/ordinary-native-clock.norito.wal"),
            )?;
        let startup =
            KagemushaNativeOrdinaryRuntimeStartupV1::from_protected_android_storage_and_native_account(
                composition,
                Arc::clone(&context),
                storage.clone(),
                account,
                clock_disposition,
                Disposition::Fresh,
                Disposition::Fresh,
                Disposition::Fresh,
                Disposition::Fresh,
                vec![],
                vec![],
                capacity,
                vec![],
            )?;
        let retirement_generation = startup.retirement.capture_original()?;
        // This registration is not selection. The measured product's complete checked loan
        // must return successfully before its Native-created continuation can yield a receipt.
        startup.retirement.require_original(retirement_generation)?;
        let checked = startup
            .inventory
            .recheck()
            .map_err(|_| Error::Rejected)
            .and_then(|()| context.require_jni_owner(env, class))
            .and_then(|()| named_same(&directory, &storage));
        if checked.is_err() {
            retire(&startup);
        }
        checked?;
        startup.retirement.require_original(retirement_generation)?;
        Ok((
            [
                1_u16.to_le_bytes().to_vec(),
                original_signatory.as_bytes().to_vec(),
                wallet.into_bytes(),
            ],
            startup,
            retirement_generation,
        ))
    }

    fn retire(startup: &KagemushaNativeOrdinaryRuntimeStartupV1) {
        // Preserve the permanent existing startup retirement generation fence.
        let _ = startup.retirement.retire();
        if let Ok(mut owner) = startup.owner.lock() {
            owner.installation_failed = true;
        }
        let _ = startup.registry.revoke_selection();
        if let Ok(mut active) = startup.active.lock() {
            *active = None;
        }
    }
    struct PendingLoan {
        composition: Option<NativeCompositionOriginal>,
        context: Arc<InstalledContext>,
        application: GlobalRef,
        continuation: GlobalRef,
        used: bool,
        completed: Option<(
            [Vec<u8>; 3],
            Arc<KagemushaNativeOrdinaryRuntimeStartupV1>,
            u64,
        )>,
    }
    thread_local! {
        static PENDING: std::cell::RefCell<Option<PendingLoan>> = const { std::cell::RefCell::new(None) };
    }
    struct ActiveLoan {
        complete: bool,
    }
    impl Drop for ActiveLoan {
        fn drop(&mut self) {
            // A lost/unwound intake result can follow successful STARTUP.set but precede
            // completed-tuple retention. Always deny that same original before any drain.
            let _ = retire_incomplete_existing_account_loan(
                self.complete,
                retire_pending_native_composition,
            );
            PENDING.with(|pending| {
                if let Some(mut original) = pending.borrow_mut().take()
                    && let Some((_, startup, _)) = original.completed.take()
                {
                    // Includes a failed/changed activation postcheck, panic and JNI exception.
                    if !self.complete {
                        retire(&startup);
                    }
                }
            });
        }
    }
    fn consume<'local>(
        env: &mut JNIEnv<'local>,
        class: &jni::objects::JClass<'local>,
        continuation: &JObject<'local>,
        signatory: &JString<'local>,
        seed_array: &JByteArray<'local>,
    ) -> Result<(), Error> {
        // No source/key selector can create this scope: it exists only during the outer Native
        // call to the measured manifest Application's actual protected-storage loan method.
        let (composition, context, application) = PENDING.with(|pending| -> Result<_, Error> {
            let mut pending = pending.borrow_mut();
            let original = pending.as_mut().ok_or(Error::Rejected)?;
            if original.used
                || !env
                    .is_same_object(continuation, original.continuation.as_obj())
                    .map_err(|_| Error::Rejected)?
            {
                return Err(Error::Rejected);
            }
            let composition = original.composition.take().ok_or(Error::Rejected)?;
            composition.require_current()?;
            original.used = true;
            Ok((
                composition,
                Arc::clone(&original.context),
                original.application.clone(),
            ))
        })?;
        composition.require_current()?;
        context.require_jni_owner(env, class)?;
        composition.require_current()?;
        if env
            .get_array_length(seed_array)
            .map_err(|_| Error::Rejected)?
            != 32
        {
            return Err(Error::Rejected);
        }
        composition.require_current()?;
        let seed = Zeroizing::new(
            env.convert_byte_array(seed_array)
                .map_err(|_| Error::Rejected)?,
        );
        env.set_byte_array_region(seed_array, 0, &[0_i8; 32])
            .map_err(|_| Error::Rejected)?;
        if !(1..=512).contains(
            &env.call_method(signatory, "length", "()I", &[])
                .and_then(|v| v.i())
                .map_err(|_| Error::Rejected)?,
        ) {
            return Err(Error::Rejected);
        }
        let signatory: String = env
            .get_string(signatory)
            .map_err(|_| Error::Rejected)?
            .into();
        composition.require_current()?;
        context.require_jni_owner(env, class)?;
        let completed = intake(
            composition,
            env,
            application.as_obj(),
            class,
            &signatory,
            &seed,
            context,
        )?;
        PENDING.with(|pending| -> Result<(), Error> {
            let mut pending = pending.borrow_mut();
            let original = pending.as_mut().ok_or(Error::Rejected)?;
            if original.completed.is_some() {
                retire(&completed.1);
                return Err(Error::Rejected);
            }
            original.completed = Some(completed);
            Ok(())
        })
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaOrdinaryRuntimeJniV1_nativeConsumeExistingAndroidAccountV1<
        'local,
    >(
        mut env: JNIEnv<'local>,
        class: jni::objects::JClass<'local>,
        continuation: JObject<'local>,
        signatory: JString<'local>,
        seed: JByteArray<'local>,
    ) -> jni::sys::jboolean {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            consume(&mut env, &class, &continuation, &signatory, &seed)
        }))
        .ok()
        .and_then(Result::ok)
        .map_or(0, |()| 1)
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaOrdinaryRuntimeJniV1_nativeExistingAndroidAccountV1<
        'local,
    >(
        mut env: JNIEnv<'local>,
        class: jni::objects::JClass<'local>,
        application: JObject<'local>,
    ) -> jni::sys::jobjectArray {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<_, Error> {
                if super::has_registered_runtime()
                    || PENDING.with(|pending| pending.borrow().is_some())
                {
                    return Err(Error::Rejected);
                }
                // Claim before the first measured Application/source/key I/O. The same
                // original is lent once to the protected reader and drives final publication;
                // any early logout, failure or unwind permanently retires that original.
                let composition = claim_existing_account_before_receiver(
                    claim_native_composition_original,
                    || InstalledContext::require_early_retained_jni_class(&mut env, &class),
                )?;
                let context = InstalledContext::from_application(&mut env, &application)?;
                composition.require_current()?;
                context.require_jni_owner(&mut env, &class)?;
                composition.require_current()?;
                let continuation =
                    context.existing_account_storage_continuation(&mut env, &application)?;
                composition.require_current()?;
                context.require_jni_owner(&mut env, &class)?;
                let original = PendingLoan {
                    composition: Some(composition),
                    context: Arc::clone(&context),
                    application: env
                        .new_global_ref(&application)
                        .map_err(|_| Error::Rejected)?,
                    continuation: env
                        .new_global_ref(&continuation)
                        .map_err(|_| Error::Rejected)?,
                    used: false,
                    completed: None,
                };
                PENDING.with(|pending| *pending.borrow_mut() = Some(original));
                let mut guard = ActiveLoan { complete: false };
                env.call_method(&application, "withKagemushaExistingAccountKeyOriginal",
                "(Lorg/hyperledger/iroha/sdk/offline/KagemushaOrdinaryExistingAccountIntakeV1;)V",
                &[JValue::Object(&continuation)]).map_err(|_| Error::Rejected)?;
                context.require_jni_owner(&mut env, &class)?;
                // The checked activation completed its own postcheck before the final product method
                // returned. A raw JNI seed call, foreign continuation or second call never reaches here.
                let (fields, startup, retirement_generation) =
                    PENDING.with(|pending| -> Result<_, Error> {
                        let pending = pending.borrow();
                        let original = pending.as_ref().ok_or(Error::Rejected)?;
                        if !original.used {
                            return Err(Error::Rejected);
                        }
                        let (fields, startup, retirement_generation) =
                            original.completed.as_ref().ok_or(Error::Rejected)?;
                        startup
                            .retirement
                            .require_original(*retirement_generation)?;
                        Ok((fields.clone(), Arc::clone(startup), *retirement_generation))
                    })?;
                let byte_array_class = env.find_class("[B").map_err(|_| Error::Rejected)?;
                let response = env
                    .new_object_array(3, byte_array_class, JObject::null())
                    .map_err(|_| Error::Rejected)?;
                for (i, bytes) in fields.iter().enumerate() {
                    let value = env
                        .byte_array_from_slice(bytes)
                        .map_err(|_| Error::Rejected)?;
                    env.set_object_array_element(&response, i as i32, value)
                        .map_err(|_| Error::Rejected)?;
                }
                context.require_jni_owner(&mut env, &class)?;
                // The exact original protected activation postcheck has returned successfully.
                // Only this source-owned continuation commits the previously unavailable startup.
                startup
                    .account_publication
                    .commit_original(&startup.retirement, retirement_generation)?;
                context.require_jni_owner(&mut env, &class)?;
                startup.retirement.require_original(retirement_generation)?;
                guard.complete = true;
                drop(guard);
                Ok(response.into_raw())
            }));
        result
            .ok()
            .and_then(Result::ok)
            .unwrap_or(std::ptr::null_mut())
    }
    #[cfg(test)]
    mod storage_tests {
        //! Exercise the actual protected-directory opener without a JNI or runtime owner.
        use super::*;
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        #[test]
        fn canonical_storage_reopens_the_same_protected_directory_and_preserves_originals() {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary.path().canonicalize().unwrap();
            let (path, held) = open_local_storage(&root).unwrap();
            assert_eq!(path, root.join("kagemusha-core-v1"));
            let first = held.metadata().unwrap();
            assert_eq!(first.mode() & 0o777, 0o700);
            assert_eq!(first.uid(), unsafe { libc::geteuid() });
            let original = path.join("retained-original.norito.wal");
            std::fs::write(&original, b"inert original storage bytes").unwrap();
            let (reopened_path, reopened) = open_local_storage(&root).unwrap();
            let second = reopened.metadata().unwrap();
            assert_eq!(reopened_path, path);
            assert_eq!((second.dev(), second.ino()), (first.dev(), first.ino()));
            assert_eq!(
                std::fs::read(original).unwrap(),
                b"inert original storage bytes"
            );
            assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        }

        #[test]
        fn canonical_storage_refuses_aliases_and_open_permissions_without_repair() {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary.path().canonicalize().unwrap();
            let outside = tempfile::tempdir().unwrap();
            let outside_original = outside.path().join("preserved-original");
            std::fs::write(&outside_original, b"outside original").unwrap();
            let path = root.join("kagemusha-core-v1");
            symlink(outside.path(), &path).unwrap();
            assert!(matches!(open_local_storage(&root), Err(Error::Rejected)));
            assert!(
                std::fs::symlink_metadata(&path)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(
                std::fs::read(&outside_original).unwrap(),
                b"outside original"
            );
            assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 1);
            std::fs::remove_file(&path).unwrap();
            std::fs::create_dir(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            assert!(matches!(open_local_storage(&root), Err(Error::Rejected)));
            assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o755);
            assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_during_receiver_authentication_refuses_first_existing_account_read_and_publication()
     {
        let registration = Mutex::new(StartupRegistration::new());
        let reads = std::sync::atomic::AtomicUsize::new(0);
        let publications = std::sync::atomic::AtomicUsize::new(0);
        let result = claim_existing_account_before_receiver(
            || registration.lock().unwrap().claim(),
            || retire_registered_composition(&registration),
        )
        .and_then(|original| {
            original.install(
                |_| {
                    reads.fetch_add(1, AtomicOrdering::SeqCst);
                    Ok(Arc::new(7_u8))
                },
                |_| {
                    publications.fetch_add(1, AtomicOrdering::SeqCst);
                    Ok(())
                },
            )
        });
        assert_eq!(result, Err(Error::Rejected));
        assert_eq!(reads.load(AtomicOrdering::SeqCst), 0);
        assert_eq!(publications.load(AtomicOrdering::SeqCst), 0);
        let mut registration = registration.lock().unwrap();
        assert!(matches!(registration.claim(), Err(Error::Rejected)));
        assert_eq!(
            registration.original.as_ref().unwrap().capture_original(),
            Err(Error::Rejected),
        );
    }

    #[test]
    fn failed_receiver_authentication_consumes_existing_account_composition() {
        let registration = Mutex::new(StartupRegistration::new());
        let result = claim_existing_account_before_receiver(
            || registration.lock().unwrap().claim(),
            || Err(Error::Rejected),
        );
        assert!(matches!(result, Err(Error::Rejected)));
        let mut registration = registration.lock().unwrap();
        assert!(matches!(registration.claim(), Err(Error::Rejected)));
        assert_eq!(
            registration.original.as_ref().unwrap().capture_original(),
            Err(Error::Rejected),
        );
    }

    #[test]
    fn incomplete_loan_scope_retires_successfully_published_original_without_completed_tuple() {
        let mut registration = StartupRegistration::new();
        let original = registration.claim().unwrap();
        let retirement = Arc::clone(&original.retirement);
        let published = OnceLock::new();
        let returned = original
            .install(
                |same_retirement| Ok(same_retirement),
                |same_retirement| published.set(same_retirement).map_err(|_| Error::Rejected),
            )
            .unwrap();
        assert!(Arc::ptr_eq(&retirement, &returned));
        assert!(Arc::ptr_eq(&retirement, published.get().unwrap()));
        assert_eq!(retirement.capture_original(), Ok(0));
        let registration = Mutex::new(registration);
        // The actual shared Drop helper must deny even if the published result never reached
        // PendingLoan.completed. This is original lifetime control, not Android authority.
        assert_eq!(
            retire_incomplete_existing_account_loan(false, || {
                retire_registered_composition(&registration)
            }),
            Ok(()),
        );
        assert_eq!(retirement.capture_original(), Err(Error::Rejected));
        assert!(matches!(
            registration.lock().unwrap().claim(),
            Err(Error::Rejected),
        ));
    }

    #[test]
    fn empty_resource_budget_is_refused_without_any_key_generation() {
        assert!(resource_capacity(0).is_err());
        assert!(
            resource_capacity(4 * KagemushaDurableCapacityV1::MINIMUM_INBOX_BYTES - 1).is_err()
        );
    }
    #[test]
    fn local_budget_is_finite_and_keeps_half_unassigned() {
        let cap = resource_capacity(8 * 1024 * 1024).unwrap();
        assert_eq!(cap.inbox_bytes, 2 * 1024 * 1024);
        assert_eq!(cap.outbox_bytes, 2 * 1024 * 1024);
        assert!(cap.inbox_bytes.checked_add(cap.outbox_bytes).unwrap() <= 8 * 1024 * 1024 / 2);
    }
}

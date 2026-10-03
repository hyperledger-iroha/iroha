//! Actual ordinary-purpose Android producer. Compiled SDK root and package descriptors remain
//! owned here; no measurement/root fields cross C/JNI and no first-device owner is promoted.
use super::KagemushaCoreCoordinatorBackendErrorV1 as Error;
use iroha::client::KagemushaNativeOrdinaryInstalledContextV1 as ContextOriginals;
use std::{path::Path, sync::Arc};

/// Closed real Android package + signed ordinary originals owner, held by actual Native startup.
/// No public constructor, decoder, managed callback or caller Root/measurement setter exists.
pub(super) struct AndroidOrdinaryInstalledContextOwnerV1 {
    #[cfg(any(target_os = "android", all(test, unix)))]
    package: android::PackageOwner,
    #[cfg(any(target_os = "android", all(test, unix)))]
    directories: Vec<android::HeldDirectory>,
    originals: Arc<ContextOriginals>,
}
impl AndroidOrdinaryInstalledContextOwnerV1 {
    pub(super) fn recheck(&self) -> Result<(), Error> {
        #[cfg(any(target_os = "android", all(test, unix)))]
        {
            self.package.recheck().map_err(|_| Error::Rejected)?;
            for directory in &self.directories {
                directory.recheck().map_err(|_| Error::Rejected)?;
            }
        }
        self.originals.recheck().map_err(|_| Error::Rejected)
    }
    pub(super) fn runtime_authority(
        &self,
    ) -> Result<Arc<iroha::client::KagemushaNativeInstalledRuntimeAuthorityV1>, Error> {
        self.recheck()?;
        self.originals
            .runtime_authority()
            .map_err(|_| Error::Rejected)
    }
    pub(super) fn recursive_profile(
        &self,
    ) -> Result<iroha_core_zk::kagemusha_v1_recursion::KagemushaRecursiveVerifierProfileV1, Error>
    {
        self.recheck()?;
        self.originals
            .recursive_profile()
            .map_err(|_| Error::Rejected)
    }
    pub(super) fn inventory(
        &self,
    ) -> Result<Arc<iroha::client::KagemushaAdmittedOrdinaryNativeInventoryV1>, Error> {
        self.recheck()?;
        self.originals.inventory().map_err(|_| Error::Rejected)
    }
    pub(super) fn inventory_path(&self) -> &Path {
        self.originals.inventory_path()
    }
    pub(super) fn inventory_sha256(&self) -> [u8; 32] {
        self.originals.inventory_sha256()
    }
    pub(super) fn public_original_root(&self) -> &Path {
        self.originals.public_original_root()
    }
    #[cfg(any(target_os = "android", all(test, unix)))]
    pub(super) fn from_application<'local>(
        env: &mut jni::JNIEnv<'local>,
        application: &jni::objects::JObject<'local>,
    ) -> Result<Arc<Self>, Error> {
        android::load(env, application).map_err(|_| Error::Rejected)
    }
    /// Bind the actual static JNI caller to the retained installed package and defining loader.
    /// The class is a JVM-provided JNI receiver, never a decoded/offered authority field.
    #[cfg(any(target_os = "android", all(test, unix)))]
    pub(super) fn require_jni_owner<'local>(
        &self,
        env: &mut jni::JNIEnv<'local>,
        class: &jni::objects::JClass<'local>,
    ) -> Result<(), Error> {
        self.recheck()?;
        self.package
            .require_retained_jni_class(env, class)
            .map_err(|_| Error::Rejected)?;
        self.recheck()
    }
    /// Create only the fixed continuation through the actual measured product class loader.
    #[cfg(any(target_os = "android", all(test, unix)))]
    pub(super) fn existing_account_storage_continuation<'local>(
        &self,
        env: &mut jni::JNIEnv<'local>,
        application: &jni::objects::JObject<'local>,
    ) -> Result<jni::objects::JObject<'local>, Error> {
        self.recheck()?;
        let original = self
            .package
            .existing_account_storage_continuation(env, application)
            .map_err(|_| Error::Rejected)?;
        self.recheck()?;
        Ok(original)
    }
    /// Identity-only check for cancellation, close and logout after source admission fails.
    /// It grants no source/current-read capability and never refreshes signed authority.
    #[cfg(any(target_os = "android", all(test, unix)))]
    pub(super) fn require_retained_jni_class<'local>(
        &self,
        env: &mut jni::JNIEnv<'local>,
        class: &jni::objects::JClass<'local>,
    ) -> Result<(), Error> {
        self.package
            .require_retained_jni_class(env, class)
            .map_err(|_| Error::Rejected)
    }
    /// Authenticate cleanup from the original Application/VM/loader even before construction.
    /// This identity-only path performs no installed-source admission or wallet I/O.
    #[cfg(any(target_os = "android", test))]
    pub(super) fn require_early_retained_jni_class<'local>(
        env: &mut jni::JNIEnv<'local>,
        class: &jni::objects::JClass<'local>,
    ) -> Result<(), Error> {
        java_owner::EARLY_APPLICATION
            .original()
            .and_then(|owner| owner.require_retained_jni_class(env, class))
            .map_err(|_| Error::Rejected)
    }
}

/// Bind the genuine Application and fixed JNI receiver before any wallet composition.
/// This entry retains identity only and reads no key, activation, released source or journal.
/// Host tests typecheck the same JNI wrapper; they do not execute Android/JVM owners.
#[cfg(any(target_os = "android", test))]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaOrdinaryRuntimeJniV1_nativeBindApplicationV1<
    'local,
>(
    mut env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
    application: jni::objects::JObject<'local>,
) -> jni::sys::jboolean {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        java_owner::EARLY_APPLICATION.bind(
            &mut env,
            |env| java_owner::EarlyApplicationOwner::capture(env, &application, &class),
            |owner, env| {
                owner.require_retained_jni_class(env, &class)?;
                owner.require_application(env, &application)
            },
        )
    }));
    if matches!(result, Ok(Ok(_))) {
        jni::sys::JNI_TRUE
    } else {
        jni::sys::JNI_FALSE
    }
}

/// Immediately deny the original pending/published composition, without source or wallet I/O.
/// It grants no monetary authority and does not replace Core close or phase-five revocation.
/// Host tests typecheck the same JNI wrapper; they supply no Android runtime qualification.
#[cfg(any(target_os = "android", test))]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaOrdinaryRuntimeJniV1_nativeRetireOriginalV1<
    'local,
>(
    mut env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
) -> jni::sys::jboolean {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), Error> {
        AndroidOrdinaryInstalledContextOwnerV1::require_early_retained_jni_class(&mut env, &class)?;
        super::ordinary_native_startup::retire_pending_native_composition()?;
        AndroidOrdinaryInstalledContextOwnerV1::require_early_retained_jni_class(&mut env, &class)
    }));
    if matches!(result, Ok(Ok(()))) {
        jni::sys::JNI_TRUE
    } else {
        jni::sys::JNI_FALSE
    }
}

// This single production implementation is also enabled in host unit-test builds.
// Host tests do not execute Android classes or establish device/source qualification.
#[cfg(any(target_os = "android", test))]
mod java_owner {
    use jni::{
        JNIEnv, JavaVM,
        objects::{GlobalRef, JClass, JObject, JString, JValue},
    };
    use std::sync::{Arc, Mutex, OnceLock};
    type Result<T> = std::result::Result<T, ()>;

    /// The binding attempt is consumed before capture; failure never permits a replacement.
    /// Capturing/rechecking JVM objects holds neither the attempt mutex nor a wallet lock.
    pub(super) struct OriginalIdentityBinding<T> {
        attempted: Mutex<bool>,
        original: OnceLock<Arc<T>>,
    }
    impl<T> OriginalIdentityBinding<T> {
        pub(super) const fn new() -> Self {
            Self {
                attempted: Mutex::new(false),
                original: OnceLock::new(),
            }
        }
        pub(super) fn bind<C>(
            &self,
            context: &mut C,
            capture: impl FnOnce(&mut C) -> Result<T>,
            require_original: impl FnOnce(&T, &mut C) -> Result<()>,
        ) -> Result<Arc<T>> {
            if let Some(original) = self.original.get() {
                require_original(original, context)?;
                return Ok(original.clone());
            }
            {
                let mut attempted = self.attempted.lock().map_err(|_| ())?;
                if *attempted {
                    return Err(());
                }
                *attempted = true;
            }
            let original = Arc::new(capture(context)?);
            require_original(&original, context)?;
            self.original.set(original.clone()).map_err(|_| ())?;
            Ok(original)
        }
        pub(super) fn original(&self) -> Result<Arc<T>> {
            self.original.get().cloned().ok_or(())
        }
    }

    pub(super) static EARLY_APPLICATION: OriginalIdentityBinding<EarlyApplicationOwner> =
        OriginalIdentityBinding::new();

    /// Identity only: no release/package bytes, activation, key or Native runtime is read.
    pub(super) struct EarlyApplicationOwner {
        pub(super) vm: JavaVM,
        pub(super) application: GlobalRef,
        pub(super) java: JavaOwner,
    }
    impl EarlyApplicationOwner {
        pub(super) fn capture<'local>(
            env: &mut JNIEnv<'local>,
            app: &JObject<'local>,
            class: &JClass<'local>,
        ) -> Result<Self> {
            let java = JavaOwner::capture(env, app)?;
            java.require_retained_jni_class(env, class)?;
            Ok(Self {
                vm: env.get_java_vm().map_err(|_| ())?,
                application: env.new_global_ref(app).map_err(|_| ())?,
                java,
            })
        }
        pub(super) fn require_application<'local>(
            &self,
            env: &mut JNIEnv<'local>,
            app: &JObject<'local>,
        ) -> Result<()> {
            if env.get_java_vm().map_err(|_| ())?.get_java_vm_pointer()
                != self.vm.get_java_vm_pointer()
                || !env
                    .is_same_object(app, self.application.as_obj())
                    .map_err(|_| ())?
            {
                return Err(());
            }
            self.java.recheck(env, app)
        }
        pub(super) fn require_retained_jni_class<'local>(
            &self,
            env: &mut JNIEnv<'local>,
            class: &JClass<'local>,
        ) -> Result<()> {
            if env.get_java_vm().map_err(|_| ())?.get_java_vm_pointer()
                != self.vm.get_java_vm_pointer()
            {
                return Err(());
            }
            self.java.require_retained_jni_class(env, class)
        }
    }

    const ORDINARY_RUNTIME_JNI_CLASS: &str =
        "org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryRuntimeJniV1";

    fn installed_class_names_match(manifest: &str, application: &str, jni: &str) -> bool {
        !manifest.is_empty() && application == manifest && jni == ORDINARY_RUNTIME_JNI_CLASS
    }

    #[cfg(test)]
    mod tests {
        use super::{
            ORDINARY_RUNTIME_JNI_CLASS, OriginalIdentityBinding, installed_class_names_match,
        };
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        };
        use std::time::Duration;

        #[test]
        fn failed_identity_capture_consumes_attempt_without_creating_an_owner() {
            let binding = OriginalIdentityBinding::<u8>::new();
            assert!(binding.bind(&mut (), |_| Err(()), |_, _| Ok(())).is_err());
            assert!(binding.original().is_err());
            assert!(binding.bind(&mut (), |_| Ok(2), |_, _| Ok(())).is_err());
            assert!(binding.original().is_err());
        }

        #[test]
        fn failed_original_check_cannot_recapture_or_publish_identity() {
            let binding = OriginalIdentityBinding::new();
            assert!(binding.bind(&mut (), |_| Ok(1_u8), |_, _| Err(())).is_err());
            assert!(binding.original().is_err());
            assert!(binding.bind(&mut (), |_| Ok(2), |_, _| Ok(())).is_err());
            assert!(binding.original().is_err());
        }

        #[test]
        fn repeated_binding_rechecks_the_exact_original_without_replacement() {
            let binding = OriginalIdentityBinding::new();
            let captures = AtomicUsize::new(0);
            let first = binding
                .bind(
                    &mut (),
                    |_| {
                        captures.fetch_add(1, Ordering::SeqCst);
                        Ok(7_u8)
                    },
                    |_, _| Ok(()),
                )
                .unwrap();
            let repeated = binding
                .bind(
                    &mut (),
                    |_| panic!("must not recapture"),
                    |original, _| {
                        assert_eq!(*original, 7);
                        Ok(())
                    },
                )
                .unwrap();
            assert!(Arc::ptr_eq(&first, &repeated));
            assert!(binding.bind(&mut (), |_| Ok(8), |_, _| Err(())).is_err());
            assert!(Arc::ptr_eq(&first, &binding.original().unwrap()));
            assert_eq!(captures.load(Ordering::SeqCst), 1);
        }

        #[test]
        fn capture_does_not_hold_the_attempt_lock_or_admit_a_concurrent_binding() {
            let binding = Arc::new(OriginalIdentityBinding::new());
            let (entered_tx, entered_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let first_binding = binding.clone();
            let first = std::thread::spawn(move || {
                first_binding.bind(
                    &mut (),
                    |_| {
                        entered_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                        Ok(7_u8)
                    },
                    |_, _| Ok(()),
                )
            });
            entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(binding.original().is_err());
            let (refused_tx, refused_rx) = mpsc::channel();
            let second_binding = binding.clone();
            let second = std::thread::spawn(move || {
                refused_tx
                    .send(
                        second_binding
                            .bind(&mut (), |_| Ok(8), |_, _| Ok(()))
                            .is_err(),
                    )
                    .unwrap();
            });
            let refused = refused_rx.recv_timeout(Duration::from_secs(5));
            release_tx.send(()).unwrap();
            let first = first.join().unwrap().unwrap();
            second.join().unwrap();
            assert!(refused.unwrap());
            assert_eq!(*first, 7);
            assert!(Arc::ptr_eq(&first, &binding.original().unwrap()));
        }

        // These pure tests cover exact names only; they never authenticate Android runtime owners,
        // defining loaders, APK/DEX/JNI originals or release authority on a host JVM.
        #[test]
        fn installed_application_and_fixed_jni_names_are_exact() {
            assert!(installed_class_names_match(
                "pg.bpng.digitalkina.DigitalKinaApp",
                "pg.bpng.digitalkina.DigitalKinaApp",
                ORDINARY_RUNTIME_JNI_CLASS,
            ));
            for application in [
                "android.app.Application",
                "pg.bpng.digitalkina.DigitalKinaApp$Proxy",
                "pg.bpng.digitalkina.DigitalKinaAppSubclass",
                "pg.bpng.digitalkina.digitalkinaapp",
                ".DigitalKinaApp",
            ] {
                assert!(!installed_class_names_match(
                    "pg.bpng.digitalkina.DigitalKinaApp",
                    application,
                    ORDINARY_RUNTIME_JNI_CLASS,
                ));
            }
        }

        #[test]
        fn absent_manifest_and_alternative_jni_owners_are_refused() {
            assert!(!installed_class_names_match(
                "",
                "",
                ORDINARY_RUNTIME_JNI_CLASS,
            ));
            for jni in [
                "",
                "org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryRuntimeJniV1$Proxy",
                "org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryRuntimeJniV2",
                "other.package.KagemushaOrdinaryRuntimeJniV1",
            ] {
                assert!(!installed_class_names_match(
                    "pg.bpng.digitalkina.DigitalKinaApp",
                    "pg.bpng.digitalkina.DigitalKinaApp",
                    jni,
                ));
            }
        }
    }

    /// Runtime objects are retained, not reconstructed from class names or loader paths.
    pub(super) struct JavaOwner {
        application_class: GlobalRef,
        base_context: GlobalRef,
        loader: GlobalRef,
        jni_class: GlobalRef,
        manifest_class: String,
    }
    impl JavaOwner {
        pub(super) fn capture<'local>(
            env: &mut JNIEnv<'local>,
            app: &JObject<'local>,
        ) -> Result<Self> {
            if app.is_null() {
                return Err(());
            }
            let bootstrap = JObject::null();
            let framework_application = resolve_class(env, "android.app.Application", &bootstrap)?;
            let framework_application = env.auto_local(framework_application);
            let framework_loader = class_loader(env, &framework_application)?;
            let framework_loader = env.auto_local(framework_loader);
            if !env
                .is_instance_of(app, &*framework_application)
                .map_err(|_| ())?
            {
                return Err(());
            }
            let base = object(
                env,
                app,
                "getBaseContext",
                "()Landroid/content/Context;",
                &[],
            )
            .map_err(|_| ())?;
            let base = env.auto_local(base);
            let framework_context = resolve_class(env, "android.content.Context", &bootstrap)?;
            let framework_context = env.auto_local(framework_context);
            let base_class = env.auto_local(env.get_object_class(&base).map_err(|_| ())?);
            let base_loader = class_loader(env, &base_class)?;
            let base_loader = env.auto_local(base_loader);
            if !env
                .is_instance_of(&base, &*framework_context)
                .map_err(|_| ())?
                || !env
                    .is_same_object(&base_loader, &framework_loader)
                    .map_err(|_| ())?
            {
                return Err(());
            }
            // Read the installed owner from the actual framework context, so an Application
            // wrapper/substitute cannot authenticate itself with overridden context getters.
            for context in [app, &*base] {
                let actual = object(
                    env,
                    context,
                    "getApplicationContext",
                    "()Landroid/content/Context;",
                    &[],
                )
                .map_err(|_| ())?;
                let actual = env.auto_local(actual);
                if !env.is_same_object(app, &actual).map_err(|_| ())? {
                    return Err(());
                }
            }
            let info = application_info(env, &base)?;
            let info = env.auto_local(info);
            let framework_info =
                resolve_class(env, "android.content.pm.ApplicationInfo", &bootstrap)?;
            let framework_info = env.auto_local(framework_info);
            let info_class = env.auto_local(env.get_object_class(&info).map_err(|_| ())?);
            let app_info = application_info(env, app)?;
            let app_info = env.auto_local(app_info);
            if !env
                .is_same_object(&info_class, &framework_info)
                .map_err(|_| ())?
                || !env.is_same_object(&info, &app_info).map_err(|_| ())?
            {
                return Err(());
            }
            let raw = env
                .get_field(&info, "className", "Ljava/lang/String;")
                .map_err(|_| ())?
                .l()
                .map_err(|_| ())?;
            // Android's installed ApplicationInfo already resolves the manifest class name.
            // A missing declaration/default Application is not this product's actual owner.
            let manifest_class = string(env, raw).map_err(|_| ())?;
            let application_class = env.auto_local(env.get_object_class(app).map_err(|_| ())?);
            let loader = class_loader(env, &base)?;
            let loader = env.auto_local(loader);
            let app_loader = class_loader(env, app)?;
            let app_loader = env.auto_local(app_loader);
            let defining_loader = class_loader(env, &application_class)?;
            let defining_loader = env.auto_local(defining_loader);
            if loader.is_null()
                || !env.is_same_object(&loader, &app_loader).map_err(|_| ())?
                || !env
                    .is_same_object(&loader, &defining_loader)
                    .map_err(|_| ())?
                || env
                    .is_same_object(&loader, &framework_loader)
                    .map_err(|_| ())?
            {
                return Err(());
            }
            let declared = resolve_class(env, &manifest_class, &loader)?;
            let declared = env.auto_local(declared);
            if !env
                .is_same_object(&application_class, &declared)
                .map_err(|_| ())?
            {
                return Err(());
            }
            // Resolve only the fixed ordinary JNI owner through this actual retained loader.
            // Neither FindClass's calling-thread loader nor a caller-supplied classpath is used.
            let jni_class = resolve_class(env, ORDINARY_RUNTIME_JNI_CLASS, &loader)?;
            let jni_class = env.auto_local(jni_class);
            let jni_loader = class_loader(env, &jni_class)?;
            let jni_loader = env.auto_local(jni_loader);
            if !env.is_same_object(&loader, &jni_loader).map_err(|_| ())?
                || !installed_class_names_match(
                    &manifest_class,
                    &string_method(env, &application_class, "getName").map_err(|_| ())?,
                    &string_method(env, &jni_class, "getName").map_err(|_| ())?,
                )
            {
                return Err(());
            }
            Ok(Self {
                application_class: env.new_global_ref(&application_class).map_err(|_| ())?,
                base_context: env.new_global_ref(&base).map_err(|_| ())?,
                loader: env.new_global_ref(&loader).map_err(|_| ())?,
                jni_class: env.new_global_ref(&jni_class).map_err(|_| ())?,
                manifest_class,
            })
        }
        pub(super) fn recheck<'local>(
            &self,
            env: &mut JNIEnv<'local>,
            app: &JObject<'local>,
        ) -> Result<()> {
            let current = Self::capture(env, app)?;
            if current.manifest_class != self.manifest_class {
                return Err(());
            }
            for (held, actual) in [
                (&self.application_class, &current.application_class),
                (&self.base_context, &current.base_context),
                (&self.loader, &current.loader),
                (&self.jni_class, &current.jni_class),
            ] {
                if !env
                    .is_same_object(held.as_obj(), actual.as_obj())
                    .map_err(|_| ())?
                {
                    return Err(());
                }
            }
            Ok(())
        }
        pub(super) fn existing_account_storage_continuation<'local>(
            &self,
            env: &mut JNIEnv<'local>,
            app: &JObject<'local>,
        ) -> Result<JObject<'local>> {
            self.recheck(env, app)?;
            let class = resolve_class(
                env,
                "org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryExistingAccountIntakeV1",
                self.loader.as_obj(),
            )?;
            let class = env.auto_local(class);
            let loader = class_loader(env, &class)?;
            let loader = env.auto_local(loader);
            if !env
                .is_same_object(&loader, self.loader.as_obj())
                .map_err(|_| ())?
            {
                return Err(());
            }
            // The class and product method are final and defined in the actual installed owner;
            // no supplied callback, subclass or ambient calling-thread classpath is used.
            let modifiers = env
                .call_method(&class, "getModifiers", "()I", &[])
                .and_then(|v| v.i())
                .map_err(|_| ())?;
            if modifiers & 0x10 == 0 {
                return Err(());
            }
            let class_class = resolve_class(env, "java.lang.Class", &JObject::null())?;
            let parameters = env
                .new_object_array(1, class_class, JObject::null())
                .map_err(|_| ())?;
            env.set_object_array_element(&parameters, 0, &class)
                .map_err(|_| ())?;
            let name = env
                .new_string("withKagemushaExistingAccountKeyOriginal")
                .map_err(|_| ())?;
            let method = env
                .call_method(
                    self.application_class.as_obj(),
                    "getDeclaredMethod",
                    "(Ljava/lang/String;[Ljava/lang/Class;)Ljava/lang/reflect/Method;",
                    &[JValue::Object(&name), JValue::Object(&parameters)],
                )
                .and_then(|v| v.l())
                .map_err(|_| ())?;
            let method = env.auto_local(method);
            let modifiers = env
                .call_method(&method, "getModifiers", "()I", &[])
                .and_then(|v| v.i())
                .map_err(|_| ())?;
            if modifiers & 0x11 != 0x11 || modifiers & 0x08 != 0 {
                return Err(());
            }
            let declaring = env
                .call_method(&method, "getDeclaringClass", "()Ljava/lang/Class;", &[])
                .and_then(|v| v.l())
                .map_err(|_| ())?;
            let declaring = env.auto_local(declaring);
            if !env
                .is_same_object(&declaring, self.application_class.as_obj())
                .map_err(|_| ())?
            {
                return Err(());
            }
            env.new_object(&*class, "()V", &[]).map_err(|_| ())
        }
        pub(super) fn base_context(&self) -> &JObject<'static> {
            self.base_context.as_obj()
        }
        pub(super) fn application_info<'local>(
            &self,
            env: &mut JNIEnv<'local>,
        ) -> Result<JObject<'local>> {
            application_info(env, self.base_context())
        }
        pub(super) fn require_retained_jni_class<'local>(
            &self,
            env: &mut JNIEnv<'local>,
            class: &JClass<'local>,
        ) -> Result<()> {
            if class.is_null()
                || !env
                    .is_same_object(class, self.jni_class.as_obj())
                    .map_err(|_| ())?
            {
                return Err(());
            }
            let loader = class_loader(env, class)?;
            let loader = env.auto_local(loader);
            if !env
                .is_same_object(&loader, self.loader.as_obj())
                .map_err(|_| ())?
            {
                return Err(());
            }
            Ok(())
        }
    }
    fn resolve_class<'local>(
        env: &mut JNIEnv<'local>,
        name: &str,
        loader: &JObject<'local>,
    ) -> Result<JClass<'local>> {
        let name = env.new_string(name).map_err(|_| ())?;
        let name = env.auto_local(name);
        env.call_static_method(
            "java/lang/Class",
            "forName",
            "(Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;",
            &[
                JValue::Object(&name),
                JValue::Bool(0),
                JValue::Object(loader),
            ],
        )
        .map_err(|_| ())?
        .l()
        .map(JClass::from)
        .map_err(|_| ())
    }
    fn class_loader<'local>(
        env: &mut JNIEnv<'local>,
        owner: &JObject<'local>,
    ) -> Result<JObject<'local>> {
        object(
            env,
            owner,
            "getClassLoader",
            "()Ljava/lang/ClassLoader;",
            &[],
        )
    }
    fn application_info<'local>(
        env: &mut JNIEnv<'local>,
        context: &JObject<'local>,
    ) -> Result<JObject<'local>> {
        object(
            env,
            context,
            "getApplicationInfo",
            "()Landroid/content/pm/ApplicationInfo;",
            &[],
        )
    }
    fn object<'local>(
        env: &mut JNIEnv<'local>,
        owner: &JObject<'local>,
        name: &str,
        signature: &str,
        arguments: &[JValue<'local, '_>],
    ) -> Result<JObject<'local>> {
        env.call_method(owner, name, signature, arguments)
            .map_err(|_| ())?
            .l()
            .map_err(|_| ())
    }
    fn string<'local>(env: &mut JNIEnv<'local>, value: JObject<'local>) -> Result<String> {
        let value = env.auto_local(JString::from(value));
        let value = env.get_string(&value).map_err(|_| ())?;
        value.to_str().map(str::to_owned).map_err(|_| ())
    }
    fn string_method<'local>(
        env: &mut JNIEnv<'local>,
        owner: &JObject<'local>,
        method: &str,
    ) -> Result<String> {
        let value = object(env, owner, method, "()Ljava/lang/String;", &[])?;
        string(env, value)
    }
}

#[cfg(any(target_os = "android", all(test, unix)))]
mod android {
    use super::java_owner::{EARLY_APPLICATION, EarlyApplicationOwner};
    use super::*;
    use crate::kagemusha_hardware_evidence_v1::android_startup::android_package::*;
    use iroha_data_model::kagemusha::{
        KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_MAX_V1,
        KagemushaOrdinaryInstalledContextCompiledBindingV1, KagemushaOrdinaryInstalledContextV1,
    };
    use jni::{
        JNIEnv,
        objects::{JClass, JObject, JValue},
    };
    use sha2::{Digest as _, Sha256};
    use std::{
        fs::{File, OpenOptions},
        io::{Read as _, Write as _},
        os::unix::fs::{
            DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
        },
        path::PathBuf,
        sync::Mutex,
    };
    type Result<T> = std::result::Result<T, ()>;
    const COMPILED: &[u8] = include_bytes!(concat!(
        env!("OUT_DIR"),
        "/ordinary-context-compiled-binding.bin"
    ));
    const ASSET_ROOT: &str = "config/ordinary-native";
    // Serializes publication of public originals only, never provides authority or readiness.
    static COPY: Mutex<()> = Mutex::new(());
    pub(super) struct PackageOwner {
        identity: Arc<EarlyApplicationOwner>,
        base: HeldPackage,
        splits: Vec<HeldPackage>,
        library: Option<HeldPackage>,
        apk: String,
        split_paths: Vec<String>,
        loaded: String,
        name: String,
        version: u64,
        cert: [u8; 32],
        dex: [u8; 32],
        jni: [u8; 32],
    }
    impl PackageOwner {
        fn measure<'local>(env: &mut JNIEnv<'local>, app: &JObject<'local>) -> Result<Self> {
            let identity = EARLY_APPLICATION.original()?;
            identity.require_application(env, app)?;
            let java = &identity.java;
            let context = java.base_context();
            let name = string_method(env, context, "getPackageName").map_err(|_| ())?;
            let info = java.application_info(env)?;
            let info = env.auto_local(info);
            let raw = env
                .get_field(&info, "sourceDir", "Ljava/lang/String;")
                .map_err(|_| ())?
                .l()
                .map_err(|_| ())?;
            let apk = string(env, raw).map_err(|_| ())?;
            let base = HeldPackage::open(PathBuf::from(&apk)).map_err(|_| ())?;
            let split_paths = installed_split_paths(env, &info).map_err(|_| ())?;
            let mut splits = Vec::new();
            for path in &split_paths {
                if path == &apk {
                    return Err(());
                }
                let held = HeldPackage::open(PathBuf::from(path)).map_err(|_| ())?;
                let zip = zip_file(env, path).map_err(|_| ())?;
                let zip = env.auto_local(zip);
                let accepted = require_resource_split(env, &zip).map_err(|_| ());
                let closed = env.call_method(&zip, "close", "()V", &[]).map_err(|_| ());
                accepted?;
                closed?;
                held.recheck().map_err(|_| ())?;
                splits.push(held);
            }
            // Sole existing framework helper has explicit SDK_INT guards for API26/27
            // versionCode/signatures and API28+ longVersionCode/signingInfo.
            let (version, cert) = package_identity(env, context, &name).map_err(|_| ())?;
            let loaded = loaded_library_path().map_err(|_| ())?;
            let library = if loaded.contains("!/") {
                None
            } else {
                Some(HeldPackage::open(PathBuf::from(&loaded)).map_err(|_| ())?)
            };
            let zip = zip_file(env, &apk).map_err(|_| ())?;
            let zip = env.auto_local(zip);
            let measured: Result<([u8; 32], [u8; 32])> = (|| {
                Ok((
                    dex_digest(env, &zip).map_err(|_| ())?,
                    loaded_library_digest(env, &zip, &apk, &split_paths).map_err(|_| ())?,
                ))
            })();
            let closed = env.call_method(&zip, "close", "()V", &[]).map_err(|_| ());
            let (dex, jni) = measured?;
            closed?;
            base.recheck().map_err(|_| ())?;
            for file in &splits {
                file.recheck().map_err(|_| ())?;
            }
            if let Some(file) = &library {
                file.recheck().map_err(|_| ())?;
            }
            Ok(Self {
                identity,
                base,
                splits,
                library,
                apk,
                split_paths,
                loaded,
                name,
                version,
                cert,
                dex,
                jni,
            })
        }
        pub(super) fn recheck(&self) -> Result<()> {
            self.base.recheck().map_err(|_| ())?;
            for file in &self.splits {
                file.recheck().map_err(|_| ())?;
            }
            if let Some(file) = &self.library {
                file.recheck().map_err(|_| ())?;
            }
            let mut env = self.identity.vm.attach_current_thread().map_err(|_| ())?;
            let app = self.identity.application.as_obj();
            self.identity.require_application(&mut env, app)?;
            let context = self.identity.java.base_context();
            if string_method(&mut env, context, "getPackageName").map_err(|_| ())? != self.name {
                return Err(());
            }
            let info = self.identity.java.application_info(&mut env)?;
            let info = env.auto_local(info);
            let path = env
                .get_field(&info, "sourceDir", "Ljava/lang/String;")
                .map_err(|_| ())?
                .l()
                .map_err(|_| ())?;
            if string(&mut env, path).map_err(|_| ())? != self.apk
                || installed_split_paths(&mut env, &info).map_err(|_| ())? != self.split_paths
                || package_identity(&mut env, context, &self.name).map_err(|_| ())?
                    != (self.version, self.cert)
                || loaded_library_path().map_err(|_| ())? != self.loaded
            {
                return Err(());
            }
            let zip = zip_file(&mut env, &self.apk).map_err(|_| ())?;
            let zip = env.auto_local(zip);
            let measured: Result<([u8; 32], [u8; 32])> = (|| {
                Ok((
                    dex_digest(&mut env, &zip).map_err(|_| ())?,
                    loaded_library_digest(&mut env, &zip, &self.apk, &self.split_paths)
                        .map_err(|_| ())?,
                ))
            })();
            let closed = env.call_method(&zip, "close", "()V", &[]).map_err(|_| ());
            let values = measured?;
            closed?;
            if values != (self.dex, self.jni) {
                return Err(());
            }
            self.base.recheck().map_err(|_| ())?;
            for file in &self.splits {
                file.recheck().map_err(|_| ())?;
            }
            if let Some(file) = &self.library {
                file.recheck().map_err(|_| ())?;
            }
            Ok(())
        }
        pub(super) fn existing_account_storage_continuation<'local>(
            &self,
            env: &mut JNIEnv<'local>,
            app: &JObject<'local>,
        ) -> Result<JObject<'local>> {
            self.identity.require_application(env, app)?;
            self.identity
                .java
                .existing_account_storage_continuation(env, app)
        }
        pub(super) fn require_retained_jni_class<'local>(
            &self,
            env: &mut JNIEnv<'local>,
            class: &JClass<'local>,
        ) -> Result<()> {
            self.identity.require_retained_jni_class(env, class)
        }
    }
    pub(super) fn load<'local>(
        env: &mut JNIEnv<'local>,
        app: &JObject<'local>,
    ) -> Result<Arc<AndroidOrdinaryInstalledContextOwnerV1>> {
        if COMPILED.is_empty() {
            return Err(());
        }
        let compiled =
            KagemushaOrdinaryInstalledContextCompiledBindingV1::decode_original(COMPILED)
                .map_err(|_| ())?;
        if compiled.native_abi != crate::CONNECT_NORITO_BRIDGE_ABI_VERSION {
            return Err(());
        }
        let package = PackageOwner::measure(env, app)?;
        let assets = object(
            env,
            app,
            "getAssets",
            "()Landroid/content/res/AssetManager;",
            &[],
        )
        .map_err(|_| ())?;
        let signed = asset(
            env,
            &assets,
            &format!("{ASSET_ROOT}/ordinary-installed-context.signed.bin"),
            KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_MAX_V1 + 192,
        )
        .map_err(|_| ())?;
        if signed.len() <= 192 {
            return Err(());
        }
        let at = signed.len() - 192;
        let data =
            KagemushaOrdinaryInstalledContextV1::decode_original(&signed[..at]).map_err(|_| ())?;
        let approvals: [[u8; 64]; 3] = std::array::from_fn(|i| {
            signed[at + i * 64..at + (i + 1) * 64]
                .try_into()
                .expect("bounded approvals")
        });
        let runtime = asset(
            env,
            &assets,
            &format!("{ASSET_ROOT}/runtime-manifest.json"),
            32 * 1024 * 1024,
        )
        .map_err(|_| ())?;
        let sdk = asset(
            env,
            &assets,
            &format!("{ASSET_ROOT}/sdk-release.json"),
            32 * 1024 * 1024,
        )
        .map_err(|_| ())?;
        data.authenticate(&compiled, &approvals, &runtime, &sdk)
            .map_err(|_| ())?;
        let abi = android_abi().map_err(|_| ())?;
        let index = data
            .libraries
            .binary_search_by(|l| l.abi.as_str().cmp(abi))
            .map_err(|_| ())?;
        if data.package_name != package.name
            || data.version_code != package.version
            || data.certificate_sha256 != package.cert
            || data.dex_sha256 != package.dex
            || data.libraries[index].sha256 != package.jni
        {
            return Err(());
        }
        let backup =
            object(env, app, "getNoBackupFilesDir", "()Ljava/io/File;", &[]).map_err(|_| ())?;
        let parent = PathBuf::from(string_method(env, &backup, "getAbsolutePath").map_err(|_| ())?);
        let digest: [u8; 32] = Sha256::digest(&signed).into();
        let root = parent
            .join("ordinary-native-installed")
            .join(hex::encode(digest));
        // Reentrant/concurrent package preparation refuses without waiting under framework JNI.
        let _copy = COPY.try_lock().map_err(|_| ())?;
        let mut directories = Vec::new();
        directories.push(HeldDirectory::open(&parent, false)?);
        directories.push(HeldDirectory::open(
            &parent.join("ordinary-native-installed"),
            true,
        )?);
        directories.push(HeldDirectory::open(&root, true)?);
        for (name, expected, length) in [
            (
                "ordinary-installed-context.signed.bin",
                digest,
                u64::try_from(signed.len()).map_err(|_| ())?,
            ),
            (
                "runtime-manifest.json",
                data.runtime_manifest_sha256,
                u64::try_from(runtime.len()).map_err(|_| ())?,
            ),
            (
                "sdk-release.json",
                data.sdk_release_sha256,
                u64::try_from(sdk.len()).map_err(|_| ())?,
            ),
            (
                "runtime-authority.ed25519.bin",
                Sha256::digest(data.runtime_signer).into(),
                32,
            ),
            (
                "recursive-verifier-profile.bin",
                data.recursive_profile_sha256,
                data.recursive_profile_size,
            ),
        ] {
            copy_asset(
                env,
                &assets,
                &root,
                name,
                expected,
                length,
                &package,
                &mut directories,
            )?;
        }
        // Packet size is bounded independently; Native's existing packet decoder supplies all
        // original semantics. No managed record parses/relabels its Norito body.
        copy_asset_bounded(
            env,
            &assets,
            &root,
            "ordinary-native-inventory.signed.bin",
            data.inventory_sha256,
            4 * 1024 * 1024 + 140,
            None,
            &package,
            &mut directories,
        )?;
        for original in &data.originals {
            let name = format!("public-originals/{}", original.path);
            copy_asset(
                env,
                &assets,
                &root,
                &name,
                original.sha256,
                original.byte_len,
                &package,
                &mut directories,
            )?;
        }
        package.recheck()?;
        let originals = Arc::new(
            ContextOriginals::from_native_installed_originals(
                &root,
                COMPILED,
                &package.name,
                package.version,
                package.cert,
                package.dex,
                abi,
                package.jni,
                crate::CONNECT_NORITO_BRIDGE_ABI_VERSION,
            )
            .map_err(|_| ())?,
        );
        let owner = Arc::new(AndroidOrdinaryInstalledContextOwnerV1 {
            package,
            directories,
            originals,
        });
        owner.recheck().map_err(|_| ())?;
        Ok(owner)
    }
    /// Retained descriptor custody for the actual framework-selected private directory chain.
    /// Child creation changes directory timestamps, so stable identity is inode/owner/mode/link
    /// and process, with canonical non-symbolic names rechecked around every copy operation.
    pub(super) struct HeldDirectory {
        path: PathBuf,
        file: File,
        identity: (u64, u64, u32, u32, u32),
        process: u32,
    }
    impl HeldDirectory {
        fn open(path: &Path, create: bool) -> Result<Self> {
            if !path.is_absolute() {
                return Err(());
            }
            match std::fs::symlink_metadata(path) {
                Ok(_) => (),
                Err(e) if create && e.kind() == std::io::ErrorKind::NotFound => {
                    std::fs::DirBuilder::new()
                        .mode(0o700)
                        .create(path)
                        .map_err(|_| ())?;
                }
                Err(_) => return Err(()),
            }
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)
                .map_err(|_| ())?;
            let metadata = file.metadata().map_err(|_| ())?;
            let this = Self {
                path: path.to_owned(),
                file,
                identity: (
                    metadata.dev(),
                    metadata.ino(),
                    metadata.uid(),
                    metadata.gid(),
                    metadata.mode(),
                ),
                process: std::process::id(),
            };
            this.recheck()?;
            Ok(this)
        }
        pub(super) fn recheck(&self) -> Result<()> {
            if self.process != std::process::id()
                || self.path.canonicalize().map_err(|_| ())? != self.path
            {
                return Err(());
            }
            let mut prefix = PathBuf::new();
            for component in self.path.components() {
                prefix.push(component.as_os_str());
                if std::fs::symlink_metadata(&prefix)
                    .map_err(|_| ())?
                    .file_type()
                    .is_symlink()
                {
                    return Err(());
                }
            }
            for metadata in [
                self.file.metadata().map_err(|_| ())?,
                std::fs::symlink_metadata(&self.path).map_err(|_| ())?,
            ] {
                if !metadata.is_dir()
                    || metadata.file_type().is_symlink()
                    || metadata.uid() != unsafe { libc::geteuid() }
                    || metadata.mode() & 0o077 != 0
                    || (
                        metadata.dev(),
                        metadata.ino(),
                        metadata.uid(),
                        metadata.gid(),
                        metadata.mode(),
                    ) != self.identity
                {
                    return Err(());
                }
            }
            Ok(())
        }
        fn sync(&self) -> Result<()> {
            self.recheck()?;
            self.file.sync_all().map_err(|_| ())?;
            self.recheck()
        }
    }
    fn recheck_directories(directories: &[HeldDirectory]) -> Result<()> {
        for d in directories {
            d.recheck()?;
        }
        Ok(())
    }
    fn copy_asset<'local>(
        env: &mut JNIEnv<'local>,
        assets: &JObject<'local>,
        root: &Path,
        name: &str,
        sha: [u8; 32],
        length: u64,
        package: &PackageOwner,
        directories: &mut Vec<HeldDirectory>,
    ) -> Result<()> {
        copy_asset_bounded(
            env,
            assets,
            root,
            name,
            sha,
            length,
            Some(length),
            package,
            directories,
        )
    }
    fn copy_asset_bounded<'local>(
        env: &mut JNIEnv<'local>,
        assets: &JObject<'local>,
        root: &Path,
        name: &str,
        sha: [u8; 32],
        maximum: u64,
        length: Option<u64>,
        package: &PackageOwner,
        directories: &mut Vec<HeldDirectory>,
    ) -> Result<()> {
        if maximum == 0
            || maximum > 16 * 1024 * 1024 * 1024
            || name
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
            || name.starts_with('/')
        {
            return Err(());
        }
        package.recheck()?;
        recheck_directories(directories)?;
        let target = root.join(name);
        let mut parent = root.to_owned();
        for part in name
            .split('/')
            .collect::<Vec<_>>()
            .iter()
            .take(name.split('/').count() - 1)
        {
            parent.push(part);
            if !directories.iter().any(|d| d.path == parent) {
                directories.push(HeldDirectory::open(&parent, true)?);
            }
            recheck_directories(directories)?;
        }
        if target.try_exists().map_err(|_| ())? {
            check_public_file(&target, sha, maximum, length)?;
            recheck_directories(directories)?;
            return package.recheck();
        }
        let temp = target.with_extension("ordinary-public-partial");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temp)
            .map_err(|_| ())?;
        let key = env
            .new_string(format!("{ASSET_ROOT}/{name}"))
            .map_err(|_| ())?;
        let stream = object(
            env,
            assets,
            "open",
            "(Ljava/lang/String;)Ljava/io/InputStream;",
            &[JValue::Object(&key)],
        )
        .map_err(|_| ())?;
        let stream = env.auto_local(stream);
        let array = env.new_byte_array(8192).map_err(|_| ())?;
        let array = env.auto_local(array);
        let mut hasher = Sha256::new();
        let mut total = 0_u64;
        let copied = (|| {
            loop {
                let count = env
                    .call_method(&stream, "read", "([B)I", &[JValue::Object(&array)])
                    .map_err(|_| ())?
                    .i()
                    .map_err(|_| ())?;
                if count == -1 {
                    break;
                }
                if !(1..=8192).contains(&count) {
                    return Err(());
                }
                total = total.checked_add(count as u64).ok_or(())?;
                if total > maximum {
                    return Err(());
                }
                let bytes = env.convert_byte_array(&array).map_err(|_| ())?;
                let slice = &bytes[..count as usize];
                file.write_all(slice).map_err(|_| ())?;
                hasher.update(slice);
            }
            if total == 0
                || length.is_some_and(|n| n != total)
                || <[u8; 32]>::from(hasher.finalize()) != sha
            {
                return Err(());
            }
            Ok(())
        })();
        let closed = env
            .call_method(&stream, "close", "()V", &[])
            .map_err(|_| ());
        copied?;
        closed?;
        file.set_permissions(std::fs::Permissions::from_mode(0o444))
            .map_err(|_| ())?;
        file.sync_all().map_err(|_| ())?;
        package.recheck()?;
        recheck_directories(directories)?;
        // A create-only public original is never replaced. Incomplete/uncertain prior copies
        // remain explicit recovery failures; no key, journal or financial operation is recreated.
        let parent_owner = directories.iter().find(|d| d.path == parent).ok_or(())?;
        std::fs::hard_link(&temp, &target).map_err(|_| ())?;
        parent_owner.sync()?;
        std::fs::remove_file(&temp).map_err(|_| ())?;
        parent_owner.sync()?;
        check_public_file(&target, sha, maximum, length)?;
        recheck_directories(directories)?;
        package.recheck()
    }
    fn check_public_file(
        path: &Path,
        sha: [u8; 32],
        maximum: u64,
        length: Option<u64>,
    ) -> Result<()> {
        if path.canonicalize().map_err(|_| ())? != path {
            return Err(());
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| ())?;
        let before = file.metadata().map_err(|_| ())?;
        if !before.is_file()
            || before.uid() != unsafe { libc::geteuid() }
            || before.nlink() != 1
            || before.mode() & 0o777 != 0o444
            || before.len() == 0
            || before.len() > maximum
            || length.is_some_and(|n| n != before.len())
        {
            return Err(());
        }
        let mut digest = Sha256::new();
        let mut reader = &file;
        let mut chunk = [0_u8; 8192];
        let mut total = 0_u64;
        loop {
            let n = reader.read(&mut chunk).map_err(|_| ())?;
            if n == 0 {
                break;
            }
            total = total.checked_add(n as u64).ok_or(())?;
            if total > maximum {
                return Err(());
            }
            digest.update(&chunk[..n]);
        }
        let held = file.metadata().map_err(|_| ())?;
        let named = std::fs::symlink_metadata(path).map_err(|_| ())?;
        let identity = |m: &std::fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.uid(),
                m.gid(),
                m.mode(),
                m.nlink(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        if total != before.len()
            || <[u8; 32]>::from(digest.finalize()) != sha
            || !named.is_file()
            || named.file_type().is_symlink()
            || identity(&before) != identity(&held)
            || identity(&before) != identity(&named)
        {
            return Err(());
        }
        Ok(())
    }
}

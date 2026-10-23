//! Genuine signed journals keep fresh admission while sharing their original parent ancestry.

use super::super::count_plan_read_work;
use super::*;
use crate::operations::preparation::count_request_verifications;
use iroha_fs::{CustodyWorkV1, PrivateDirectory, PublishMode};
use std::{ffi::OsStr, fs};

struct SignedJournals {
    parent: PrivateDirectory,
    service: AccountService,
    transport: Arc<Transport>,
    configure: StreamTokenCustodyConfigureRequest,
    enroll: StreamTokenCustodyEnrollRequest,
    // Drop retained native owners before removing their temporary directory on Windows.
    _root: tempfile::TempDir,
}
impl SignedJournals {
    fn new() -> Self {
        Self::nested(0)
    }
    fn nested(depth: usize) -> Self {
        let (service, transport) = service();
        let now = current_unix_ms().unwrap();
        let configure = configure(&service.config, now);
        let enroll = enroll(&service.config, now);
        let root = tempfile::tempdir().unwrap();
        let mut parent = PrivateDirectory::open_or_create(root.path().join("parent")).unwrap();
        for index in 0..depth {
            parent = parent.create_child(format!("ancestor-{index}")).unwrap();
        }
        service
            .prepare_stream_token_custody_configure(&configure, &parent.path().join("configure"))
            .unwrap();
        service
            .prepare_stream_token_custody_enroll(&enroll, &parent.path().join("enroll"))
            .unwrap();
        Self {
            parent,
            service,
            transport,
            configure,
            enroll,
            _root: root,
        }
    }
    fn name(enroll: bool) -> &'static OsStr {
        OsStr::new(if enroll { "enroll" } else { "configure" })
    }
    fn verify(&self, enroll: bool) -> Result<SignedTransaction> {
        self.verify_at(enroll, Self::name(enroll))
    }
    fn verify_at(&self, enroll: bool, name: &OsStr) -> Result<SignedTransaction> {
        if enroll {
            self.service.verify_stream_token_custody_enroll_journal(
                &self.parent,
                name,
                &self.enroll,
            )
        } else {
            self.service.verify_stream_token_custody_configure_journal(
                &self.parent,
                name,
                &self.configure,
            )
        }
    }
    // Test-only control for the old absolute acquisition, through the same canonical inspector.
    fn absolute(&self, enroll: bool) -> Result<SignedTransaction> {
        let path = self.parent.path().join(Self::name(enroll));
        let preparation = if enroll {
            self.service
                .inspect_stream_token_custody_enroll_preparation(&path, &self.enroll)
        } else {
            self.service
                .inspect_stream_token_custody_configure_preparation(&path, &self.configure)
        }?;
        preparation.into_signed_transaction()
    }
    fn calls(&self) -> (usize, usize, usize) {
        (
            self.transport.requests.load(Ordering::SeqCst),
            self.transport.quotes.load(Ordering::SeqCst),
            self.transport.submissions.load(Ordering::SeqCst),
        )
    }
}

fn result(value: Result<SignedTransaction>) -> Result<Vec<u8>, String> {
    value
        .map(|value| value.encode_versioned())
        .map_err(|error| error.to_string())
}
fn files(path: &Path) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    let mut names: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let bytes = fs::read(path.join(&name)).unwrap();
            (name, bytes)
        })
        .collect()
}
fn measured<T>(read: impl FnOnce() -> T) -> (T, CustodyWorkV1) {
    let before = CustodyWorkV1::current_thread();
    let result = read();
    (result, CustodyWorkV1::current_thread().since(before))
}

#[test]
fn signed_parent_verification_keeps_exact_wire_fresh_plan_and_native_ancestry_work() {
    let fixture = SignedJournals::new();
    let calls = fixture.calls();
    for enroll in [false, true] {
        let path = fixture.parent.path().join(SignedJournals::name(enroll));
        let before = files(&path);
        let old = measured(|| {
            count_request_verifications(|| {
                count_plan_read_work(|| result(fixture.absolute(enroll)))
            })
        });
        for _ in 0..2 {
            let new = measured(|| {
                count_request_verifications(|| {
                    count_plan_read_work(|| result(fixture.verify(enroll)))
                })
            });
            assert_eq!(old.0, new.0);
            let ((wire, plan), verified) = &new.0;
            assert!(wire.is_ok());
            assert_eq!(*plan, (1, 1));
            assert_eq!(*verified, 1);
            #[cfg(unix)]
            {
                // The absolute control has one extra admission walk of its newly opened
                // parent. This counts validation work, not a measurement of peak live FDs.
                assert_eq!(old.1.ancestry_walks, new.1.ancestry_walks + 1);
                let depth = u64::try_from(fixture.parent.path().components().count()).unwrap();
                assert_eq!(old.1.ancestry_opens, new.1.ancestry_opens + depth);
            }
            let held = Journal::open(&path).unwrap();
            assert!(fixture.verify(enroll).is_err());
            drop(held);
            assert_eq!(result(fixture.verify(enroll)), *wire);
        }
        assert_eq!(files(&path), before);
    }
    assert_eq!(fixture.calls(), calls);
}

#[test]
fn signed_parent_verification_preserves_active_decoder_refusal_and_original_retry() {
    let fixture = SignedJournals::new();
    let calls = fixture.calls();
    for enroll in [false, true] {
        let path = fixture.parent.path().join(SignedJournals::name(enroll));
        let before = files(&path);
        let expected = result(fixture.verify(enroll));
        assert!(expected.is_ok());
        for allocation in [0, 1, 128 * 1024 * 1024] {
            let limits =
                || norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, allocation, 64);
            let old = norito::core::with_decode_limits_measured(limits(), || {
                result(fixture.absolute(enroll))
            });
            let new = norito::core::with_decode_limits_measured(limits(), || {
                result(fixture.verify(enroll))
            });
            assert_eq!(old, new, "enroll={enroll}, allocation={allocation}");
            assert_eq!(new.0.is_ok(), allocation > 1);
            assert_eq!(result(fixture.verify(enroll)), expected);
            assert_eq!(files(&path), before);
        }
    }
    assert_eq!(fixture.calls(), calls);
}

#[test]
fn signed_parent_verification_refuses_changed_request_records_and_lock_then_retries() {
    let fixture = SignedJournals::new();
    let calls = fixture.calls();
    for enroll in [false, true] {
        let name = SignedJournals::name(enroll);
        let path = fixture.parent.path().join(name);
        let before = files(&path);
        let expected = result(fixture.verify(enroll));
        assert!(expected.is_ok());
        let mismatched = if enroll {
            let mut changed = fixture.enroll.clone();
            changed.deadline_unix_ms += 1;
            fixture.service.verify_stream_token_custody_enroll_journal(
                &fixture.parent,
                name,
                &changed,
            )
        } else {
            let mut changed = fixture.configure.clone();
            changed.deadline_unix_ms += 1;
            fixture
                .service
                .verify_stream_token_custody_configure_journal(&fixture.parent, name, &changed)
        };
        assert!(mismatched.is_err());
        assert_eq!(result(fixture.verify(enroll)), expected);
        for name in ["absent", "../configure"] {
            assert!(fixture.verify_at(enroll, OsStr::new(name)).is_err());
        }
        assert!(!fixture.parent.path().join("absent").exists());

        let original_lock = path.join("lock");
        let parked_lock = fixture.parent.path().join("parked-lock");
        fs::rename(&original_lock, &parked_lock).unwrap();
        assert!(fixture.verify(enroll).is_err());
        assert!(!original_lock.exists());
        fs::rename(&parked_lock, &original_lock).unwrap();
        assert_eq!(result(fixture.verify(enroll)), expected);

        let directory = fixture.parent.open_child(name).unwrap();
        let request = fs::read(path.join("preparation.json")).unwrap();
        let payload = fs::read(path.join("payload.json")).unwrap();
        let original_text = std::str::from_utf8(&request).unwrap();
        let schema = "iroha.wallet.native-preparation.v1";
        assert_eq!(original_text.matches(schema).count(), 1);
        // Change only the schema string; retain the canonical DTO's original field order.
        let changed = original_text.replace(schema, "different-wallet-preparation");
        directory
            .write_atomic("preparation.json", changed.as_bytes(), PublishMode::Replace)
            .unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(path.join("payload.json"))
            .unwrap()
            .set_len(u64::try_from(iroha_operation_journal::MAX_JOURNAL_BYTES + 1).unwrap())
            .unwrap();
        let old = result(fixture.absolute(enroll));
        let new = result(fixture.verify(enroll));
        assert_eq!(old, new);
        assert_eq!(
            new.unwrap_err(),
            "preparation differs from the original wallet identity or finite authorization"
        );
        directory
            .write_atomic("preparation.json", &request, PublishMode::Replace)
            .unwrap();
        directory
            .write_atomic("payload.json", &payload, PublishMode::Replace)
            .unwrap();
        assert_eq!(result(fixture.verify(enroll)), expected);

        let operation = fs::read(path.join("operation.json")).unwrap();
        directory
            .write_atomic("operation.json", b"{}", PublishMode::Replace)
            .unwrap();
        assert!(fixture.verify(enroll).is_err());
        directory
            .write_atomic("operation.json", &operation, PublishMode::Replace)
            .unwrap();
        assert_eq!(result(fixture.verify(enroll)), expected);
        assert_eq!(files(&path), before);
        // Every ordinary refusal released the journal lock.
        drop(Journal::open(&path).unwrap());
    }
    assert_eq!(fixture.calls(), calls);
}

#[cfg(unix)]
#[test]
fn signed_parent_verification_refuses_same_bytes_replacement_and_restores_exact_owner() {
    let fixture = SignedJournals::new();
    let calls = fixture.calls();
    let wires = [false, true].map(|enroll| result(fixture.verify(enroll)).unwrap());
    let bytes = [false, true]
        .map(|enroll| files(&fixture.parent.path().join(SignedJournals::name(enroll))));
    let parent_identity = fixture.parent.identity().unwrap();
    let displaced = fixture._root.path().join("displaced-parent");
    fs::rename(fixture.parent.path(), &displaced).unwrap();
    for enroll in [false, true] {
        assert!(fixture.verify(enroll).is_err());
    }
    let replacement = PrivateDirectory::open_or_create(fixture.parent.path()).unwrap();
    assert_ne!(replacement.identity().unwrap(), parent_identity);
    for (enroll, records) in [false, true].into_iter().zip(&bytes) {
        let child = replacement
            .create_child(SignedJournals::name(enroll))
            .unwrap();
        for (name, bytes) in records {
            child
                .write_atomic(name, bytes, PublishMode::CreateNew)
                .unwrap();
        }
    }
    for (enroll, wire) in [false, true].into_iter().zip(&wires) {
        // The original absolute lookup accepts the genuinely signed identical copy. The
        // retained-parent API must refuse its changed native owner before decoding records.
        assert_eq!(result(fixture.absolute(enroll)).unwrap(), *wire);
        let (refused, verified) = count_request_verifications(|| fixture.verify(enroll));
        assert!(refused.is_err());
        assert_eq!(verified, 0);
    }
    drop(replacement);
    fs::remove_dir_all(fixture.parent.path()).unwrap();
    fs::rename(&displaced, fixture.parent.path()).unwrap();
    assert_eq!(fixture.parent.identity().unwrap(), parent_identity);
    for (enroll, wire) in [false, true].into_iter().zip(&wires) {
        assert_eq!(result(fixture.verify(enroll)).unwrap(), *wire);
    }
    assert_eq!(fixture.calls(), calls);
}

#[cfg(unix)]
#[test]
fn signed_parent_verification_fits_fixed_native_descriptor_budget() {
    const CHILD_MARKER: &str = "IROHA_WALLET_SIGNED_PARENT_DESCRIPTOR_TEST";
    const CHILD_PASSED: &str = "signed-parent fixed-descriptor regression completed";
    const SELECTOR: &str = "operations::stream_token_custody::tests::parent_journal_tests::signed_parent_verification_fits_fixed_native_descriptor_budget";
    if std::env::var_os(CHILD_MARKER).is_none() {
        // Follow the existing iroha_fs isolated descriptor-test pattern. Cap only this
        // child's soft limit, never increase it, and leave its inherited hard limit alone.
        // Executable and exact selector are arguments, never interpolated into shell code.
        let output = std::process::Command::new("/bin/sh")
            .args([
                "-c",
                "limit=$(ulimit -Sn) || exit 1; case \"$limit\" in unlimited) ulimit -Sn 256 || exit 1 ;; *[!0-9]*|'') exit 1 ;; *) if [ \"$limit\" -gt 256 ]; then ulimit -Sn 256 || exit 1; fi ;; esac; exec \"$1\" --exact \"$2\" --nocapture --test-threads=1",
                "iroha-wallet-signed-parent-test",
            ])
            .arg(std::env::current_exe().unwrap())
            .arg(SELECTOR)
            .env(CHILD_MARKER, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success()
                && String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .any(|line| line == CHILD_PASSED),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }

    let fixture = SignedJournals::nested(12);
    let calls = fixture.calls();
    let before = [false, true]
        .map(|enroll| files(&fixture.parent.path().join(SignedJournals::name(enroll))));
    let wires = [false, true].map(|enroll| result(fixture.verify(enroll)).unwrap());
    assert!(fixture.parent.path().components().count() > 8);
    let mut fillers = Vec::with_capacity(256);
    let mut exhausted = None;
    for _ in 0..256 {
        match fs::File::open("/dev/null") {
            Ok(file) => fillers.push(file),
            Err(error) => {
                exhausted = Some(error);
                break;
            }
        }
    }
    assert_eq!(
        exhausted
            .expect("the isolated fixed budget must actually exhaust")
            .raw_os_error(),
        Some(24),
    );
    assert!(fillers.len() > 8);
    for _ in 0..8 {
        drop(fillers.pop().unwrap());
    }
    for (enroll, wire) in [false, true].into_iter().zip(&wires) {
        let error = fixture.absolute(enroll).unwrap_err();
        assert_eq!(
            error.chain().find_map(|cause| {
                cause
                    .downcast_ref::<std::io::Error>()
                    .and_then(std::io::Error::raw_os_error)
            }),
            Some(24),
            "the original absolute verifier must refuse specifically on EMFILE: {error:#}",
        );
        // The same fillers remain live: only acquisition through the original parent
        // differs. Repeated calls still reacquire the child/lock and verify every record.
        for _ in 0..2 {
            let ((actual, plan), verified) = count_request_verifications(|| {
                count_plan_read_work(|| result(fixture.verify(enroll)))
            });
            assert_eq!(actual.unwrap(), *wire);
            assert_eq!(plan, (1, 1));
            assert_eq!(verified, 1);
        }
    }
    drop(fillers);
    for (enroll, records) in [false, true].into_iter().zip(&before) {
        let path = fixture.parent.path().join(SignedJournals::name(enroll));
        assert_eq!(files(&path), *records);
        drop(Journal::open(&path).unwrap());
    }
    assert_eq!(fixture.calls(), calls);
    let root = fixture._root.path().to_path_buf();
    drop(fixture);
    assert!(!root.exists());
    eprintln!("{CHILD_PASSED}");
}

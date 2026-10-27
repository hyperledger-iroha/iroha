//! Genuine request retirement shares original parent custody and keeps fresh locked checks.

use super::super::count_plan_read_work;
use super::*;
use crate::operations::preparation::count_request_verifications;
use iroha_fs::{PrivateDirectory, PublishMode};
use std::{ffi::OsStr, fs, path::PathBuf};

struct RequestJournals {
    parent: PrivateDirectory,
    service: AccountService,
    transport: Arc<Transport>,
    configure: StreamTokenCustodyConfigureRequest,
    enroll: StreamTokenCustodyEnrollRequest,
    // Release all original native owners before deleting their temporary root on Windows.
    _root: tempfile::TempDir,
}
impl RequestJournals {
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
            .retain_stream_token_custody_configure_request(
                &configure,
                &parent.path().join("configure"),
            )
            .unwrap();
        service
            .retain_stream_token_custody_enroll_request(&enroll, &parent.path().join("enroll"))
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
    fn path(&self, enroll: bool) -> PathBuf {
        self.parent.path().join(Self::name(enroll))
    }
    fn retire(&self, enroll: bool) -> Result<RetiredNativeRequest> {
        self.retire_at(enroll, Self::name(enroll))
    }
    fn retire_at(&self, enroll: bool, name: &OsStr) -> Result<RetiredNativeRequest> {
        if enroll {
            self.service.retire_stream_token_custody_enroll_unprepared(
                &self.parent,
                name,
                &self.enroll,
            )
        } else {
            self.service
                .retire_stream_token_custody_configure_unprepared(
                    &self.parent,
                    name,
                    &self.configure,
                )
        }
    }
    // The same canonical retirement body with its former absolute acquisition, test-only.
    fn absolute(&self, enroll: bool) -> Result<RetiredNativeRequest> {
        let (kind, expected) = if enroll {
            (
                NativeOperationKind::StreamTokenCustodyEnroll,
                CustodyExpectation::Enroll(&self.enroll),
            )
        } else {
            (
                NativeOperationKind::StreamTokenCustodyConfigure,
                CustodyExpectation::Configure(&self.configure),
            )
        };
        self.service.retire_preparation(
            &self.path(enroll),
            kind,
            OperationExpectation::Custody(expected),
        )
    }
    fn prepare(&self, enroll: bool) {
        if enroll {
            self.service
                .prepare_stream_token_custody_enroll(&self.enroll, &self.path(enroll))
                .unwrap();
        } else {
            self.service
                .prepare_stream_token_custody_configure(&self.configure, &self.path(enroll))
                .unwrap();
        }
    }
    fn calls(&self) -> (usize, usize, usize) {
        (
            self.transport.requests.load(Ordering::SeqCst),
            self.transport.quotes.load(Ordering::SeqCst),
            self.transport.submissions.load(Ordering::SeqCst),
        )
    }
}
fn result(value: Result<RetiredNativeRequest>) -> Result<(PathBuf, String), String> {
    value
        .map(|receipt| {
            (
                receipt.journal_path().to_owned(),
                receipt.request_sha256().to_owned(),
            )
        })
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

#[test]
fn parent_retirement_preserves_original_receipt_fresh_checks_and_released_lock() {
    let fixture = RequestJournals::new();
    let calls = fixture.calls();
    for enroll in [false, true] {
        let path = fixture.path(enroll);
        let before = files(&path);
        assert!(!path.join("retired.json").exists());
        let ((receipt, plan), verified) =
            count_request_verifications(|| count_plan_read_work(|| result(fixture.retire(enroll))));
        let receipt = receipt.unwrap();
        assert_eq!(receipt.0, path);
        assert_eq!(verified, 1);
        assert_eq!(plan, (1, 1));
        let after = files(&path);
        assert_eq!(after.len(), before.len() + 1);
        for record in &before {
            assert!(after.contains(record));
        }
        assert!(path.join("retired.json").exists());
        for _ in 0..2 {
            let old = count_request_verifications(|| {
                count_plan_read_work(|| result(fixture.absolute(enroll)))
            });
            let new = count_request_verifications(|| {
                count_plan_read_work(|| result(fixture.retire(enroll)))
            });
            assert_eq!(old, new);
            assert_eq!(new.0.0.as_ref().unwrap(), &receipt);
            assert_eq!(new.0.1, (1, 1));
            assert_eq!(new.1, 1);
            let held = Journal::open(&path).unwrap();
            assert!(fixture.retire(enroll).is_err());
            drop(held);
            assert_eq!(result(fixture.retire(enroll)).unwrap(), receipt);
        }
        assert_eq!(files(&path), after);
        let directory = fixture
            .parent
            .open_child(RequestJournals::name(enroll))
            .unwrap();
        let retirement = fs::read(path.join("retired.json")).unwrap();
        let broken = std::str::from_utf8(&retirement).unwrap().replacen(
            "iroha.wallet.native-retirement.v1",
            "iroha.wallet.native-retirement.v0",
            1,
        );
        assert_ne!(broken.as_bytes(), retirement);
        directory
            .write_atomic("retired.json", broken.as_bytes(), PublishMode::Replace)
            .unwrap();
        let refused = result(fixture.retire(enroll));
        assert!(
            refused
                .as_ref()
                .unwrap_err()
                .contains("retirement conflicts with native preparation history")
        );
        assert_eq!(refused, result(fixture.absolute(enroll)));
        // Original record authentication still precedes an ordinary selected-request error.
        let mismatched = if enroll {
            let mut request = fixture.enroll.clone();
            request.deadline_unix_ms += 1;
            fixture
                .service
                .retire_stream_token_custody_enroll_unprepared(
                    &fixture.parent,
                    RequestJournals::name(enroll),
                    &request,
                )
        } else {
            let mut request = fixture.configure.clone();
            request.deadline_unix_ms += 1;
            fixture
                .service
                .retire_stream_token_custody_configure_unprepared(
                    &fixture.parent,
                    RequestJournals::name(enroll),
                    &request,
                )
        };
        assert_eq!(result(mismatched), refused);
        directory
            .write_atomic("retired.json", &retirement, PublishMode::Replace)
            .unwrap();
        assert_eq!(result(fixture.retire(enroll)).unwrap(), receipt);
        assert_eq!(files(&path), after);
    }
    assert_eq!(fixture.calls(), calls);
}

#[test]
fn parent_retirement_preserves_decoder_refusals_and_original_retry() {
    let fixture = RequestJournals::new();
    let calls = fixture.calls();
    for enroll in [false, true] {
        let path = fixture.path(enroll);
        let before = files(&path);
        for allocation in [0, 1] {
            let limits =
                || norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, allocation, 64);
            let old = norito::core::with_decode_limits_measured(limits(), || {
                result(fixture.absolute(enroll))
            });
            let new = norito::core::with_decode_limits_measured(limits(), || {
                result(fixture.retire(enroll))
            });
            assert_eq!(old, new);
            assert!(new.0.is_err());
            assert_eq!(files(&path), before);
        }
        let expected = result(fixture.retire(enroll)).unwrap();
        let after = files(&path);
        for allocation in [0, 1, 128 * 1024 * 1024] {
            let limits =
                || norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, allocation, 64);
            let old = norito::core::with_decode_limits_measured(limits(), || {
                result(fixture.absolute(enroll))
            });
            let new = norito::core::with_decode_limits_measured(limits(), || {
                result(fixture.retire(enroll))
            });
            assert_eq!(old, new);
            assert_eq!(new.0.is_ok(), allocation > 1);
            assert_eq!(result(fixture.retire(enroll)).unwrap(), expected);
            assert_eq!(files(&path), after);
        }
    }
    assert_eq!(fixture.calls(), calls);
}

#[test]
fn parent_retirement_refuses_changed_request_inventory_and_lock_before_publication() {
    let fixture = RequestJournals::new();
    let calls = fixture.calls();
    for enroll in [false, true] {
        let path = fixture.path(enroll);
        let before = files(&path);
        let changed = if enroll {
            let mut request = fixture.enroll.clone();
            request.deadline_unix_ms += 1;
            fixture
                .service
                .retire_stream_token_custody_enroll_unprepared(
                    &fixture.parent,
                    RequestJournals::name(enroll),
                    &request,
                )
        } else {
            let mut request = fixture.configure.clone();
            request.deadline_unix_ms += 1;
            fixture
                .service
                .retire_stream_token_custody_configure_unprepared(
                    &fixture.parent,
                    RequestJournals::name(enroll),
                    &request,
                )
        };
        assert!(changed.is_err());
        for name in ["absent", "../configure"] {
            assert!(fixture.retire_at(enroll, OsStr::new(name)).is_err());
        }
        assert!(!fixture.parent.path().join("absent").exists());
        assert_eq!(files(&path), before);
        let held = Journal::open(&path).unwrap();
        let (refused, verified) = count_request_verifications(|| fixture.retire(enroll));
        assert!(refused.is_err());
        assert_eq!(verified, 0);
        drop(held);
        let lock = path.join("lock");
        let parked = fixture.parent.path().join("parked-lock");
        fs::rename(&lock, &parked).unwrap();
        assert!(fixture.retire(enroll).is_err());
        assert!(!lock.exists());
        fs::rename(&parked, &lock).unwrap();
        let directory = fixture
            .parent
            .open_child(RequestJournals::name(enroll))
            .unwrap();
        let request = fs::read(path.join("preparation.json")).unwrap();
        let text = std::str::from_utf8(&request).unwrap();
        let broken = text.replacen(
            "iroha.wallet.native-preparation.v1",
            "iroha.wallet.native-preparation.v0",
            1,
        );
        assert_ne!(broken.as_bytes(), request);
        directory
            .write_atomic("preparation.json", broken.as_bytes(), PublishMode::Replace)
            .unwrap();
        assert_eq!(
            result(fixture.retire(enroll)),
            result(fixture.absolute(enroll))
        );
        assert!(fixture.retire(enroll).is_err());
        assert!(!path.join("retired.json").exists());
        directory
            .write_atomic("preparation.json", &request, PublishMode::Replace)
            .unwrap();
        for name in ["unexpected.json", "submission.json"] {
            directory
                .write_atomic(name, b"{}", PublishMode::CreateNew)
                .unwrap();
            let mutated = files(&path);
            let old = result(fixture.absolute(enroll));
            let new = result(fixture.retire(enroll));
            assert!(new.is_err());
            assert_eq!(old, new);
            assert_eq!(files(&path), mutated);
            assert!(!path.join("retired.json").exists());
            directory.remove_private(name).unwrap();
        }
        assert_eq!(files(&path), before);
        let receipt = result(fixture.retire(enroll)).unwrap();
        assert_eq!(receipt.0, path);
        assert_eq!(result(fixture.retire(enroll)).unwrap(), receipt);
        drop(Journal::open(&path).unwrap());
    }
    assert_eq!(fixture.calls(), calls);
}

#[test]
fn parent_retirement_refuses_genuine_payload_and_signed_journals_without_changes() {
    let fixture = RequestJournals::new();
    for enroll in [false, true] {
        fixture.prepare(enroll);
        let calls = fixture.calls();
        let path = fixture.path(enroll);
        let before = files(&path);
        let signed = result(fixture.retire(enroll));
        assert!(
            signed
                .as_ref()
                .unwrap_err()
                .contains("only an exact RequestOnly preparation can retire")
        );
        assert_eq!(signed, result(fixture.absolute(enroll)));
        assert_eq!(files(&path), before);
        let directory = fixture
            .parent
            .open_child(RequestJournals::name(enroll))
            .unwrap();
        let operation = fs::read(path.join("operation.json")).unwrap();
        directory.remove_private("operation.json").unwrap();
        let payload = files(&path);
        let refused = result(fixture.retire(enroll));
        assert!(
            refused
                .as_ref()
                .unwrap_err()
                .contains("only an exact RequestOnly preparation can retire")
        );
        assert_eq!(refused, result(fixture.absolute(enroll)));
        assert_eq!(files(&path), payload);
        assert!(!path.join("retired.json").exists());
        directory
            .write_atomic("operation.json", &operation, PublishMode::CreateNew)
            .unwrap();
        assert_eq!(result(fixture.retire(enroll)), signed);
        assert_eq!(files(&path), before);
        drop(Journal::open(&path).unwrap());
        assert_eq!(fixture.calls(), calls);
    }
}

#[cfg(unix)]
#[test]
fn parent_retirement_refuses_equal_bytes_replacement_and_retries_original_owner() {
    let fixture = RequestJournals::new();
    let calls = fixture.calls();
    let before = [false, true].map(|enroll| files(&fixture.path(enroll)));
    let identity = fixture.parent.identity().unwrap();
    let displaced = fixture._root.path().join("displaced-parent");
    fs::rename(fixture.parent.path(), &displaced).unwrap();
    let replacement = PrivateDirectory::open_or_create(fixture.parent.path()).unwrap();
    assert_ne!(replacement.identity().unwrap(), identity);
    for (enroll, records) in [false, true].into_iter().zip(&before) {
        let child = replacement
            .create_child(RequestJournals::name(enroll))
            .unwrap();
        for (name, bytes) in records {
            child
                .write_atomic(name, bytes, PublishMode::CreateNew)
                .unwrap();
        }
        // The absolute control accepts and retires the byte-identical replacement. The
        // original parent's authority must instead refuse before decoding or publishing.
        assert!(fixture.absolute(enroll).is_ok());
        let copied = files(&fixture.path(enroll));
        let (refused, verified) = count_request_verifications(|| fixture.retire(enroll));
        assert!(refused.is_err());
        assert_eq!(verified, 0);
        assert_eq!(files(&fixture.path(enroll)), copied);
        assert_eq!(
            files(&displaced.join(RequestJournals::name(enroll))),
            *records
        );
    }
    drop(replacement);
    fs::remove_dir_all(fixture.parent.path()).unwrap();
    fs::rename(&displaced, fixture.parent.path()).unwrap();
    assert_eq!(fixture.parent.identity().unwrap(), identity);
    for enroll in [false, true] {
        assert!(fixture.retire(enroll).is_ok());
    }
    assert_eq!(fixture.calls(), calls);
}

#[cfg(unix)]
#[test]
fn parent_retirement_fits_fixed_native_descriptor_budget() {
    const CHILD_MARKER: &str = "IROHA_WALLET_PARENT_RETIREMENT_DESCRIPTOR_TEST";
    const CHILD_PASSED: &str = "parent-retirement fixed-descriptor regression completed";
    const SELECTOR: &str = "operations::stream_token_custody::tests::parent_retirement_tests::parent_retirement_fits_fixed_native_descriptor_budget";
    if std::env::var_os(CHILD_MARKER).is_none() {
        // Lower only the isolated child's soft limit when needed. Never raise an inherited
        // limit or alter the parent/hard limit. Executable and selector are separate arguments.
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", "limit=$(ulimit -Sn) || exit 1; case \"$limit\" in unlimited) ulimit -Sn 256 || exit 1 ;; *[!0-9]*|'') exit 1 ;; *) if [ \"$limit\" -gt 256 ]; then ulimit -Sn 256 || exit 1; fi ;; esac; exec \"$1\" --exact \"$2\" --nocapture --test-threads=1", "iroha-wallet-parent-retirement-test"])
            .arg(std::env::current_exe().unwrap()).arg(SELECTOR).env(CHILD_MARKER, "1").output().unwrap();
        assert!(
            output.status.success()
                && String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .any(|line| line == CHILD_PASSED),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let fixture = RequestJournals::nested(12);
    let calls = fixture.calls();
    let before = [false, true].map(|enroll| files(&fixture.path(enroll)));
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
            .expect("fixed budget must actually exhaust")
            .raw_os_error(),
        Some(24)
    );
    assert!(fillers.len() > 8);
    for _ in 0..8 {
        drop(fillers.pop().unwrap());
    }
    let mut receipts = Vec::new();
    for enroll in [false, true] {
        let error = fixture.absolute(enroll).unwrap_err();
        assert_eq!(
            error.chain().find_map(|cause| cause
                .downcast_ref::<std::io::Error>()
                .and_then(std::io::Error::raw_os_error)),
            Some(24),
            "original absolute retirement must fail on EMFILE: {error:#}"
        );
        // Fillers remain held through real publication and repeated authentication. The
        // new path freshly owns the child/lock and executes the original RequestOnly recipe.
        let ((receipt, plan), verified) =
            count_request_verifications(|| count_plan_read_work(|| result(fixture.retire(enroll))));
        let receipt = receipt.unwrap();
        assert_eq!(receipt.0, fixture.path(enroll));
        assert_eq!(plan, (1, 1));
        assert_eq!(verified, 1);
        for _ in 0..2 {
            let ((repeated, plan), verified) = count_request_verifications(|| {
                count_plan_read_work(|| result(fixture.retire(enroll)))
            });
            assert_eq!(repeated.unwrap(), receipt);
            assert_eq!(plan, (1, 1));
            assert_eq!(verified, 1);
        }
        receipts.push(receipt);
    }
    drop(fillers);
    for ((enroll, records), receipt) in [false, true].into_iter().zip(&before).zip(&receipts) {
        let path = fixture.path(enroll);
        let after = files(&path);
        assert_eq!(after.len(), records.len() + 1);
        for record in records {
            assert!(after.contains(record));
        }
        assert!(path.join("retired.json").exists());
        assert_eq!(&result(fixture.absolute(enroll)).unwrap(), receipt);
        drop(Journal::open(&path).unwrap());
    }
    assert_eq!(fixture.calls(), calls);
    let root = fixture._root.path().to_path_buf();
    drop(fixture);
    assert!(!root.exists());
    eprintln!("{CHILD_PASSED}");
}

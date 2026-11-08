//! Test-only interruption after an actual provider release and before owner publication.
//!
//! This observer never creates or changes monetary files. It only reads marker originals
//! written by the provider and arms the public SimFs crash control at a real sync boundary.
//! This module is part of the separate engineering E2E draft, outside the Native aggregate.

use iroha_core_zk::kagemusha_wallet_advance_v1::{
    KagemushaWalletCustodyDirV1 as Dir, KagemushaWalletFsV1 as Fs,
    KagemushaWalletListedEntryV1 as Entry, KagemushaWalletMarkerPhaseV1 as Phase,
    KagemushaWalletMarkerRecordV1 as Marker, KagemushaWalletSimFaultV1 as Fault,
    KagemushaWalletSimFsV1 as SimFs, KagemushaWalletSlotIdV1 as Slot,
};
use std::{
    io,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct Boundary {
    expected: Option<([u8; 32], u128)>,
    retirement: Option<(Dir, Marker)>,
    fired: Option<(u64, Marker)>,
}

/// Fresh simulated filesystem with one optional exact release-retirement crash.
#[derive(Clone, Default)]
pub(super) struct ReleaseFaultFs {
    inner: SimFs,
    boundary: Arc<Mutex<Boundary>>,
}

impl ReleaseFaultFs {
    /// Arm only after Begin finality, immediately before executing the existing review.
    pub(super) fn arm(&self, scheme: [u8; 32], next_sequence: u128) {
        let mut boundary = self.boundary.lock().unwrap();
        assert!(boundary.expected.is_none());
        assert!(boundary.retirement.is_none());
        assert!(boundary.fired.is_none());
        boundary.expected = Some((scheme, next_sequence));
    }

    /// The actual marker and step observed at the injected crash; no fabricated verdict.
    pub(super) fn fired(&self) -> Option<(u64, Marker)> {
        self.boundary.lock().unwrap().fired.clone()
    }

    /// Public simulator controls for crash verification and power loss/restart.
    pub(super) fn simulation(&self) -> &SimFs {
        &self.inner
    }

    fn retiring_release(&self, dir: &Dir, name: &str) -> Option<Marker> {
        let (scheme, sequence) = self.boundary.lock().unwrap().expected?;
        let [slots, slot, markers] = dir.components() else {
            return None;
        };
        if slots != "slots" || markers != "markers" {
            return None;
        }
        let slot = Slot::parse(slot)?;
        let bytes = self.inner.visible_file(dir, name)?;
        let selected = Marker::decode(&bytes, &slot, &scheme).ok()?;
        if selected.phase() != Phase::Selected || selected.head()?.0 != sequence {
            return None;
        }
        // A11 retires Selected only after completion and Released are already durable.
        // Require that exact relationship from originals, never a guessed generation/name.
        self.inner.visible_names(dir).into_iter().find_map(|other| {
            let bytes = self.inner.visible_file(dir, &other)?;
            let released = Marker::decode(&bytes, &slot, &scheme).ok()?;
            (released.phase() == Phase::Released
                && released.head() == selected.head()
                && released.selected_generation() == Some(selected.generation())
                && released.generation() > selected.generation()
                && released.completion_digest().is_some())
            .then_some(released)
        })
    }
}

impl Fs for ReleaseFaultFs {
    type StagedFile = <SimFs as Fs>::StagedFile;
    type Lock = <SimFs as Fs>::Lock;

    fn create_new(&self, dir: &Dir, name: &str) -> io::Result<Self::StagedFile> {
        self.inner.create_new(dir, name)
    }
    fn write_all(&self, file: &mut Self::StagedFile, bytes: &[u8]) -> io::Result<()> {
        self.inner.write_all(file, bytes)
    }
    fn sync_staged(&self, file: &Self::StagedFile) -> io::Result<()> {
        self.inner.sync_staged(file)
    }
    fn sync_named(&self, dir: &Dir, name: &str) -> io::Result<()> {
        self.inner.sync_named(dir, name)
    }
    fn rename_noreplace(&self, dir: &Dir, from: &str, to: &str) -> io::Result<()> {
        self.inner.rename_noreplace(dir, from, to)
    }
    fn rename_replace(&self, dir: &Dir, from: &str, to: &str) -> io::Result<()> {
        self.inner.rename_replace(dir, from, to)
    }
    fn rename_entry_noreplace(&self, dir: &Dir, from: &str, to: &str) -> io::Result<()> {
        self.inner.rename_entry_noreplace(dir, from, to)
    }
    fn unlink(&self, dir: &Dir, name: &str) -> io::Result<()> {
        let released = self.retiring_release(dir, name);
        self.inner.unlink(dir, name)?;
        if let Some(released) = released {
            let mut boundary = self.boundary.lock().unwrap();
            assert!(boundary.retirement.is_none());
            boundary.retirement = Some((dir.clone(), released));
        }
        Ok(())
    }
    fn sync_dir(&self, dir: &Dir) -> io::Result<()> {
        let crash = {
            let mut boundary = self.boundary.lock().unwrap();
            if boundary
                .retirement
                .as_ref()
                .is_some_and(|(retirement_dir, _)| retirement_dir == dir)
            {
                let (_, released) = boundary.retirement.take().unwrap();
                let step = self.inner.steps();
                boundary.fired = Some((step, released));
                boundary.expected = None;
                Some(step)
            } else {
                None
            }
        };
        if let Some(step) = crash {
            self.inner.inject(step, Fault::CrashAfter);
        }
        // CrashAfter performs this real sync, then makes every further filesystem step fail.
        // The same call cannot reach coordinator sync_manifest/StepEntry publication.
        self.inner.sync_dir(dir)
    }
    fn mkdir(&self, parent: &Dir, name: &str) -> io::Result<()> {
        self.inner.mkdir(parent, name)
    }
    fn remove_dir(&self, parent: &Dir, name: &str) -> io::Result<()> {
        self.inner.remove_dir(parent, name)
    }
    fn read(&self, dir: &Dir, name: &str, limit: usize) -> io::Result<Vec<u8>> {
        self.inner.read(dir, name, limit)
    }
    fn list(&self, dir: &Dir) -> io::Result<Vec<Entry>> {
        self.inner.list(dir)
    }
    fn available_bytes(&self) -> io::Result<u64> {
        self.inner.available_bytes()
    }
    fn try_lock(&self) -> io::Result<Self::Lock> {
        self.inner.try_lock()
    }
    fn staging_name(&self) -> io::Result<String> {
        self.inner.staging_name()
    }
}

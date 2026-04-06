//! Shared helpers for resolving Torii data directory overrides.

use std::{
    path::{Path, PathBuf},
    sync::{Condvar, Mutex, OnceLock, RwLock},
    thread::ThreadId,
};

use iroha_config::parameters::defaults;

fn override_slot() -> &'static Mutex<Option<PathBuf>> {
    static OVERRIDE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| Mutex::new(None))
}

fn base_dir_slot() -> &'static RwLock<PathBuf> {
    static BASE_DIR: OnceLock<RwLock<PathBuf>> = OnceLock::new();
    BASE_DIR.get_or_init(|| RwLock::new(defaults::torii::data_dir()))
}

struct ExclusiveState {
    owner: Option<OverrideOwner>,
    depth: usize,
}

impl ExclusiveState {
    fn new() -> Self {
        Self {
            owner: None,
            depth: 0,
        }
    }

    fn acquire(&mut self, owner: OverrideOwner) {
        match self.owner {
            Some(current_owner) if current_owner == owner => {
                self.depth += 1;
            }
            None => {
                self.owner = Some(owner);
                self.depth = 1;
            }
            Some(_) => unreachable!("exclusive state acquire should wait before retrying"),
        }
    }

    fn try_release(&mut self, owner: OverrideOwner) -> bool {
        let current_owner = self.owner.expect("exclusive lock owned when releasing");
        assert_eq!(
            current_owner, owner,
            "override guard released by non-owner task/thread"
        );
        self.depth -= 1;
        if self.depth == 0 {
            self.owner = None;
            return true;
        }
        false
    }
}

fn exclusive_state() -> &'static (Mutex<ExclusiveState>, Condvar) {
    static STATE: OnceLock<(Mutex<ExclusiveState>, Condvar)> = OnceLock::new();
    STATE.get_or_init(|| (Mutex::new(ExclusiveState::new()), Condvar::new()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverrideOwner {
    Task(tokio::task::Id),
    Thread(ThreadId),
}

impl OverrideOwner {
    fn current() -> Self {
        tokio::task::try_id().map_or_else(|| Self::Thread(std::thread::current().id()), Self::Task)
    }
}

#[must_use]
pub struct OverrideGuard {
    previous: Option<PathBuf>,
    owner: OverrideOwner,
}

impl OverrideGuard {
    pub fn new(path: &Path) -> Self {
        let owner = OverrideOwner::current();
        let (lock, cvar) = exclusive_state();
        let mut state = lock
            .lock()
            .expect("failed to acquire override exclusivity lock");
        while let Some(current_owner) = state.owner {
            if current_owner == owner {
                break;
            }
            state = cvar
                .wait(state)
                .expect("failed waiting on override exclusivity");
        }
        state.acquire(owner);
        drop(state);

        let mut guard = override_slot()
            .lock()
            .expect("failed to acquire data dir override lock");
        let previous = guard.replace(path.to_path_buf());

        Self { previous, owner }
    }
}

impl Drop for OverrideGuard {
    fn drop(&mut self) {
        let mut guard = override_slot()
            .lock()
            .expect("failed to acquire data dir override lock");
        *guard = self.previous.take();
        drop(guard);

        let (lock, cvar) = exclusive_state();
        let mut state = lock
            .lock()
            .expect("failed to reacquire override exclusivity lock");
        if state.try_release(self.owner) {
            cvar.notify_one();
        }
    }
}

pub fn current_override() -> Option<PathBuf> {
    override_slot()
        .lock()
        .expect("failed to acquire data dir override lock")
        .clone()
}

pub fn set_base_dir(path: PathBuf) {
    *base_dir_slot()
        .write()
        .expect("failed to acquire base dir lock") = path;
}

pub fn base_dir() -> PathBuf {
    if let Some(dir) = current_override() {
        return dir;
    }
    base_dir_slot()
        .read()
        .expect("failed to acquire base dir lock")
        .clone()
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        sync::{Mutex, MutexGuard, OnceLock},
    };

    use super::*;

    fn clear_override() {
        let owner = OverrideOwner::current();
        let (lock, cvar) = exclusive_state();
        let mut state = lock
            .lock()
            .expect("failed to acquire override exclusivity lock");
        while let Some(current_owner) = state.owner {
            if current_owner == owner {
                break;
            }
            state = cvar
                .wait(state)
                .expect("failed waiting on override exclusivity");
        }
        state.acquire(owner);
        drop(state);

        override_slot()
            .lock()
            .expect("failed to acquire data dir override lock")
            .take();

        let mut state = lock
            .lock()
            .expect("failed to reacquire override exclusivity lock");
        if state.try_release(owner) {
            cvar.notify_one();
        }
    }

    #[test]
    fn override_visible_while_guard_active() {
        clear_override();
        let dir = tempfile::tempdir().expect("temp dir");
        let expected = dir.path().to_path_buf();

        {
            let _guard = OverrideGuard::new(dir.path());
            assert_eq!(current_override(), Some(expected.clone()));
        }

        assert!(current_override().is_none());
    }

    #[test]
    fn override_restores_previous_value() {
        clear_override();
        let dir_one = tempfile::tempdir().expect("temp dir");
        let dir_two = tempfile::tempdir().expect("temp dir");

        let guard_one = OverrideGuard::new(dir_one.path());
        assert_eq!(current_override(), Some(dir_one.path().to_path_buf()));

        {
            let _guard_two = OverrideGuard::new(dir_two.path());
            assert_eq!(current_override(), Some(dir_two.path().to_path_buf()));
        }

        assert_eq!(current_override(), Some(dir_one.path().to_path_buf()));
        drop(guard_one);
        assert!(current_override().is_none());
    }

    struct BaseDirResetGuard {
        _lock: MutexGuard<'static, ()>,
        previous: PathBuf,
    }

    fn base_dir_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    impl BaseDirResetGuard {
        fn new(path: &Path) -> Self {
            let lock = base_dir_test_lock()
                .lock()
                .expect("failed to acquire base dir test lock");
            let previous = base_dir();
            set_base_dir(path.to_path_buf());
            Self {
                _lock: lock,
                previous,
            }
        }
    }

    impl Drop for BaseDirResetGuard {
        fn drop(&mut self) {
            set_base_dir(self.previous.clone());
            clear_override();
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn base_dir_uses_config_and_ignores_env_override() {
        clear_override();
        let temp = tempfile::tempdir().expect("temp dir");
        let _guard = BaseDirResetGuard::new(temp.path());

        // SAFETY: adjusting env vars is confined to this test process.
        unsafe { std::env::set_var("IROHA_TORII_DATA_DIR", temp.path().join("ignored")) };
        assert_eq!(base_dir(), temp.path());
        // SAFETY: resetting the env var after the test keeps global state clean.
        unsafe { std::env::remove_var("IROHA_TORII_DATA_DIR") };
    }

    #[test]
    fn override_guard_takes_precedence_over_base_dir() {
        clear_override();
        let base = tempfile::tempdir().expect("temp dir");
        let override_dir = tempfile::tempdir().expect("temp dir");
        let _guard = BaseDirResetGuard::new(base.path());

        {
            let _override_guard = OverrideGuard::new(override_dir.path());
            assert_eq!(base_dir(), override_dir.path());
        }

        assert_eq!(base_dir(), base.path());
    }
}

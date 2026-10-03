//! Private source-owned protected-key publication and stable account correlation.
//! These states create no custody, signed observation, session permit or monetary authority.
use super::{Error, StartupRetirement};
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct ProtectedAccountPublication {
    committed: AtomicBool,
}
impl ProtectedAccountPublication {
    pub(super) const fn new(committed: bool) -> Self {
        Self {
            committed: AtomicBool::new(committed),
        }
    }
    pub(super) fn require_operational(&self, retirement: &StartupRetirement) -> Result<(), Error> {
        let generation = retirement.capture_original()?;
        if !self.committed.load(Ordering::Acquire) {
            return Err(Error::Rejected);
        }
        retirement.require_original(generation)
    }
    pub(super) fn require_phase(
        &self,
        retirement: &StartupRetirement,
        phase: u8,
    ) -> Result<(), Error> {
        match phase {
            1 | 2 | 6 => self.require_operational(retirement),
            // Cleanup remains available while the fixed Application loan/postcheck runs.
            3..=5 => Ok(()),
            _ => Err(Error::Rejected),
        }
    }
    pub(super) fn commit_original(
        &self,
        retirement: &StartupRetirement,
        generation: u64,
    ) -> Result<(), Error> {
        retirement.require_original(generation)?;
        self.committed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| Error::Rejected)?;
        if retirement.require_original(generation).is_err() {
            self.committed.store(false, Ordering::Release);
            return Err(Error::Rejected);
        }
        Ok(())
    }
}

struct Original<K> {
    identity: u64,
    retirement_generation: u64,
    wallet: K,
    signatory: K,
}
/// The first successful real registry-issued ID is lifetime correlation, not a read ticket.
/// Only actual immutable S/W and the unretired source can preserve it during renewal.
pub(super) struct StableAccountSelectionIdentity<K> {
    original: Option<Original<K>>,
}
impl<K: Eq + Clone> StableAccountSelectionIdentity<K> {
    pub(super) const fn new() -> Self {
        Self { original: None }
    }
    pub(super) fn require_same_accounts(
        &self,
        generation: u64,
        wallet: &K,
        signatory: &K,
    ) -> Result<(), Error> {
        if generation != 0 || wallet == signatory {
            return Err(Error::Rejected);
        }
        if let Some(original) = &self.original
            && (original.retirement_generation != generation
                || &original.wallet != wallet
                || &original.signatory != signatory)
        {
            return Err(Error::Rejected);
        }
        Ok(())
    }
    pub(super) fn complete_read(
        &mut self,
        current_handle: u64,
        generation: u64,
        wallet: &K,
        signatory: &K,
    ) -> Result<(), Error> {
        if current_handle == 0 {
            return Err(Error::Rejected);
        }
        self.require_same_accounts(generation, wallet, signatory)?;
        if self.original.is_none() {
            self.original = Some(Original {
                identity: current_handle,
                retirement_generation: generation,
                wallet: wallet.clone(),
                signatory: signatory.clone(),
            });
        }
        Ok(())
    }
    pub(super) fn original(
        &self,
        generation: u64,
        wallet: &K,
        signatory: &K,
    ) -> Result<u64, Error> {
        self.require_same_accounts(generation, wallet, signatory)?;
        Ok(self.original.as_ref().ok_or(Error::Rejected)?.identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_protected_key_blocks_acquire_begin_finish_and_authority_phases() {
        let publication = ProtectedAccountPublication::new(false);
        let retirement = StartupRetirement::new();
        // acquire_before_install, begin, finish and require_current use this exact gate.
        for _entry in ["acquire", "begin", "finish", "current"] {
            assert_eq!(
                publication.require_operational(&retirement),
                Err(Error::Rejected)
            );
        }
        for phase in [1, 2, 6] {
            assert_eq!(
                publication.require_phase(&retirement, phase),
                Err(Error::Rejected)
            );
        }
        for phase in [3, 4, 5] {
            assert_eq!(publication.require_phase(&retirement, phase), Ok(()));
        }
    }
    #[test]
    fn protected_postcheck_commits_once_under_exact_original_retirement() {
        let publication = ProtectedAccountPublication::new(false);
        let retirement = StartupRetirement::new();
        let original = retirement.capture_original().unwrap();
        assert_eq!(publication.commit_original(&retirement, original), Ok(()));
        for phase in [1, 2, 6] {
            assert_eq!(publication.require_phase(&retirement, phase), Ok(()));
        }
        assert_eq!(
            publication.commit_original(&retirement, original),
            Err(Error::Rejected)
        );
        retirement.retire().unwrap();
        assert_eq!(
            publication.require_operational(&retirement),
            Err(Error::Rejected)
        );
    }
    #[test]
    fn cleanup_during_protected_loan_permanently_refuses_commit_and_receipt() {
        let publication = ProtectedAccountPublication::new(false);
        let retirement = StartupRetirement::new();
        let original = retirement.capture_original().unwrap();
        assert_eq!(publication.require_phase(&retirement, 5), Ok(()));
        retirement.retire().unwrap();
        assert_eq!(
            publication.commit_original(&retirement, original),
            Err(Error::Rejected)
        );
        assert_eq!(retirement.require_original(original), Err(Error::Rejected));
        assert_eq!(
            publication.require_operational(&retirement),
            Err(Error::Rejected)
        );
        for phase in [3, 4, 5] {
            assert_eq!(publication.require_phase(&retirement, phase), Ok(()));
        }
    }
    #[test]
    fn existing_trusted_native_constructor_is_operational_without_platform_intake() {
        let publication = ProtectedAccountPublication::new(true);
        let retirement = StartupRetirement::new();
        assert_eq!(publication.require_operational(&retirement), Ok(()));
        retirement.retire().unwrap();
        assert_eq!(
            publication.require_operational(&retirement),
            Err(Error::Rejected)
        );
    }
    #[test]
    fn finite_same_account_renewal_preserves_first_authenticated_selection_identity() {
        // Public controls do not stand in for genuine signed reads or Native custody.
        let mut identity = StableAccountSelectionIdentity::new();
        assert_eq!(identity.original(0, &2_u8, &1_u8), Err(Error::Rejected));
        identity.complete_read(11, 0, &2_u8, &1_u8).unwrap();
        assert_eq!(identity.original(0, &2_u8, &1_u8), Ok(11));
        identity.complete_read(29, 0, &2_u8, &1_u8).unwrap();
        assert_eq!(identity.original(0, &2_u8, &1_u8), Ok(11));
    }
    #[test]
    fn renewal_cannot_replace_account_wallet_retirement_or_absent_read() {
        let mut identity = StableAccountSelectionIdentity::new();
        assert_eq!(
            identity.complete_read(0, 0, &2_u8, &1_u8),
            Err(Error::Rejected)
        );
        assert_eq!(
            identity.complete_read(11, 0, &1_u8, &1_u8),
            Err(Error::Rejected)
        );
        identity.complete_read(11, 0, &2_u8, &1_u8).unwrap();
        for (generation, wallet, signatory) in [(0, 3, 1), (0, 2, 3), (1, 2, 1)] {
            assert_eq!(
                identity.require_same_accounts(generation, &wallet, &signatory),
                Err(Error::Rejected)
            );
            assert_eq!(
                identity.complete_read(29, generation, &wallet, &signatory),
                Err(Error::Rejected)
            );
            assert_eq!(
                identity.original(generation, &wallet, &signatory),
                Err(Error::Rejected)
            );
        }
        assert_eq!(identity.original(0, &2_u8, &1_u8), Ok(11));
    }
    #[test]
    fn unknown_phase_never_admits_pending_or_committed_publication() {
        let retirement = StartupRetirement::new();
        for committed in [false, true] {
            let publication = ProtectedAccountPublication::new(committed);
            for phase in 0..=u8::MAX {
                if !(1..=6).contains(&phase) {
                    assert_eq!(
                        publication.require_phase(&retirement, phase),
                        Err(Error::Rejected)
                    );
                }
            }
        }
    }
}

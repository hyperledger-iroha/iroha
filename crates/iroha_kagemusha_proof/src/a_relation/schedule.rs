//! Fixed operation-task partitions and the canonical sixteen sigma selectors.

use iroha_plonk::frontend::Error;
use iroha_plonk_recursion::obligation::ledger::Variant;

/// Global sigma selector order from the wire's sorted `(operation_tag, mask)` set.
/// Refresh kinds share the one constrained tag7 union key.
pub const SIGMA_SELECTORS: [(u8, u8); 16] = [
    (1, 0),
    (2, 0),
    (3, 0),
    (3, 1),
    (3, 2),
    (3, 3),
    (3, 4),
    (3, 5),
    (3, 6),
    (3, 7),
    (4, 0),
    (4, 1),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
];

/// Find the unique global selector; undefined operation/mask pairs are rejected.
pub fn sigma_selector(operation_tag: u8, mask: u8) -> Option<u8> {
    SIGMA_SELECTORS
        .iter()
        .position(|pair| *pair == (operation_tag, mask))
        .and_then(|i| u8::try_from(i).ok())
}

/// Named constraint groups assigned to fixed A stages, never private booleans.
/// Source-key review must establish that each named group actually executes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum OperationTask {
    /// Bootstrap's exact zero state, identity and opening rules.
    BootstrapState = 1,
    /// Bootstrap issuer/credential/provider receipt authentication.
    BootstrapAuthorization = 2,
    /// Load arithmetic and exact depth32 insert-only recovery entry.
    LoadRecovery = 3,
    /// Load issuer authorization and exact operation receipt.
    LoadAuthorization = 4,
    /// Send current credential, exact Request and held fee terms.
    SendObjects = 5,
    /// Send pending descriptor insertion into the adjusted lineage map.
    SendPending = 6,
    /// Send conditional fee insertion and untouched maps/burned total.
    SendFeeAndCarry = 7,
}
impl OperationTask {
    /// Stable context-schema code, not an operation's wire tag.
    pub const fn code(self) -> u8 {
        self as u8
    }
    /// Exact currently implemented complete operation task set.
    /// Other variants need their own task schema before complete composition.
    pub const fn required(variant: Variant) -> Option<&'static [Self]> {
        match variant {
            Variant::Bootstrap => Some(&[Self::BootstrapState, Self::BootstrapAuthorization]),
            Variant::Load => Some(&[Self::LoadRecovery, Self::LoadAuthorization]),
            Variant::Send => Some(&[Self::SendObjects, Self::SendPending, Self::SendFeeAndCarry]),
            _ => None,
        }
    }
    pub(super) fn validate(variant: Variant, groups: &[Vec<Self>]) -> Result<(), Error> {
        let required = Self::required(variant).ok_or(Error::Synthesis)?;
        if groups
            .iter()
            .any(|group| group.windows(2).any(|w| w[0] >= w[1]))
        {
            return Err(Error::Synthesis);
        }
        let mut actual = groups.iter().flatten().copied().collect::<Vec<_>>();
        actual.sort_unstable();
        if actual != required {
            return Err(Error::Synthesis);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_selectors_exhaust_all_sixteen_without_aliases() {
        for (i, &(tag, mask)) in SIGMA_SELECTORS.iter().enumerate() {
            assert_eq!(sigma_selector(tag, mask), Some(u8::try_from(i).unwrap()));
        }
        for tag in 0..=9 {
            for mask in 0..=8 {
                assert_eq!(
                    sigma_selector(tag, mask).is_some(),
                    SIGMA_SELECTORS.contains(&(tag, mask))
                );
            }
        }
        assert_eq!(sigma_selector(2, 0), Some(1));
        assert_eq!(sigma_selector(7, 0), Some(14));
    }
    #[test]
    fn operation_partitions_require_every_task_once_and_reject_relabelling() {
        use OperationTask::{SendFeeAndCarry, SendObjects, SendPending};
        let honest = vec![vec![SendObjects], vec![SendPending], vec![SendFeeAndCarry]];
        assert!(OperationTask::validate(Variant::Send, &honest).is_ok());
        for i in 0..3 {
            let mut missing = honest.clone();
            missing[i].clear();
            assert!(OperationTask::validate(Variant::Send, &missing).is_err());
            let mut repeated = honest.clone();
            repeated[i].push(honest[i][0]);
            assert!(OperationTask::validate(Variant::Send, &repeated).is_err());
        }
        assert!(OperationTask::validate(Variant::Load, &honest).is_err());
        assert!(
            OperationTask::validate(
                Variant::Send,
                &[vec![SendPending, SendObjects, SendFeeAndCarry]]
            )
            .is_err()
        );
    }
}

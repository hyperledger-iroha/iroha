//! Freshness of the native World cut that authorizes a new transfer (§7.1 step 2, §7.2 step 1).
//!
//! A burn or record is checked against a World cut the caller selects with `--at-height` and
//! `--world-snapshot`. An idle Taira chain produces no blocks, so the cut's age is bounded in
//! blocks, never in wall-clock time: the cut must be the endpoint's certified tip or trail it by
//! at most [`MAX_CUT_LAG_BLOCKS`]. The wallet authenticates every block from the cut to the
//! claimed tip before it accepts the claim. An endpoint can still understate its tip; the bound
//! stops a stale or reused cut, not an endpoint that hides newer blocks.

use core::fmt;

/// Most certified blocks the selected World cut may trail the endpoint's tip before a new burn
/// or record is refused.
pub const MAX_CUT_LAG_BLOCKS: u64 = 4;

/// The selected cut cannot authorize a new burn or record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutFreshnessError {
    /// The endpoint claims a tip below the cut it already certified.
    TipBelowCut {
        /// Height of the selected cut.
        cut_height: u64,
        /// Tip the endpoint claims.
        tip_height: u64,
    },
    /// The cut trails the tip by more than [`MAX_CUT_LAG_BLOCKS`].
    Stale {
        /// Height of the selected cut.
        cut_height: u64,
        /// Tip the endpoint claims.
        tip_height: u64,
    },
}

impl fmt::Display for CutFreshnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TipBelowCut {
                cut_height,
                tip_height,
            } => write!(
                f,
                "the endpoint claims tip {tip_height} below the certified cut {cut_height}"
            ),
            Self::Stale {
                cut_height,
                tip_height,
            } => write!(
                f,
                "the selected World cut at height {cut_height} trails the certified tip \
                 {tip_height} by more than {MAX_CUT_LAG_BLOCKS} blocks; export a current \
                 snapshot and rerun with its `--at-height`"
            ),
        }
    }
}

impl std::error::Error for CutFreshnessError {}

/// Accept a cut at `cut_height` against an endpoint tip claim of `tip_height`.
///
/// # Errors
/// Refuses a tip below the cut, or a cut more than [`MAX_CUT_LAG_BLOCKS`] behind the tip.
pub fn check_cut_freshness(cut_height: u64, tip_height: u64) -> Result<(), CutFreshnessError> {
    let Some(lag) = tip_height.checked_sub(cut_height) else {
        return Err(CutFreshnessError::TipBelowCut {
            cut_height,
            tip_height,
        });
    };
    if lag > MAX_CUT_LAG_BLOCKS {
        return Err(CutFreshnessError::Stale {
            cut_height,
            tip_height,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cut_within_a_few_blocks_of_the_tip_is_accepted_and_an_older_one_refused() {
        assert_eq!(check_cut_freshness(10, 10), Ok(()));
        assert_eq!(check_cut_freshness(10, 10 + MAX_CUT_LAG_BLOCKS), Ok(()));
        let stale = check_cut_freshness(10, 11 + MAX_CUT_LAG_BLOCKS);
        assert_eq!(
            stale,
            Err(CutFreshnessError::Stale {
                cut_height: 10,
                tip_height: 11 + MAX_CUT_LAG_BLOCKS,
            })
        );
        assert!(stale.unwrap_err().to_string().contains("--at-height"));
        assert_eq!(
            check_cut_freshness(10, 9),
            Err(CutFreshnessError::TipBelowCut {
                cut_height: 10,
                tip_height: 9,
            })
        );
        assert!(check_cut_freshness(2, u64::MAX).is_err());
        assert_eq!(check_cut_freshness(u64::MAX, u64::MAX), Ok(()));
    }
}

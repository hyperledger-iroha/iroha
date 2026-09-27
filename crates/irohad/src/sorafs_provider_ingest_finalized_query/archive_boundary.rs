//! Archive startup and activation boundary rules shared by the finalized-archive query adapters.
//!
//! Provider-ingest and reputation archives both reconcile the recovered State tip against the
//! durable Kura tip and an optional authenticated pending V2 tip before they qualify.

/// Authenticated startup boundary of one finalized archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchiveStartupBoundaryV1 {
    Bootstrap,
    Qualified,
    PendingTip { height: u64 },
}
/// Classify the recovered State/Kura startup boundary, optionally at a pending V2 tip.
pub(crate) fn classify_archive_startup_boundary(
    state_height: u64,
    kura_height: u64,
    pending_v2_tip_height: Option<u64>,
) -> Result<ArchiveStartupBoundaryV1, &'static str> {
    let Some(pending_height) = pending_v2_tip_height else {
        if state_height != kura_height {
            return Err("State and Kura heights differ without an authenticated pending V2 tip");
        }
        return Ok(if state_height == 0 {
            ArchiveStartupBoundaryV1::Bootstrap
        } else {
            ArchiveStartupBoundaryV1::Qualified
        });
    };
    if pending_height == 0 || pending_height != kura_height {
        return Err("pending V2 tip does not equal the exact non-zero durable Kura tip");
    }
    if state_height != pending_height && state_height.checked_add(1) != Some(pending_height) {
        return Err("State is not at the pending V2 tip or its exact predecessor");
    }
    Ok(ArchiveStartupBoundaryV1::PendingTip {
        height: pending_height,
    })
}
/// Accept only an archive at the pending V2 tip or its replay-capturable exact predecessor.
pub(crate) fn validate_pending_archive_tip(
    pending_tip_height: u64,
    state_height: u64,
    archive_tip_height: Option<u64>,
) -> Result<(), &'static str> {
    match archive_tip_height {
        Some(height) if height == pending_tip_height => Ok(()),
        Some(height)
            if height.checked_add(1) == Some(pending_tip_height) && state_height == height =>
        {
            Ok(())
        }
        None if pending_tip_height == 1 && state_height == 0 => Ok(()),
        Some(_) => {
            Err("archive is not at the pending V2 tip or a replay-capturable exact predecessor")
        }
        None => Err("non-genesis pending V2 replay requires an authenticated archive anchor"),
    }
}
/// Whether pending V2 replay completed; `Ok(false)` keeps waiting at the exact pending tip.
pub(crate) fn classify_pending_replay_completion(
    expected_height: u64,
    durable_height: u64,
    pending_tip_height: Option<u64>,
) -> Result<bool, &'static str> {
    if durable_height < expected_height {
        return Err("recovery durable height regressed below its authenticated pending tip");
    }
    match pending_tip_height {
        None => Ok(true),
        Some(height) if height == expected_height && durable_height == expected_height => Ok(false),
        Some(height) if height == durable_height && height > expected_height => Ok(true),
        Some(_) => Err("recovery exposed a mismatched pending V2 durable tip"),
    }
}
/// Live-visibility gate applied to the archive tip before queries are served.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchiveActivationGateV1 {
    StrictLive,
    AwaitingGenesis,
    PendingTip { height: u64 },
}
impl ArchiveActivationGateV1 {
    pub(crate) fn accepts_visible_archive_tip(self, archive_tip_height: u64) -> bool {
        match self {
            Self::PendingTip { height } => archive_tip_height >= height,
            Self::StrictLive | Self::AwaitingGenesis => true,
        }
    }
}

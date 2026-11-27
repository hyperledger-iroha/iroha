//! Host diagnostic on valid public structural fixtures. The isolated worker calls
//! the stock bounded decoder once. Allocation callbacks describe requested bytes.
use std::alloc::{GlobalAlloc, Layout, System};
use std::env;
use std::error::Error;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

use iroha_data_model::governance::{
    anonymous_ballot::{
        AuthenticatedBallotContextV1, BallotBodyContextV1, CredentialCommitmentV1, Digest512,
        FrozenParliamentAttemptPolicyV1, ParliamentBallotParametersV1,
        ParliamentLifecycleParametersV1, ProofProfileIdV1, framing::enc,
    },
    types::{
        BodyElectionAttemptId, GovernanceAttemptId, PARLIAMENT_BODIES_V1, ParliamentBody,
        RiskTierV1,
    },
};

static ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static REALLOCS: AtomicUsize = AtomicUsize::new(0);
static FREES: AtomicUsize = AtomicUsize::new(0);
static REQUESTED: AtomicUsize = AtomicUsize::new(0);
static RELEASED: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static UNDERFLOW: AtomicBool = AtomicBool::new(false);

fn update(added: usize, released: usize) {
    let total = REQUESTED.fetch_add(added, Relaxed) + added;
    let retired = RELEASED.fetch_add(released, Relaxed) + released;
    if let Some(live) = total.checked_sub(retired) {
        PEAK.fetch_max(live, Relaxed);
    } else {
        UNDERFLOW.store(true, Relaxed);
    }
}

struct Meter;
// Forward every call to the system allocator using its original pointer/layout.
// Metrics never allocate and the measured decoder runs synchronously.
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if ACTIVE.load(Relaxed) && !ptr.is_null() {
            ALLOCS.fetch_add(1, Relaxed);
            update(layout.size(), 0);
        }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if ACTIVE.load(Relaxed) && !ptr.is_null() {
            ALLOCS.fetch_add(1, Relaxed);
            update(layout.size(), 0);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ACTIVE.load(Relaxed) {
            FREES.fetch_add(1, Relaxed);
            update(0, layout.size());
        }
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let grown = unsafe { System.realloc(ptr, layout, size) };
        if ACTIVE.load(Relaxed) && !grown.is_null() {
            REALLOCS.fetch_add(1, Relaxed);
            update(size, layout.size());
        }
        grown
    }
}
#[global_allocator]
static ALLOCATOR: Meter = Meter;

fn fixture(seats: u16) -> Result<AuthenticatedBallotContextV1, Box<dyn Error>> {
    let mut lifecycle = ParliamentLifecycleParametersV1::default();
    lifecycle.sortition.minimum_citizenship_bond = u128::MAX.into();
    let frozen = FrozenParliamentAttemptPolicyV1::freeze_v1(
        &PARLIAMENT_BODIES_V1,
        &lifecycle,
        Some(&ParliamentBallotParametersV1::default()),
        1_000,
        3_600,
    )?;
    let commitments = (0..seats)
        .map(|i| {
            let mut public = [0x77_u8; 128];
            public[..2].copy_from_slice(&i.to_be_bytes());
            CredentialCommitmentV1::new(public)
        })
        .collect();
    let value = AuthenticatedBallotContextV1 {
        body_context: BallotBodyContextV1 {
            network_id:
                "hash:214A4C8F95074B216BE2F72EB93166506DAE0B1026ED01EF5A760632CD93ABAB#50FA"
                    .parse()?,
            governance_attempt_id: GovernanceAttemptId::new([0x11; 32]),
            election_attempt_id: BodyElectionAttemptId::new([0x22; 32]),
            body_role: ParliamentBody::ConfirmationJury,
        },
        proposal_digest: Digest512::new([0x88; 64]),
        body_context_digest: Digest512::new([0x33; 64]),
        list_digest: Digest512::new([0x99; 64]),
        seats,
        commitments,
        profile_id: ProofProfileIdV1::V1,
        policy: frozen.ballot_policy(RiskTierV1::Standard, ParliamentBody::ConfirmationJury)?,
        policy_hash: Digest512::new([0xaa; 64]),
    };
    value.validate()?;
    Ok(value)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let action = args.next().ok_or("generate or measure")?;
    let seats: u16 = args.next().ok_or("seats")?.parse()?;
    let path = args.next().ok_or("canonical public frame")?;
    assert!(args.next().is_none() && [3, 125, 126, 500, 1000].contains(&seats));
    if action == "generate" {
        fs::write(path, enc(&fixture(seats)?)?)?;
        return Ok(());
    }
    let metered = action == "measure";
    assert!(metered || action == "time");
    let wire = fs::read(path)?;
    let observer =
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX);
    let start = Instant::now();
    ACTIVE.store(metered, Relaxed);
    let (result, usage) = norito::core::with_decode_limits_measured(observer, || {
        AuthenticatedBallotContextV1::decode_canonical_bounded(&wire)
    });
    let elapsed = start.elapsed().as_nanos();
    ACTIVE.store(false, Relaxed);
    let context = result?;
    assert_eq!(context.seats, seats);
    let live_at_return = REQUESTED
        .load(Relaxed)
        .checked_sub(RELEASED.load(Relaxed))
        .ok_or("process counters crossed baseline")?;
    let peak = PEAK.load(Relaxed);
    assert_eq!(wire, enc(&context)?);
    assert_eq!(context, fixture(seats)?);
    ACTIVE.store(metered, Relaxed);
    drop(context);
    ACTIVE.store(false, Relaxed);
    let retained = REQUESTED
        .load(Relaxed)
        .checked_sub(RELEASED.load(Relaxed))
        .ok_or("destruction crossed baseline")?;
    assert!(!UNDERFLOW.load(Relaxed));
    assert_eq!(usage.total_elements(), usize::from(seats) + 10);
    assert!(usage.total_allocated_bytes() <= 327_680);
    println!(
        "{}",
        norito::json::to_json(&norito::json!({
          "metered":metered,"seats":seats,"wireBytes":(wire.len()),"codecElements":(usage.total_elements()),"nominalCodecBytes":(usage.total_allocated_bytes()),
          "allocatorAllocations":(ALLOCS.load(Relaxed)),"allocatorReallocations":(REALLOCS.load(Relaxed)),"allocatorFreesAfterDrop":(FREES.load(Relaxed)),
          "sumOfAllocationAndReallocationRequests":(REQUESTED.load(Relaxed)),"logicalHeapPeakDuringDecode":peak,
          "logicalHeapAtDecoderReturn":live_at_return,"logicalHeapAfterContextDrop":retained,"elapsedNs":(u64::try_from(elapsed)?)
        }))?
    );
    Ok(())
}

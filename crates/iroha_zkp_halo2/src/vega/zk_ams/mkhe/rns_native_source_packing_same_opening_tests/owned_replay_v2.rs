//! Actual owned-entry checks with a private borrowed fixture predecessor.
//!
//! This module is reachable only through the existing cfg(test) test owner.
//! The cached artifact is a real local proof over synthetic values, not live
//! source provenance. No composite transition or production adapter is added.

use super::*;

type Error = RnsNativeSourcePackingSameOpeningErrorV1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Successor,
    Context,
    SafeCore,
    Begin,
    Axes,
    Schedule,
    FirstPoint,
    Replay,
    Finish,
    ReplayDrop,
    Outer,
    OwnerDrop,
}

#[derive(Default)]
struct Probe {
    events: RefCell<Vec<Event>>,
    point_reads: Cell<usize>,
    source_dropped: Rc<Cell<bool>>,
}

impl Probe {
    fn event(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }

    fn point(&self) {
        if self.point_reads.get() == 0 {
            self.event(Event::FirstPoint);
        }
        self.point_reads.set(self.point_reads.get() + 1);
    }
}

#[derive(Clone, Copy)]
enum Fault {
    None,
    Context,
    SafeCore,
    Begin,
    BeginPanic,
    SourceAxes,
    Point,
    Replay,
    Finish,
}

// Private to this test module. The only constructor creates the fixture source
// itself; the actual owned entry still takes only this predecessor by value.
struct OwnedFixture<'proof> {
    wire: &'proof [u8],
    context: RnsNativeSourcePackingSameOpeningContextV1,
    source: Option<FixtureReplaySourceV1>,
    fault: Fault,
    probe: Rc<Probe>,
}

impl<'proof> OwnedFixture<'proof> {
    fn new(wire: &'proof [u8], fault: Fault, probe: Rc<Probe>) -> Self {
        let action = match fault {
            Fault::Point => ReplayActionV1::PointError,
            Fault::Replay => ReplayActionV1::Error,
            Fault::Finish => ReplayActionV1::FinishError,
            _ => ReplayActionV1::Success,
        };
        let mut source = FixtureReplaySourceV1::new_v1(
            FixtureModeV1::NonIdentitySignedQ,
            action,
            Some(Rc::clone(&probe.source_dropped)),
        );
        if matches!(fault, Fault::SourceAxes) {
            source.authenticated_source_axes.source_binding_digest[0] ^= 1;
        }
        Self {
            wire,
            context: context_v1(),
            source: Some(source),
            fault,
            probe,
        }
    }
}

impl Drop for OwnedFixture<'_> {
    fn drop(&mut self) {
        self.probe.event(Event::OwnerDrop);
    }
}

impl<'proof> RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1<'proof>
    for OwnedFixture<'proof>
{
    fn same_opening_successor_v1(&self) -> &'proof [u8] {
        self.probe.event(Event::Successor);
        self.wire
    }

    fn successor_independent_safe_core_v1(&self) -> RnsNativeSourcePackingSafeCoreV1 {
        self.probe.event(Event::SafeCore);
        let mut core = self.context.safe_core;
        if matches!(self.fault, Fault::SafeCore) {
            core.terminal_predecessor_context_binding_digest[0] ^= 1;
        }
        core
    }

    fn combined_outer_bindings_v1(&self) -> RnsNativeSourcePackingCombinedOuterBindingsV1 {
        // These assertions fail if the production entry reads outer data
        // before its exact replay is finished, dropped and consumed.
        assert!(self.source.is_none());
        assert!(self.probe.source_dropped.get());
        assert_eq!(self.probe.events.borrow().last(), Some(&Event::ReplayDrop));
        self.probe.event(Event::Outer);
        outer_bindings_v1(91)
    }
}

impl<'proof> RnsNativeSourcePackingOwnedReplayPredecessorV2<'proof> for OwnedFixture<'proof> {
    type Replay<'owner>
        = BorrowedReplay<'owner>
    where
        Self: 'owner;

    fn authenticated_same_opening_context_v2(
        &self,
    ) -> Result<RnsNativeSourcePackingSameOpeningContextV1, Error> {
        self.probe.event(Event::Context);
        if matches!(self.fault, Fault::Context) {
            return Err(Error::SourceUnavailable);
        }
        Ok(self.context)
    }

    fn begin_authenticated_replay_v2(&mut self) -> Result<Self::Replay<'_>, Error> {
        self.probe.event(Event::Begin);
        if matches!(self.fault, Fault::BeginPanic) {
            panic!("fixture replay acquisition must not be attempted for a rejected frame");
        }
        if matches!(self.fault, Fault::Begin) || self.source.is_none() {
            return Err(Error::SourceUnavailable);
        }
        Ok(BorrowedReplay {
            source: &mut self.source,
            probe: &self.probe,
        })
    }
}

// The GAT borrows the actual owner's source slot, not a detached source or a
// cloned capability. Finishing or dropping this borrow consumes that slot.
struct BorrowedReplay<'owner> {
    source: &'owner mut Option<FixtureReplaySourceV1>,
    probe: &'owner Probe,
}

impl Drop for BorrowedReplay<'_> {
    fn drop(&mut self) {
        drop(self.source.take());
        self.probe.event(Event::ReplayDrop);
    }
}

impl RnsNativeSourcePackingAggregateReplayV1 for BorrowedReplay<'_> {
    fn authenticated_source_axes_v1(&self) -> RnsNativeSourcePackingAuthenticatedSourceAxesV1 {
        self.probe.event(Event::Axes);
        self.source
            .as_ref()
            .expect("owned source")
            .authenticated_source_axes_v1()
    }

    fn canonical_replay_schedule_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.probe.event(Event::Schedule);
        self.source
            .as_ref()
            .expect("owned source")
            .canonical_replay_schedule_digest_v1()
    }

    fn difference_low_commitment_v1(&self, group: usize, digit: usize) -> Result<Point, Error> {
        self.probe.point();
        self.source
            .as_ref()
            .expect("owned source")
            .difference_low_commitment_v1(group, digit)
    }

    fn difference_top_commitment_v1(&self, group: usize) -> Result<Point, Error> {
        self.probe.point();
        self.source
            .as_ref()
            .expect("owned source")
            .difference_top_commitment_v1(group)
    }

    fn signed_commitment_v1(
        &self,
        record: usize,
        role: RnsNativeSignedSourceRoleV1,
        plane: usize,
    ) -> Result<Point, Error> {
        self.probe.point();
        self.source
            .as_ref()
            .expect("owned source")
            .signed_commitment_v1(record, role, plane)
    }

    fn replay_tau_aggregate_v1(
        &mut self,
        tau: Scalar,
        destination: &mut ZeroizingT256ScalarVecV1,
    ) -> Result<RnsNativeSourcePackingReplayReceiptV1, Error> {
        self.probe.event(Event::Replay);
        self.source
            .as_mut()
            .expect("owned source")
            .replay_tau_aggregate_v1(tau, destination)
    }

    fn finish_v1(self) -> Result<RnsNativeSourcePackingReplayReceiptV1, Error> {
        self.probe.event(Event::Finish);
        self.source.take().expect("owned source").finish_v1()
    }
}

const COMPLETE_REPLAY: &[Event] = &[
    Event::Successor,
    Event::Context,
    Event::SafeCore,
    Event::Begin,
    Event::Axes,
    Event::Schedule,
    Event::FirstPoint,
    Event::Replay,
    Event::Finish,
    Event::ReplayDrop,
];

#[test]
fn actual_owned_entry_verifies_cached_proof_and_consumes_the_exact_replay() {
    let artifact = nonidentity_public_fixture_v1();
    let probe = Rc::new(Probe::default());
    let previous = OwnedFixture::new(&artifact.wire, Fault::None, Rc::clone(&probe));
    let mut verified = verify_rns_native_source_packing_same_opening_owned_v2(previous)
        .expect("actual owned entry verifies the existing real fixture proof");
    let mut expected = COMPLETE_REPLAY.to_vec();
    expected.push(Event::Outer);
    assert_eq!(*probe.events.borrow(), expected);
    assert_eq!(
        probe.point_reads.get(),
        DIFFERENCE_GROUPS_V1 * (RADIX_LOW_DIGITS_V1 + 1) + SIGNED_OWNERS_V1
    );
    assert!(probe.source_dropped.get());
    assert!(verified.previous.source.is_none());
    assert_eq!(verified.residual(), artifact.residual_v1());
    assert_eq!(verified.point_root(), artifact.point_root);
    assert_eq!(verified.manifest_digest, artifact.manifest_digest);
    assert_eq!(
        verified.source_context_digest,
        artifact.source_context_digest
    );
    assert_eq!(
        verified.replay_receipt_digest,
        artifact.replay_receipt_digest
    );
    assert_eq!(
        verified.pre_challenge_binding_digest,
        artifact.pre_challenge_binding_digest
    );
    assert_eq!(verified.tau_digest, artifact.tau_digest);
    assert_eq!(verified.q_digest, artifact.q_digest);
    assert_eq!(verified.proof_digest, artifact.proof_digest);
    assert_eq!(verified.residual_digest, artifact.residual_digest);
    assert_eq!(verified.binding_digest, artifact.binding_digest);
    assert!(matches!(
        verified.previous.begin_authenticated_replay_v2(),
        Err(Error::SourceUnavailable)
    ));
    expected.push(Event::Begin);
    drop(verified);
    expected.push(Event::OwnerDrop);
    assert_eq!(*probe.events.borrow(), expected);
}

#[test]
fn actual_owned_entry_rejects_wrong_equation_before_outer_binding() {
    let artifact = nonidentity_public_fixture_v1();
    let frame = artifact.frame_v1();
    let mut wrong = artifact.wire.clone();
    wrong[HEADER_BYTES_V1 + POINT_BYTES_V1..HEADER_BYTES_V1 + SCHNORR_PAYLOAD_BYTES_V1]
        .copy_from_slice(&(frame.z + Scalar::one()).to_le_bytes());
    rewrite_codec_v1(&mut wrong);
    let probe = Rc::new(Probe::default());
    let previous = OwnedFixture::new(&wrong, Fault::None, Rc::clone(&probe));
    assert!(matches!(
        verify_rns_native_source_packing_same_opening_owned_v2(previous),
        Err(Error::InvalidProof)
    ));
    let mut expected = COMPLETE_REPLAY.to_vec();
    expected.push(Event::OwnerDrop);
    assert_eq!(*probe.events.borrow(), expected);
    assert!(probe.source_dropped.get());
    assert_eq!(
        probe.point_reads.get(),
        DIFFERENCE_GROUPS_V1 * (RADIX_LOW_DIGITS_V1 + 1) + SIGNED_OWNERS_V1
    );
}

#[test]
fn actual_owned_entry_context_source_and_replay_failures_consume_without_outer_access() {
    use Event::*;
    let artifact = nonidentity_public_fixture_v1();
    let cases: &[(Fault, Error, &[Event], usize)] = &[
        (
            Fault::Context,
            Error::SourceUnavailable,
            &[Successor, Context, OwnerDrop],
            0,
        ),
        (
            Fault::SafeCore,
            Error::InvalidContext,
            &[Successor, Context, SafeCore, OwnerDrop],
            0,
        ),
        (
            Fault::Begin,
            Error::SourceUnavailable,
            &[Successor, Context, SafeCore, Begin, OwnerDrop],
            0,
        ),
        (
            Fault::SourceAxes,
            Error::InvalidContext,
            &[
                Successor, Context, SafeCore, Begin, Axes, ReplayDrop, OwnerDrop,
            ],
            0,
        ),
        (
            Fault::Point,
            Error::SourceUnavailable,
            &[
                Successor, Context, SafeCore, Begin, Axes, Schedule, FirstPoint, ReplayDrop,
                OwnerDrop,
            ],
            1,
        ),
        (
            Fault::Replay,
            Error::SourceUnavailable,
            &[
                Successor, Context, SafeCore, Begin, Axes, Schedule, FirstPoint, Replay,
                ReplayDrop, OwnerDrop,
            ],
            DIFFERENCE_GROUPS_V1 * (RADIX_LOW_DIGITS_V1 + 1) + SIGNED_OWNERS_V1,
        ),
        (
            Fault::Finish,
            Error::SourceUnavailable,
            &[
                Successor, Context, SafeCore, Begin, Axes, Schedule, FirstPoint, Replay, Finish,
                ReplayDrop, OwnerDrop,
            ],
            DIFFERENCE_GROUPS_V1 * (RADIX_LOW_DIGITS_V1 + 1) + SIGNED_OWNERS_V1,
        ),
    ];
    for &(fault, error, expected, point_reads) in cases {
        let probe = Rc::new(Probe::default());
        let previous = OwnedFixture::new(&artifact.wire, fault, Rc::clone(&probe));
        assert!(
            matches!(verify_rns_native_source_packing_same_opening_owned_v2(previous), Err(observed) if observed == error)
        );
        assert_eq!(probe.events.borrow().as_slice(), expected);
        assert_eq!(probe.point_reads.get(), point_reads);
        assert!(probe.source_dropped.get());
    }
}

#[test]
fn exact_frame_preflight_precedes_context_failures_and_replay_acquisition_panics() {
    let artifact = nonidentity_public_fixture_v1();
    let mut malformed = artifact.wire.clone();
    malformed[0] ^= 1;
    rewrite_codec_v1(&mut malformed);
    let mut bad_codec = artifact.wire.clone();
    *bad_codec.last_mut().expect("nonempty proof") ^= 1;
    let mut invalid_point = artifact.wire.clone();
    invalid_point[HEADER_BYTES_V1..HEADER_BYTES_V1 + POINT_BYTES_V1].fill(0);
    rewrite_codec_v1(&mut invalid_point);
    let mut invalid_scalar = artifact.wire.clone();
    invalid_scalar[HEADER_BYTES_V1 + POINT_BYTES_V1..HEADER_BYTES_V1 + SCHNORR_PAYLOAD_BYTES_V1]
        .fill(0xff);
    rewrite_codec_v1(&mut invalid_scalar);
    for (wire, error) in [
        (malformed, Error::InvalidHeader),
        (
            vec![0; FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1 + 1],
            Error::ProofCapExceeded,
        ),
        (bad_codec, Error::InvalidIntegrity),
        (invalid_point, Error::InvalidPoint),
        (invalid_scalar, Error::InvalidScalar),
    ] {
        for fault in [
            Fault::None,
            Fault::Context,
            Fault::SafeCore,
            Fault::Begin,
            Fault::BeginPanic,
        ] {
            let probe = Rc::new(Probe::default());
            let previous = OwnedFixture::new(&wire, fault, Rc::clone(&probe));
            assert!(
                matches!(verify_rns_native_source_packing_same_opening_owned_v2(previous), Err(observed) if observed == error)
            );
            assert_eq!(*probe.events.borrow(), [Event::Successor, Event::OwnerDrop]);
            assert_eq!(probe.point_reads.get(), 0);
            assert!(probe.source_dropped.get());
        }
    }
}

#[test]
fn valid_frame_reaches_acquisition_panic_and_still_consumes_the_owner() {
    let artifact = nonidentity_public_fixture_v1();
    let probe = Rc::new(Probe::default());
    let result = catch_unwind(AssertUnwindSafe(|| {
        let previous = OwnedFixture::new(&artifact.wire, Fault::BeginPanic, Rc::clone(&probe));
        let _ = verify_rns_native_source_packing_same_opening_owned_v2(previous);
    }));
    assert!(
        result.is_err(),
        "the positive control must reach the panic-producing acquisition"
    );
    assert_eq!(
        *probe.events.borrow(),
        [
            Event::Successor,
            Event::Context,
            Event::SafeCore,
            Event::Begin,
            Event::OwnerDrop
        ]
    );
    assert_eq!(probe.point_reads.get(), 0);
    assert!(probe.source_dropped.get());
}

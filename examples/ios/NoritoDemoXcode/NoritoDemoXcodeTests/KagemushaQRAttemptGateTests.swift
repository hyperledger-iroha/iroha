// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import XCTest
import IrohaSwift

final class KagemushaQRAttemptGateTests: XCTestCase {
  private func admit(_ record: KagemushaQRAttemptGate.Record, _ gate: inout KagemushaQRAttemptGate) throws {
    let id = UUID(); try gate.beginScan(id); XCTAssertTrue(try gate.observed(id)); try gate.admit(record, scan: id)
  }
  func testFreshSenderRequiresRequestAuthenticationReleaseAndNativeVerification() throws {
    var gate = KagemushaQRAttemptGate(role: .sender, recovery: false, previouslyDispatched: false)
    XCTAssertThrowsError(try gate.authenticated(at: 10))
    try admit(.request, &gate)
    XCTAssertThrowsError(try gate.releasedPayment(status: 1))
    try gate.authenticated(at: 10)
    XCTAssertThrowsError(try gate.releasedPayment(status: 2))
    XCTAssertThrowsError(try gate.releasedPayment(status: 47), "a delivery projection is not a Send completion")
    try gate.releasedPayment(status: 1); try admit(.credited, &gate)
    XCTAssertThrowsError(try gate.credited(evidence: .credited, archive: .receiver, corePending: false, arrival: 20, verified: 21))
    let sample = try XCTUnwrap(gate.credited(evidence: .credited, archive: .notArchived, corePending: true, arrival: 20, verified: 25))
    XCTAssertEqual(sample.0, 10); XCTAssertEqual(sample.1, 15); XCTAssertTrue(gate.terminal)
  }
  func testCreditedCannotPrecedePaymentRelease() throws {
    var gate = KagemushaQRAttemptGate(role: .sender, recovery: false, previouslyDispatched: false)
    try admit(.request, &gate)
    let id = UUID(); try gate.beginScan(id)
    XCTAssertThrowsError(try gate.admit(.credited, scan: id))
  }
  func testRecoveryUsesExactRequestAndNeverInventsFreshLatency() throws {
    var gate = KagemushaQRAttemptGate(role: .sender, recovery: true, previouslyDispatched: true)
    XCTAssertThrowsError(try gate.releasedPayment(status: 1))
    try admit(.request, &gate)
    XCTAssertThrowsError(try gate.authenticated(at: 10))
    try gate.releasedPayment(status: 1); try admit(.credited, &gate)
    XCTAssertNil(try gate.credited(evidence: .credited, archive: .notArchived, corePending: true, arrival: 20, verified: 25))
  }
  func testPredispatchRecoveryStillNeedsFreshAuthentication() throws {
    var gate = KagemushaQRAttemptGate(role: .sender, recovery: true, previouslyDispatched: false)
    try admit(.request, &gate)
    XCTAssertThrowsError(try gate.releasedPayment(status: 1))
    try gate.authenticated(at: 10); try gate.releasedPayment(status: 1); try admit(.credited, &gate)
    XCTAssertNil(try gate.credited(evidence: .credited, archive: .notArchived, corePending: true, arrival: 20, verified: 25))
  }
  func testReceiverDisplayCloseNeverMeansPeerDelivery() throws {
    var gate = KagemushaQRAttemptGate(role: .receiver, recovery: false, previouslyDispatched: false)
    XCTAssertThrowsError(try gate.closedReceiverDisplay())
    XCTAssertThrowsError(try gate.received())
    try admit(.payment, &gate)
    XCTAssertThrowsError(try gate.displayedCredited())
    try gate.received(); XCTAssertFalse(gate.terminal)
    try gate.displayedCredited(); XCTAssertFalse(gate.terminal)
    try gate.closedReceiverDisplay(); XCTAssertTrue(gate.terminal)
  }
  func testReceiverRecoveryRescansPayment() throws {
    var gate = KagemushaQRAttemptGate(role: .receiver, recovery: true, previouslyDispatched: true)
    try admit(.payment, &gate); try gate.received(); try gate.displayedCredited(); try gate.closedReceiverDisplay()
    XCTAssertTrue(gate.terminal)
  }
  func testStaleCancelledAndWrongOrderCallbacksCannotAdvance() throws {
    var gate = KagemushaQRAttemptGate(role: .sender, recovery: false, previouslyDispatched: false)
    let id = UUID(); try gate.beginScan(id)
    XCTAssertFalse(try gate.observed(UUID())); XCTAssertEqual(gate.observations, 0)
    XCTAssertThrowsError(try gate.beginScan(UUID()))
    XCTAssertThrowsError(try gate.admit(.payment, scan: id))
    try gate.admit(.request, scan: id); XCTAssertFalse(try gate.observed(id))
    gate.fail(); XCTAssertFalse(try gate.observed(id)); XCTAssertThrowsError(try gate.beginScan(UUID()))
  }
  func testObservationStorageIsBoundedEvenForIdenticalDuplicates() throws {
    var gate = KagemushaQRAttemptGate(role: .sender, recovery: false, previouslyDispatched: false)
    let id = UUID(); try gate.beginScan(id)
    for _ in 0..<KagemushaQRAttemptGate.maximumObservations { XCTAssertTrue(try gate.observed(id)) }
    XCTAssertThrowsError(try gate.observed(id))
    XCTAssertEqual(gate.observations, KagemushaQRAttemptGate.maximumObservations)
  }
  func testClockRegressionCannotQualify() throws {
    var gate = KagemushaQRAttemptGate(role: .sender, recovery: false, previouslyDispatched: false)
    try admit(.request, &gate); try gate.authenticated(at: 20); try gate.releasedPayment(status: 1); try admit(.credited, &gate)
    XCTAssertThrowsError(try gate.credited(evidence: .credited, archive: .notArchived, corePending: true, arrival: 19, verified: 21))
    XCTAssertThrowsError(try gate.credited(evidence: .credited, archive: .notArchived, corePending: true, arrival: 21, verified: 20))
    XCTAssertFalse(gate.terminal)
  }
  func testReceiverCannotConsumeAnotherRequestOrCreditedBeforePayment() throws {
    var gate = KagemushaQRAttemptGate(role: .receiver, recovery: false, previouslyDispatched: false)
    let id = UUID(); try gate.beginScan(id)
    XCTAssertThrowsError(try gate.admit(.request, scan: id))
    XCTAssertThrowsError(try gate.admit(.credited, scan: id))
    try gate.admit(.payment, scan: id)
    XCTAssertThrowsError(try gate.beginScan(UUID()))
    try gate.received(); try gate.displayedCredited(); try gate.closedReceiverDisplay()
    XCTAssertTrue(gate.terminal)
  }
  func testPayerDeliveryDoesNotWaitForArchiveOrCorePending() throws {
    let evidence: [KagemushaWalletCreditProjectionV1.Evidence] = [.unfolded, .credited]
    let archives: [KagemushaWalletCreditProjectionV1.Archive] = [.awaitingFold, .removed, .retained, .notArchived]
    for recovery in [false, true] {
      for value in evidence { for archive in archives { for pending in [false, true] {
        var gate = KagemushaQRAttemptGate(role: .sender, recovery: recovery, previouslyDispatched: recovery)
        try admit(.request, &gate)
        if !recovery { try gate.authenticated(at: 10) }
        try gate.releasedPayment(status: 1); try admit(.credited, &gate)
        let sample = try gate.credited(evidence: value, archive: archive, corePending: pending,
          arrival: 20, verified: 25)
        if recovery { XCTAssertNil(sample) }
        else {
          let elapsed = try XCTUnwrap(sample)
          XCTAssertEqual(elapsed.0, 10); XCTAssertEqual(elapsed.1, 15)
        }
        XCTAssertTrue(gate.terminal)
      } } }
    }
  }
  func testVerifiedBurnedDeliveryNeverPassesPositiveTrialOrProducesLatency() throws {
    let archives: [KagemushaWalletCreditProjectionV1.Archive] = [.awaitingFold, .removed, .retained, .notArchived]
    for recovery in [false, true] {
      for archive in archives { for pending in [false, true] {
        var gate = KagemushaQRAttemptGate(role: .sender, recovery: recovery, previouslyDispatched: recovery)
        try admit(.request, &gate)
        if !recovery { try gate.authenticated(at: 10) }
        try gate.releasedPayment(status: 1); try admit(.credited, &gate)
        XCTAssertThrowsError(try gate.credited(evidence: .burned, archive: archive, corePending: pending,
          arrival: 20, verified: 25)) { error in
          guard case KagemushaQRAttemptGate.Failure.burned = error else {
            return XCTFail("Native verified burn evidence must remain distinct from invalid evidence")
          }
        }
        XCTAssertFalse(gate.terminal, "burned evidence cannot mark a successful positive trial")
        gate.fail(); XCTAssertTrue(gate.terminal)
      } }
    }
  }
  func testConditionalReviewRequiresActualRenewalAndExcludesLatency() throws {
    var gate = KagemushaQRAttemptGate(role: .sender, recovery: false,
      previouslyDispatched: true, continuation: true)
    XCTAssertThrowsError(try gate.renewed())
    try admit(.request, &gate)
    XCTAssertThrowsError(try gate.authenticated(at: 10))
    XCTAssertThrowsError(try gate.releasedPayment(status: 1), "an old fence alone cannot release a conditional Send")
    try gate.renewed(); XCTAssertThrowsError(try gate.renewed())
    try gate.authenticated(at: 10); try gate.releasedPayment(status: 1); try admit(.credited, &gate)
    XCTAssertNil(try gate.credited(evidence: .credited, archive: .notArchived, corePending: true,
      arrival: 20, verified: 25))
  }
  func testRetainedReviewAndExistingOutcomeCannotQualifyFreshQRLatency() throws {
    for existing in [false, true] {
      var gate = KagemushaQRAttemptGate(role: .sender, recovery: false, previouslyDispatched: false)
      try admit(.request, &gate)
      if existing { try gate.observedExisting() }
      else {
        XCTAssertThrowsError(try gate.retainedReview())
        try gate.authenticated(at: 10); try gate.retainedReview()
      }
      for status: Int32 in [0,2,11,65,66,67] { XCTAssertThrowsError(try gate.releasedPayment(status: status)) }
      try gate.releasedPayment(status: 1); try admit(.credited, &gate)
      XCTAssertNil(try gate.credited(evidence: .credited, archive: .notArchived, corePending: false,
        arrival: 20, verified: 25))
    }
  }

}

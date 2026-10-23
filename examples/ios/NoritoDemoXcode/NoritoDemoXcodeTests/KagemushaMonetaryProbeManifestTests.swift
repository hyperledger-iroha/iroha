// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import Foundation
import XCTest

@testable import IrohaSwift

/// Test-only intent/decision guards; no reconstructed Native authority or fake proof output.
final class KagemushaMonetaryProbeManifestTests: XCTestCase {
  func testQRCommandsAdmitOnlyLocalSelectionAndNoImportedOpticalEvidence() throws {
    let identity: Set<String> = ["setup-id", "request-id"]
    for action in ["qr-send", "qr-continue-send", "qr-recover-send", "qr-recover-receive"] {
      try KagemushaMonetaryProbeManifest.validate(action, names: identity)
      for imported in ["qr-text", "image", "payment", "credited", "envelope", "caller-success"] {
        XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(action, names: identity.union([imported])))
      }
      XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(action, names: ["request-id"]))
    }
    try KagemushaMonetaryProbeManifest.validate("qr-receive", names: identity.union(["amount"]))
    try KagemushaMonetaryProbeManifest.validate("qr-receive", names: identity.union(["amount", "fee-schedule", "fee-certificate"]))
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate("qr-receive", names: identity))
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate("qr-receive", names: identity.union(["amount", "fee-schedule"])))
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate("qr-import", names: ["qr-text"]))
  }
  func testNearbyCommandInputsAreClosedAndRecoveryHasSameOriginalIDs() throws {
    let sender: Set<String> = ["request-id", "session-id", "request-hash", "peer-certificate"]
    for action in ["nearby-send", "nearby-continue-send", "nearby-recover-send"] {
      try KagemushaMonetaryProbeManifest.validate(action, names: sender)
      XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(action, names: sender.union(["payment"])))
    }
    for action in ["nearby-receive", "nearby-recover-receive"] {
      try KagemushaMonetaryProbeManifest.validate(action, names: sender.union(["setup-id"]))
      XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(action, names: sender))
    }
    try KagemushaMonetaryProbeManifest.validate("nearby-prepare", names: ["setup-id", "amount"])
    try KagemushaMonetaryProbeManifest.validate("nearby-prepare", names: ["setup-id", "amount", "fee-schedule", "fee-certificate"])
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate("nearby-prepare", names: ["setup-id", "amount", "fee-schedule"]))
    XCTAssertEqual(try KagemushaMonetaryProbeManifest.maximum("session-id"), 16)
    XCTAssertEqual(try KagemushaMonetaryProbeManifest.maximum("peer-certificate"), 32)
  }
  func testOnlyWholeOptionalFeeAndUnloadBundlesAreAccepted() throws {
    try KagemushaMonetaryProbeManifest.validate("request", names: ["setup-id", "amount"])
    try KagemushaMonetaryProbeManifest.validate(
      "request", names: ["setup-id", "amount", "fee-schedule", "fee-certificate"])
    XCTAssertThrowsError(
      try KagemushaMonetaryProbeManifest.validate(
        "request", names: ["setup-id", "amount", "fee-schedule"]))
    XCTAssertThrowsError(
      try KagemushaMonetaryProbeManifest.validate(
        "review-unload", names: ["request-id", "amount", "quote", "beneficiary"]))
    XCTAssertThrowsError(
      try KagemushaMonetaryProbeManifest.validate(
        "load", names: ["request-id", "receipt", "finality", "success"]))
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate("fake-proof", names: []))
  }
  func testReceiverFirstInputsRequirePaymentCompanionAndRejectRetiredCommands() throws {
    try KagemushaMonetaryProbeManifest.validate(
      "receive", names: ["request-id", "payment", "payer-companion"])
    let invalid: [Set<String>] = [
      ["request-id", "payment"], ["request-id", "payment", "offer"],
      ["request-id", "payment", "payer-companion", "offer"],
    ]
    for names in invalid {
      XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate("receive", names: names))
    }
    for action in ["request", "nearby-prepare"] {
      XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(
        action, names: ["setup-id", "offer"]))
    }
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(
      "offer", names: ["setup-id", "amount"]))
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(
      "decode-offer", names: ["envelope"]))
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.maximum("offer"))
    XCTAssertEqual(try KagemushaMonetaryProbeManifest.maximum("payer-companion"), 2_048)
  }
  func testUnsignedAmountUsesExactTwoLimbsWithoutFloatingPointOrZero() throws {
    let value = try KagemushaMonetaryProbeManifest.amount(Data(repeating: 255, count: 16))
    XCTAssertEqual(value.low, UInt64.max)
    XCTAssertEqual(value.high, UInt64.max)
    var bytes = Data(repeating: 0, count: 16)
    bytes[8] = 1
    XCTAssertEqual(try KagemushaMonetaryProbeManifest.amount(bytes), .init(low: 0, high: 1))
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.amount(Data(repeating: 0, count: 16)))
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.amount(Data(repeating: 1, count: 15)))
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.word(Data(repeating: 0, count: 32)))
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.word(Data(repeating: 1, count: 31)))
  }
  func testConfirmedAndCancelledDecisionsBindExactLiveReview() throws {
    let invocation = UUID().uuidString
    let projection = Data([7, 8, 9])
    var row: [String: Any] = [
      "decision": "confirm", "invocation": invocation, "reviewSequence": 12,
      "reviewSHA256": EnrollmentProbeFiles.digest(projection),
    ]
    func decision(_ row: [String: Any]) throws -> KagemushaMonetaryProbeManifest.Decision {
      try .init(
        EnrollmentProbeFiles.encode(row), invocation: invocation, sequence: 12,
        projection: projection)
    }
    XCTAssertTrue(try decision(row).confirmed)
    row["decision"] = "cancel"
    XCTAssertFalse(try decision(row).confirmed)
    row["reviewSequence"] = 11
    XCTAssertThrowsError(try decision(row))
    row["reviewSequence"] = 12
    row["invocation"] = UUID().uuidString
    XCTAssertThrowsError(try decision(row))
    row["invocation"] = invocation
    row["reviewSHA256"] = EnrollmentProbeFiles.digest(Data([1]))
    XCTAssertThrowsError(try decision(row))
    row["reviewSHA256"] = EnrollmentProbeFiles.digest(projection)
    row["allowStale"] = true
    XCTAssertThrowsError(try decision(row))
  }
  func testIntentBindingCannotChangeAmountsOrOriginalsOnRetry() {
    let first = KagemushaMonetaryProbeManifest.binding(
      "load", inputs: ["amount": Data([1]), "id": Data([2])])
    let reordered = KagemushaMonetaryProbeManifest.binding(
      "load", inputs: ["id": Data([2]), "amount": Data([1])])
    XCTAssertEqual(first, reordered)
    XCTAssertNotEqual(
      first,
      KagemushaMonetaryProbeManifest.binding(
        "load", inputs: ["amount": Data([3]), "id": Data([2])]))
    XCTAssertNotEqual(
      first,
      KagemushaMonetaryProbeManifest.binding(
        "send", inputs: ["amount": Data([1]), "id": Data([2])]))
  }
  func testReceiveBindingRetainsExactPaymentAndCompanionAcrossRetries() {
    let originals = ["payment": Data([1]), "payer-companion": Data([2])]
    let retained = KagemushaMonetaryProbeManifest.binding("receive", inputs: originals)
    XCTAssertEqual(retained, KagemushaMonetaryProbeManifest.binding(
      "receive", inputs: ["payer-companion": Data([2]), "payment": Data([1])]))
    for name in originals.keys {
      var changed = originals
      changed[name] = Data([3])
      XCTAssertNotEqual(retained, KagemushaMonetaryProbeManifest.binding("receive", inputs: changed))
    }
  }
  func testDeliveryVerificationRequiresExactSendAndReceiptWithoutArchiveControls() throws {
    try KagemushaMonetaryProbeManifest.validate("observe-credited", names: ["request-id", "credited"])
    let invalid: [Set<String>] = [[], ["request-id"], ["credited"],
      ["request-id", "credited", "archive"], ["request-id", "credited", "success"],
      ["request-id", "credited", "payment"], ["request-id", "credited", "newer-credited"]]
    for names in invalid {
      XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate("observe-credited", names: names))
    }
    // Exact status remains an observation; fresh Archive selection belongs only to Native.
    for action in ["observe-credited", "credited-status"] {
      try KagemushaMonetaryProbeManifest.validate(action, names: ["request-id", "credited"])
      XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(
        action, names: ["request-id", "credited", "verify-only"]))
    }
    for retired in ["verify-delivery", "accept-credited"] {
      XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(retired, names: ["request-id", "credited"]))
    }
    for action in ["resume-deferred-receipt", "drain-deferred-archive", "begin-exchange", "end-exchange", "discard-review"] {
      try KagemushaMonetaryProbeManifest.validate(action, names: [])
      for names: Set<String> in [["credited"], ["request-id"], ["charging"], ["count"], ["age"], ["force"]] {
        XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(action, names: names))
      }
    }
  }
  func testFreshCommandsAreAdmittedBeforeActorWhileRecoveryAndCleanupStayAvailable() {
    for action in ["request", "review-send", "review-unload", "decide-review", "receive", "observe-credited",
      "begin-time", "finish-time", "nearby-prepare", "nearby-send", "nearby-receive", "qr-send", "qr-receive"] {
      XCTAssertTrue(KagemushaMonetaryProbeManifest.exchangeActions.contains(action), action)
    }
    for action in ["resume", "retry", "fold-once", "snapshot", "resume-deferred-receipt",
      "drain-deferred-archive", "end-exchange", "cancel-time", "discard-review", "release-wallet", "stop"] {
      XCTAssertFalse(KagemushaMonetaryProbeManifest.exchangeActions.contains(action), action)
    }
    XCTAssertTrue(KagemushaMonetaryProbeManifest.exchangeActions.isSubset(of: KagemushaMonetaryProbeManifest.actions))
  }
  func testAttestationDiagnosticsKeepSafeStageDomainAndCodeWithoutDescriptions() {
    let fields = EnrollmentProbeFailureMetadata.fields(
      KagemushaWalletAppleEnrollmentAttestationErrorV1.appAttestFailed(
        stage: .attestKey, domain: "DCErrorDomain", code: 4))
    XCTAssertEqual(fields["attestationStage"] as? String, "attestKey")
    XCTAssertEqual(fields["attestationDomain"] as? String, "DCErrorDomain")
    XCTAssertEqual(fields["attestationCode"] as? Int, 4)
    let locked = EnrollmentProbeFailureMetadata.fields(
      KagemushaWalletAppleEnrollmentAttestationErrorV1.paymentKeyUnavailable(.locked))
    XCTAssertEqual(locked["keyFailure"] as? String, "locked")
    let unexpected = EnrollmentProbeFailureMetadata.fields(
      KagemushaWalletAppleEnrollmentAttestationErrorV1.appAttestFailed(
        stage: .generateKey, domain: "contains secret\ntext", code: 5))
    XCTAssertEqual(unexpected["attestationDomain"] as? String, "unrecognized-domain")
  }
  func testConditionalReviewInputsAndEarlyExchangeAdmissionRemainClosed() throws {
    for action in ["review-send", "continue-review-send"] {
      try KagemushaMonetaryProbeManifest.validate(action, names: ["request-id", "request", "destination"])
      XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(action,
        names: ["request-id", "request", "destination", "absence-proof"]))
    }
    for action in ["review-unload", "continue-review-unload"] {
      try KagemushaMonetaryProbeManifest.validate(action, names: ["request-id", "amount"])
      try KagemushaMonetaryProbeManifest.validate(action,
        names: ["request-id", "amount", "quote", "certificates", "beneficiary"])
      XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate(action,
        names: ["request-id", "amount", "quote", "beneficiary"]))
    }
    try KagemushaMonetaryProbeManifest.validate("retry-reviewed", names: [])
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.validate("retry-reviewed", names: ["request-id"]))
    for action in ["continue-review-send", "continue-review-unload", "retry-reviewed", "nearby-continue-send", "qr-continue-send"] {
      XCTAssertTrue(KagemushaMonetaryProbeManifest.exchangeActions.contains(action))
    }
  }
  func testOnlyAcknowledgedLiveApprovalCanDispatchAndOnlyTypedRetentionCanRetry() throws {
    var state = KagemushaProbeReviewApproval()
    XCTAssertThrowsError(try state.dispatch(retry: false))
    XCTAssertThrowsError(try state.dispatch(retry: true))
    XCTAssertThrowsError(try state.retained())
    try state.acknowledgedFence()
    XCTAssertThrowsError(try state.acknowledgedFence())
    XCTAssertThrowsError(try state.dispatch(retry: true))
    try state.dispatch(retry: false)
    XCTAssertThrowsError(try state.dispatch(retry: false))
    try state.retained()
    XCTAssertEqual(state.phase, .approvedLiveRetained)
    XCTAssertThrowsError(try state.acknowledgedFence())
    XCTAssertThrowsError(try state.dispatch(retry: false))
    try state.dispatch(retry: true); try state.retained()
    try state.dispatch(retry: true); state.finish()
    XCTAssertThrowsError(try state.dispatch(retry: true))
    XCTAssertThrowsError(try state.retained())
    var reopened = KagemushaProbeReviewApproval()
    XCTAssertThrowsError(try reopened.dispatch(retry: true), "files cannot restore live approval")
  }

}

/// Exact controller JSON and refusal rules; these tests do not execute Native retention.
final class KagemushaActivationRetentionProbeTests: XCTestCase {
  private let runID = "AED28BDB-83D3-45E9-A976-D6B4E03224C9"
  private let wire = Data([0, 1, 2, 255])
  private let awaiting = KagemushaWalletLedgerOutcomeV1.awaiting(entrypoint: Data(repeating: 0x0f, count: 32))

  func testReceiptBindsExactWireAndInvocationUsingOnlyStringFields() throws {
    let original = try XCTUnwrap(KagemushaActivationRetentionProbe.receipt(
      signedWire: wire, outcome: awaiting, returned: UInt64.max, invocation: runID, sequence: 3))
    let row = try XCTUnwrap(JSONSerialization.jsonObject(with: original) as? [String: String])
    XCTAssertEqual(row, [
      "schema": "iroha.kagemusha.wallet-activation-retained.v1", "action": "retain-activation",
      "signedWireSHA256": EnrollmentProbeFiles.digest(wire), "signedWireBytes": "4",
      "nativeResult": "awaiting", "entrypointHex": String(repeating: "0f", count: 32),
      "nativeReturnedTicks": String(UInt64.max), "invocation": runID, "sequence": "3",
    ])
    let changed = try XCTUnwrap(KagemushaActivationRetentionProbe.receipt(
      signedWire: Data([0, 1, 2, 254]), outcome: awaiting, returned: UInt64.max,
      invocation: runID, sequence: 3))
    XCTAssertNotEqual(original, changed)
  }

  func testProgressConfirmationAndRejectionCannotAuthorizeInitialSubmission() throws {
    let commit = KagemushaWalletLedgerCommitV1(height: 4, commitHash: Data(repeating: 1, count: 32))
    for outcome in [KagemushaWalletLedgerOutcomeV1.confirmed(commit), .rejected(commit)] {
      XCTAssertNil(try KagemushaActivationRetentionProbe.receipt(
        signedWire: wire, outcome: outcome, returned: 1, invocation: runID, sequence: 1))
    }
  }

  func testMalformedControllerIdentityAndWireCannotProduceReceipt() throws {
    for (bytes, ticks, run, sequence) in [
      (Data(), UInt64(1), runID, 1),
      (Data(repeating: 1, count: 65_537), 1, runID, 1),
      (wire, 0, runID, 1), (wire, 1, "invalid", 1),
      (wire, 1, runID.lowercased(), 1), (wire, 1, runID, 10_000),
      (wire, 1, runID, -1),
    ] {
      XCTAssertThrowsError(try KagemushaActivationRetentionProbe.receipt(
        signedWire: bytes, outcome: awaiting, returned: ticks, invocation: run, sequence: sequence))
    }
  }
  func testReviewCancellationNeedsNoExecutableAdmissionButStillUsesClosedInput() throws {
    let base: [String: Any] = ["decision": "cancel", "invocation": "data-only",
      "reviewSequence": 3, "reviewSHA256": "data-only"]
    func bytes(_ value: [String: Any]) throws -> Data {
      try PropertyListSerialization.data(fromPropertyList: value, format: .binary, options: 0)
    }
    XCTAssertFalse(try KagemushaMonetaryProbeManifest.reviewDecisionConfirms(bytes(base)))
    var confirmed = base; confirmed["decision"] = "confirm"
    XCTAssertTrue(try KagemushaMonetaryProbeManifest.reviewDecisionConfirms(bytes(confirmed)))
    for choice in ["", "CONFIRM", "cancel "] {
      var row = base; row["decision"] = choice
      XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.reviewDecisionConfirms(bytes(row)))
    }
    var extra = base; extra["retry"] = true
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.reviewDecisionConfirms(bytes(extra)))
    XCTAssertThrowsError(try KagemushaMonetaryProbeManifest.Decision(bytes(base),
      invocation: UUID().uuidString, sequence: 3, projection: Data([1])))
  }


}

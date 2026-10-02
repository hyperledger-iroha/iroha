import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Untrusted grammar and scripted transport tests. No sample authenticates native/FI authority.
final class KagemushaAppPlatformPreparedProjectionV1Tests: XCTestCase {
  func testC451PreparationRejectsRetiredLayoutAndSubstitutedNativeScope() throws {
    let f = try preparation()
    let p = try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: digest(0x11), enrollmentChallengeHash: nil)
    XCTAssertEqual(p.platform, 4); XCTAssertEqual(p.appleCounterFloor, 7)
    var enumTaggedC = f
    let cStart = Data("iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0".utf8).count + 8
    enumTaggedC[7][cStart + 2] = 4
    enumTaggedC[4] = Data(SHA256.hash(data: enumTaggedC[7]))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: enumTaggedC,
      approvalID: digest(0x11), enrollmentChallengeHash: nil))
    XCTAssertEqual(p.approval!.clientDataHash, Data(SHA256.hash(data: f[1])))
    for index in [0, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13] {
      var bad = f; bad[index] = Data()
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: bad,
        approvalID: digest(0x11), enrollmentChallengeHash: nil), "field \(index)")
    }
    var retired = f
    let domain = Data("iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0".utf8)
    retired[7].replaceSubrange(domain.count..<(domain.count + 8), with: u64(443))
    retired[7].removeSubrange((domain.count + 8 + 427)..<(domain.count + 8 + 435))
    retired[4] = Data(SHA256.hash(data: retired[7]))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: retired,
      approvalID: digest(0x11), enrollmentChallengeHash: nil))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: digest(0x12), enrollmentChallengeHash: nil))
  }

  func testBootstrapCannotBecomeOrdinaryWDespiteMatchingSubjectHash() throws {
    var f = try preparation()
    f[13][331] = 0
    f[13].replaceSubrange(428..<460, with: Data(repeating: 0, count: 32))
    let start = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    f[1].replaceSubrange((start + 195)..<(start + 227), with: Data(SHA256.hash(data: f[13])))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: digest(0x11), enrollmentChallengeHash: nil))
  }

  func testPreparationRejectsCredentialAndGenerationSubstitutionDespiteRecomputedSubjectHash() throws {
    let f = try preparation()
    let start = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    for changedCredential in [true, false] {
      var bad = f
      if changedCredential {
        bad[13].replaceSubrange(155..<187, with: digest(0x67))
      } else {
        bad[13].replaceSubrange(323..<331, with: u64(3))
      }
      bad[1].replaceSubrange((start + 195)..<(start + 227),
        with: Data(SHA256.hash(data: bad[13])))
      // The standalone codec still accepts a correctly hashed S/W pair; only
      // the called native preparation projection binds the original C/credential.
      XCTAssertNoThrow(try KagemushaAppApprovalSigningProjectionV1(
        nativeSigningBytes: bad[1], nativeFinancialSubject: bad[13]))
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: bad,
        approvalID: digest(0x11), enrollmentChallengeHash: nil))
    }
  }

  func testPreparationRejectsOriginalCScopeSubstitutionDespiteRecomputedSubjectHash() throws {
    let f = try preparation()
    let start = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    for (range, replacement) in [(59..<91, digest(0x91)), (187..<219, digest(0x92)),
      (219..<251, digest(0x93)), (251..<283, digest(0x94)), (283..<291, u64(2))] {
      var bad = f
      bad[13].replaceSubrange(range, with: replacement)
      bad[1].replaceSubrange((start + 195)..<(start + 227),
        with: Data(SHA256.hash(data: bad[13])))
      XCTAssertNoThrow(try KagemushaAppApprovalSigningProjectionV1(
        nativeSigningBytes: bad[1], nativeFinancialSubject: bad[13]))
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: bad,
        approvalID: digest(0x11), enrollmentChallengeHash: nil), "range \(range)")
    }
  }

  func testEnrollmentProjectionRequiresEmptyCredentialAndFinancialSubject() throws {
    var f = try preparation()
    f[1] = enrollmentPossession(key: f[6]); f[8] = Data(); f[13] = Data()
    let p = try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: nil, enrollmentChallengeHash: Data(SHA256.hash(data:challenge())))
    XCTAssertNil(p.approval); XCTAssertTrue(p.credentialDigest.isEmpty)
    var oldAttempt=f
    let eStart=Data("iroha:kagemusha:v1:app-enrollment-possession\0".utf8).count+8
    oldAttempt[1].replaceSubrange((eStart+3)..<(eStart+35),with:digest(1))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields:oldAttempt,
      approvalID:nil,enrollmentChallengeHash:Data(SHA256.hash(data:challenge()))))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields:f,
      approvalID:nil,enrollmentChallengeHash:digest(1)))

    for index in [8, 13] {
      var bad = f; bad[index] = digest(0x55)
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: bad,
        approvalID: nil, enrollmentChallengeHash: Data(SHA256.hash(data:challenge()))))
    }
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: digest(1), enrollmentChallengeHash: nil))
  }

  func testFixedReceiptRejectsOmittedTrailingAndCrossPlatformCounterFields() throws {
    let r = receipt(raw: Data([1, 2, 3]))
    let p = try KagemushaAppPlatformReceiptProjectionV1(r)
    XCTAssertEqual(r.count, 184); XCTAssertEqual(p.appleCounter, 8)
    let nonzeroBasedSlice = (Data(repeating: 0, count: 7) + r).dropFirst(7)
    XCTAssertEqual(try KagemushaAppPlatformReceiptProjectionV1(nonzeroBasedSlice).appleCounter, 8)
    XCTAssertThrowsError(try KagemushaAppPlatformReceiptProjectionV1(Data(r.dropLast())))
    XCTAssertThrowsError(try KagemushaAppPlatformReceiptProjectionV1(r + Data([0])))
    var bad = r; bad[179] = 0
    XCTAssertThrowsError(try KagemushaAppPlatformReceiptProjectionV1(bad))
    bad = r; bad[10] = 0
    XCTAssertThrowsError(try KagemushaAppPlatformReceiptProjectionV1(bad))
    bad = r; bad.replaceSubrange(180..<184, with: Data(repeating: 0, count: 4))
    XCTAssertThrowsError(try KagemushaAppPlatformReceiptProjectionV1(bad))
  }

  func testPhaseGrammarRejectsUnknownAndRecoveredEvidenceSubstitution() throws {
    let ticket = u64(9), raw = Data([1, 2, 3])
    try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval, [u32(3), ticket, raw])
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval,
      [u32(8), ticket]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval,
      [u32(3), ticket, Data(repeating: 1, count: 4097)]))
    try KagemushaAppPlatformFrameV1.validateResponse(.appOperationApproval, [u32(5), ticket],
      [Data([2]), raw, receipt(raw: raw)])
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appOperationApproval,
      [u32(5), ticket], [Data([2]), Data([4]), receipt(raw: raw)]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
      [u32(5), ticket], [Data([2]), raw, receipt(raw: raw)]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appOperationApproval,
      [u32(2), ticket], [Data([1]), raw, Data()]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appOperationApproval,
      [u32(3), ticket, raw], [digest(1)]))
  }


  func testRetailPhaseGrammarBindsMethodTicketOriginalsAndBounds() throws {
    let ticket = u64(17), challenge = Data(repeating: 0x31, count: 32768), message = digest(0x32)
    let requests: [[Data]] = [[u32(9), ticket, challenge, message], [u32(10), ticket],
      [u32(11), ticket, Data(repeating: 0x33, count: 64)],
      [u32(12), ticket, Data(repeating: 0x34, count: 16384)], [u32(13), ticket], [u32(14), ticket]]
    for request in requests {
      XCTAssertNoThrow(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession, request))
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval, request))
      var missingTicket = request; missingTicket[1] = Data(repeating: 0, count: 8)
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession, missingTicket))
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession, request + [Data()]))
    }
    for count in [0, 32769] {
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession,
        [u32(9), ticket, Data(repeating: 1, count: count), message]))
    }
    for count in [0, 16385] {
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession,
        [u32(12), ticket, Data(repeating: 1, count: count)]))
    }
    for count in [0, 63, 65] {
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession,
        [u32(11), ticket, Data(repeating: 1, count: count)]))
    }
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession,
      [u32(9), ticket, challenge, Data(repeating: 0, count: 32)]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession,
      [u32(15), ticket]))
  }

  func testRetailResponsesRejectSubstitutionAndInvalidRecoveryStates() throws {
    let ticket = u64(17), raw = Data([0x31]), message = digest(0x32), signature = Data(repeating: 0x33, count: 64)
    let request = [u32(9), u64(9), raw, message]
    let fields = [ticket, raw, message, digest(0x34), digest(0x35)]
    try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession, request, fields)
    for index in 0..<5 {
      var changed = fields; changed[index] = index == 1 ? Data([0x99]) : Data()
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession, request, changed))
    }
    for bad in [[Data([0]), Data()], [Data([1]), signature], [Data([2]), Data()], [Data([3]), signature]] {
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession, [u32(10), ticket], bad))
    }
    XCTAssertNoThrow(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
      [u32(11), ticket, signature], [Data(SHA256.hash(data: signature))]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
      [u32(11), ticket, signature], [digest(0x55)]))
    for bad in [[Data([0]), signature, Data()], [Data([1]), Data(), Data([1])],
      [Data([2]), signature, Data([1])], [Data([3]), signature, Data()],
      [Data([3]), signature, Data(repeating: 1, count: 16385)], [Data([4]), Data(), Data()]] {
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession, [u32(13), ticket], bad))
    }
    for state in UInt8(0)...1 {
      XCTAssertNoThrow(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
        [u32(13), ticket], [Data([state]), Data(), Data()]))
    }
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
      [u32(12), ticket, raw], [digest(1)]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
      [u32(14), ticket], [digest(1)]))
  }

  func testPossessionChallengeSelectorRechecksNativeOwnerAndReturnsIndependentBytes() throws {
    let (endpoint, bridge, possession, _) = try retailContext()
    let count = endpoint.phases.count
    var selector = try possession.originalEnrollmentChallengeHash()
    XCTAssertEqual(endpoint.phases.dropFirst(count), [6])
    XCTAssertEqual(selector, Data(SHA256.hash(data: challenge())))
    selector[0] ^= 0xff
    XCTAssertEqual(try possession.originalEnrollmentChallengeHash(), Data(SHA256.hash(data: challenge())))
    try bridge.close()
    XCTAssertThrowsError(try possession.originalEnrollmentChallengeHash())
  }

  func testPossessionChallengeSelectorRejectsChangedNativeScope() throws {
    let (endpoint, bridge, possession, _) = try retailContext()
    endpoint.responseOverrides[6] = [digest(0xfe), Data(SHA256.hash(data: endpoint.prepared[1]))]
    XCTAssertThrowsError(try possession.originalEnrollmentChallengeHash())
    try bridge.close()
  }

  func testConsumedEnrollmentReadbackReturnsExactOriginalWithoutDevicePhases() throws {
    let (endpoint, bridge, possession) = try consumedPossessionContext()
    defer { try? bridge.close() }
    let raw = Data([0x91, 0x92]), receipt = possessionReceipt(endpoint.prepared, raw: Data([0x91, 0x92]))
    endpoint.responseOverrides[5] = [Data([2]), raw, receipt]
    let before = endpoint.phases.count
    let original = try possession.recoverOriginalConsumedAssertion()
    XCTAssertEqual(original.rawAssertion, raw)
    XCTAssertEqual(original.receipt.canonicalReceipt, receipt)
    XCTAssertEqual(original.receipt.enrollmentChallengeHash, endpoint.prepared[4])
    XCTAssertEqual(original.receipt.keyAlias, String(decoding: endpoint.prepared[3], as: UTF8.self))
    XCTAssertEqual(original.receipt.keyID, endpoint.prepared[6])
    XCTAssertEqual(original.receipt.observedCounter, 8)
    XCTAssertEqual(Array(endpoint.phases.dropFirst(before)), [6, 6, 5, 6, 5, 6])
    var copied = original.rawAssertion; copied[0] ^= 1
    var receiptCopy = original.receipt.canonicalReceipt; receiptCopy[19] ^= 1
    XCTAssertNotEqual(copied, raw); XCTAssertNotEqual(receiptCopy, receipt)
    let retried = try possession.recoverOriginalConsumedAssertion()
    XCTAssertEqual(retried.rawAssertion, raw)
    XCTAssertEqual(retried.receipt.canonicalReceipt, receipt)
    XCTAssertFalse(endpoint.phases.contains(2)); XCTAssertFalse(endpoint.phases.contains(3))
    XCTAssertFalse(endpoint.phases.contains(4))
  }

  func testConsumedEnrollmentReadbackRejectsPreparedAndRetainedButUnconsumed() throws {
    for state in UInt8(0)...1 {
      let (endpoint, bridge, possession) = try consumedPossessionContext()
      let raw = state == 1 ? Data([0x91]) : Data()
      endpoint.responseOverrides[5] = [Data([state]), raw, Data()]
      XCTAssertThrowsError(try possession.recoverOriginalConsumedAssertion())
      XCTAssertEqual(endpoint.closeCalls, 1)
      let calls = endpoint.phases.count
      XCTAssertThrowsError(try possession.recoverOriginalConsumedAssertion())
      XCTAssertEqual(endpoint.phases.count, calls)
      try bridge.close()
    }
  }

  func testConsumedEnrollmentReadbackRejectsForeignPurposeTicketSelectorScopeAndDigest() throws {
    for offset in [10, 11, 19, 51, 83, 115, 147] {
      let (endpoint, bridge, possession) = try consumedPossessionContext()
      let raw = Data([0x91])
      var receipt = possessionReceipt(endpoint.prepared, raw: raw); receipt[offset] ^= 1
      endpoint.responseOverrides[5] = [Data([2]), raw, receipt]
      XCTAssertThrowsError(try possession.recoverOriginalConsumedAssertion(), "offset \(offset)")
      XCTAssertEqual(endpoint.closeCalls, 1)
      try bridge.close()
    }
  }

  func testConsumedEnrollmentReadbackRejectsCounterAtOrBeforeOriginalFloor() throws {
    for counter: UInt32 in [0, 6, 7] {
      let (endpoint, bridge, possession) = try consumedPossessionContext()
      let raw = Data([0x91])
      endpoint.responseOverrides[5] = [Data([2]), raw, possessionReceipt(endpoint.prepared, raw: raw, counter: counter)]
      XCTAssertThrowsError(try possession.recoverOriginalConsumedAssertion())
      XCTAssertEqual(endpoint.closeCalls, 1)
      try bridge.close()
    }
  }

  func testConsumedEnrollmentReadbackRejectsRawSubstitutionAndBounds() throws {
    for raw in [Data(), Data([0x92]), Data(repeating: 0x91, count: 4097)] {
      let (endpoint, bridge, possession) = try consumedPossessionContext()
      endpoint.responseOverrides[5] = [Data([2]), raw, possessionReceipt(endpoint.prepared, raw: Data([0x91]))]
      XCTAssertThrowsError(try possession.recoverOriginalConsumedAssertion())
      XCTAssertEqual(endpoint.closeCalls, 1)
      try bridge.close()
    }
  }

  func testConsumedEnrollmentReadbackRejectsSelfConsistentChangedSecondOriginal() throws {
    for changeRaw in [false, true] {
      let (endpoint, bridge, possession) = try consumedPossessionContext()
      let first = Data([0x91]), changed = changeRaw ? Data([0x92]) : first
      endpoint.possessionReadbacks = [
        [Data([2]), first, possessionReceipt(endpoint.prepared, raw: first)],
        [Data([2]), changed, possessionReceipt(endpoint.prepared, raw: changed, counter: changeRaw ? 8 : 9)],
      ]
      XCTAssertThrowsError(try possession.recoverOriginalConsumedAssertion())
      XCTAssertEqual(endpoint.closeCalls, 1)
      XCTAssertEqual(endpoint.phases.filter { $0 == 5 }.count, 2)
      try bridge.close()
    }
  }

  func testConsumedEnrollmentReadbackLostResponseRevokesWithoutRetryOrDeviceCall() throws {
    let (endpoint, bridge, possession) = try consumedPossessionContext()
    endpoint.responseOverrides[5] = [Data([2]), Data([0x91]), possessionReceipt(endpoint.prepared, raw: Data([0x91]))]
    endpoint.losePhase = 5
    XCTAssertThrowsError(try possession.recoverOriginalConsumedAssertion())
    XCTAssertEqual(endpoint.closeCalls, 1)
    let calls = endpoint.phases.count
    XCTAssertThrowsError(try possession.recoverOriginalConsumedAssertion())
    XCTAssertEqual(endpoint.phases.count, calls)
    XCTAssertFalse(endpoint.phases.contains(2)); XCTAssertFalse(endpoint.phases.contains(3))
    XCTAssertFalse(endpoint.phases.contains(4))
    try bridge.close()
  }

  func testConsumedEnrollmentReadbackRejectsChangedCurrentOwnerBeforeEvidenceRead() throws {
    let (endpoint, bridge, possession) = try consumedPossessionContext()
    endpoint.responseOverrides[6] = [digest(0xfe), Data(SHA256.hash(data: endpoint.prepared[1]))]
    XCTAssertThrowsError(try possession.recoverOriginalConsumedAssertion())
    XCTAssertFalse(endpoint.phases.contains(5))
    XCTAssertEqual(endpoint.closeCalls, 1)
    try bridge.close()
  }

  func testRetailHolderCorrelatesPhaseEightIdentityAndDistinctTicketNamespaces() throws {
    let (endpoint, bridge, possession, identity) = try retailContext()
    XCTAssertTrue(endpoint.phases.contains(8))
    let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    XCTAssertNotEqual(endpoint.prepared[0], endpoint.retailFields[0])
    XCTAssertEqual(try held.originalChallengeBytes(), endpoint.challenge)
    var copy = try held.accountSigningMessage(); copy[0] ^= 1
    XCTAssertEqual(try held.accountSigningMessage(), endpoint.message)
    XCTAssertEqual(try held.recoverOriginals().state, .prepared)
    let retry = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    XCTAssertEqual(try retry.originalChallengeBytes(), endpoint.challenge)
    XCTAssertEqual(endpoint.closeCalls, 0)
    try bridge.close()
  }

  func testRetailPreparationRejectsForeignPhaseEightScopeAndCredential() throws {
    for changedScope in [true, false] {
      let (endpoint, _, possession, _) = try retailContext()
      let (foreign, foreignBridge, _, foreignIdentity) = try retailContext(
        scope: changedScope ? digest(0x98) : digest(0x99), credential: digest(0x75))
      XCTAssertTrue(foreign.phases.contains(8))
      XCTAssertThrowsError(try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
        accountSigningMessage: endpoint.message, identity: foreignIdentity))
      XCTAssertEqual(endpoint.closeCalls, 1)
      if changedScope { XCTAssertFalse(endpoint.phases.contains(9)) }
      try foreignBridge.close()
    }
  }

  func testRetailOriginalSubstitutionRevokesBeforeSigning() throws {
    for index in 0..<5 {
      let (endpoint, _, possession, identity) = try retailContext()
      let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
        accountSigningMessage: endpoint.message, identity: identity)
      endpoint.retailFields[index][0] ^= 1
      XCTAssertThrowsError(try held.fenceAccountSigning(), "field \(index)")
      XCTAssertFalse(endpoint.phases.contains(10))
      XCTAssertEqual(endpoint.closeCalls, 1)
      let count = endpoint.phases.count
      XCTAssertThrowsError(try held.originalChallengeBytes())
      XCTAssertEqual(endpoint.phases.count, count)
    }
  }

  func testRetailCurrentPossessionScopeDriftRevokesBeforeRetailDispatch() throws {
    let (endpoint, _, possession, identity) = try retailContext()
    let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    endpoint.responseOverrides[6] = [digest(0x76), Data(SHA256.hash(data: endpoint.prepared[1]))]
    let phaseNineCount = endpoint.phases.filter { $0 == 9 }.count
    XCTAssertThrowsError(try held.accountSigningMessage())
    XCTAssertEqual(endpoint.phases.filter { $0 == 9 }.count, phaseNineCount)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testRetailFreshFenceGrantsSigningOnceAndRecoveryUnknownNeverGrantsAgain() throws {
    let (endpoint, _, possession, identity) = try retailContext()
    let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    guard case let .signOriginalMessage(message) = try held.fenceAccountSigning() else {
      return XCTFail("expected the one freshly fenced message")
    }
    XCTAssertEqual(message, endpoint.message)
    XCTAssertEqual(try held.recoverOriginals().state, .invocationUnknown)
    // Even a scripted endpoint willing to return fresh permission must not be called.
    endpoint.responseOverrides[10] = [Data([1]), Data()]
    XCTAssertThrowsError(try held.fenceAccountSigning())
    XCTAssertEqual(endpoint.phases.filter { $0 == 10 }.count, 1)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testRetailRecoveredUnknownPreventsFirstManagedSigningCall() throws {
    let (endpoint, _, possession, identity) = try retailContext()
    endpoint.stage = 1
    let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    XCTAssertEqual(try held.recoverOriginals().state, .invocationUnknown)
    XCTAssertThrowsError(try held.fenceAccountSigning())
    XCTAssertFalse(endpoint.phases.contains(10))
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testRetailRecoveryCannotRegressInvokedToPrepared() throws {
    let (endpoint, _, possession, identity) = try retailContext()
    let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    _ = try held.fenceAccountSigning()
    endpoint.responseOverrides[13] = [Data([0]), Data(), Data()]
    XCTAssertThrowsError(try held.recoverOriginals())
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testRetailRetainsExactWalletAndMaximumCertificateOriginalsAcrossRetries() throws {
    let (endpoint, bridge, possession, identity) = try retailContext()
    let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    _ = try held.fenceAccountSigning()
    let signature = Data(repeating: 0x45, count: 64), certificate = Data(repeating: 0x46, count: 16384)
    try held.retainOriginalAccountSignature(signature)
    try held.retainOriginalAccountSignature(signature)
    guard case let .retainedOriginalSignature(retained) = try held.fenceAccountSigning() else {
      return XCTFail("a retained signature must never authorize a new signing invocation")
    }
    XCTAssertEqual(retained, signature)
    let first = try held.acceptOriginalEnrollmentCertificate(certificate)
    let retry = try held.acceptOriginalEnrollmentCertificate(certificate)
    XCTAssertEqual(first.enrollmentID, retry.enrollmentID)
    XCTAssertEqual(first.pendingScope, identity.pendingScope)
    let recovered = try held.recoverOriginals()
    XCTAssertEqual(recovered.state, .certificateRetained)
    XCTAssertEqual(recovered.accountSignature, signature)
    XCTAssertEqual(recovered.certificateOriginal, certificate)
    XCTAssertEqual(endpoint.phases.filter { $0 == 11 }.count, 2)
    XCTAssertEqual(endpoint.phases.filter { $0 == 12 }.count, 2)
    XCTAssertEqual(endpoint.closeCalls, 0)
    try bridge.close()
  }

  func testRetailChangedSignatureOrCertificateRetryClosesBeforeIntake() throws {
    for changeCertificate in [false, true] {
      let (endpoint, _, possession, identity) = try retailContext()
      let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
        accountSigningMessage: endpoint.message, identity: identity)
      _ = try held.fenceAccountSigning()
      try held.retainOriginalAccountSignature(Data(repeating: 0x45, count: 64))
      if changeCertificate {
        _ = try held.acceptOriginalEnrollmentCertificate(Data([0x46]))
        XCTAssertThrowsError(try held.acceptOriginalEnrollmentCertificate(Data([0x47])))
        XCTAssertEqual(endpoint.phases.filter { $0 == 12 }.count, 1)
      } else {
        XCTAssertThrowsError(try held.retainOriginalAccountSignature(Data(repeating: 0x47, count: 64)))
        XCTAssertEqual(endpoint.phases.filter { $0 == 11 }.count, 1)
      }
      XCTAssertEqual(endpoint.closeCalls, 1)
    }
  }

  func testRetailCertificateScopeSubstitutionClosesWithoutConfirmation() throws {
    let (endpoint, _, possession, identity) = try retailContext()
    let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    _ = try held.fenceAccountSigning()
    try held.retainOriginalAccountSignature(Data(repeating: 0x45, count: 64))
    endpoint.responseOverrides[12] = [digest(0x50), digest(0x51)]
    XCTAssertThrowsError(try held.acceptOriginalEnrollmentCertificate(Data([0x46])))
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testRetailExactCertificateRetryRejectsChangedConfirmation() throws {
    let (endpoint, _, possession, identity) = try retailContext()
    let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    _ = try held.fenceAccountSigning()
    try held.retainOriginalAccountSignature(Data(repeating: 0x45, count: 64))
    _ = try held.acceptOriginalEnrollmentCertificate(Data([0x46]))
    endpoint.responseOverrides[12] = [digest(0x76), identity.pendingScope]
    XCTAssertThrowsError(try held.acceptOriginalEnrollmentCertificate(Data([0x46])))
    XCTAssertEqual(endpoint.phases.filter { $0 == 12 }.count, 2)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testRetailRecoveredCertificateRequiresPhaseTwelveBeforeConfirmation() throws {
    let (endpoint, bridge, possession, identity) = try retailContext()
    endpoint.stage = 3; endpoint.signature = Data(repeating: 0x45, count: 64)
    endpoint.certificate = Data([0x46])
    let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    let recovered = try held.recoverOriginals()
    XCTAssertEqual(recovered.state, .certificateRetained)
    XCTAssertEqual(recovered.certificateOriginal, endpoint.certificate)
    XCTAssertFalse(endpoint.phases.contains(12))
    let confirmation = try held.acceptOriginalEnrollmentCertificate(recovered.certificateOriginal)
    XCTAssertEqual(confirmation.pendingScope, identity.pendingScope)
    XCTAssertEqual(endpoint.phases.filter { $0 == 12 }.count, 1)
    try bridge.close()
  }

  func testRetailRecoveryRejectsChangedOrDisappearingRetainedOriginals() throws {
    for response in [[Data([2]), Data(repeating: 0x47, count: 64), Data()],
      [Data([1]), Data(), Data()]] {
      let (endpoint, _, possession, identity) = try retailContext()
      let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
        accountSigningMessage: endpoint.message, identity: identity)
      _ = try held.fenceAccountSigning()
      try held.retainOriginalAccountSignature(Data(repeating: 0x45, count: 64))
      endpoint.responseOverrides[13] = response
      XCTAssertThrowsError(try held.recoverOriginals())
      XCTAssertEqual(endpoint.closeCalls, 1)
    }
  }

  func testRetailBoundsRejectBeforeNativeDispatch() throws {
    let (endpoint, bridge, possession, identity) = try retailContext()
    for count in [0, 32769] {
      let before = endpoint.phases.count
      XCTAssertThrowsError(try possession.prepareRetailEnrollment(originalChallenge: Data(repeating: 1, count: count),
        accountSigningMessage: endpoint.message, identity: identity))
      XCTAssertEqual(endpoint.phases.count, before)
    }
    let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    let before = endpoint.phases.count
    for count in [0, 16385] {
      XCTAssertThrowsError(try held.acceptOriginalEnrollmentCertificate(Data(repeating: 1, count: count)))
    }
    for count in [0, 63, 65] {
      XCTAssertThrowsError(try held.retainOriginalAccountSignature(Data(repeating: 1, count: count)))
    }
    XCTAssertEqual(endpoint.phases.count, before)
    XCTAssertEqual(endpoint.closeCalls, 0)
    try bridge.close()
  }

  func testLostRetailPreparationResponseClosesWithoutAnotherIntake() throws {
    let (endpoint, _, possession, identity) = try retailContext()
    endpoint.losePhase = 9
    XCTAssertThrowsError(try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity))
    let before = endpoint.phases.count
    XCTAssertThrowsError(try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity))
    XCTAssertEqual(endpoint.phases.count, before)
    XCTAssertEqual(endpoint.phases.filter { $0 == 9 }.count, 1)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testLostRetailMutationResponseClosesAndCannotRepeatTheAction() throws {
    for phase in [UInt32(10), 11, 12] {
      let (endpoint, _, possession, identity) = try retailContext()
      let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
        accountSigningMessage: endpoint.message, identity: identity)
      let signature = Data(repeating: 0x45, count: 64)
      if phase > 10 { _ = try held.fenceAccountSigning() }
      if phase > 11 { try held.retainOriginalAccountSignature(signature) }
      endpoint.losePhase = phase
      switch phase {
      case 10: XCTAssertThrowsError(try held.fenceAccountSigning())
      case 11: XCTAssertThrowsError(try held.retainOriginalAccountSignature(signature))
      default: XCTAssertThrowsError(try held.acceptOriginalEnrollmentCertificate(Data([0x46])))
      }
      let before = endpoint.phases.count
      XCTAssertThrowsError(try held.fenceAccountSigning())
      XCTAssertThrowsError(try held.recoverOriginals())
      XCTAssertEqual(endpoint.phases.count, before)
      XCTAssertEqual(endpoint.phases.filter { $0 == phase }.count, 1)
      XCTAssertEqual(endpoint.closeCalls, 1)
    }
  }

  func testRetailCancellationIsOnlyPreparedAndRetiresTheHolder() throws {
    for invoked in [false, true] {
      let (endpoint, bridge, possession, identity) = try retailContext()
      let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
        accountSigningMessage: endpoint.message, identity: identity)
      if invoked {
        _ = try held.fenceAccountSigning()
        XCTAssertThrowsError(try held.cancel())
        XCTAssertFalse(endpoint.phases.contains(14))
        XCTAssertEqual(endpoint.closeCalls, 1)
      } else {
        try held.cancel()
        XCTAssertEqual(endpoint.phases.filter { $0 == 14 }.count, 1)
        let before = endpoint.phases.count
        XCTAssertThrowsError(try held.originalChallengeBytes())
        XCTAssertEqual(endpoint.phases.count, before)
        try bridge.close()
      }
    }
  }

  func testBootstrapProjectionRequiresExactZeroStateAndNeverCreatesOrdinaryMoneyProjection() throws {
    let (endpoint, bridge, _, _) = try retailContext()
    let f = try bootstrapPreparation(endpoint)
    let p = try KagemushaAppPlatformPreparedProjectionV1(nativeBootstrapFields: f,
      operationID: Data(SHA256.hash(data: Data("iroha:kagemusha:v1:ordinary-bootstrap-operation-id\0".utf8) + Data([0x46]))),
      credentialDigest: endpoint.credential)
    XCTAssertNil(p.approval); XCTAssertNotNil(p.bootstrapApproval)
    XCTAssertNoThrow(try KagemushaAppPlatformPreparedProjectionV1.validateBootstrapTransport(f,
      operationID: p.bootstrapApproval!.operationID))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1.validateApprovalTransport(f,
      operationID: p.bootstrapApproval!.operationID))
    try KagemushaAppPlatformFrameV1.validateResponse(.appOperationApproval,
      [u32(8), p.bootstrapApproval!.operationID], f)
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appOperationApproval,
      [u32(1), p.bootstrapApproval!.operationID], f))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: p.bootstrapApproval!.operationID, enrollmentChallengeHash: nil))
    let wStart = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    for offset in [364, 396, 428, 444] {
      var changed = f; changed[13][offset] = 1
      changed[1].replaceSubrange((wStart + 195)..<(wStart + 227), with: Data(SHA256.hash(data: changed[13])))
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1.validateBootstrapTransport(changed,
        operationID: p.bootstrapApproval!.operationID), "zero field \(offset)")
    }
    var preparePurpose = f; preparePurpose[1][wStart + 2] = 2
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1.validateBootstrapTransport(preparePurpose,
      operationID: p.bootstrapApproval!.operationID))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeBootstrapFields: f,
      operationID: p.bootstrapApproval!.operationID, credentialDigest: digest(0xfe)))
    try bridge.close()
  }

  func testBootstrapRequiresRetainedFIAndStableOriginalCertificateSelector() throws {
    let (endpoint, bridge, possession, identity) = try retailContext()
    let held = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    XCTAssertThrowsError(try held.prepareBootstrapAppApproval())
    XCTAssertTrue(endpoint.bootstrapPhases.isEmpty)
    try bridge.close()
    let (second, secondBridge, retail, bootstrap) = try bootstrapContext()
    let original = try bootstrap.signingBytes()
    let retry = try retail.prepareBootstrapAppApproval()
    XCTAssertEqual(try retry.signingBytes(), original)
    let expected = Data(SHA256.hash(data: Data("iroha:kagemusha:v1:ordinary-bootstrap-operation-id\0".utf8) + second.certificate))
    XCTAssertEqual(second.bootstrapSelectors, [expected, expected])
    try secondBridge.close()
  }

  func testBootstrapRejectsForeignCredentialBeforeReturningHolder() throws {
    let (endpoint, _, retail, _) = try bootstrapContext()
    var f = try bootstrapPreparation(endpoint)
    let start = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    f[8] = digest(0xee); f[13].replaceSubrange(155..<187, with: f[8])
    f[1].replaceSubrange((start + 163)..<(start + 195), with: f[8])
    f[1].replaceSubrange((start + 195)..<(start + 227), with: Data(SHA256.hash(data: f[13])))
    endpoint.bootstrapFields = f
    XCTAssertThrowsError(try retail.prepareBootstrapAppApproval())
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testBootstrapHolderRechecksFIAndNativeOriginalScopeBeforeFence() throws {
    for fiDrift in [true, false] {
      let (endpoint, _, _, bootstrap) = try bootstrapContext()
      if fiDrift { endpoint.retailFields[4] = digest(0xfe) }
      else { endpoint.bootstrapResponseOverrides[6] = [digest(0xfe), digest(0xfd)] }
      XCTAssertThrowsError(try bootstrap.fence())
      XCTAssertFalse(endpoint.bootstrapPhases.contains(2))
      XCTAssertEqual(endpoint.closeCalls, 1)
      let count = endpoint.bootstrapPhases.count
      XCTAssertThrowsError(try bootstrap.signingBytes())
      XCTAssertEqual(endpoint.bootstrapPhases.count, count)
    }
  }

  func testBootstrapNativeCaptureOriginalAndActualFileIntentRetryNeverInvokeAppleAgain() async throws {
    let (endpoint, bridge, _, bootstrap) = try bootstrapContext(floor: 0)
    let projection = try bootstrap.recheck()
    let raw = try bootstrapAssertion(projection)
    let service = BootstrapService(raw: raw)
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
      attributes: [.posixPermissions: 0o700])
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(directoryURL: directory,
      keyID: projection.keyAlias)
    let provider = KagemushaAppAttestBootstrapApprovalProviderV1(service: service,
      intentStore: store, expectedRelease: try bootstrapRelease())
    let first = try await provider.captureBootstrap(bootstrap)
    XCTAssertEqual(first.enrollmentID, digest(0x75)); XCTAssertEqual(first.observedCounter, 1)
    XCTAssertEqual(first.rawAssertionDigest, Data(SHA256.hash(data: raw)))
    XCTAssertEqual(try store.load(keyID: projection.keyAlias), .ready(counter: 1))
    let retry = try await provider.recoverCapturedBootstrap(bootstrap)
    XCTAssertEqual(retry.canonicalReceipt, first.canonicalReceipt)
    let calls = await service.calls(); XCTAssertEqual(calls, 1)
    XCTAssertEqual(endpoint.bootstrapPhases.filter { $0 == 4 }.count, 1)
    XCTAssertEqual(endpoint.bootstrapRaw, raw)
    try bridge.close()
  }

  func testBootstrapMeasuredAppleOriginalAuthenticatesFullBytesAndCounter() throws {
    let (endpoint, bridge, _, bootstrap) = try bootstrapContext(floor: 0)
    let p = try bootstrap.recheck()
    let raw = try bootstrapAssertion(p, measured: true)
    let value = try bootstrapEvidence(raw, projection: p)
    XCTAssertEqual(value.observedCounter, 1)
    XCTAssertEqual(value.rawAssertion, raw)
    XCTAssertThrowsError(try bootstrapEvidence(bootstrapAssertion(p, measured: true, wrongHash: true), projection: p))
    endpoint.bootstrapRaw = raw; endpoint.bootstrapStage = 2
    let r = try bootstrap.consume(original: value)
    XCTAssertEqual(r.signingDigest, p.bootstrapApproval!.clientDataHash)
    try bridge.close()
  }

  func testBootstrapLostCaptureResponseRevokesHolderWithoutSecondInvocation() throws {
    let (endpoint, _, _, bootstrap) = try bootstrapContext(floor: 0)
    let p = try bootstrap.recheck(), raw = try bootstrapAssertion(p)
    _ = try bootstrap.fence(); try bootstrap.retainOriginal(raw)
    endpoint.loseBootstrapPhase = 4
    XCTAssertThrowsError(try bootstrap.consume(original: bootstrapEvidence(raw, projection: p)))
    let count = endpoint.bootstrapPhases.count
    XCTAssertThrowsError(try bootstrap.fence())
    XCTAssertThrowsError(try bootstrap.recover())
    XCTAssertEqual(endpoint.bootstrapPhases.count, count)
    XCTAssertEqual(endpoint.bootstrapPhases.filter { $0 == 4 }.count, 1)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testTypedBootstrapPhaseEightRejectsTicketGrammarAndOrdinarySubjects() throws {
    let (endpoint, bridge, _, bootstrap) = try bootstrapContext()
    let f = try bootstrapPreparation(endpoint), id = try bootstrap.recheck().bootstrapApproval!.operationID
    XCTAssertEqual(endpoint.bootstrapPhases.first, 8)
    for bad in [[u32(8)], [u32(8), u64(31)], [u32(8), Data(repeating: 0, count: 32)],
      [u32(8), id, Data([1])]] {
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval, bad))
    }
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession, [u32(8), id]))
    var cash = f; cash[13][331] = 1; cash[13][444] = 1
    let start = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    cash[1].replaceSubrange((start + 195)..<(start + 227), with: Data(SHA256.hash(data: cash[13])))
    XCTAssertNoThrow(try KagemushaAppPlatformPreparedProjectionV1.validateApprovalTransport(cash, operationID: id))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1.validateBootstrapTransport(cash, operationID: id))
    try bridge.close()
  }

  func testTypedBootstrapRejectsRehashedForeignCScope() throws {
    let (endpoint, bridge, _, bootstrap) = try bootstrapContext()
    let f = try bootstrapPreparation(endpoint), id = try bootstrap.recheck().bootstrapApproval!.operationID
    let start = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    for (range, bytes) in [(59..<91, digest(91)), (155..<187, digest(92)),
      (187..<219, digest(93)), (219..<251, digest(94)), (251..<283, digest(95)),
      (283..<291, u64(3)), (323..<331, u64(3))] {
      var changed = f; changed[13].replaceSubrange(range, with: bytes)
      changed[1].replaceSubrange((start + 195)..<(start + 227), with: Data(SHA256.hash(data: changed[13])))
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1.validateBootstrapTransport(changed,
        operationID: id), "range \(range)")
    }
    try bridge.close()
  }

  func testBootstrapInvokedOrRegressedRecoveryNeverPermitsAnotherFence() throws {
    for regress in [false, true] {
      let (endpoint, _, _, bootstrap) = try bootstrapContext()
      XCTAssertEqual(try bootstrap.fence().state, 1)
      if regress { endpoint.bootstrapStage = 0 }
      XCTAssertThrowsError(try bootstrap.fence())
      XCTAssertEqual(endpoint.bootstrapPhases.filter { $0 == 2 }.count, 1)
      XCTAssertEqual(endpoint.closeCalls, 1)
    }
  }

  func testBootstrapRetainedRawAndReceiptCannotChangeOrRegress() throws {
    for changeReceipt in [false, true] {
      let (endpoint, _, _, bootstrap) = try bootstrapContext(floor: 0)
      let p = try bootstrap.recheck(), raw = try bootstrapAssertion(p)
      _ = try bootstrap.fence(); try bootstrap.retainOriginal(raw)
      let consumed = try bootstrap.consume(original: bootstrapEvidence(raw, projection: p))
      if changeReceipt {
        var changed = consumed.canonicalReceipt; changed[51] ^= 1
        endpoint.bootstrapResponseOverrides[5] = [Data([2]), raw, changed]
      } else {
        endpoint.bootstrapResponseOverrides[5] = [Data([1]), raw + Data([0]), Data()]
      }
      XCTAssertThrowsError(try bootstrap.recover())
      XCTAssertEqual(endpoint.closeCalls, 1)
    }
  }

  func testLostBootstrapPreparationReplyRevokesBeforeAnotherReservation() throws {
    let (endpoint, _, retail, _) = try bootstrapContext()
    endpoint.loseBootstrapPhase = 8
    XCTAssertThrowsError(try retail.prepareBootstrapAppApproval())
    let before = endpoint.bootstrapPhases.count
    XCTAssertThrowsError(try retail.prepareBootstrapAppApproval())
    XCTAssertEqual(endpoint.bootstrapPhases.count, before)
    XCTAssertEqual(endpoint.bootstrapPhases.filter { $0 == 8 }.count, 2)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testLostBootstrapFenceOrRawRetentionReplyCannotRepeat() throws {
    for phase in [UInt32(2), 3] {
      let (endpoint, _, _, bootstrap) = try bootstrapContext(floor: 0)
      let raw = try bootstrapAssertion(bootstrap.recheck())
      if phase == 3 { _ = try bootstrap.fence() }
      endpoint.loseBootstrapPhase = phase
      if phase == 2 { XCTAssertThrowsError(try bootstrap.fence()) }
      else { XCTAssertThrowsError(try bootstrap.retainOriginal(raw)) }
      let before = endpoint.bootstrapPhases.count
      XCTAssertThrowsError(try bootstrap.fence()); XCTAssertThrowsError(try bootstrap.recover())
      XCTAssertEqual(endpoint.bootstrapPhases.count, before)
      XCTAssertEqual(endpoint.bootstrapPhases.filter { $0 == phase }.count, 1)
      XCTAssertEqual(endpoint.closeCalls, 1)
    }
  }

  func testBootstrapColdRecoveryAfterLostRetentionOrConsumeNeverCallsAppleAgain() async throws {
    for phase in [UInt32(3), 4] {
      let (endpoint, _, _, bootstrap) = try bootstrapContext(floor: 0)
      let p = try bootstrap.recheck(), service = BootstrapService(raw: try bootstrapAssertion(p))
      let directory = try bootstrapPrivateDirectory()
      let store = try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(directoryURL: directory, keyID: p.keyAlias)
      let provider = KagemushaAppAttestBootstrapApprovalProviderV1(service: service,
        intentStore: store, expectedRelease: try bootstrapRelease())
      endpoint.loseBootstrapPhase = phase
      do { _ = try await provider.captureBootstrap(bootstrap); XCTFail() } catch {}
      XCTAssertEqual(endpoint.closeCalls, 1)
      endpoint.loseBootstrapPhase = nil
      let (reopenedBridge, recovered) = try reopenBootstrap(endpoint)
      let reopenedStore = try KagemushaAppAttestFileIntentStoreV1(directoryURL: directory)
      let recoveryProvider = KagemushaAppAttestBootstrapApprovalProviderV1(service: service,
        intentStore: reopenedStore, expectedRelease: try bootstrapRelease())
      let receipt = try await recoveryProvider.recoverCapturedBootstrap(recovered)
      XCTAssertEqual(receipt.enrollmentID, digest(0x75))
      XCTAssertEqual(receipt.signingDigest, p.bootstrapApproval!.clientDataHash)
      XCTAssertEqual(try reopenedStore.load(keyID: p.keyAlias), .ready(counter: 1))
      let calls = await service.calls(); XCTAssertEqual(calls, 1)
      XCTAssertEqual(endpoint.bootstrapPhases.filter { $0 == 2 }.count, 1)
      try reopenedBridge.close()
    }
  }

  func testConcreteBootstrapIntentAdvanceRejectsForeignFieldsAndRepeatedAdvance() throws {
    let (_, bridge, _, bootstrap) = try bootstrapContext(floor: 0)
    let p = try bootstrap.recheck(), raw = try bootstrapAssertion(p)
    _ = try bootstrap.fence(); try bootstrap.retainOriginal(raw)
    let receipt = try bootstrap.consume(original: bootstrapEvidence(raw, projection: p))
    let directory = try bootstrapPrivateDirectory()
    let store = try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(directoryURL: directory, keyID: p.keyAlias)
    let hash = p.bootstrapApproval!.clientDataHash
    try store.reserve(keyID: p.keyAlias, previousCounter: 0, selectionDigest: hash)
    try store.complete(keyID: p.keyAlias, counter: 1, selectionDigest: hash, rawAssertion: raw)
    let before = try store.load(keyID: p.keyAlias)
    for (key, count, digest, bytes) in [(p.keyAlias + "x", UInt32(1), hash, raw),
      (p.keyAlias, 2, hash, raw), (p.keyAlias, 1, self.digest(9), raw), (p.keyAlias, 1, hash, raw + Data([0]))] {
      XCTAssertThrowsError(try store.advanceAfterNativeBootstrapCapture(keyID: key, counter: count,
        signingDigest: digest, rawAssertion: bytes, receipt: receipt))
      XCTAssertEqual(try store.load(keyID: p.keyAlias), before)
    }
    try store.advanceAfterNativeBootstrapCapture(keyID: p.keyAlias, counter: 1,
      signingDigest: hash, rawAssertion: raw, receipt: receipt)
    let reopened = try KagemushaAppAttestFileIntentStoreV1(directoryURL: directory)
    XCTAssertEqual(try reopened.load(keyID: p.keyAlias), .ready(counter: 1))
    XCTAssertThrowsError(try reopened.advanceAfterNativeBootstrapCapture(keyID: p.keyAlias, counter: 1,
      signingDigest: hash, rawAssertion: raw, receipt: receipt))
    XCTAssertEqual(try reopened.load(keyID: p.keyAlias), .ready(counter: 1))
    try bridge.close()
  }

  func testConcreteBootstrapIntentAdvanceRejectsAnotherTypedWReceipt() throws {
    let (_, firstBridge, _, first) = try bootstrapContext(floor: 0)
    let p = try first.recheck(), raw = try bootstrapAssertion(p), hash = p.bootstrapApproval!.clientDataHash
    let (endpoint, secondBridge, retail, _) = try bootstrapContext(floor: 0)
    var fields = try XCTUnwrap(endpoint.bootstrapFields)
    let nonce = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8 + 35
    fields[1][nonce] ^= 1; endpoint.bootstrapFields = fields
    let foreign = try retail.prepareBootstrapAppApproval(), foreignProjection = try foreign.recheck()
    let foreignRaw = try bootstrapAssertion(foreignProjection)
    _ = try foreign.fence(); try foreign.retainOriginal(foreignRaw)
    let receipt = try foreign.consume(original: bootstrapEvidence(foreignRaw, projection: foreignProjection))
    let store = try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(directoryURL: bootstrapPrivateDirectory(), keyID: p.keyAlias)
    try store.reserve(keyID: p.keyAlias, previousCounter: 0, selectionDigest: hash)
    try store.complete(keyID: p.keyAlias, counter: 1, selectionDigest: hash, rawAssertion: raw)
    let before = try store.load(keyID: p.keyAlias)
    XCTAssertThrowsError(try store.advanceAfterNativeBootstrapCapture(keyID: p.keyAlias, counter: 1,
      signingDigest: hash, rawAssertion: raw, receipt: receipt))
    XCTAssertEqual(try store.load(keyID: p.keyAlias), before)
    try firstBridge.close(); try secondBridge.close()
  }

  func testBootstrapCancellationIsOnlyPreparedAndRetiresTheHolder() throws {
    for invoked in [false, true] {
      let (endpoint, bridge, _, bootstrap) = try bootstrapContext()
      if invoked {
        _ = try bootstrap.fence()
        XCTAssertThrowsError(try bootstrap.cancel())
        XCTAssertFalse(endpoint.bootstrapPhases.contains(7))
      } else {
        try bootstrap.cancel()
        let count = endpoint.bootstrapPhases.count
        XCTAssertThrowsError(try bootstrap.signingBytes())
        XCTAssertEqual(endpoint.bootstrapPhases.count, count)
      }
      try bridge.close()
    }
  }

  private func bootstrapPrivateDirectory() throws -> URL {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("typed-bootstrap-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
      attributes: [.posixPermissions: 0o700])
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    return directory
  }
  private func reopenBootstrap(_ endpoint: RetailEndpoint) throws
    -> (KagemushaCoreCoordinatorBridgeV1, KagemushaNativePreparedBootstrapAppApprovalV1) {
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/untrusted-scripted-bootstrap-reopen", endpoint: endpoint)
    let possession = try bridge.prepareAppEnrollmentPossession(originalEnrollmentChallengeHash: endpoint.prepared[4])
    let identity = try possession.acceptOriginalFinalCredential(Data([0x71]))
    let retail = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    _ = try retail.acceptOriginalEnrollmentCertificate(endpoint.certificate)
    return (bridge, try retail.prepareBootstrapAppApproval())
  }

  private func bootstrapContext(floor: UInt32? = nil) throws -> (RetailEndpoint,
    KagemushaCoreCoordinatorBridgeV1, KagemushaNativePreparedRetailEnrollmentV1,
    KagemushaNativePreparedBootstrapAppApprovalV1) {
    let (endpoint, bridge, possession, identity) = try retailContext(floor: floor)
    let retail = try possession.prepareRetailEnrollment(originalChallenge: endpoint.challenge,
      accountSigningMessage: endpoint.message, identity: identity)
    _ = try retail.fenceAccountSigning()
    try retail.retainOriginalAccountSignature(Data(repeating: 0x45, count: 64))
    _ = try retail.acceptOriginalEnrollmentCertificate(Data([0x46]))
    endpoint.bootstrapFields = try bootstrapPreparation(endpoint)
    return (endpoint, bridge, retail, try retail.prepareBootstrapAppApproval())
  }
  private func bootstrapPreparation(_ endpoint: RetailEndpoint) throws -> [Data] {
    var f = try preparation()
    let start = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    let certificate = endpoint.certificate.isEmpty ? Data([0x46]) : endpoint.certificate
    let id = Data(SHA256.hash(data: Data("iroha:kagemusha:v1:ordinary-bootstrap-operation-id\0".utf8) + certificate))
    f[0] = u64(31); f[8] = endpoint.credential; f[9] = digest(0x77); f[10] = endpoint.prepared[10]
    f[13][331] = 0; f[13].replaceSubrange(364..<460, with: Data(repeating: 0, count: 96))
    f[13].replaceSubrange(155..<187, with: f[8])
    f[1].replaceSubrange((start + 3)..<(start + 35), with: id)
    f[1].replaceSubrange((start + 163)..<(start + 195), with: f[8])
    f[1].replaceSubrange((start + 195)..<(start + 227), with: Data(SHA256.hash(data: f[13])))
    return f
  }
  /// Known-public test policy; this digest supplies no signed Native release authority.
  private func bootstrapRelease() throws -> KagemushaAppAttestExpectedReleaseV1 {
    try .init(validationCategory: 2, bundleVersion: "100", authenticatedAppReleaseDigest:
      KagemushaAppAttestExpectedReleaseV1.canonicalReleaseDigest(validationCategory: 2, bundleVersion: "100"))
  }
  private func bootstrapEvidence(_ raw: Data, projection: KagemushaAppPlatformPreparedProjectionV1) throws
    -> KagemushaAppAttestBootstrapApprovalOriginalV1 {
    try KagemushaAppAttestBootstrapApprovalOriginalV1(rawAssertion: raw,
      nativeProjection: projection.bootstrapApproval!, enrolledKeyID: projection.keyID,
      enrolledPublicKeyX963: projection.publicKeyX963, expectedAppIDHash: projection.appSigningIdentityDigest,
      expectedRelease: try bootstrapRelease(), nativeCounterFloor: projection.appleCounterFloor!)
  }
  private func bootstrapAssertion(_ projection: KagemushaAppPlatformPreparedProjectionV1,
    measured: Bool = false, wrongHash: Bool = false) throws -> Data {
    func length(_ count: Int, major: UInt8) -> Data {
      count < 24 ? Data([major << 5 | UInt8(count)]) : Data([major << 5 | 24, UInt8(count)])
    }
    func text(_ s: String) -> Data { let bytes = Data(s.utf8); return length(bytes.count, major: 3) + bytes }
    func bytes(_ d: Data) -> Data { length(d.count, major: 2) + d }
    var auth = projection.appSigningIdentityDigest + Data([measured ? 0xc0 : 0x40, 0, 0, 0, 1])
    if measured {
      auth += Data([0xa2]) + text("validationCategory") + bytes(Data([2, 0, 0, 0]))
        + text("bundleVersion") + text("100")
    }
    let hash = wrongHash ? Data(SHA256.hash(data: projection.financialSubject)) : projection.bootstrapApproval!.clientDataHash
    let nonce = Data(SHA256.hash(data: auth + hash))
    let signer = try P256.Signing.PrivateKey(rawRepresentation: digest(7))
    let der = try signer.signature(for: nonce).derRepresentation
    return Data([0xa2]) + text("signature") + bytes(der) + text("authenticatorData") + bytes(auth)
  }
  private actor BootstrapService: KagemushaAppAttestServiceV1 {
    nonisolated let isSupported = true
    let raw: Data
    private var count = 0
    init(raw: Data) { self.raw = raw }
    func generateKey() async throws -> String { throw KagemushaAppAttestEvidenceErrorV1.unsupportedDevice }
    func attestKey(_ keyID: String, clientDataHash: Data) async throws -> Data { throw KagemushaAppAttestEvidenceErrorV1.unsupportedDevice }
    func generateAssertion(_ keyID: String, clientDataHash: Data) async throws -> Data { count += 1; return raw }
    func calls() -> Int { count }
  }

  private func retailContext(scope: Data? = nil, credential: Data? = nil, floor: UInt32? = nil) throws
    -> (RetailEndpoint, KagemushaCoreCoordinatorBridgeV1, KagemushaNativePreparedAppEnrollmentPossessionV1,
      KagemushaNativeOrdinaryAppIdentityConfirmationV1) {
    var f = try preparation()
    f[1] = enrollmentPossession(key: f[6]); f[8] = Data(); f[13] = Data()
    if let scope { f[9] = scope }
    if let floor { f[10] = u32(floor) }
    let endpoint = RetailEndpoint(prepared: f, credential: credential ?? digest(0x74))
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/untrusted-scripted-retail", endpoint: endpoint)
    let possession = try bridge.prepareAppEnrollmentPossession(originalEnrollmentChallengeHash: f[4])
    let identity = try possession.acceptOriginalFinalCredential(Data([0x71]))
    return (endpoint, bridge, possession, identity)
  }

  private func consumedPossessionContext() throws
    -> (RetailEndpoint, KagemushaCoreCoordinatorBridgeV1, KagemushaNativePreparedAppEnrollmentPossessionV1) {
    var f = try preparation()
    f[1] = enrollmentPossession(key: f[6]); f[8] = Data(); f[13] = Data()
    let endpoint = RetailEndpoint(prepared: f, credential: digest(0x74))
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/untrusted-scripted-consumed-E", endpoint: endpoint)
    return (endpoint, bridge, try bridge.prepareAppEnrollmentPossession(originalEnrollmentChallengeHash: f[4]))
  }

  private func possessionReceipt(_ prepared: [Data], raw: Data, counter: UInt32 = 8) -> Data {
    var bytes = Data("KGMAPP1\0".utf8) + Data([1, 0, 2]) + prepared[0]
    for field in [prepared[4], prepared[9], Data(SHA256.hash(data: prepared[1])),
      Data(SHA256.hash(data: raw)), prepared[9]] { bytes.append(field) }
    bytes.append(1); bytes.append(u32(counter)); return bytes
  }

  /// Scripted transport and deliberately untrusted frame samples exercise managed correlation
  /// only. These bytes supply no genuine FI signature, native authority or cash execution.
  private final class RetailEndpoint: KagemushaCoreCoordinatorEndpointV1 {
    let prepared: [Data]
    let credential: Data
    let challenge = Data(repeating: 0x72, count: 32768)
    let message = Data(repeating: 0x73, count: 32)
    var retailFields: [Data]
    var phases = [UInt32]()
    var responseOverrides = [UInt32: [Data]]()
    var possessionReadbacks = [[Data]]()
    var losePhase: UInt32?
    var stage: UInt8 = 0
    var signature = Data(), certificate = Data()
    var bootstrapFields: [Data]?
    var bootstrapPhases = [UInt32](), bootstrapSelectors = [Data]()
    var bootstrapResponseOverrides = [UInt32: [Data]]()
    var bootstrapStage: UInt8 = 0
    var bootstrapRaw = Data(), bootstrapReceipt = Data()
    var loseBootstrapPhase: UInt32?
    var closeCalls = 0
    init(prepared: [Data], credential: Data) {
      self.prepared = prepared.map { Data($0) }; self.credential = Data(credential)
      retailFields = [Data([17]) + Data(repeating: 0, count: 7),
        Data(repeating: 0x72, count: 32768), Data(repeating: 0x73, count: 32), prepared[9], credential]
    }
    func contract() throws -> [UInt32] { [2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21] }
    func install(storagePath: Data) throws {}
    func open(storagePath: Data) throws -> UInt64 { 1 }
    func close(handle: UInt64) throws { XCTAssertEqual(handle, 1); closeCalls += 1 }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
      XCTAssertEqual(handle, 1)
      if method == 19 { return try invokeBootstrap(request) }
      XCTAssertEqual(method, 20)
      let q = try KagemushaCoreCoordinatorFrameV1.decodeRequest(.appEnrollmentPossession, frame: request)
      let phase = KagemushaAppPlatformPreparedProjectionV1.u32(q[0]); phases.append(phase)
      let response: [Data]
      switch phase {
      case 1:
        XCTAssertEqual(q[1], prepared[4]); response = prepared
      case 5:
        XCTAssertEqual(q[1], prepared[0])
        response = possessionReadbacks.isEmpty ? [Data([0]), Data(), Data()] : possessionReadbacks.removeFirst()
      case 6:
        XCTAssertEqual(q[1], prepared[0]); response = [prepared[9], Data(SHA256.hash(data: prepared[1]))]
      case 8:
        XCTAssertEqual(q[1], prepared[0]); response = [credential, prepared[9]]
      case 9:
        XCTAssertEqual(q[1], prepared[0]); XCTAssertEqual(q[2], challenge); XCTAssertEqual(q[3], message)
        response = retailFields
      case 10:
        XCTAssertEqual(q[1], retailFields[0])
        if stage == 0 { stage = 1; response = [Data([1]), Data()] }
        else if stage == 2 || stage == 3 { response = [Data([2]), signature] }
        else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      case 11:
        XCTAssertEqual(q[1], retailFields[0])
        guard stage == 1 || ((stage == 2 || stage == 3) && signature == q[2]) else {
          throw KagemushaCoreCoordinatorErrorV1.unavailable
        }
        signature = q[2]; stage = max(stage, 2); response = [Data(SHA256.hash(data: signature))]
      case 12:
        XCTAssertEqual(q[1], retailFields[0])
        guard stage == 2 || (stage == 3 && certificate == q[2]) else {
          throw KagemushaCoreCoordinatorErrorV1.unavailable
        }
        certificate = q[2]; stage = 3; response = [Data(repeating: 0x75, count: 32), prepared[9]]
      case 13:
        XCTAssertEqual(q[1], retailFields[0]); response = [Data([stage]), signature, certificate]
      case 14:
        XCTAssertEqual(q[1], retailFields[0])
        guard stage == 0 else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
        stage = 4; response = []
      default: throw KagemushaCoreCoordinatorErrorV1.invalidFrame("unexpected scripted retail phase")
      }
      if losePhase == phase { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      return try KagemushaCoreCoordinatorFrameV1.encodeResponse(.appEnrollmentPossession,
        requestFrame: request, fields: responseOverrides[phase] ?? response)
    }
    private func invokeBootstrap(_ request: Data) throws -> Data {
      let q = try KagemushaCoreCoordinatorFrameV1.decodeRequest(.appOperationApproval, frame: request)
      let phase = KagemushaAppPlatformPreparedProjectionV1.u32(q[0]); bootstrapPhases.append(phase)
      guard let f = bootstrapFields else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      let response: [Data]
      switch phase {
      case 8:
        bootstrapSelectors.append(q[1]); response = f
      case 2:
        XCTAssertEqual(q[1], f[0])
        if bootstrapStage == 0 { bootstrapStage = 1; response = [Data([1]), Data(), Data()] }
        else if bootstrapStage == 2 { response = [Data([2]), bootstrapRaw, Data()] }
        else if bootstrapStage == 3 { response = [Data([3]), bootstrapRaw, bootstrapReceipt] }
        else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      case 3:
        XCTAssertEqual(q[1], f[0]); XCTAssertTrue(bootstrapStage == 1 || bootstrapRaw == q[2])
        bootstrapRaw = q[2]; bootstrapStage = max(bootstrapStage, 2)
        response = [Data(SHA256.hash(data: bootstrapRaw))]
      case 4:
        XCTAssertEqual(q[1], f[0]); XCTAssertEqual(bootstrapStage, 2)
        let w = try KagemushaBootstrapAppApprovalSigningProjectionV1(nativeSigningBytes: f[1],
          nativeFinancialSubject: f[13], credentialDigest: f[8])
        bootstrapReceipt = Data("KGMAPP1\0".utf8) + Data([1, 0, 1]) + f[0]
        for d in [w.operationID, f[9], w.clientDataHash, Data(SHA256.hash(data: bootstrapRaw)), f[8]] {
          bootstrapReceipt.append(d)
        }
        bootstrapReceipt += Data([1, 1, 0, 0, 0]); bootstrapStage = 3; response = [bootstrapReceipt]
      case 5:
        XCTAssertEqual(q[1], f[0])
        if bootstrapStage == 0 { response = [Data([0]), Data(), Data()] }
        else if bootstrapStage == 2 { response = [Data([1]), bootstrapRaw, Data()] }
        else if bootstrapStage == 3 { response = [Data([2]), bootstrapRaw, bootstrapReceipt] }
        else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      case 6:
        XCTAssertEqual(q[1], f[0]); response = [f[9], Data(SHA256.hash(data: f[1]))]
      case 7:
        XCTAssertEqual(q[1], f[0]); XCTAssertEqual(bootstrapStage, 0); bootstrapStage = 4; response = []
      default: throw KagemushaCoreCoordinatorErrorV1.invalidFrame("unknown scripted Bootstrap phase")
      }
      if loseBootstrapPhase == phase { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      return try KagemushaCoreCoordinatorFrameV1.encodeResponse(.appOperationApproval,
        requestFrame: request, fields: bootstrapResponseOverrides[phase] ?? response)
    }
  }

  private func preparation() throws -> [Data] {
    let point = try P256.Signing.PrivateKey(rawRepresentation: digest(7)).publicKey.x963Representation
    let key = Data(SHA256.hash(data: point))
    let fixture = try String(contentsOfFile: #filePath.replacingOccurrences(
      of: "KagemushaAppPlatformPreparedProjectionV1Tests.swift",
      with: "Fixtures/kagemusha_app_platform_messages_v1.tsv"), encoding: .utf8)
    let row = fixture.split(separator: "\n").first { $0.hasPrefix("s_mint_native_first\t") }!
    let hex = row.split(separator: "\t")[1]
    var subject = Data(stride(from: 0, to: hex.count, by: 2).map { offset -> UInt8 in
      let start = hex.index(hex.startIndex, offsetBy: offset)
      return UInt8(hex[start..<hex.index(start, offsetBy: 2)], radix: 16)!
    })
    // These are explicitly untrusted correlated projections; the actual Rust
    // codec vectors remain byte-exact in their separate conformance test.
    subject.replaceSubrange(155..<187, with: digest(0x66))
    subject.replaceSubrange(323..<331, with: u64(2))
    subject.replaceSubrange(59..<91, with: digest(7))
    subject.replaceSubrange(187..<219, with: digest(5))
    subject.replaceSubrange(219..<251, with: digest(6))
    subject.replaceSubrange(251..<283, with: digest(8))
    subject.replaceSubrange(283..<291, with: u64(1))
    var w = Data("iroha:kagemusha:v1:app-operation-approval\0".utf8) + u64(275) + Data([1, 0, 1])
    for d in [digest(0x11), digest(0x22), digest(4), digest(11), key, digest(0x66),
      Data(SHA256.hash(data: Data(subject))), digest(0x88)] { w.append(d) }
    w.append(u64(1000)); w.append(u64(121000))
    let c = challenge()
    return [u64(9), w, Data([4]), Data(key.base64EncodedString().utf8),
      Data(SHA256.hash(data: c)), point, key, c, digest(0x66), digest(0x99),
      u32(7), Data([0]), digest(0xaa), Data(subject)]
  }

  private func challenge() -> Data {
    var c = Data("iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0".utf8)
      + u64(451) + Data([1, 0, 2])
    for i in 1...13 { c.append(digest(UInt8(i))) }
    for i in [UInt64(1), 2, 1000, 121000] { c.append(u64(i)) }
    return c
  }
  private func enrollmentPossession(key: Data) -> Data {
    var e = Data("iroha:kagemusha:v1:app-enrollment-possession\0".utf8)
      + u64(371) + Data([1, 0, 1])
    for d in [Data(SHA256.hash(data:challenge())), digest(2), digest(3), digest(4), digest(5), digest(11),
      digest(7), digest(8), digest(6), key, digest(0x77)] { e.append(d) }
    e.append(u64(1000)); e.append(u64(121000)); return e
  }
  private func receipt(raw: Data) -> Data {
    var r = Data("KGMAPP1\0".utf8) + Data([1, 0, 1]) + u64(9)
    for d in [digest(0x11), digest(0x99), digest(0x22), Data(SHA256.hash(data: raw)), digest(0x66)] {
      r.append(d)
    }
    r.append(1); r.append(u32(8)); return r
  }
  private func digest(_ byte: UInt8) -> Data { Data(repeating: byte, count: 32) }
  private func u32(_ value: UInt32) -> Data {
    var value = value.littleEndian; return withUnsafeBytes(of: &value) { Data($0) }
  }
  private func u64(_ value: UInt64) -> Data {
    var value = value.littleEndian; return withUnsafeBytes(of: &value) { Data($0) }
  }
}

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

  private func retailContext(scope: Data? = nil, credential: Data? = nil) throws
    -> (RetailEndpoint, KagemushaCoreCoordinatorBridgeV1, KagemushaNativePreparedAppEnrollmentPossessionV1,
      KagemushaNativeOrdinaryAppIdentityConfirmationV1) {
    var f = try preparation()
    f[1] = enrollmentPossession(key: f[6]); f[8] = Data(); f[13] = Data()
    if let scope { f[9] = scope }
    let endpoint = RetailEndpoint(prepared: f, credential: credential ?? digest(0x74))
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/untrusted-scripted-retail", endpoint: endpoint)
    let possession = try bridge.prepareAppEnrollmentPossession(originalEnrollmentChallengeHash: f[4])
    let identity = try possession.acceptOriginalFinalCredential(Data([0x71]))
    return (endpoint, bridge, possession, identity)
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
    var losePhase: UInt32?
    var stage: UInt8 = 0
    var signature = Data(), certificate = Data()
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
      XCTAssertEqual(handle, 1); XCTAssertEqual(method, 20)
      let q = try KagemushaCoreCoordinatorFrameV1.decodeRequest(.appEnrollmentPossession, frame: request)
      let phase = KagemushaAppPlatformPreparedProjectionV1.u32(q[0]); phases.append(phase)
      let response: [Data]
      switch phase {
      case 1:
        XCTAssertEqual(q[1], prepared[4]); response = prepared
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

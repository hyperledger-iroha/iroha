import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

private enum AppAttestFixtureError: Error { case lostReply }

private actor AppAttestFixtureService: KagemushaAppAttestServiceV1 {
  nonisolated let isSupported: Bool
  private let assertion: Data
  private let losesReply: Bool
  private(set) var assertionCalls = 0
  private(set) var lastHash: Data?

  init(assertion: Data, supported: Bool = true, losesReply: Bool = false) {
    self.assertion = assertion
    isSupported = supported
    self.losesReply = losesReply
  }

  func generateKey() async throws -> String { "dedicated-key" }
  func attestKey(_ keyID: String, clientDataHash: Data) async throws -> Data {
    lastHash = clientDataHash
    return Data([0xa1])
  }
  func generateAssertion(_ keyID: String, clientDataHash: Data) async throws -> Data {
    assertionCalls += 1
    lastHash = clientDataHash
    if losesReply { throw AppAttestFixtureError.lostReply }
    return assertion
  }
  func observations() -> (Int, Data?) { (assertionCalls, lastHash) }
}

private actor BlockingAppAttestFixtureService: KagemushaAppAttestServiceV1 {
  nonisolated let isSupported = true
  private let assertion: Data
  private var started = false
  private var startWaiter: CheckedContinuation<Void, Never>?
  private var assertionWaiter: CheckedContinuation<Data, Never>?
  private(set) var assertionCalls = 0

  init(assertion: Data) { self.assertion = assertion }
  func generateKey() async throws -> String { "dedicated-key" }
  func attestKey(_ keyID: String, clientDataHash: Data) async throws -> Data { Data([0xa1]) }
  func generateAssertion(_ keyID: String, clientDataHash: Data) async throws -> Data {
    assertionCalls += 1
    started = true
    startWaiter?.resume()
    startWaiter = nil
    return await withCheckedContinuation { assertionWaiter = $0 }
  }
  func waitUntilStarted() async {
    if started { return }
    await withCheckedContinuation { startWaiter = $0 }
  }
  func release() {
    assertionWaiter?.resume(returning: assertion)
    assertionWaiter = nil
  }
  func calls() -> Int { assertionCalls }
}

private final class AppAttestFixtureIntentStore: KagemushaAppAttestAssertionIntentStoringV1, @unchecked Sendable {
  private let lock = NSLock()
  private var record: KagemushaAppAttestAssertionIntentV1
  private(set) var reservations = 0

  init(counter: UInt32) { record = .ready(counter: counter) }

  func load(keyID: String) throws -> KagemushaAppAttestAssertionIntentV1 {
    lock.lock(); defer { lock.unlock() }
    return record
  }

  func reserve(keyID: String, previousCounter: UInt32, selectionDigest: Data) throws {
    lock.lock(); defer { lock.unlock() }
    switch record {
    case .ready(let counter) where counter == previousCounter:
      break
    default: throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    record = .pending(previousCounter: previousCounter, selectionDigest: selectionDigest)
    reservations += 1
  }

  func complete(keyID: String, counter: UInt32, selectionDigest: Data, rawAssertion: Data) throws {
    lock.lock(); defer { lock.unlock() }
    guard counter > 0,
      record == .pending(previousCounter: counter - 1, selectionDigest: selectionDigest) else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    record = .complete(counter: counter, selectionDigest: selectionDigest, rawAssertion: rawAssertion)
  }

  func advanceAfterCommitted(keyID: String, counter: UInt32, selectionDigest: Data,
    rawAssertion: Data, acknowledgment: KagemushaAppAttestCoreCommitAcknowledgmentV1) throws {
    lock.lock(); defer { lock.unlock() }
    guard record == .complete(counter: counter, selectionDigest: selectionDigest,
        rawAssertion: rawAssertion),
      acknowledgment.keyIDDigest == Data(SHA256.hash(data: Data(keyID.utf8))),
      acknowledgment.selectionDigest == selectionDigest,
      acknowledgment.rawAssertionDigest == Data(SHA256.hash(data: rawAssertion)),
      acknowledgment.committedCounter == counter else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    record = .ready(counter: counter)
  }

  func reservationCount() -> Int {
    lock.lock(); defer { lock.unlock() }
    return reservations
  }
}

/// Test-only response projection; it is not a qualified native coordinator.
private final class AppAttestCommitEndpoint: KagemushaCoreCoordinatorEndpointV1 {
  enum Mode: Equatable { case unavailable, substituted, valid }
  private let lock = NSLock()
  private var mode: Mode = .unavailable
  private var invocationCount = 0

  func configure(_ newMode: Mode) { lock.lock(); mode = newMode; lock.unlock() }
  func calls() -> Int { lock.lock(); defer { lock.unlock() }; return invocationCount }
  func contract() throws -> [UInt32] { [2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff, 1, 14] }
  func open(storagePath: Data) throws -> UInt64 { 7 }
  func close(handle: UInt64) throws {}

  func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
    lock.lock()
    let selected = mode
    invocationCount += 1
    lock.unlock()
    guard selected != .unavailable else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    guard method == KagemushaCoreCoordinatorMethodV1.acknowledgeCommittedAppAttest.rawValue else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("wrong method")
    }
    let fields = try KagemushaCoreCoordinatorFrameV1.decodeRequest(
      .acknowledgeCommittedAppAttest, frame: request)
    let previous = fields[4].enumerated().reduce(UInt32(0)) {
      $0 | (UInt32($1.element) << ($1.offset * 8))
    }
    let response = [fields[0], Data(SHA256.hash(data: fields[1])),
      Data(SHA256.hash(data: fields[2])), Data(SHA256.hash(data: fields[3])),
      KagemushaCoreCoordinatorFrameV1.u32(previous + 1), fields[5], fields[6]]
    var frame = try KagemushaCoreCoordinatorFrameV1.encodeResponse(
      .acknowledgeCommittedAppAttest, requestFrame: request, fields: response)
    if selected == .substituted { frame[20] ^= 1 }
    return frame
  }
}

final class KagemushaAppAttestEvidenceV1Tests: XCTestCase {
  private let appIDHash = Data(repeating: 0x39, count: 32)
  private static let selectionDomain = Data("iroha:kagemusha:v1:hardware-transition-selection\0".utf8)
  private static let signingKey = try! P256.Signing.PrivateKey(
    rawRepresentation: Data(repeating: 1, count: 32))
  private var assertionKey: Data { Self.signingKey.publicKey.x963Representation }

  private func release(category: UInt32 = 4, version: String = "1") throws
    -> KagemushaAppAttestExpectedReleaseV1 {
    let digest = try KagemushaAppAttestExpectedReleaseV1.canonicalReleaseDigest(
      validationCategory: category, bundleVersion: version)
    return try KagemushaAppAttestExpectedReleaseV1(
      validationCategory: category, bundleVersion: version,
      authenticatedAppReleaseDigest: digest)
  }

  private func binding(_ body: Data = Data(repeating: 0x17, count: 403)) throws
    -> KagemushaAppAttestTransitionBindingV1 {
    var signingBytes = Self.selectionDomain
    for shift in stride(from: 0, to: 64, by: 8) {
      signingBytes.append(UInt8(truncatingIfNeeded: UInt64(body.count) >> shift))
    }
    signingBytes.append(body)
    return try KagemushaAppAttestTransitionBindingV1(coreSelectionSigningBytes: signingBytes)
  }

  private func cborLength(_ length: Int, major: UInt8) -> Data {
    if length < 24 { return Data([major << 5 | UInt8(length)]) }
    if length <= 255 { return Data([major << 5 | 24, UInt8(length)]) }
    return Data([major << 5 | 25, UInt8(length >> 8), UInt8(length & 0xff)])
  }

  private func cborBytes(_ value: Data) -> Data { cborLength(value.count, major: 2) + value }
  private func cborText(_ value: String) -> Data {
    let bytes = Data(value.utf8)
    return cborLength(bytes.count, major: 3) + bytes
  }

  private func assertion(counter: UInt32, appIDHash: Data? = nil, flags: UInt8 = 0x40,
    category: UInt32? = 4, bundleVersion: String? = "1",
    includeExtensionMap: Bool = true, clientDataHash: Data? = nil) -> Data {
    var auth = appIDHash ?? self.appIDHash
    auth.append(flags)
    auth.append(UInt8(counter >> 24))
    auth.append(UInt8(truncatingIfNeeded: counter >> 16))
    auth.append(UInt8(truncatingIfNeeded: counter >> 8))
    auth.append(UInt8(truncatingIfNeeded: counter))
    var extensions: [(String, Data)] = []
    if let category {
      let value = Data([UInt8(truncatingIfNeeded: category),
        UInt8(truncatingIfNeeded: category >> 8),
        UInt8(truncatingIfNeeded: category >> 16),
        UInt8(truncatingIfNeeded: category >> 24)])
      extensions.append(("validationCategory", cborBytes(value)))
    }
    if let bundleVersion {
      extensions.append(("bundleVersion", cborText(bundleVersion)))
    }
    if includeExtensionMap {
      auth.append(cborLength(extensions.count, major: 5))
      for (name, value) in extensions {
        auth.append(cborText(name))
        auth.append(value)
      }
    }
    let hash = clientDataHash ?? (try! binding().clientDataHash)
    let nonce = Data(SHA256.hash(data: auth + hash))
    let signature = try! Self.signingKey.signature(for: nonce).derRepresentation
    return Data([0xa2]) + cborText("authenticatorData") + cborBytes(auth)
      + cborText("signature") + cborBytes(signature)
  }

  func testCanonicalCoreFrameAndEnrollmentAreDomainSeparated() throws {
    let selected = try binding()
    XCTAssertEqual(selected.canonicalSelectionSigningBytes.count, 460)
    XCTAssertEqual(selected.clientDataHash, Data(SHA256.hash(data: selected.canonicalSelectionSigningBytes)))
    var body = Data(repeating: 0x17, count: 403)
    body[0] ^= 1
    XCTAssertNotEqual(try binding(body).clientDataHash, selected.clientDataHash)
    var wrongLength = selected.canonicalSelectionSigningBytes
    wrongLength[Self.selectionDomain.count] = 0
    XCTAssertThrowsError(try KagemushaAppAttestTransitionBindingV1(coreSelectionSigningBytes: wrongLength))
    var wrongDomain = selected.canonicalSelectionSigningBytes
    wrongDomain[0] ^= 1
    XCTAssertThrowsError(try KagemushaAppAttestTransitionBindingV1(coreSelectionSigningBytes: wrongDomain))
    XCTAssertThrowsError(try binding(Data(repeating: 0x17, count: 402)))
    XCTAssertThrowsError(try binding(Data(repeating: 0x17, count: 404)))
    XCTAssertThrowsError(try KagemushaAppAttestTransitionBindingV1(
      coreSelectionSigningBytes: Data(repeating: 1, count: 1_025)))
    let enrollment = try KagemushaAppAttestEnrollmentBindingV1(
      clientNonce: Data(repeating: 1, count: 32),
      serverNonce: Data(repeating: 2, count: 32),
      releaseID: Data(repeating: 3, count: 32),
      profileID: Data(repeating: 4, count: 32),
      attestedKeyID: Data(repeating: 5, count: 32),
      laneID: Data(repeating: 6, count: 32))
    XCTAssertNotEqual(enrollment.clientDataHash, selected.clientDataHash)
    var expectedEnrollmentClientData = Data(
      "iroha:kagemusha:v1:app-device-attestation-challenge\0".utf8)
    expectedEnrollmentClientData.append(Data(repeating: 1, count: 32))
    expectedEnrollmentClientData.append(Data(repeating: 2, count: 32))
    expectedEnrollmentClientData.append(Data(repeating: 3, count: 32))
    expectedEnrollmentClientData.append(Data(repeating: 4, count: 32))
    expectedEnrollmentClientData.append(Data(repeating: 5, count: 32))
    expectedEnrollmentClientData.append(Data(repeating: 6, count: 32))
    XCTAssertEqual(enrollment.canonicalClientData, expectedEnrollmentClientData)
    for changedField in 0..<6 {
      var fields = (1...6).map { Data(repeating: UInt8($0), count: 32) }
      fields[changedField][0] ^= 1
      let changed = try KagemushaAppAttestEnrollmentBindingV1(
        clientNonce: fields[0], serverNonce: fields[1], releaseID: fields[2],
        profileID: fields[3], attestedKeyID: fields[4], laneID: fields[5])
      XCTAssertNotEqual(changed.clientDataHash, enrollment.clientDataHash)
    }
    XCTAssertThrowsError(try KagemushaAppAttestEnrollmentBindingV1(
      clientNonce: Data(count: 31), serverNonce: Data(repeating: 2, count: 32),
      releaseID: Data(repeating: 3, count: 32), profileID: Data(repeating: 4, count: 32),
      attestedKeyID: Data(repeating: 5, count: 32), laneID: Data(repeating: 6, count: 32)))
  }

  func testAssertionParsesHardwareCounterAndSignatureDigest() throws {
    let binding = try binding()
    let raw = assertion(counter: 0x01020304)
    let evidence = try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: raw, clientDataHash: binding.clientDataHash, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    XCTAssertEqual(evidence.signCount, 0x01020304)
    XCTAssertGreaterThan(evidence.authenticatorData.count, 37)
    XCTAssertEqual(evidence.releaseMeasurement,
      .signed(validationCategory: 4, bundleVersion: "1"))
    XCTAssertGreaterThan(evidence.signatureDER.count, 64)
    let nonce = Data(SHA256.hash(data: evidence.authenticatorData + binding.clientDataHash))
    XCTAssertEqual(evidence.assertionNonce, nonce)
    XCTAssertEqual(evidence.signatureMessageDigest, Data(SHA256.hash(data: nonce)))
    try evidence.validateExactNext(previousCounter: 0x01020303)
    XCTAssertThrowsError(try evidence.validateExactNext(previousCounter: 0x01020302)) {
      XCTAssertEqual($0 as? KagemushaAppAttestEvidenceErrorV1, .assertionCounterMismatch)
    }
    XCTAssertThrowsError(try evidence.validateExactNext(previousCounter: UInt32.max)) {
      XCTAssertEqual($0 as? KagemushaAppAttestEvidenceErrorV1, .assertionCounterMismatch)
    }
  }

  func testAuthenticatedReleaseDigestMatchesRustAndRejectsSubstitution() throws {
    let digest = try KagemushaAppAttestExpectedReleaseV1.canonicalReleaseDigest(
      validationCategory: 4, bundleVersion: "1.0")
    XCTAssertEqual(digest, Data([
      0x6b, 0xba, 0x24, 0x1e, 0x1c, 0xae, 0x5b, 0xce,
      0x0b, 0x29, 0xe7, 0xe2, 0xfb, 0x72, 0x93, 0xce,
      0x82, 0x65, 0x3a, 0xf8, 0xdc, 0x39, 0xc2, 0x77,
      0xd1, 0x2c, 0x51, 0xc0, 0x75, 0x0c, 0x3e, 0xc0,
    ]))
    let valid = try KagemushaAppAttestExpectedReleaseV1(
      validationCategory: 4, bundleVersion: "1.0",
      authenticatedAppReleaseDigest: digest)
    XCTAssertEqual(valid.appReleaseDigest, digest)
    XCTAssertThrowsError(try KagemushaAppAttestExpectedReleaseV1(
      validationCategory: 2, bundleVersion: "1.0",
      authenticatedAppReleaseDigest: digest)) { error in
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .releaseMismatch)
    }
    XCTAssertThrowsError(try KagemushaAppAttestExpectedReleaseV1(
      validationCategory: 4, bundleVersion: "1.1",
      authenticatedAppReleaseDigest: digest)) { error in
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .releaseMismatch)
    }
    XCTAssertThrowsError(try KagemushaAppAttestExpectedReleaseV1(
      validationCategory: 4, bundleVersion: "1.0",
      authenticatedAppReleaseDigest: Data(repeating: 0, count: 32)))
  }

  func testMalformedAssertionAndWrongAppIdentityFailClosed() throws {
    let hash = try binding().clientDataHash
    let raw = assertion(counter: 1)
    let wrongApp = assertion(counter: 1, appIDHash: Data(repeating: 0x41, count: 32))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: wrongApp, clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 0), clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 1, flags: 0x41),
      clientDataHash: hash, expectedAppIDHash: appIDHash, expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: raw + Data([0]), clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: Data([0xa2]) + cborText("signature") + cborBytes(Data([0x30]))
        + cborText("signature") + cborBytes(Data([0x30])),
      clientDataHash: hash, expectedAppIDHash: appIDHash, expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey))
  }

  func testAssertionReleaseExtensionsWhenPresentAreExactAndPinned() throws {
    let hash = try binding().clientDataHash
    let expected = try release()
    XCTAssertThrowsError(try KagemushaAppAttestExpectedReleaseV1(
      validationCategory: 0, bundleVersion: "1",
      authenticatedAppReleaseDigest: Data(repeating: 1, count: 32)))
    XCTAssertThrowsError(try KagemushaAppAttestExpectedReleaseV1(
      validationCategory: 4, bundleVersion: "",
      authenticatedAppReleaseDigest: Data(repeating: 1, count: 32)))
    for raw in [
      assertion(counter: 1, category: nil),
      assertion(counter: 1, bundleVersion: nil),
      assertion(counter: 1, category: nil, bundleVersion: nil),
    ] {
      XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
        rawAssertion: raw, clientDataHash: hash, expectedAppIDHash: appIDHash,
        expectedRelease: expected, enrolledAssertionPublicKeyX963: assertionKey)) { error in
        XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1,
          .invalidAssertionObject)
      }
    }
    for raw in [
      assertion(counter: 1, category: 2),
      assertion(counter: 1, bundleVersion: "2"),
      assertion(counter: 1, category: 0x0400_0000),
    ] {
      XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
        rawAssertion: raw, clientDataHash: hash, expectedAppIDHash: appIDHash,
        expectedRelease: expected, enrolledAssertionPublicKeyX963: assertionKey)) { error in
        XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1,
          .releaseMismatch)
      }
    }
    let testFlight = try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 1, category: 2, bundleVersion: "42"),
      clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release(category: 2, version: "42"), enrolledAssertionPublicKeyX963: assertionKey)
    let appStore = try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 1), clientDataHash: hash,
      expectedAppIDHash: appIDHash, expectedRelease: expected, enrolledAssertionPublicKeyX963: assertionKey)
    XCTAssertNotEqual(testFlight.signatureMessageDigest, appStore.signatureMessageDigest)
  }

  func testPhysicalIOS26AssertionHasNoReleaseMeasurement() throws {
    let hash = try binding().clientDataHash
    let raw = assertion(counter: 1, flags: 0x40, category: nil,
      bundleVersion: nil, includeExtensionMap: false)
    let evidence = try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: raw, clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    XCTAssertEqual(evidence.authenticatorData.count, 37)
    XCTAssertEqual(evidence.authenticatorData[32], 0x40)
    XCTAssertEqual(evidence.releaseMeasurement, .unavailable)
    let nonce = Data(SHA256.hash(data: evidence.authenticatorData + hash))
    XCTAssertEqual(evidence.assertionNonce, nonce)
    XCTAssertEqual(evidence.signatureMessageDigest, Data(SHA256.hash(data: nonce)))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 1, flags: 0xc0, category: nil,
        bundleVersion: nil, includeExtensionMap: false),
      clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 1, flags: 0x40, category: 2),
      clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey))
  }

  func testAssertionSignatureRejectsDifferentKeyAndChangedChallenge() throws {
    let raw = assertion(counter: 1, category: nil, bundleVersion: nil,
      includeExtensionMap: false)
    let hash = try binding().clientDataHash
    let otherKey = try P256.Signing.PrivateKey(
      rawRepresentation: Data(repeating: 2, count: 32)).publicKey.x963Representation
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: raw, clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: otherKey)) {
      XCTAssertEqual($0 as? KagemushaAppAttestEvidenceErrorV1, .invalidAssertionSignature)
    }
    var changed = hash
    changed[0] ^= 1
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: raw, clientDataHash: changed, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)) {
      XCTAssertEqual($0 as? KagemushaAppAttestEvidenceErrorV1, .invalidAssertionSignature)
    }
  }

  func testReleaseMismatchLeavesDurablePendingIntent() async throws {
    let store = AppAttestFixtureIntentStore(counter: 0)
    let service = AppAttestFixtureService(assertion: assertion(counter: 1, category: 2))
    let provider = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    let selected = try binding()
    do {
      _ = try await provider.assertTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 0)
      XCTFail("wrong release accepted")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1,
        .assertionOutcomeUnknown)
    }
    XCTAssertEqual(try store.load(keyID: "dedicated-key"),
      .pending(previousCounter: 0, selectionDigest: selected.clientDataHash))
    let observations = await service.observations()
    XCTAssertEqual(observations.0, 1)
  }

  func testExactNextAssertionReservesAndPersistsBeforeReturn() async throws {
    let store = AppAttestFixtureIntentStore(counter: 4)
    let raw = assertion(counter: 5)
    let service = AppAttestFixtureService(assertion: raw)
    let provider = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    let selected = try binding()
    let evidence = try await provider.assertTransition(
      keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 4)
    XCTAssertEqual(evidence.rawAssertion, raw)
    XCTAssertEqual(evidence.signCount, 5)
    XCTAssertEqual(try store.load(keyID: "dedicated-key"),
      .complete(counter: 5, selectionDigest: selected.clientDataHash, rawAssertion: raw))
    let observations = await service.observations()
    XCTAssertEqual(observations.0, 1)
    XCTAssertEqual(observations.1, selected.clientDataHash)
    XCTAssertEqual(store.reservationCount(), 1)
    let restarted = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    do {
      _ = try await restarted.assertTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 5)
      XCTFail("uncommitted predecessor advanced the lane")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .journalMismatch)
    }
    let afterReopenObservations = await service.observations()
    XCTAssertEqual(afterReopenObservations.0, 1)
  }

  func testCompletedIntentRecoversExactSignedSelectionWithoutAnotherHardwareCall() async throws {
    let store = AppAttestFixtureIntentStore(counter: 4)
    let raw = assertion(counter: 5)
    let service = AppAttestFixtureService(assertion: raw)
    let selected = try binding()
    let first = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    _ = try await first.assertTransition(
      keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 4)
    let restarted = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    let recovered = try await restarted.recoverCompletedTransition(
      keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 4)
    XCTAssertEqual(recovered.rawAssertion, raw)
    XCTAssertEqual(recovered.signCount, 5)
    let wrongSelection = try binding(Data(repeating: 0x18, count: 403))
    do {
      _ = try await restarted.recoverCompletedTransition(
        keyID: "dedicated-key", binding: wrongSelection, expectedPreviousCounter: 4)
      XCTFail("different selection recovered")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .journalMismatch)
    }
    do {
      _ = try await restarted.recoverCompletedTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 3)
      XCTFail("different predecessor recovered")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .journalMismatch)
    }
    XCTAssertEqual(try store.load(keyID: "dedicated-key"),
      .complete(counter: 5, selectionDigest: selected.clientDataHash, rawAssertion: raw))
    do {
      _ = try await restarted.assertTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 5)
      XCTFail("a completed assertion advanced without a Core commit acknowledgment")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .journalMismatch)
    }
    let observations = await service.observations()
    XCTAssertEqual(observations.0, 1)
  }

  func testCommitAcknowledgmentAdvancesOnlyAfterExactNativeResponse() async throws {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("kagemusha-app-attest-commit-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
      attributes: [.posixPermissions: NSNumber(value: 0o700)])
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    let store = try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(
      directoryURL: directory, keyID: "dedicated-key")
    let raw = assertion(counter: 1)
    let service = AppAttestFixtureService(assertion: raw)
    let provider = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    let selected = try binding()
    _ = try await provider.assertTransition(
      keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 0)
    let endpoint = AppAttestCommitEndpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(
      storagePath: "/private/coordinator", endpoint: endpoint)
    let coordinator = KagemushaNativeCoreCoordinatorAdapterV1(bridge: bridge)
    let operationID = Data(repeating: 0x31, count: 32)
    let certificate = Data(repeating: 0x32, count: 32)
    let envelope = Data(repeating: 0x33, count: 32)
    func acknowledge() async throws {
      try await provider.acknowledgeCommittedTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 0,
        operationID: operationID, terminalCertificateDigest: certificate,
        installedEnvelopeDigest: envelope, coordinator: coordinator)
    }
    do { try await acknowledge(); XCTFail("unavailable native owner advanced App Attest") } catch {}
    XCTAssertEqual(try store.load(keyID: "dedicated-key"),
      .complete(counter: 1, selectionDigest: selected.clientDataHash, rawAssertion: raw))
    endpoint.configure(.substituted)
    do { try await acknowledge(); XCTFail("substituted native response advanced App Attest") } catch {}
    XCTAssertEqual(try store.load(keyID: "dedicated-key"),
      .complete(counter: 1, selectionDigest: selected.clientDataHash, rawAssertion: raw))
    endpoint.configure(.valid)
    try await acknowledge()
    let reopened = try KagemushaAppAttestFileIntentStoreV1(directoryURL: directory)
    XCTAssertEqual(try reopened.load(keyID: "dedicated-key"), .ready(counter: 1))
    do { try await acknowledge(); XCTFail("acknowledgment replay advanced App Attest") } catch {}
    XCTAssertEqual(try reopened.load(keyID: "dedicated-key"), .ready(counter: 1))
    let nextSelected = try binding(Data(repeating: 0x18, count: 403))
    let nextService = AppAttestFixtureService(assertion: assertion(
      counter: 2, clientDataHash: nextSelected.clientDataHash))
    let next = try KagemushaAppAttestEvidenceProviderV1(
      service: nextService, intentStore: reopened, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    let nextEvidence = try await next.assertTransition(
      keyID: "dedicated-key", binding: nextSelected, expectedPreviousCounter: 1)
    XCTAssertEqual(nextEvidence.signCount, 2)
    let observations = await service.observations()
    XCTAssertEqual(observations.0, 1)
    XCTAssertEqual(endpoint.calls(), 3)
  }

  func testRecoveryRejectsPendingAndTamperedCompletedAssertion() async throws {
    let store = AppAttestFixtureIntentStore(counter: 0)
    let service = AppAttestFixtureService(assertion: Data())
    let selected = try binding()
    let provider = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    try store.reserve(keyID: "dedicated-key", previousCounter: 0,
      selectionDigest: selected.clientDataHash)
    do {
      _ = try await provider.recoverCompletedTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 0)
      XCTFail("pending hardware outcome recovered")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .assertionOutcomeUnknown)
    }
    var tampered = assertion(counter: 1)
    tampered[tampered.index(before: tampered.endIndex)] ^= 1
    try store.complete(keyID: "dedicated-key", counter: 1,
      selectionDigest: selected.clientDataHash, rawAssertion: tampered)
    do {
      _ = try await provider.recoverCompletedTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 0)
      XCTFail("tampered assertion recovered")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1,
        .invalidAssertionSignature)
    }
    let observations = await service.observations()
    XCTAssertEqual(observations.0, 0)
  }

  func testSkippedCounterFreezesAcrossProviderRecreation() async throws {
    let store = AppAttestFixtureIntentStore(counter: 4)
    let service = AppAttestFixtureService(assertion: assertion(counter: 6))
    let selected = try binding()
    let provider = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    do {
      _ = try await provider.assertTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 4)
      XCTFail("skipped counter accepted")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .assertionOutcomeUnknown)
    }
    let restarted = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    do {
      _ = try await restarted.assertTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 4)
      XCTFail("pending intent retried")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .journalMismatch)
    }
    XCTAssertEqual(try store.load(keyID: "dedicated-key"),
      .pending(previousCounter: 4, selectionDigest: selected.clientDataHash))
    let observations = await service.observations()
    XCTAssertEqual(observations.0, 1)
  }

  func testLostReplyFreezesWithoutSecondHardwareCall() async throws {
    let store = AppAttestFixtureIntentStore(counter: 0)
    let service = AppAttestFixtureService(assertion: assertion(counter: 1), losesReply: true)
    let provider = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    let selected = try binding()
    for _ in 0..<2 {
      do {
        _ = try await provider.assertTransition(
          keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 0)
        XCTFail("uncertain transition accepted")
      } catch {
        XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .assertionOutcomeUnknown)
      }
    }
    let observations = await service.observations()
    XCTAssertEqual(observations.0, 1)
    XCTAssertEqual(try store.load(keyID: "dedicated-key"),
      .pending(previousCounter: 0, selectionDigest: selected.clientDataHash))
  }

  func testConcurrentAssertionCannotEnterHardwareTwice() async throws {
    let store = AppAttestFixtureIntentStore(counter: 0)
    let service = BlockingAppAttestFixtureService(assertion: assertion(counter: 1))
    let provider = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    let selected = try binding()
    let first = Task {
      try await provider.assertTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 0)
    }
    await service.waitUntilStarted()
    do {
      _ = try await provider.assertTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 0)
      XCTFail("concurrent assertion accepted")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .assertionAlreadyInFlight)
    }
    let calls = await service.calls()
    XCTAssertEqual(calls, 1)
    await service.release()
    let evidence = try await first.value
    XCTAssertEqual(evidence.signCount, 1)
  }

  func testUnsupportedDeviceDoesNotReserve() async throws {
    let store = AppAttestFixtureIntentStore(counter: 0)
    let service = AppAttestFixtureService(assertion: assertion(counter: 1), supported: false)
    let provider = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release(), enrolledAssertionPublicKeyX963: assertionKey)
    do {
      _ = try await provider.assertTransition(
        keyID: "dedicated-key", binding: binding(), expectedPreviousCounter: 0)
      XCTFail("unsupported device asserted")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .unsupportedDevice)
    }
    XCTAssertEqual(store.reservationCount(), 0)
  }
}

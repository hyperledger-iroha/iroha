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
    guard record == .pending(previousCounter: counter - 1, selectionDigest: selectionDigest) else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    record = .complete(counter: counter, selectionDigest: selectionDigest, rawAssertion: rawAssertion)
  }

  func reservationCount() -> Int {
    lock.lock(); defer { lock.unlock() }
    return reservations
  }
}

final class KagemushaAppAttestEvidenceV1Tests: XCTestCase {
  private let appIDHash = Data(repeating: 0x39, count: 32)
  private static let selectionDomain = Data("iroha:kagemusha:v1:hardware-transition-selection\0".utf8)

  private func release(category: UInt32 = 4, version: String = "1") throws
    -> KagemushaAppAttestExpectedReleaseV1 {
    let digest = try KagemushaAppAttestExpectedReleaseV1.canonicalReleaseDigest(
      validationCategory: category, bundleVersion: version)
    return try KagemushaAppAttestExpectedReleaseV1(
      validationCategory: category, bundleVersion: version,
      authenticatedAppReleaseDigest: digest)
  }

  private func binding(_ body: Data = Data(repeating: 0x17, count: 192)) throws
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

  private func assertion(counter: UInt32, appIDHash: Data? = nil, flags: UInt8 = 0x81,
    category: UInt32? = 4, bundleVersion: String? = "1") -> Data {
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
    auth.append(cborLength(extensions.count, major: 5))
    for (name, value) in extensions {
      auth.append(cborText(name))
      auth.append(value)
    }
    let signature = Data([0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01])
    return Data([0xa2]) + cborText("authenticatorData") + cborBytes(auth)
      + cborText("signature") + cborBytes(signature)
  }

  func testCanonicalCoreFrameAndEnrollmentAreDomainSeparated() throws {
    let selected = try binding()
    XCTAssertEqual(selected.clientDataHash, Data(SHA256.hash(data: selected.canonicalSelectionSigningBytes)))
    var body = Data(repeating: 0x17, count: 192)
    body[0] ^= 1
    XCTAssertNotEqual(try binding(body).clientDataHash, selected.clientDataHash)
    var wrongLength = selected.canonicalSelectionSigningBytes
    wrongLength[Self.selectionDomain.count] = 0
    XCTAssertThrowsError(try KagemushaAppAttestTransitionBindingV1(coreSelectionSigningBytes: wrongLength))
    var wrongDomain = selected.canonicalSelectionSigningBytes
    wrongDomain[0] ^= 1
    XCTAssertThrowsError(try KagemushaAppAttestTransitionBindingV1(coreSelectionSigningBytes: wrongDomain))
    XCTAssertThrowsError(try KagemushaAppAttestTransitionBindingV1(
      coreSelectionSigningBytes: Data(repeating: 1, count: 1_025)))
    let enrollment = try KagemushaAppAttestEnrollmentBindingV1(
      releaseDigest: Data(repeating: 1, count: 32),
      laneDigest: Data(repeating: 2, count: 32),
      serverChallenge: Data(repeating: 3, count: 32))
    XCTAssertNotEqual(enrollment.clientDataHash, selected.clientDataHash)
    XCTAssertThrowsError(try KagemushaAppAttestEnrollmentBindingV1(
      releaseDigest: Data(count: 31), laneDigest: Data(count: 32), serverChallenge: Data(count: 32)))
  }

  func testAssertionParsesHardwareCounterAndSignatureDigest() throws {
    let binding = try binding()
    let raw = assertion(counter: 0x01020304)
    let evidence = try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: raw, clientDataHash: binding.clientDataHash, expectedAppIDHash: appIDHash,
      expectedRelease: release())
    XCTAssertEqual(evidence.signCount, 0x01020304)
    XCTAssertGreaterThan(evidence.authenticatorData.count, 37)
    XCTAssertEqual(evidence.validationCategory, 4)
    XCTAssertEqual(evidence.bundleVersion, "1")
    XCTAssertEqual(evidence.signatureDER, Data([0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01]))
    XCTAssertEqual(evidence.signatureMessageDigest,
      Data(SHA256.hash(data: evidence.authenticatorData + binding.clientDataHash)))
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
      expectedRelease: release()))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 0), clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release()))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 1, flags: 0x41),
      clientDataHash: hash, expectedAppIDHash: appIDHash, expectedRelease: release()))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: raw + Data([0]), clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release()))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: Data([0xa2]) + cborText("signature") + cborBytes(Data([0x30]))
        + cborText("signature") + cborBytes(Data([0x30])),
      clientDataHash: hash, expectedAppIDHash: appIDHash, expectedRelease: release()))
  }

  func testAssertionReleaseExtensionsAreRequiredAndPinned() throws {
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
        expectedRelease: expected)) { error in
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
        expectedRelease: expected)) { error in
        XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1,
          .releaseMismatch)
      }
    }
    let testFlight = try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 1, category: 2, bundleVersion: "42"),
      clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release(category: 2, version: "42"))
    let appStore = try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 1), clientDataHash: hash,
      expectedAppIDHash: appIDHash, expectedRelease: expected)
    XCTAssertNotEqual(testFlight.signatureMessageDigest, appStore.signatureMessageDigest)
  }

  func testEDUnsetStillRequiresAndBindsExactSignedExtensions() throws {
    let hash = try binding().clientDataHash
    let raw = assertion(counter: 1, flags: 0x01)
    let evidence = try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: raw, clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release())
    XCTAssertEqual(evidence.authenticatorData[32], 0x01)
    XCTAssertEqual(evidence.validationCategory, 4)
    XCTAssertEqual(evidence.bundleVersion, "1")
    XCTAssertEqual(evidence.signatureMessageDigest,
      Data(SHA256.hash(data: evidence.authenticatorData + hash)))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 1, flags: 0x01, bundleVersion: nil),
      clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release()))
    XCTAssertThrowsError(try KagemushaAppAttestAssertionEvidenceV1(
      rawAssertion: assertion(counter: 1, flags: 0x01, category: 2),
      clientDataHash: hash, expectedAppIDHash: appIDHash,
      expectedRelease: release()))
  }

  func testReleaseMismatchLeavesDurablePendingIntent() async throws {
    let store = AppAttestFixtureIntentStore(counter: 0)
    let service = AppAttestFixtureService(assertion: assertion(counter: 1, category: 2))
    let provider = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release())
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
      expectedRelease: release())
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
      expectedRelease: release())
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

  func testSkippedCounterFreezesAcrossProviderRecreation() async throws {
    let store = AppAttestFixtureIntentStore(counter: 4)
    let service = AppAttestFixtureService(assertion: assertion(counter: 6))
    let selected = try binding()
    let provider = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release())
    do {
      _ = try await provider.assertTransition(
        keyID: "dedicated-key", binding: selected, expectedPreviousCounter: 4)
      XCTFail("skipped counter accepted")
    } catch {
      XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1, .assertionOutcomeUnknown)
    }
    let restarted = try KagemushaAppAttestEvidenceProviderV1(
      service: service, intentStore: store, expectedAppIDHash: appIDHash,
      expectedRelease: release())
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
      expectedRelease: release())
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
      expectedRelease: release())
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
      expectedRelease: release())
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

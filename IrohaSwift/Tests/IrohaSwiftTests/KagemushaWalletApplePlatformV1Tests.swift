import CryptoKit
import Darwin
import Foundation
import Security
import XCTest

@testable import IrohaSwift

/// In-memory keychain: payment keys are software P-256 keys (a Secure Enclave is not needed to
/// exercise the adapter's logic), generic passwords are byte values.
private final class FakeWalletKeychain: KagemushaWalletAppleKeychainV1, @unchecked Sendable {
  private struct StoredKey {
    let tag: Data
    let label: String?
    let key: SecKey
  }

  private let lock = NSLock()
  private var keys: [StoredKey] = []
  private var passwords: [String: Data] = [:]
  private var log: [String] = []
  private var generations: [[String: Any]] = []

  var copyStatus: OSStatus?
  var copyResultOverride: CFTypeRef??
  var addStatus: OSStatus?
  var updateStatus: OSStatus?
  var deleteStatus: OSStatus?
  var createStatus: OSStatus?
  var persistGeneratedKeys = true

  var operations: [String] { locked { log } }
  var generationAttributes: [[String: Any]] { locked { generations } }

  static func softwareKey() -> SecKey {
    var error: Unmanaged<CFError>?
    let attributes: [String: Any] = [
      kSecAttrKeyType as String: kSecAttrKeyTypeECSECPrimeRandom,
      kSecAttrKeySizeInBits as String: 256,
    ]
    return SecKeyCreateRandomKey(attributes as CFDictionary, &error)!
  }

  @discardableResult
  func storeKey(tag: Data, label: String?) -> SecKey {
    let key = Self.softwareKey()
    locked { keys.append(StoredKey(tag: tag, label: label, key: key)) }
    return key
  }

  func copyMatching(_ query: [String: Any]) -> (status: OSStatus, result: CFTypeRef?) {
    locked {
      log.append("copy")
      if let copyStatus { return (copyStatus, copyResultOverride ?? nil) }
      if let override = copyResultOverride { return (errSecSuccess, override) }
      if Self.isKey(query) {
        let tag = query[kSecAttrApplicationTag as String] as? Data
        let matches = keys.filter { $0.tag == tag }
        guard !matches.isEmpty else { return (errSecItemNotFound, nil) }
        let items: [[String: Any]] = matches.map { stored in
          var item: [String: Any] = [kSecValueRef as String: stored.key]
          if let label = stored.label { item[kSecAttrLabel as String] = label }
          return item
        }
        return (errSecSuccess, items as NSArray)
      }
      guard let value = passwords[Self.passwordName(query)] else { return (errSecItemNotFound, nil) }
      return (errSecSuccess, value as NSData)
    }
  }

  func add(_ attributes: [String: Any]) -> OSStatus {
    locked {
      log.append("add")
      if let addStatus { return addStatus }
      let name = Self.passwordName(attributes)
      guard passwords[name] == nil else { return errSecDuplicateItem }
      passwords[name] = attributes[kSecValueData as String] as? Data
      return errSecSuccess
    }
  }

  func update(_ query: [String: Any], _ attributes: [String: Any]) -> OSStatus {
    locked {
      log.append("update")
      if let updateStatus { return updateStatus }
      let name = Self.passwordName(query)
      guard passwords[name] != nil else { return errSecItemNotFound }
      passwords[name] = attributes[kSecValueData as String] as? Data
      return errSecSuccess
    }
  }

  func delete(_ query: [String: Any]) -> OSStatus {
    locked {
      log.append("delete")
      if let deleteStatus { return deleteStatus }
      let tag = query[kSecAttrApplicationTag as String] as? Data
      let before = keys.count
      keys.removeAll { $0.tag == tag }
      return keys.count == before ? errSecItemNotFound : errSecSuccess
    }
  }

  func createRandomKey(_ attributes: [String: Any]) -> (key: SecKey?, status: OSStatus) {
    locked {
      log.append("create")
      generations.append(attributes)
      if let createStatus { return (nil, createStatus) }
      let key = Self.softwareKey()
      let privateAttributes = attributes[kSecPrivateKeyAttrs as String] as? [String: Any] ?? [:]
      if persistGeneratedKeys, let tag = privateAttributes[kSecAttrApplicationTag as String] as? Data {
        keys.append(
          StoredKey(tag: tag, label: privateAttributes[kSecAttrLabel as String] as? String, key: key))
      }
      return (key, errSecSuccess)
    }
  }

  private static func isKey(_ query: [String: Any]) -> Bool {
    (query[kSecClass as String] as? String) == (kSecClassKey as String)
  }

  private static func passwordName(_ query: [String: Any]) -> String {
    "\(query[kSecAttrService as String] as? String ?? "")\u{0}\(query[kSecAttrAccount as String] as? String ?? "")"
  }

  private func locked<T>(_ body: () -> T) -> T {
    lock.lock()
    defer { lock.unlock() }
    return body()
  }
}

/// Scripted protected-data canary: queued `errno` answers, then `fallback`.
private final class FakeCanary: @unchecked Sendable {
  private let lock = NSLock()
  private var queue: [Int32] = []
  private var count = 0
  var fallback: Int32 = 0

  func push(_ codes: Int32...) {
    lock.lock()
    queue.append(contentsOf: codes)
    lock.unlock()
  }

  func next() -> Int32 {
    lock.lock()
    defer { lock.unlock() }
    count += 1
    return queue.isEmpty ? fallback : queue.removeFirst()
  }

  var reads: Int {
    lock.lock()
    defer { lock.unlock() }
    return count
  }
}

private struct FakeAppAttestFailure: Error {}

private final class FakeWalletAppAttest: KagemushaAppAttestServiceV1, @unchecked Sendable {
  private let lock = NSLock()
  private var log: [String] = []
  private var keyCount = 0
  var supported = true
  var failingStage: String?

  var isSupported: Bool { supported }
  var calls: [String] {
    lock.lock()
    defer { lock.unlock() }
    return log
  }

  func generateKey() async throws -> String {
    let keyID = nextKeyID()
    try check("generateKey")
    return keyID
  }

  func attestKey(_ keyID: String, clientDataHash: Data) async throws -> Data {
    record("attestKey:\(keyID):\(kagemushaWalletAppleHex(clientDataHash))")
    try check("attestKey")
    return Data("attestation".utf8) + clientDataHash
  }

  func generateAssertion(_ keyID: String, clientDataHash: Data) async throws -> Data {
    record("generateAssertion:\(keyID):\(kagemushaWalletAppleHex(clientDataHash))")
    try check("generateAssertion")
    return Data("assertion".utf8) + clientDataHash
  }

  private func nextKeyID() -> String {
    lock.lock()
    defer { lock.unlock() }
    keyCount += 1
    log.append("generateKey")
    return "app-attest-key-\(keyCount)"
  }

  private func record(_ entry: String) {
    lock.lock()
    log.append(entry)
    lock.unlock()
  }

  private func check(_ stage: String) throws {
    if failingStage == stage { throw FakeAppAttestFailure() }
  }
}

final class KagemushaWalletApplePlatformV1Tests: XCTestCase {
  private let slot = KagemushaWalletAppleSlotV1(Data((1...32).map { UInt8($0) }))!
  private let challenge = Data(repeating: 0xc4, count: 32)
  private let keyBinding = Data(repeating: 0x5b, count: 32)

  private func makePlatform(
    keychain: FakeWalletKeychain = FakeWalletKeychain(), canary: FakeCanary = FakeCanary(),
    appAttest: FakeWalletAppAttest = FakeWalletAppAttest(), secureEnclave: Bool = true,
    passcode: Bool? = true, base: URL? = nil,
    sysctl: @escaping @Sendable (String) -> Result<[UInt8], KagemushaWalletAppleUnavailableV1> = {
      _ in .failure(.io(ENOENT))
    },
    time: KagemushaWalletAppleContinuousTimeV1 = .init(ticks: 0, numer: 1, denom: 1),
    liveCanary: Bool = false
  ) -> KagemushaWalletApplePlatformV1 {
    let directory = base ?? FileManager.default.temporaryDirectory
      .appendingPathComponent("kagemusha-wallet-apple-unused-\(UUID().uuidString)")
    var probe: @Sendable (String) -> Int32 = { _ in canary.next() }
    if liveCanary { probe = KagemushaWalletAppleSystemV1.live.probeReadable }
    let system = KagemushaWalletAppleSystemV1(
      keychain: keychain, secureEnclaveAvailable: { secureEnclave }, passcodeSet: { passcode },
      applicationSupportDirectory: { .success(directory) }, probeReadable: probe, sysctl: sysctl,
      continuousTime: { time }, diagnostic: { _ in })
    return KagemushaWalletApplePlatformV1(appAttest: appAttest, system: system)
  }

  private func request(
    _ profile: KagemushaWalletAppleKeyProfileV1 = .secureElement
  ) -> KagemushaWalletAppleKeyGenerationRequestV1 {
    KagemushaWalletAppleKeyGenerationRequestV1(challengeDigest: challenge, profile: profile)!
  }

  private func temporaryDirectory() throws -> URL {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("kagemusha-wallet-apple-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    return directory
  }

  private func failure<Success>(
    _ result: Result<Success, KagemushaWalletAppleUnavailableV1>
  ) -> KagemushaWalletAppleUnavailableV1? {
    if case .failure(let reason) = result { return reason }
    return nil
  }

  private func generatedKey(_ platform: KagemushaWalletApplePlatformV1) throws -> Data {
    guard case .generated(let publicKey) = platform.keyGenerate(slot, request()) else {
      XCTFail("payment key was not generated")
      throw FakeAppAttestFailure()
    }
    return publicKey
  }

  // MARK: Slot and generation binding

  func testSlotRequiresThirtyTwoNonzeroBytesAndNamesTheKeychainItems() {
    XCTAssertEqual(
      slot.keychainName,
      "kgm-w1-0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20")
    XCTAssertEqual(slot.applicationTag, Data(slot.keychainName.utf8))
    XCTAssertNil(KagemushaWalletAppleSlotV1(Data(repeating: 1, count: 31)))
    XCTAssertNil(KagemushaWalletAppleSlotV1(Data(repeating: 1, count: 33)))
    XCTAssertNil(KagemushaWalletAppleSlotV1(Data(repeating: 0, count: 32)))
  }

  func testGenerationLabelBindsChallengeAndProfileAndParsesStrictly() {
    let bound = request(.secureElementOrTee)
    XCTAssertEqual(bound.label, "kgm-w1-binding:2:" + String(repeating: "c4", count: 32))
    XCTAssertEqual(KagemushaWalletAppleKeyGenerationRequestV1(label: bound.label), bound)
    XCTAssertEqual(
      KagemushaWalletAppleKeyGenerationRequestV1(label: request(.secureElement).label)?.profile,
      .secureElement)
    let digest = String(repeating: "c4", count: 32)
    for label in [
      "kgm-w1-binding:3:" + digest, "kgm-w1-binding:0:" + digest,
      "kgm-w1-binding:1:" + digest.uppercased(), "kgm-w1-binding:1-" + digest,
      "kgm-w1-binding:1:" + digest.dropLast(2), "kgm-w1-binding:1:" + digest + "00",
      "kgm-w1-bindinG:1:" + digest, "",
    ] {
      XCTAssertNil(KagemushaWalletAppleKeyGenerationRequestV1(label: label), label)
    }
    XCTAssertNil(
      KagemushaWalletAppleKeyGenerationRequestV1(
        challengeDigest: Data(repeating: 1, count: 31), profile: .secureElement))
  }

  // MARK: Status mapping

  func testKeychainStatusesMapToTriStateAndExplicitWriteOutcomes() {
    typealias Platform = KagemushaWalletApplePlatformV1
    XCTAssertEqual(Platform.unavailable(errSecInteractionNotAllowed), .locked)
    XCTAssertEqual(Platform.unavailable(errSecItemNotFound), .platform(errSecItemNotFound))
    XCTAssertEqual(Platform.unavailable(errSecIO), .platform(errSecIO))

    XCTAssertEqual(Platform.addOutcome(errSecSuccess), .published)
    XCTAssertEqual(Platform.addOutcome(errSecDuplicateItem), .notPublished(.destinationExists))
    XCTAssertEqual(Platform.addOutcome(errSecInteractionNotAllowed), .notPublished(.failed(.locked)))
    XCTAssertEqual(Platform.addOutcome(errSecDiskFull), .notPublished(.noSpace))
    XCTAssertEqual(Platform.addOutcome(errSecParam), .notPublished(.failed(.platform(errSecParam))))
    XCTAssertEqual(
      Platform.addOutcome(errSecMissingEntitlement),
      .notPublished(.failed(.platform(errSecMissingEntitlement))))
    XCTAssertEqual(Platform.addOutcome(errSecIO), .uncertain(.platform(errSecIO)))
    XCTAssertEqual(
      Platform.addOutcome(errSecInternalComponent), .uncertain(.platform(errSecInternalComponent)))

    XCTAssertEqual(Platform.updateOutcome(errSecSuccess), .published)
    XCTAssertEqual(Platform.updateOutcome(errSecItemNotFound), .notPublished(.destinationAbsent))
    XCTAssertEqual(Platform.updateOutcome(errSecInteractionNotAllowed), .notPublished(.failed(.locked)))
    XCTAssertEqual(Platform.updateOutcome(errSecIO), .uncertain(.platform(errSecIO)))

    XCTAssertNil(failure(Platform.storageState(errno: 0)))
    XCTAssertEqual(failure(Platform.storageState(errno: EPERM)), .locked)
    XCTAssertEqual(failure(Platform.storageState(errno: ENOENT)), .io(ENOENT))
    XCTAssertEqual(failure(Platform.storageState(errno: EACCES)), .io(EACCES))
  }

  // MARK: Payment key

  func testKeyProbeIsTriStateAndNeverInfersAbsenceFromAnError() {
    let keychain = FakeWalletKeychain()
    let platform = makePlatform(keychain: keychain)
    XCTAssertEqual(platform.keyProbe(slot), .absent)

    keychain.copyStatus = errSecInteractionNotAllowed
    XCTAssertEqual(platform.keyProbe(slot), .unavailable(.locked))
    keychain.copyStatus = errSecIO
    XCTAssertEqual(platform.keyProbe(slot), .unavailable(.platform(errSecIO)))
    keychain.copyStatus = errSecMissingEntitlement
    XCTAssertEqual(platform.keyProbe(slot), .unavailable(.platform(errSecMissingEntitlement)))
    keychain.copyStatus = nil

    keychain.copyResultOverride = .some(nil)
    XCTAssertEqual(
      platform.keyProbe(slot),
      .unavailable(.platform(KagemushaWalletAppleStatusV1.malformedKeychainResult)))
    keychain.copyResultOverride = .some([[String: Any]]() as NSArray)
    XCTAssertEqual(
      platform.keyProbe(slot),
      .unavailable(.platform(KagemushaWalletAppleStatusV1.malformedKeychainResult)))
    keychain.copyResultOverride = nil

    let key = keychain.storeKey(tag: slot.applicationTag, label: request().label)
    let expected = SecKeyCopyExternalRepresentation(SecKeyCopyPublicKey(key)!, nil)! as Data
    XCTAssertEqual(expected.count, 65)
    XCTAssertEqual(expected.first, 0x04)
    XCTAssertEqual(platform.keyProbe(slot), .present(expected))

    keychain.storeKey(tag: slot.applicationTag, label: nil)
    XCTAssertEqual(
      platform.keyProbe(slot),
      .unavailable(.platform(KagemushaWalletAppleStatusV1.ambiguousPaymentKey)))
  }

  func testProtectedDataBracketsSurroundEveryAbsence() {
    let keychain = FakeWalletKeychain()
    let canary = FakeCanary()
    let platform = makePlatform(keychain: keychain, canary: canary)

    canary.push(EPERM)
    XCTAssertEqual(platform.keyProbe(slot), .unavailable(.locked))
    XCTAssertEqual(keychain.operations, [], "no keychain query while protected data is locked")

    canary.push(0, EPERM)
    XCTAssertEqual(platform.keyProbe(slot), .unavailable(.locked))
    canary.push(0, EPERM)
    XCTAssertEqual(platform.anchorRead(slot), .unavailable(.locked))
    canary.push(0, ENOENT)
    XCTAssertEqual(platform.anchorRead(slot), .unavailable(.io(ENOENT)))
    XCTAssertEqual(platform.keyProbe(slot), .absent)

    let key = keychain.storeKey(tag: slot.applicationTag, label: nil)
    canary.push(0, EPERM)
    XCTAssertEqual(
      platform.keyProbe(slot), .present(KagemushaWalletApplePlatformV1.x963PublicKey(of: key)!),
      "a present answer stands even if the device locks afterwards")
  }

  func testKeyGenerationUsesSecureEnclavePolicyAndBindsTheChallenge() throws {
    let keychain = FakeWalletKeychain()
    let platform = makePlatform(keychain: keychain)
    let publicKey = try generatedKey(platform)
    XCTAssertEqual(publicKey.count, 65)
    XCTAssertEqual(platform.keyProbe(slot), .present(publicKey))

    let attributes = try XCTUnwrap(keychain.generationAttributes.first)
    XCTAssertEqual(attributes[kSecAttrTokenID as String] as? String, kSecAttrTokenIDSecureEnclave as String)
    XCTAssertEqual(attributes[kSecAttrKeyType as String] as? String, kSecAttrKeyTypeECSECPrimeRandom as String)
    XCTAssertEqual(attributes[kSecAttrKeySizeInBits as String] as? Int, 256)
    let privateAttributes = try XCTUnwrap(attributes[kSecPrivateKeyAttrs as String] as? [String: Any])
    XCTAssertEqual(privateAttributes[kSecAttrIsPermanent as String] as? Bool, true)
    XCTAssertEqual(privateAttributes[kSecAttrApplicationTag as String] as? Data, slot.applicationTag)
    XCTAssertEqual(privateAttributes[kSecAttrLabel as String] as? String, request().label)
    let access = try XCTUnwrap(privateAttributes[kSecAttrAccessControl as String])
    XCTAssertEqual(CFGetTypeID(access as AnyObject), SecAccessControlGetTypeID())

    XCTAssertEqual(platform.keyGenerate(slot, request()), .alreadyPresent)
    XCTAssertEqual(keychain.generationAttributes.count, 1, "an existing key is never replaced")
  }

  func testKeyGenerationRefusesOrReportsUnknownOutcomes() {
    XCTAssertEqual(
      makePlatform(secureEnclave: false).keyGenerate(slot, request()),
      .unavailable(.platform(KagemushaWalletAppleStatusV1.secureEnclaveUnavailable)))
    let unsupported = FakeWalletAppAttest()
    unsupported.supported = false
    XCTAssertEqual(
      makePlatform(appAttest: unsupported).keyGenerate(slot, request()),
      .unavailable(.platform(KagemushaWalletAppleStatusV1.appAttestUnsupported)))

    let canary = FakeCanary()
    canary.push(EPERM)
    let lockedKeychain = FakeWalletKeychain()
    XCTAssertEqual(
      makePlatform(keychain: lockedKeychain, canary: canary).keyGenerate(slot, request()),
      .unavailable(.locked))
    XCTAssertEqual(lockedKeychain.generationAttributes.count, 0)

    let cases: [(OSStatus, KagemushaWalletAppleKeyGenerationV1)] = [
      (errSecDuplicateItem, .alreadyPresent),
      (errSecInteractionNotAllowed, .unavailable(.locked)),
      (errSecIO, .unavailable(.platform(errSecIO))),
    ]
    for (status, expected) in cases {
      let keychain = FakeWalletKeychain()
      keychain.createStatus = status
      XCTAssertEqual(makePlatform(keychain: keychain).keyGenerate(slot, request()), expected)
    }

    let unpersisted = FakeWalletKeychain()
    unpersisted.persistGeneratedKeys = false
    XCTAssertEqual(
      makePlatform(keychain: unpersisted).keyGenerate(slot, request()),
      .unavailable(.platform(KagemushaWalletAppleStatusV1.paymentKeyNotPersisted)))
  }

  func testPaymentKeyIsNeverBoundToUserAuthentication() throws {
    typealias Platform = KagemushaWalletApplePlatformV1
    XCTAssertEqual(
      Platform.paymentKeyAccessibility as String,
      kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly as String)
    XCTAssertEqual(Platform.paymentKeyAccessFlags, [.privateKeyUsage])
    XCTAssertTrue(
      Platform.paymentKeyAccessFlags.isDisjoint(
        with: [.userPresence, .biometryAny, .biometryCurrentSet, .devicePasscode, .applicationPassword]))
    if case .failure(let reason) = Platform.paymentKeyAccessControl() {
      XCTFail("access control: \(reason)")
    }
  }

  func testKeySignReturnsDERThatVerifiesOverTheSHA256Preimage() throws {
    let keychain = FakeWalletKeychain()
    let platform = makePlatform(keychain: keychain)
    let publicKey = try generatedKey(platform)
    let preimage = Data("iroha:kagemusha:wallet:v1:receipt-body\u{0}example".utf8)
    let der: Data
    switch platform.keySign(slot, preimage: preimage) {
    case .success(let signature): der = signature
    case .failure(let reason): return XCTFail("sign: \(reason)")
    }
    XCTAssertEqual(der.first, 0x30, "strict DER sequence; low-S normalization is Rust's")
    let signature = try P256.Signing.ECDSASignature(derRepresentation: der)
    let verifier = try P256.Signing.PublicKey(x963Representation: publicKey)
    XCTAssertTrue(verifier.isValidSignature(signature, for: preimage))
    XCTAssertFalse(verifier.isValidSignature(signature, for: preimage + Data([0])))

    let other = KagemushaWalletAppleSlotV1(Data(repeating: 9, count: 32))!
    XCTAssertEqual(failure(platform.keySign(other, preimage: preimage)), .platform(errSecItemNotFound))
    keychain.copyStatus = errSecInteractionNotAllowed
    XCTAssertEqual(failure(platform.keySign(slot, preimage: preimage)), .locked)
  }

  func testKeyDeleteIsConfirmedByABracketedProbe() throws {
    let keychain = FakeWalletKeychain()
    let canary = FakeCanary()
    let platform = makePlatform(keychain: keychain, canary: canary)
    _ = try generatedKey(platform)

    canary.push(EPERM)
    XCTAssertEqual(platform.keyDelete(slot), .notRemoved(.locked))
    XCTAssertFalse(keychain.operations.contains("delete"))

    keychain.deleteStatus = errSecInteractionNotAllowed
    XCTAssertEqual(platform.keyDelete(slot), .notRemoved(.locked))
    keychain.deleteStatus = errSecIO
    XCTAssertEqual(platform.keyDelete(slot), .uncertain(.platform(errSecIO)))
    keychain.deleteStatus = nil

    canary.push(0, 0, EPERM)
    XCTAssertEqual(platform.keyDelete(slot), .uncertain(.locked))
    XCTAssertEqual(platform.keyProbe(slot), .absent)
    XCTAssertEqual(platform.keyDelete(slot), .removed)
  }

  // MARK: Rollback anchor

  func testAnchorItemIsPasscodeBoundDeviceOnlyAndKeepsTheRustBytes() {
    let value = Data((0..<57).map { UInt8(truncatingIfNeeded: $0 &* 7) })
    let attributes = KagemushaWalletApplePlatformV1.anchorAddAttributes(slot, value: value)
    XCTAssertEqual(attributes[kSecClass as String] as? String, kSecClassGenericPassword as String)
    XCTAssertEqual(
      attributes[kSecAttrService as String] as? String,
      "org.hyperledger.iroha.kagemusha.wallet.v1.marker-anchor")
    XCTAssertEqual(attributes[kSecAttrAccount as String] as? String, slot.keychainName)
    XCTAssertEqual(
      attributes[kSecAttrAccessible as String] as? String,
      kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly as String)
    XCTAssertEqual(attributes[kSecAttrSynchronizable as String] as? Bool, false)
    XCTAssertEqual(attributes[kSecUseDataProtectionKeychain as String] as? Bool, true)
    XCTAssertEqual(attributes[kSecValueData as String] as? Data, value)

    let query = KagemushaWalletApplePlatformV1.anchorQuery(slot)
    XCTAssertNil(query[kSecValueData as String])
    XCTAssertNil(query[kSecAttrAccessible as String])
    XCTAssertEqual(KagemushaWalletApplePlatformV1.anchorPolicyTag, 1)
  }

  func testAnchorIsAddOnlyAndUpdatedOnlyInPlace() {
    let keychain = FakeWalletKeychain()
    let platform = makePlatform(keychain: keychain)
    let none = Data(repeating: 0x11, count: 54)
    let raised = Data(repeating: 0x22, count: 54)

    XCTAssertEqual(platform.anchorRead(slot), .absent)
    XCTAssertEqual(platform.anchorUpdate(slot, value: raised), .notPublished(.destinationAbsent))
    XCTAssertEqual(platform.anchorCreate(slot, value: none), .published)
    XCTAssertEqual(platform.anchorRead(slot), .present(none))
    XCTAssertEqual(platform.anchorCreate(slot, value: raised), .notPublished(.destinationExists))
    XCTAssertEqual(platform.anchorRead(slot), .present(none))
    XCTAssertEqual(platform.anchorUpdate(slot, value: raised), .published)
    XCTAssertEqual(platform.anchorRead(slot), .present(raised))

    let invalid = KagemushaWalletAppleNotPublishedV1.failed(
      .platform(KagemushaWalletAppleStatusV1.invalidAnchorValue))
    XCTAssertEqual(platform.anchorCreate(slot, value: Data()), .notPublished(invalid))
    XCTAssertEqual(
      platform.anchorUpdate(slot, value: Data(repeating: 1, count: 257)), .notPublished(invalid))
    XCTAssertEqual(platform.anchorUpdate(slot, value: Data(repeating: 1, count: 256)), .published)

    keychain.copyStatus = errSecInteractionNotAllowed
    XCTAssertEqual(platform.anchorRead(slot), .unavailable(.locked))
    keychain.copyStatus = errSecIO
    XCTAssertEqual(platform.anchorRead(slot), .unavailable(.platform(errSecIO)))
    keychain.copyStatus = nil
    keychain.updateStatus = errSecIO
    XCTAssertEqual(platform.anchorUpdate(slot, value: raised), .uncertain(.platform(errSecIO)))
  }

  func testAnchorCreationIsRefusedWhileLockedOrWithoutPasscode() {
    let keychain = FakeWalletKeychain()
    let canary = FakeCanary()
    canary.push(EPERM, EPERM)
    let platform = makePlatform(keychain: keychain, canary: canary)
    let value = Data(repeating: 0x11, count: 54)
    XCTAssertEqual(platform.anchorCreate(slot, value: value), .notPublished(.failed(.locked)))
    XCTAssertEqual(platform.anchorUpdate(slot, value: value), .notPublished(.failed(.locked)))
    XCTAssertEqual(keychain.operations, [])

    let noPasscode = makePlatform(keychain: keychain, passcode: false)
    XCTAssertEqual(
      noPasscode.anchorCreate(slot, value: value),
      .notPublished(.failed(.platform(KagemushaWalletAppleStatusV1.passcodeNotSet))))
    XCTAssertEqual(keychain.operations, [])

    keychain.addStatus = errSecIO
    XCTAssertEqual(
      makePlatform(keychain: keychain, passcode: nil).anchorCreate(slot, value: value),
      .uncertain(.platform(errSecIO)))
  }

  // MARK: Custody root

  func testCustodyRootIsPrivateExcludedFromBackupAndCarriesTheCanary() throws {
    let base = try temporaryDirectory()
    let platform = makePlatform(base: base, liveCanary: true)
    XCTAssertEqual(failure(platform.storageState()), .io(ENOENT), "no canary before preparation")

    let path: String
    switch platform.custodyRootPath() {
    case .success(let prepared): path = prepared
    case .failure(let reason): return XCTFail("prepare: \(reason)")
    }
    let root = base.appendingPathComponent("kagemusha-wallet-v1", isDirectory: true)
    XCTAssertEqual(path, root.path)
    let manager = FileManager.default
    let attributes = try manager.attributesOfItem(atPath: path)
    XCTAssertEqual(attributes[.type] as? FileAttributeType, .typeDirectory)
    XCTAssertEqual((attributes[.posixPermissions] as? NSNumber)?.intValue, 0o700)
    XCTAssertEqual(try root.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup, true)
    let canary = root.appendingPathComponent("canary").path
    XCTAssertEqual(try Data(contentsOf: URL(fileURLWithPath: canary)), Data([0x01]))
    XCTAssertNil(failure(platform.storageState()))
    let inode = try manager.attributesOfItem(atPath: canary)[.systemFileNumber] as? NSNumber

    try manager.setAttributes([.posixPermissions: NSNumber(value: Int16(0o755))], ofItemAtPath: path)
    XCTAssertEqual(try platform.custodyRootPath().get(), path)
    XCTAssertEqual(
      (try manager.attributesOfItem(atPath: path)[.posixPermissions] as? NSNumber)?.intValue, 0o700,
      "mode is re-applied on every preparation")
    XCTAssertEqual(
      try manager.attributesOfItem(atPath: canary)[.systemFileNumber] as? NSNumber, inode,
      "the canary is never rewritten")
  }

  func testCustodyRootRefusesLinksAndUnexpectedEntriesWithoutRemovingThem() throws {
    let manager = FileManager.default
    let invalid = KagemushaWalletAppleUnavailableV1.platform(KagemushaWalletAppleStatusV1.invalidCustodyRoot)

    let linked = try temporaryDirectory()
    let target = try temporaryDirectory()
    let link = linked.appendingPathComponent("kagemusha-wallet-v1")
    try manager.createSymbolicLink(at: link, withDestinationURL: target)
    XCTAssertEqual(failure(makePlatform(base: linked).custodyRootPath()), invalid)
    XCTAssertEqual(try manager.destinationOfSymbolicLink(atPath: link.path), target.path)

    let file = try temporaryDirectory()
    let fileRoot = file.appendingPathComponent("kagemusha-wallet-v1")
    XCTAssertTrue(manager.createFile(atPath: fileRoot.path, contents: Data([7])))
    XCTAssertEqual(failure(makePlatform(base: file).custodyRootPath()), invalid)
    XCTAssertEqual(try Data(contentsOf: fileRoot), Data([7]))

    let canaryDirectory = try temporaryDirectory()
    try manager.createDirectory(
      at: canaryDirectory.appendingPathComponent("kagemusha-wallet-v1/canary"),
      withIntermediateDirectories: true)
    XCTAssertEqual(failure(makePlatform(base: canaryDirectory).custodyRootPath()), invalid)
  }

  func testProtectionClassesFollowTheDesign() {
    typealias Platform = KagemushaWalletApplePlatformV1
    XCTAssertEqual(Platform.custodyRootProtection, .completeUntilFirstUserAuthentication)
    XCTAssertEqual(Platform.canaryProtection, .complete)
    XCTAssertEqual((Platform.custodyRootAttributes[.posixPermissions] as? NSNumber)?.intValue, 0o700)
    XCTAssertTrue(Platform.canaryWriteOptions.contains(.withoutOverwriting))
    #if os(iOS)
    XCTAssertEqual(
      Platform.custodyRootAttributes[.protectionKey] as? FileProtectionType,
      .completeUntilFirstUserAuthentication)
    XCTAssertTrue(Platform.canaryWriteOptions.contains(.completeFileProtection))
    #endif
  }

  // MARK: Boot identity and clock

  func testBootSessionUUIDParsing() {
    typealias Platform = KagemushaWalletApplePlatformV1
    let uuid = "6F1C2B3A-1234-4ABC-8DEF-0123456789AB"
    XCTAssertEqual(
      try Platform.bootSessionUUID(fromSysctl: Array(uuid.utf8) + [0]).get(), uuid.lowercased())
    XCTAssertEqual(
      try Platform.bootSessionUUID(fromSysctl: Array(uuid.lowercased().utf8) + [0x0a, 0, 0]).get(),
      uuid.lowercased())
    for malformed in [
      String(uuid.dropLast()), uuid + "0", uuid.replacingOccurrences(of: "-4ABC", with: "4-ABC"),
      uuid.replacingOccurrences(of: "F", with: "G"), "", "\u{0}" + uuid,
    ] {
      XCTAssertEqual(
        failure(Platform.bootSessionUUID(fromSysctl: Array(malformed.utf8) + [0])), .io(0), malformed)
    }
    XCTAssertEqual(failure(Platform.bootSessionUUID(fromSysctl: [0xff, 0])), .io(0))

    let requested = FakeCanary()
    let platform = makePlatform(sysctl: { name in
      _ = requested.next()
      return name == "kern.bootsessionuuid" ? .success(Array(uuid.utf8) + [0]) : .failure(.io(EINVAL))
    })
    XCTAssertEqual(try platform.bootSessionUUID().get(), uuid.lowercased())
    XCTAssertEqual(requested.reads, 1)
    XCTAssertEqual(failure(makePlatform().bootSessionUUID()), .io(ENOENT))
  }

  func testLiveBootSessionUUIDIsWellFormedWhenReadable() {
    let platform = KagemushaWalletApplePlatformV1(
      appAttest: FakeWalletAppAttest(), system: .live)
    switch platform.bootSessionUUID() {
    case .success(let text):
      XCTAssertEqual(text.count, 36)
      XCTAssertEqual(text, text.lowercased())
    case .failure(let reason):
      // The sysctl may be unreadable in a sandbox; the answer is then unavailable, never empty.
      XCTAssertNotEqual(reason, .io(0))
    }
  }

  func testMonotonicMillisecondsConvertsWithoutOverflow() {
    typealias Time = KagemushaWalletAppleContinuousTimeV1
    let convert = KagemushaWalletApplePlatformV1.milliseconds
    XCTAssertEqual(convert(Time(ticks: 1_000_000_000, numer: 1, denom: 1)), 1_000)
    XCTAssertEqual(convert(Time(ticks: 24_000_000, numer: 125, denom: 3)), 1_000)
    XCTAssertEqual(convert(Time(ticks: 999_999, numer: 1, denom: 1)), 0)
    XCTAssertEqual(convert(Time(ticks: .max, numer: 1, denom: 1)), 18_446_744_073_709)
    XCTAssertEqual(convert(Time(ticks: .max, numer: 125, denom: 3)), 768_614_336_404_564)
    XCTAssertNil(convert(Time(ticks: 5, numer: 0, denom: 1)))
    XCTAssertNil(convert(Time(ticks: 5, numer: 1, denom: 0)))
    XCTAssertNil(convert(Time(ticks: .max, numer: .max, denom: 1)))

    XCTAssertEqual(
      try makePlatform(time: Time(ticks: 48_000_000, numer: 125, denom: 3)).monotonicMilliseconds().get(),
      2_000)
    XCTAssertEqual(
      failure(makePlatform(time: Time(ticks: 1, numer: 1, denom: 0)).monotonicMilliseconds()),
      .platform(KagemushaWalletAppleStatusV1.clockUnavailable))

    let live = KagemushaWalletApplePlatformV1(appAttest: FakeWalletAppAttest(), system: .live)
    let first = try? live.monotonicMilliseconds().get()
    let second = try? live.monotonicMilliseconds().get()
    XCTAssertNotNil(first)
    XCTAssertGreaterThanOrEqual(second ?? 0, first ?? .max)
  }

  // MARK: App Attest (E5)

  private func expectAttestationError(
    _ expected: KagemushaWalletAppleEnrollmentAttestationErrorV1,
    file: StaticString = #filePath, line: UInt = #line,
    _ body: () async throws -> KagemushaWalletAppleEnrollmentEvidenceV1
  ) async {
    do {
      _ = try await body()
      XCTFail("expected \(expected)", file: file, line: line)
    } catch let error as KagemushaWalletAppleEnrollmentAttestationErrorV1 {
      XCTAssertEqual(error, expected, file: file, line: line)
    } catch {
      XCTFail("unexpected \(error)", file: file, line: line)
    }
  }

  func testEnrollmentAttestationUsesTheBoundChallengeAndAFreshAppAttestKey() async throws {
    let appAttest = FakeWalletAppAttest()
    let platform = makePlatform(appAttest: appAttest)
    let publicKey = try generatedKey(platform)

    let evidence = try await platform.attestEnrollment(
      slot: slot, paymentPublicKey: publicKey, challengeDigest: challenge,
      keyBindingDigest: keyBinding)
    XCTAssertEqual(evidence.appAttestKeyID, "app-attest-key-1")
    XCTAssertEqual(evidence.attestation, Data("attestation".utf8) + challenge)
    XCTAssertEqual(evidence.keyBindingAssertion, Data("assertion".utf8) + keyBinding)
    XCTAssertEqual(
      appAttest.calls,
      [
        "generateKey",
        "attestKey:app-attest-key-1:" + kagemushaWalletAppleHex(challenge),
        "generateAssertion:app-attest-key-1:" + kagemushaWalletAppleHex(keyBinding),
      ])

    appAttest.failingStage = "attestKey"
    await expectAttestationError(.appAttestFailed(stage: "attestKey")) {
      try await platform.attestEnrollment(
        slot: slot, paymentPublicKey: publicKey, challengeDigest: challenge,
        keyBindingDigest: keyBinding)
    }
    appAttest.failingStage = nil
    let retried = try await platform.attestEnrollment(
      slot: slot, paymentPublicKey: publicKey, challengeDigest: challenge,
      keyBindingDigest: keyBinding)
    XCTAssertEqual(retried.appAttestKeyID, "app-attest-key-3", "every attempt uses a fresh key")
  }

  func testEnrollmentAttestationRefusesUnboundOrMissingKeys() async throws {
    let appAttest = FakeWalletAppAttest()
    let platform = makePlatform(appAttest: appAttest)
    await expectAttestationError(.paymentKeyAbsent) {
      try await platform.attestEnrollment(
        slot: slot, paymentPublicKey: Data(), challengeDigest: challenge, keyBindingDigest: keyBinding)
    }
    let publicKey = try generatedKey(platform)
    await expectAttestationError(.bindingMismatch) {
      try await platform.attestEnrollment(
        slot: slot, paymentPublicKey: publicKey, challengeDigest: Data(repeating: 0xc5, count: 32),
        keyBindingDigest: keyBinding)
    }
    var otherKey = publicKey
    otherKey[64] ^= 1
    await expectAttestationError(.bindingMismatch) {
      try await platform.attestEnrollment(
        slot: slot, paymentPublicKey: otherKey, challengeDigest: challenge, keyBindingDigest: keyBinding)
    }
    await expectAttestationError(.invalidDigest) {
      try await platform.attestEnrollment(
        slot: slot, paymentPublicKey: publicKey, challengeDigest: challenge,
        keyBindingDigest: Data(repeating: 1, count: 31))
    }
    XCTAssertEqual(appAttest.calls, [], "no App Attest call without a bound payment key")

    appAttest.supported = false
    await expectAttestationError(.appAttestUnsupported) {
      try await platform.attestEnrollment(
        slot: slot, paymentPublicKey: publicKey, challengeDigest: challenge,
        keyBindingDigest: keyBinding)
    }
  }
}

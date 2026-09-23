import CryptoKit
import Foundation

/// Client-data binding for Core's canonical, domain-separated hardware selection.
///
/// Core must construct and verify the fixed V1 subject body from its durable transition intent.
/// This type checks only the outer frame before giving the exact bytes to App Attest.
public struct KagemushaAppAttestTransitionBindingV1: Sendable {
  private static let signingDomain = Data("iroha:kagemusha:v1:hardware-transition-selection\0".utf8)
  private static let bodyBytes = 403
  private static let totalBytes = 460

  public let canonicalSelectionSigningBytes: Data

  public init(coreSelectionSigningBytes: Data) throws {
    let headerLength = Self.signingDomain.count + MemoryLayout<UInt64>.size
    guard coreSelectionSigningBytes.count == Self.totalBytes,
      headerLength + Self.bodyBytes == Self.totalBytes,
      coreSelectionSigningBytes.starts(with: Self.signingDomain) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidCanonicalSelection
    }
    var bodyLength: UInt64 = 0
    for (offset, byte) in coreSelectionSigningBytes[Self.signingDomain.count..<headerLength].enumerated() {
      bodyLength |= UInt64(byte) << (offset * 8)
    }
    guard bodyLength == UInt64(Self.bodyBytes) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidCanonicalSelection
    }
    canonicalSelectionSigningBytes = coreSelectionSigningBytes
  }

  /// Apple signs SHA256(authenticatorData || clientDataHash); here clientDataHash = SHA256(S).
  public var clientDataHash: Data {
    Data(SHA256.hash(data: canonicalSelectionSigningBytes))
  }
}

/// A distinct issuer challenge bound to one App Attest key and one approved phone lane.
public struct KagemushaAppAttestEnrollmentBindingV1: Sendable {
  private static let domain = Data("iroha:kagemusha:v1:app-device-attestation-challenge\0".utf8)

  public let clientNonce: Data
  public let serverNonce: Data
  public let releaseID: Data
  public let profileID: Data
  /// The decoded App Attest key ID: SHA256 of the key's P-256 SEC1 public point.
  public let attestedKeyID: Data
  public let laneID: Data

  public init(clientNonce: Data, serverNonce: Data, releaseID: Data,
    profileID: Data, attestedKeyID: Data, laneID: Data) throws {
    let fields = [clientNonce, serverNonce, releaseID, profileID, attestedKeyID, laneID]
    guard fields.allSatisfy({ $0.count == 32 && $0.contains(where: { $0 != 0 }) }),
      clientNonce != serverNonce else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidDigestLength
    }
    self.clientNonce = clientNonce
    self.serverNonce = serverNonce
    self.releaseID = releaseID
    self.profileID = profileID
    self.attestedKeyID = attestedKeyID
    self.laneID = laneID
  }

  public var canonicalClientData: Data {
    var bytes = Self.domain
    bytes.append(clientNonce)
    bytes.append(serverNonce)
    bytes.append(releaseID)
    bytes.append(profileID)
    bytes.append(attestedKeyID)
    bytes.append(laneID)
    return bytes
  }

  public var clientDataHash: Data {
    Data(SHA256.hash(data: canonicalClientData))
  }
}

/// Raw App Attest operations; these operations do not authorize value movement.
public protocol KagemushaAppAttestServiceV1: Sendable {
  var isSupported: Bool { get }
  func generateKey() async throws -> String
  func attestKey(_ keyID: String, clientDataHash: Data) async throws -> Data
  func generateAssertion(_ keyID: String, clientDataHash: Data) async throws -> Data
}

/// A durable assertion intent for one App Attest key.
///
/// Store implementations must atomically reserve a single pending transition across all
/// processes, sync it before returning, and sync the complete raw assertion before returning.
/// Pending and complete records freeze the lane. Only an authenticated Core
/// predecessor-commit acknowledgment may permit another assertion in a later protocol.
public enum KagemushaAppAttestAssertionIntentV1: Equatable, Sendable {
  case ready(counter: UInt32)
  case pending(previousCounter: UInt32, selectionDigest: Data)
  case complete(counter: UInt32, selectionDigest: Data, rawAssertion: Data)
}

public protocol KagemushaAppAttestAssertionIntentStoringV1: Sendable {
  func load(keyID: String) throws -> KagemushaAppAttestAssertionIntentV1
  func reserve(keyID: String, previousCounter: UInt32, selectionDigest: Data) throws
  func complete(keyID: String, counter: UInt32, selectionDigest: Data, rawAssertion: Data) throws
}

public enum KagemushaAppAttestEvidenceErrorV1: Error, Equatable, Sendable {
  case unsupportedDevice
  case invalidDigestLength
  case invalidCanonicalSelection
  case invalidAppIDHash
  case emptyKeyID
  case emptyRawObject
  case invalidAssertionObject
  case invalidAssertionSignature
  case invalidReleasePolicy
  case releaseMismatch
  case assertionCounterMismatch
  case assertionAlreadyInFlight
  case assertionOutcomeUnknown
  case journalMismatch
}

/// Release identity pinned by enrollment policy, independent of the device's claim.
/// This expectation is not itself a hardware measurement on iOS 26.
public struct KagemushaAppAttestExpectedReleaseV1: Equatable, Sendable {
  private static let digestDomain = Data("iroha:kagemusha:v1:app-attest-release\0".utf8)

  public let validationCategory: UInt32
  public let bundleVersion: String
  public let appReleaseDigest: Data

  /// The digest must come from authenticated release policy, not from the assertion.
  public init(validationCategory: UInt32, bundleVersion: String,
    authenticatedAppReleaseDigest: Data) throws {
    let calculated = try Self.canonicalReleaseDigest(
      validationCategory: validationCategory, bundleVersion: bundleVersion)
    guard authenticatedAppReleaseDigest.count == 32,
      authenticatedAppReleaseDigest.contains(where: { $0 != 0 }) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidDigestLength
    }
    guard authenticatedAppReleaseDigest == calculated else {
      throw KagemushaAppAttestEvidenceErrorV1.releaseMismatch
    }
    self.validationCategory = validationCategory
    self.bundleVersion = bundleVersion
    appReleaseDigest = authenticatedAppReleaseDigest
  }

  /// SHA-256(domain || UInt32 LE category || UInt16 LE UTF-8 length || exact UTF-8 bytes).
  public static func canonicalReleaseDigest(validationCategory: UInt32,
    bundleVersion: String) throws -> Data {
    guard [1, 2, 3, 4, 5, 6, 10].contains(validationCategory),
      !bundleVersion.isEmpty, bundleVersion.utf8.count <= 128,
      !bundleVersion.utf8.contains(0) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidReleasePolicy
    }
    let version = Data(bundleVersion.utf8)
    var preimage = digestDomain
    for shift in stride(from: 0, to: 32, by: 8) {
      preimage.append(UInt8(truncatingIfNeeded: validationCategory >> shift))
    }
    let count = UInt16(version.count)
    preimage.append(UInt8(truncatingIfNeeded: count))
    preimage.append(UInt8(truncatingIfNeeded: count >> 8))
    preimage.append(version)
    return Data(SHA256.hash(data: preimage))
  }
}

/// A release measurement is available only when Apple signs both extension fields.
public enum KagemushaAppAttestReleaseMeasurementV1: Equatable, Sendable {
  case unavailable
  case signed(validationCategory: UInt32, bundleVersion: String)
}

/// Verified App Attest assertion bytes. Core must still authenticate the predecessor and fold.
public struct KagemushaAppAttestAssertionEvidenceV1: Equatable, Sendable {
  public let rawAssertion: Data
  public let authenticatorData: Data
  public let signatureDER: Data
  public let signCount: UInt32
  public let releaseMeasurement: KagemushaAppAttestReleaseMeasurementV1
  public let clientDataHash: Data
  /// Apple's nonce: SHA256(authenticatorData || clientDataHash).
  public let assertionNonce: Data
  /// P-256-SHA256 signs the nonce, so the ECDSA message digest is SHA256(nonce).
  public let signatureMessageDigest: Data

  public init(rawAssertion: Data, clientDataHash: Data, expectedAppIDHash: Data,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1,
    enrolledAssertionPublicKeyX963: Data) throws {
    guard clientDataHash.count == 32, expectedAppIDHash.count == 32,
      expectedAppIDHash.contains(where: { $0 != 0 }) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidDigestLength
    }
    let parsed = try Self.parseAssertion(rawAssertion)
    guard parsed.authenticatorData.prefix(32) == expectedAppIDHash else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAppIDHash
    }
    let auth = [UInt8](parsed.authenticatorData)
    // iOS 26 App Attest sets 0x40 even though its assertion has no credential data.
    // Do not interpret WebAuthn's AT/ED bits as proof of a CBOR extension suffix.
    guard auth[32] == 0x40 || auth[32] == 0xc0 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    let counter = (UInt32(auth[33]) << 24) | (UInt32(auth[34]) << 16)
      | (UInt32(auth[35]) << 8) | UInt32(auth[36])
    guard counter != 0 else {
      throw KagemushaAppAttestEvidenceErrorV1.assertionCounterMismatch
    }
    let releaseMeasurement: KagemushaAppAttestReleaseMeasurementV1
    if auth.count == 37 {
      guard auth[32] == 0x40 else {
        throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
      }
      // iOS 26 supplies no signed app-version/category measurement here.
      releaseMeasurement = .unavailable
    } else {
      var extensions = AssertionCBORReaderV1(Data(auth.dropFirst(37)))
      guard try extensions.length(major: 5) == 2 else {
        throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
      }
      var category: UInt32?
      var version: String?
      for _ in 0..<2 {
        switch try extensions.text() {
        case "validationCategory" where category == nil:
          let value = try extensions.byteString()
          guard value.count == 4 else {
            throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
          }
          let bytes = [UInt8](value)
          category = UInt32(bytes[0]) | (UInt32(bytes[1]) << 8)
            | (UInt32(bytes[2]) << 16) | (UInt32(bytes[3]) << 24)
        case "bundleVersion" where version == nil:
          version = try extensions.text()
        default:
          throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
        }
      }
      guard extensions.isAtEnd, let category, let version, !version.isEmpty else {
        throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
      }
      guard category == expectedRelease.validationCategory,
        version == expectedRelease.bundleVersion else {
        throw KagemushaAppAttestEvidenceErrorV1.releaseMismatch
      }
      releaseMeasurement = .signed(validationCategory: category, bundleVersion: version)
    }
    let nonce = Data(SHA256.hash(data: parsed.authenticatorData + clientDataHash))
    guard let publicKey = try? P256.Signing.PublicKey(
      x963Representation: enrolledAssertionPublicKeyX963),
      let signature = try? P256.Signing.ECDSASignature(derRepresentation: parsed.signature),
      publicKey.isValidSignature(signature, for: nonce) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionSignature
    }
    self.rawAssertion = rawAssertion
    authenticatorData = parsed.authenticatorData
    signatureDER = parsed.signature
    signCount = counter
    self.releaseMeasurement = releaseMeasurement
    self.clientDataHash = clientDataHash
    assertionNonce = nonce
    signatureMessageDigest = Data(SHA256.hash(data: nonce))
  }

  /// Reject an assertion that skipped the committed predecessor's counter.
  /// Core must authenticate the predecessor before supplying this value.
  func validateExactNext(previousCounter: UInt32) throws {
    guard previousCounter < UInt32.max, signCount == previousCounter + 1 else {
      throw KagemushaAppAttestEvidenceErrorV1.assertionCounterMismatch
    }
  }

  private static func parseAssertion(_ raw: Data) throws -> (authenticatorData: Data, signature: Data) {
    guard !raw.isEmpty, raw.count <= 8_192 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    var reader = AssertionCBORReaderV1(raw)
    guard try reader.length(major: 5) == 2 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    var authenticatorData: Data?
    var signature: Data?
    for _ in 0..<2 {
      let name = try reader.text()
      let value = try reader.byteString()
      switch name {
      case "authenticatorData" where authenticatorData == nil: authenticatorData = value
      case "signature" where signature == nil: signature = value
      default: throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
      }
    }
    guard reader.isAtEnd, let authenticatorData, let signature,
      (37...1_024).contains(authenticatorData.count),
      (8...72).contains(signature.count), signature.first == 0x30 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    return (authenticatorData, signature)
  }
}

/// Evidence acquisition only. Core must independently verify enrollment, the authenticated
/// predecessor, strict-next counter, and proof fold before admitting money.
public actor KagemushaAppAttestEvidenceProviderV1 {
  private let service: any KagemushaAppAttestServiceV1
  private let intentStore: any KagemushaAppAttestAssertionIntentStoringV1
  private let expectedAppIDHash: Data
  private let expectedRelease: KagemushaAppAttestExpectedReleaseV1
  private let enrolledAssertionPublicKeyX963: Data
  private var assertionInFlight = false
  private var locallyUncertain = false

  public init(service: any KagemushaAppAttestServiceV1,
    intentStore: any KagemushaAppAttestAssertionIntentStoringV1,
    expectedAppIDHash: Data,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1,
    enrolledAssertionPublicKeyX963: Data) throws {
    guard expectedAppIDHash.count == 32,
      expectedAppIDHash.contains(where: { $0 != 0 }) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidDigestLength
    }
    self.service = service
    self.intentStore = intentStore
    self.expectedAppIDHash = expectedAppIDHash
    self.expectedRelease = expectedRelease
    guard (try? P256.Signing.PublicKey(
      x963Representation: enrolledAssertionPublicKeyX963)) != nil else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidReleasePolicy
    }
    self.enrolledAssertionPublicKeyX963 = enrolledAssertionPublicKeyX963
  }

  public func generateDedicatedKey() async throws -> String {
    guard service.isSupported else { throw KagemushaAppAttestEvidenceErrorV1.unsupportedDevice }
    let keyID = try await service.generateKey()
    guard !keyID.isEmpty else { throw KagemushaAppAttestEvidenceErrorV1.emptyKeyID }
    return keyID
  }

  /// Persist the key ID before this call; independently verify the returned attestation.
  public func attestDedicatedKey(keyID: String,
    binding: KagemushaAppAttestEnrollmentBindingV1) async throws -> Data {
    guard service.isSupported else { throw KagemushaAppAttestEvidenceErrorV1.unsupportedDevice }
    guard !keyID.isEmpty else { throw KagemushaAppAttestEvidenceErrorV1.emptyKeyID }
    let raw = try await service.attestKey(keyID, clientDataHash: binding.clientDataHash)
    guard !raw.isEmpty else { throw KagemushaAppAttestEvidenceErrorV1.emptyRawObject }
    return raw
  }

  /// Reserve and sync the intent before the hardware call. Every ambiguous result remains frozen.
  public func assertTransition(keyID: String, binding: KagemushaAppAttestTransitionBindingV1,
    expectedPreviousCounter: UInt32) async throws -> KagemushaAppAttestAssertionEvidenceV1 {
    guard service.isSupported else { throw KagemushaAppAttestEvidenceErrorV1.unsupportedDevice }
    guard !keyID.isEmpty else { throw KagemushaAppAttestEvidenceErrorV1.emptyKeyID }
    guard !locallyUncertain else { throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown }
    guard !assertionInFlight else { throw KagemushaAppAttestEvidenceErrorV1.assertionAlreadyInFlight }
    guard expectedPreviousCounter < UInt32.max else {
      throw KagemushaAppAttestEvidenceErrorV1.assertionCounterMismatch
    }
    assertionInFlight = true
    defer { assertionInFlight = false }
    let digest = binding.clientDataHash
    let initial = try intentStore.load(keyID: keyID)
    switch initial {
    case .ready(let counter) where counter == expectedPreviousCounter:
      break
    default:
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    try intentStore.reserve(keyID: keyID, previousCounter: expectedPreviousCounter,
      selectionDigest: digest)
    guard try intentStore.load(keyID: keyID) == .pending(
      previousCounter: expectedPreviousCounter, selectionDigest: digest) else {
      locallyUncertain = true
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    let raw: Data
    do {
      raw = try await service.generateAssertion(keyID, clientDataHash: digest)
    } catch {
      locallyUncertain = true
      throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown
    }
    let evidence: KagemushaAppAttestAssertionEvidenceV1
    do {
      evidence = try KagemushaAppAttestAssertionEvidenceV1(
        rawAssertion: raw, clientDataHash: digest, expectedAppIDHash: expectedAppIDHash,
        expectedRelease: expectedRelease,
        enrolledAssertionPublicKeyX963: enrolledAssertionPublicKeyX963)
      try evidence.validateExactNext(previousCounter: expectedPreviousCounter)
      try intentStore.complete(keyID: keyID, counter: evidence.signCount,
        selectionDigest: digest, rawAssertion: raw)
      guard try intentStore.load(keyID: keyID) == .complete(
        counter: evidence.signCount, selectionDigest: digest, rawAssertion: raw) else {
        throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
      }
    } catch {
      locallyUncertain = true
      throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown
    }
    return evidence
  }
}

private struct AssertionCBORReaderV1 {
  private let bytes: [UInt8]
  private var cursor = 0

  init(_ data: Data) { bytes = [UInt8](data) }
  var isAtEnd: Bool { cursor == bytes.count }

  mutating func length(major: UInt8) throws -> Int {
    guard cursor < bytes.count else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
    let header = bytes[cursor]
    cursor += 1
    guard header >> 5 == major else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
    let argument = header & 0x1f
    if argument < 24 { return Int(argument) }
    let width: Int
    switch argument {
    case 24: width = 1
    case 25: width = 2
    case 26: width = 4
    default: throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    guard bytes.count - cursor >= width else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    var value: UInt32 = 0
    for _ in 0..<width {
      value = (value << 8) | UInt32(bytes[cursor])
      cursor += 1
    }
    guard (width == 1 && value >= 24)
      || (width == 2 && value > UInt8.max)
      || (width == 4 && value > UInt16.max),
      value <= 8_192 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    return Int(value)
  }

  mutating func text() throws -> String {
    let count = try length(major: 3)
    let value = try take(count)
    guard let result = String(data: value, encoding: .utf8) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    return result
  }

  mutating func byteString() throws -> Data {
    let count = try length(major: 2)
    return try take(count)
  }

  private mutating func take(_ count: Int) throws -> Data {
    guard count >= 0, bytes.count - cursor >= count else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    let result = Data(bytes[cursor..<(cursor + count)])
    cursor += count
    return result
  }
}

#if os(iOS) && canImport(DeviceCheck)
import DeviceCheck

/// Thin iOS system adapter; enrollment and assertion verification remain independent.
@available(iOS 15.0, *)
public final class KagemushaAppleAppAttestServiceV1: KagemushaAppAttestServiceV1, @unchecked Sendable {
  private let service = DCAppAttestService.shared

  public init() {}
  public var isSupported: Bool { service.isSupported }
  public func generateKey() async throws -> String { try await service.generateKey() }
  public func attestKey(_ keyID: String, clientDataHash: Data) async throws -> Data {
    try await service.attestKey(keyID, clientDataHash: clientDataHash)
  }
  public func generateAssertion(_ keyID: String, clientDataHash: Data) async throws -> Data {
    try await service.generateAssertion(keyID, clientDataHash: clientDataHash)
  }
}
#endif

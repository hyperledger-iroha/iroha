import CryptoKit
import Foundation
import Security

/// The App Attest environment fixed by the authenticated app release.
public enum KagemushaAppAttestEnvironmentV1: Sendable {
  case production
  case development
}

/// A checked Apple enrollment object. Its receipt still requires independent fraud assessment.
public struct KagemushaAppAttestEnrollmentEvidenceV1: Sendable {
  public let rawAttestation: Data
  public let authenticatorData: Data
  public let keyID: String
  public let publicKeyX963: Data
  public let receipt: Data
  public let validationCategory: UInt32
  public let bundleVersion: String
}

/// Verifies one dedicated App Attest key against Apple's pinned root and an authenticated release.
///
/// This verifies enrollment evidence only. It does not authorize an offline monetary transition.
public struct KagemushaAppAttestEnrollmentVerifierV1: Sendable {
  private static let appleRootSHA256 = Data([
    0x1c, 0xb9, 0x82, 0x3b, 0xa2, 0x8b, 0xa6, 0xad,
    0x2d, 0x33, 0xa0, 0x06, 0x94, 0x1d, 0xe2, 0xae,
    0x4f, 0x51, 0x3e, 0xf1, 0xd4, 0xe8, 0x31, 0xb9,
    0xf7, 0xe0, 0xfa, 0x7b, 0x62, 0x42, 0xc9, 0x32,
  ])
  private let rootCertificateDER: Data
  private let expectedAppIDHash: Data
  private let expectedRelease: KagemushaAppAttestExpectedReleaseV1
  private let environment: KagemushaAppAttestEnvironmentV1
  private let fixedTestVerificationDate: Date?

  /// The root DER and app/release identity must come from authenticated release configuration.
  public init(rootCertificateDER: Data, expectedAppIDHash: Data,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1,
    environment: KagemushaAppAttestEnvironmentV1) throws {
    try self.init(rootCertificateDER: rootCertificateDER,
      expectedAppIDHash: expectedAppIDHash, expectedRelease: expectedRelease,
      environment: environment, fixedTestVerificationDate: nil)
  }

  // A fixed clock permits Apple's published, short-lived sample certificate to be exercised.
  internal init(rootCertificateDER: Data, expectedAppIDHash: Data,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1,
    environment: KagemushaAppAttestEnvironmentV1, verificationDate: Date) throws {
    try self.init(rootCertificateDER: rootCertificateDER,
      expectedAppIDHash: expectedAppIDHash, expectedRelease: expectedRelease,
      environment: environment, fixedTestVerificationDate: verificationDate)
  }

  private init(rootCertificateDER: Data, expectedAppIDHash: Data,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1,
    environment: KagemushaAppAttestEnvironmentV1, fixedTestVerificationDate: Date?) throws {
    guard Data(SHA256.hash(data: rootCertificateDER)) == Self.appleRootSHA256,
      SecCertificateCreateWithData(nil, rootCertificateDER as CFData) != nil,
      expectedAppIDHash.count == 32,
      expectedAppIDHash.contains(where: { $0 != 0 }) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidReleasePolicy
    }
    self.rootCertificateDER = rootCertificateDER
    self.expectedAppIDHash = expectedAppIDHash
    self.expectedRelease = expectedRelease
    self.environment = environment
    self.fixedTestVerificationDate = fixedTestVerificationDate
  }

  /// Verify the exact enrollment challenge selected for this key and monetary release.
  public func verify(rawAttestation: Data, keyID: String,
    binding: KagemushaAppAttestEnrollmentBindingV1) throws
    -> KagemushaAppAttestEnrollmentEvidenceV1 {
    try verify(rawAttestation: rawAttestation, keyID: keyID,
      clientDataHash: binding.clientDataHash)
  }

  // The guide fixture uses a published challenge rather than a KAGEMUSHA enrollment binding.
  internal func verify(rawAttestation: Data, keyID: String, clientDataHash: Data) throws
    -> KagemushaAppAttestEnrollmentEvidenceV1 {
    guard (1...128).contains(clientDataHash.count), !keyID.isEmpty,
      let credentialID = Data(base64Encoded: keyID), credentialID.count == 32,
      credentialID.base64EncodedString() == keyID else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    let object = try Self.parseAttestation(rawAttestation)
    try verifyCertificateChain(leafDER: object.leafDER,
      intermediateDER: object.intermediateDER)
    let authenticatorData = object.authenticatorData
    let nonce = Data(SHA256.hash(data: authenticatorData + clientDataHash))
    guard try Self.certificateNonceExtension(object.leafDER) == nonce else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    let leaf = SecCertificateCreateWithData(nil, object.leafDER as CFData)!
    guard let publicKey = SecCertificateCopyKey(leaf) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    var keyError: Unmanaged<CFError>?
    guard let externalKey = SecKeyCopyExternalRepresentation(publicKey, &keyError) as Data?,
      externalKey.count == 65, externalKey.first == 0x04,
      Data(SHA256.hash(data: externalKey)) == credentialID,
      (try? P256.Signing.PublicKey(x963Representation: externalKey)) != nil else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    let parsed = try parseAuthenticatorData(authenticatorData,
      credentialID: credentialID, publicKeyX963: externalKey)
    return KagemushaAppAttestEnrollmentEvidenceV1(
      rawAttestation: rawAttestation, authenticatorData: authenticatorData,
      keyID: keyID, publicKeyX963: externalKey, receipt: object.receipt,
      validationCategory: parsed.category, bundleVersion: parsed.bundleVersion)
  }

  private func verifyCertificateChain(leafDER: Data, intermediateDER: Data) throws {
    guard let leaf = SecCertificateCreateWithData(nil, leafDER as CFData),
      let intermediate = SecCertificateCreateWithData(nil, intermediateDER as CFData),
      let root = SecCertificateCreateWithData(nil, rootCertificateDER as CFData) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    var trust: SecTrust?
    guard SecTrustCreateWithCertificates([leaf, intermediate] as CFArray,
      SecPolicyCreateBasicX509(), &trust) == errSecSuccess, let trust,
      SecTrustSetAnchorCertificates(trust, [root] as CFArray) == errSecSuccess,
      SecTrustSetAnchorCertificatesOnly(trust, true) == errSecSuccess,
      SecTrustSetNetworkFetchAllowed(trust, false) == errSecSuccess,
      SecTrustSetVerifyDate(trust, (fixedTestVerificationDate ?? Date()) as CFDate) == errSecSuccess else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    var trustError: CFError?
    guard SecTrustEvaluateWithError(trust, &trustError) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
  }

  private func parseAuthenticatorData(_ raw: Data, credentialID: Data,
    publicKeyX963: Data) throws -> (category: UInt32, bundleVersion: String) {
    let bytes = [UInt8](raw)
    guard (37 + 16 + 2 + 32 + 1...1_024).contains(bytes.count),
      Data(bytes[0..<32]) == expectedAppIDHash,
      bytes[32] & 0x40 != 0,
      bytes[33..<37].allSatisfy({ $0 == 0 }) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    let expectedAAGUID: Data
    switch environment {
    case .production: expectedAAGUID = Data("appattest".utf8) + Data(repeating: 0, count: 7)
    case .development: expectedAAGUID = Data("appattestdevelop".utf8)
    }
    guard Data(bytes[37..<53]) == expectedAAGUID,
      bytes[53] == 0, bytes[54] == 32,
      Data(bytes[55..<87]) == credentialID else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    var reader = EnrollmentCBORReaderV1(Data(bytes.dropFirst(87)))
    guard try reader.length(major: 5, maximum: 5) == 5 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    var kty: Int?
    var alg: Int?
    var curve: Int?
    var x: Data?
    var y: Data?
    for _ in 0..<5 {
      switch try reader.integer() {
      case 1 where kty == nil: kty = try reader.integer()
      case 3 where alg == nil: alg = try reader.integer()
      case -1 where curve == nil: curve = try reader.integer()
      case -2 where x == nil: x = try reader.byteString(maximum: 32)
      case -3 where y == nil: y = try reader.byteString(maximum: 32)
      default: throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
      }
    }
    guard kty == 2, alg == -7, curve == 1,
      let x, x.count == 32, let y, y.count == 32,
      Data([0x04]) + x + y == publicKeyX963,
      try reader.length(major: 5, maximum: 2) == 2 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    var category: UInt32?
    var bundleVersion: String?
    for _ in 0..<2 {
      switch try reader.text(maximum: 32) {
      case "apple_validation_category_01" where category == nil:
        let value = try reader.byteString(maximum: 4)
        guard value.count == 4 else {
          throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
        }
        let bytes = [UInt8](value)
        category = UInt32(bytes[0]) | (UInt32(bytes[1]) << 8)
          | (UInt32(bytes[2]) << 16) | (UInt32(bytes[3]) << 24)
      case "apple_bundle_version_01" where bundleVersion == nil:
        bundleVersion = try reader.text(maximum: 128)
      default: throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
      }
    }
    guard reader.isAtEnd, let category, let bundleVersion,
      category == expectedRelease.validationCategory,
      bundleVersion == expectedRelease.bundleVersion else {
      throw KagemushaAppAttestEvidenceErrorV1.releaseMismatch
    }
    return (category, bundleVersion)
  }

  private struct ParsedAttestation {
    let leafDER: Data
    let intermediateDER: Data
    let receipt: Data
    let authenticatorData: Data
  }

  private static func parseAttestation(_ raw: Data) throws -> ParsedAttestation {
    guard !raw.isEmpty, raw.count <= 16_384 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    var reader = EnrollmentCBORReaderV1(raw)
    guard try reader.length(major: 5, maximum: 3) == 3 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    var format: String?
    var leafDER: Data?
    var intermediateDER: Data?
    var receipt: Data?
    var authenticatorData: Data?
    for _ in 0..<3 {
      switch try reader.text(maximum: 16) {
      case "fmt" where format == nil:
        format = try reader.text(maximum: 32)
      case "attStmt" where leafDER == nil:
        guard try reader.length(major: 5, maximum: 2) == 2 else {
          throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
        }
        for _ in 0..<2 {
          switch try reader.text(maximum: 16) {
          case "x5c" where leafDER == nil:
            guard try reader.length(major: 4, maximum: 2) == 2 else {
              throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
            }
            leafDER = try reader.byteString(maximum: 4_096)
            intermediateDER = try reader.byteString(maximum: 4_096)
          case "receipt" where receipt == nil:
            receipt = try reader.byteString(maximum: 8_192)
          default: throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
          }
        }
      case "authData" where authenticatorData == nil:
        authenticatorData = try reader.byteString(maximum: 1_024)
      default: throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
      }
    }
    guard reader.isAtEnd, format == "apple-appattest",
      let leafDER, !leafDER.isEmpty,
      let intermediateDER, !intermediateDER.isEmpty,
      let receipt, !receipt.isEmpty,
      let authenticatorData else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    return ParsedAttestation(leafDER: leafDER, intermediateDER: intermediateDER,
      receipt: receipt, authenticatorData: authenticatorData)
  }

  private static func certificateNonceExtension(_ certificate: Data) throws -> Data {
    var outer = EnrollmentDERReaderV1(certificate)
    var certificateBody = EnrollmentDERReaderV1(try outer.read(tag: 0x30))
    guard outer.isAtEnd else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
    var tbs = EnrollmentDERReaderV1(try certificateBody.read(tag: 0x30))
    _ = try certificateBody.read(tag: 0x30)
    _ = try certificateBody.read(tag: 0x03)
    guard certificateBody.isAtEnd else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    if tbs.peekTag == 0xa0 { _ = try tbs.read(tag: 0xa0) }
    for tag: UInt8 in [0x02, 0x30, 0x30, 0x30, 0x30, 0x30] {
      _ = try tbs.read(tag: tag)
    }
    var nonce: Data?
    while !tbs.isAtEnd {
      let (tag, value) = try tbs.readAny()
      if tag == 0xa3 {
        guard nonce == nil else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
        var wrapper = EnrollmentDERReaderV1(value)
        var extensions = EnrollmentDERReaderV1(try wrapper.read(tag: 0x30))
        guard wrapper.isAtEnd else {
          throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
        }
        while !extensions.isAtEnd {
          var item = EnrollmentDERReaderV1(try extensions.read(tag: 0x30))
          let oid = try item.read(tag: 0x06)
          if item.peekTag == 0x01 { _ = try item.read(tag: 0x01) }
          let extensionValue = try item.read(tag: 0x04)
          guard item.isAtEnd else {
            throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
          }
          if oid == Data([0x2a, 0x86, 0x48, 0x86, 0xf7, 0x63, 0x64, 0x08, 0x02]) {
            guard nonce == nil else {
              throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
            }
            var valueReader = EnrollmentDERReaderV1(extensionValue)
            var sequence = EnrollmentDERReaderV1(try valueReader.read(tag: 0x30))
            var context = EnrollmentDERReaderV1(try sequence.read(tag: 0xa1))
            let extracted = try context.read(tag: 0x04)
            guard valueReader.isAtEnd, sequence.isAtEnd, context.isAtEnd,
              extracted.count == 32 else {
              throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
            }
            nonce = extracted
          }
        }
      } else if tag != 0x81 && tag != 0x82 {
        throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
      }
    }
    guard let nonce else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
    return nonce
  }
}

private struct EnrollmentCBORReaderV1 {
  private let bytes: [UInt8]
  private var offset = 0

  init(_ data: Data) { bytes = [UInt8](data) }
  var isAtEnd: Bool { offset == bytes.count }

  mutating func length(major: UInt8, maximum: Int) throws -> Int {
    guard offset < bytes.count else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
    let header = bytes[offset]; offset += 1
    guard header >> 5 == major else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    let additional = header & 0x1f
    let value: Int
    switch additional {
    case 0...23: value = Int(additional)
    case 24:
      let next = try take(1)[0]
      guard next >= 24 else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
      value = Int(next)
    case 25:
      let next = try take(2)
      value = Int(next[0]) << 8 | Int(next[1])
      guard value > 255 else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
    case 26:
      let next = try take(4)
      value = Int(next[0]) << 24 | Int(next[1]) << 16 | Int(next[2]) << 8 | Int(next[3])
      guard value > 65_535 else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
    default: throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    guard value <= maximum else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
    return value
  }

  mutating func integer() throws -> Int {
    guard offset < bytes.count else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
    let major = bytes[offset] >> 5
    guard major == 0 || major == 1 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    let value = try length(major: major, maximum: 32)
    return major == 0 ? value : -1 - value
  }

  mutating func text(maximum: Int) throws -> String {
    let size = try length(major: 3, maximum: maximum)
    guard let value = String(data: try take(size), encoding: .utf8) else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    return value
  }

  mutating func byteString(maximum: Int) throws -> Data {
    try take(length(major: 2, maximum: maximum))
  }

  private mutating func take(_ size: Int) throws -> Data {
    guard size >= 0, bytes.count - offset >= size else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    let value = Data(bytes[offset..<(offset + size)])
    offset += size
    return value
  }
}

private struct EnrollmentDERReaderV1 {
  private let bytes: [UInt8]
  private var offset = 0

  init(_ data: Data) { bytes = [UInt8](data) }
  var isAtEnd: Bool { offset == bytes.count }
  var peekTag: UInt8? { isAtEnd ? nil : bytes[offset] }

  mutating func read(tag: UInt8) throws -> Data {
    let (actual, value) = try readAny()
    guard actual == tag else { throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject }
    return value
  }

  mutating func readAny() throws -> (UInt8, Data) {
    guard bytes.count - offset >= 2 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    let tag = bytes[offset]; offset += 1
    let lengthHeader = bytes[offset]; offset += 1
    let length: Int
    if lengthHeader & 0x80 == 0 {
      length = Int(lengthHeader)
    } else {
      let width = Int(lengthHeader & 0x7f)
      guard (1...3).contains(width), bytes.count - offset >= width,
        bytes[offset] != 0 else {
        throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
      }
      var value = 0
      for _ in 0..<width { value = (value << 8) | Int(bytes[offset]); offset += 1 }
      guard value >= 128,
        (width == 1 || value > 255),
        (width < 3 || value > 65_535) else {
        throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
      }
      length = value
    }
    guard bytes.count - offset >= length else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidAssertionObject
    }
    let value = Data(bytes[offset..<(offset + length)])
    offset += length
    return (tag, value)
  }
}

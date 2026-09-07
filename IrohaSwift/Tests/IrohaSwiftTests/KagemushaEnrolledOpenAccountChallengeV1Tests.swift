import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Structural projection tests only. These values do not create native authority or possession.
final class KagemushaEnrolledOpenAccountChallengeV1Tests: XCTestCase {
  private let schema = "iroha.kagemusha.v1.enrolled-open-account-challenge"
  private func digest(_ byte: UInt8) -> Data { Data(repeating: byte, count: 32) }

  private func owner() throws -> KagemushaRetailEnrollmentOwnerProjectionV1 {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while directory.path != "/" {
      let path = directory.appendingPathComponent("fixtures/offline/kagemusha_enrolled_open_selector_v1.json")
      if FileManager.default.fileExists(atPath: path.path) {
        let fixture = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: path)) as? [String: Any])
        let bytes = try XCTUnwrap(Data(hexString: XCTUnwrap(fixture["selector_canonical_hex"] as? String)))
        return try KagemushaEnrolledOpenSelectorV1.decodeCanonicalExact(bytes).owner
      }
      directory.deleteLastPathComponent()
    }
    throw NSError(domain: "Missing Rust owner projection", code: 1)
  }

  private func wide(_ byte: UInt8) throws -> KagemushaUInt128V1 {
    try .init(littleEndianBytes: Data(repeating: byte, count: 16))
  }

  private func checkpoint(change: Int = -1, lane: KagemushaDeviceLaneIDV1? = nil) throws
    -> KagemushaDurabilityAnchorStatementProjectionV1
  {
    let owner = try owner()
    func byte(_ index: Int) -> UInt8 { change == index ? 99 : UInt8(index + 1) }
    return try .init(metadataRevision: wide(byte(0)),
      lane: lane ?? .init(networkID: owner.runtime.networkID, deviceLaneID: owner.laneID,
        asset: owner.runtime.asset, scale: owner.runtime.scale),
      stateCommitment: digest(byte(1)),
      hardwareEpoch: .init(generation: wide(byte(2)), epochID: digest(byte(3))),
      devicePolicyBinding: .init(deviceKeyReference: digest(byte(4)), hardwarePolicyID: digest(byte(5))),
      stateNonceCommitment: digest(byte(6)), logicalSequence: wide(byte(7)),
      journalRevision: wide(byte(8)), inboxRevision: wide(byte(9)), snapshotCommitment: digest(byte(10)))
  }

  private func challenge(source: KagemushaEnrolledOpenAuthoritySourceProjectionV1? = nil) throws
    -> KagemushaEnrolledOpenAccountChallengeV1
  {
    let owner = try owner()
    return try .init(enrollmentID: owner.enrollmentID(), owner: owner, nonce: digest(21),
      authoritySource: source ?? .initialCertificate(certificateDigest: digest(22)),
      releaseID: digest(23), hardwarePolicyDigest: digest(24), coreAuthorizationKeyReference: digest(25))
  }

  private func recovered() throws -> KagemushaEnrolledOpenAccountChallengeV1 {
    try challenge(source: .recoveryCheckpoint(statement: checkpoint(), terminalCertificateDigest: digest(26)))
  }

  func testInitialAndFullRecoveryProjectionRoundTripWithoutLosingUInt128Bits() throws {
    for value in try [challenge(), recovered()] {
      let canonical = try value.canonicalBytes()
      XCTAssertEqual(try KagemushaEnrolledOpenAccountChallengeV1.decodeCanonicalExact(canonical), value)
      XCTAssertEqual(try KagemushaEnrolledOpenAccountChallengeV1.decodeCanonicalExact(
        (Data([0, 0]) + canonical).dropFirst(2)), value)
    }
    let checkpoint = try checkpoint()
    XCTAssertEqual(checkpoint.metadataRevision.littleEndianBytes, Data(repeating: 1, count: 16))
    XCTAssertEqual(checkpoint.hardwareEpoch.generation.littleEndianBytes, Data(repeating: 3, count: 16))
    XCTAssertEqual(checkpoint.inboxRevision.littleEndianBytes, Data(repeating: 10, count: 16))
  }

  func testSigningHashUsesBarePayloadOnceAndIrohaMarker() throws {
    for value in try [challenge(), recovered()] {
      let canonical = try value.canonicalBytes()
      let payload = try XCTUnwrap(noritoDecodeFrame(canonical)).payload
      var expected = Blake2b.hash256(payload)
      expected[31] |= 1
      let actual = value.accountSigningMessage()
      XCTAssertEqual(actual, expected)
      XCTAssertEqual(actual[31] & 1, 1)
      XCTAssertNotEqual(actual, IrohaHash.hash(canonical), "Canonical frame is not the HashOf preimage")
      XCTAssertNotEqual(actual, IrohaHash.hash(expected), "Do not prehash twice")
      XCTAssertNotEqual(actual, Data(SHA256.hash(data: payload)), "Enrollment ID's SHA256 is a different domain")
    }
  }

  func testEveryIndependentRequestPinMustMatch() throws {
    let value = try recovered(), selector = try KagemushaEnrolledOpenSelectorV1(owner: owner())
    try value.validateCorrelation(selector: selector, nonce: value.nonce, releaseID: value.releaseID,
      hardwarePolicyDigest: value.hardwarePolicyDigest, coreAuthorizationKeyReference: value.coreAuthorizationKeyReference)
    for index in 0..<5 {
      let otherOwner = try KagemushaRetailEnrollmentOwnerProjectionV1(
        accountID: value.owner.accountID, runtime: value.owner.runtime, laneID: digest(98))
      XCTAssertThrowsError(try value.validateCorrelation(
        selector: index == 0 ? .init(owner: otherOwner) : selector,
        nonce: index == 1 ? digest(98) : value.nonce,
        releaseID: index == 2 ? digest(98) : value.releaseID,
        hardwarePolicyDigest: index == 3 ? digest(98) : value.hardwarePolicyDigest,
        coreAuthorizationKeyReference: index == 4 ? digest(98) : value.coreAuthorizationKeyReference))
    }
  }

  func testEveryCheckpointIdentityFieldAndSourceKindIsCommitted() throws {
    let original = try recovered()
    for index in 0..<11 {
      let changed = try challenge(source: .recoveryCheckpoint(
        statement: checkpoint(change: index), terminalCertificateDigest: digest(26)))
      XCTAssertNotEqual(changed.accountSigningMessage(), original.accountSigningMessage(), "checkpoint component \(index)")
      XCTAssertEqual(try KagemushaEnrolledOpenAccountChallengeV1.decodeCanonicalExact(changed.canonicalBytes()), changed)
    }
    let changedTerminal = try challenge(source: .recoveryCheckpoint(
      statement: checkpoint(), terminalCertificateDigest: digest(99)))
    XCTAssertNotEqual(changedTerminal.accountSigningMessage(), original.accountSigningMessage())
    XCTAssertNotEqual(try challenge().accountSigningMessage(), original.accountSigningMessage())
  }

  func testRecoveryCheckpointLaneMustMatchAllOwnerLaneFields() throws {
    let owner = try owner()
    var changedAsset = owner.runtime.asset.canonicalPayload
    changedAsset[1] ^= 1
    for index in 0..<4 {
      let lane = try KagemushaDeviceLaneIDV1(
        networkID: index == 0 ? digest(99) : owner.runtime.networkID,
        deviceLaneID: index == 1 ? digest(99) : owner.laneID,
        asset: index == 2 ? .init(canonicalPayload: changedAsset) : owner.runtime.asset,
        scale: index == 3 ? owner.runtime.scale + 1 : owner.runtime.scale)
      XCTAssertThrowsError(try challenge(source: .recoveryCheckpoint(
        statement: checkpoint(lane: lane), terminalCertificateDigest: digest(26))))
    }
  }

  func testExactVersionDomainLifetimeAndDigestShapes() throws {
    let valid = try challenge()
    for index in 0..<8 {
      XCTAssertThrowsError(try KagemushaEnrolledOpenAccountChallengeV1(
        version: index == 0 ? 2 : 1,
        domain: index == 1 ? "other-domain" : valid.domain,
        enrollmentID: index == 2 ? digest(99) : valid.enrollmentID,
        owner: valid.owner, nonce: index == 3 ? digest(0) : valid.nonce,
        authoritySource: valid.authoritySource, releaseID: index == 4 ? Data([1]) : valid.releaseID,
        hardwarePolicyDigest: index == 5 ? digest(0) : valid.hardwarePolicyDigest,
        coreAuthorizationKeyReference: index == 6 ? digest(0) : valid.coreAuthorizationKeyReference,
        lifetimeMilliseconds: index == 7 ? 120_001 : 120_000))
    }
    XCTAssertThrowsError(try challenge(source: .initialCertificate(certificateDigest: digest(0))))
    XCTAssertThrowsError(try challenge(source: .recoveryCheckpoint(
      statement: checkpoint(), terminalCertificateDigest: Data())))
  }

  func testRejectsAllTruncationsOversizeAlternateSchemasAndMalformedSource() throws {
    for value in try [challenge(), recovered()] {
      let canonical = try value.canonicalBytes()
      for count in 0..<canonical.count {
        XCTAssertThrowsError(try KagemushaEnrolledOpenAccountChallengeV1.decodeCanonicalExact(canonical.prefix(count)))
      }
      let frame = try XCTUnwrap(noritoDecodeFrame(canonical))
      var r = CanonicalNoritoReader(data: frame.payload), fields: [Data] = []
      while r.remaining() > 0 { fields.append(try r.readCompactField()) }
      XCTAssertEqual(fields.count, 10)
      var source = fields[5]
      source[0] = 2
      fields[5] = source
      var writer = CompactNoritoWriter()
      fields.forEach { writer.writeField($0) }
      var nonminimal = frame.payload
      nonminimal.replaceSubrange(0..<1, with: [0x82, 0])
      let malformed = [writer.data, frame.payload + Data([1, 0]), nonminimal]
      for payload in malformed {
        let invalid = noritoEncode(typeName: schema, payload: payload,
          flags: NoritoHeader.compactLen, payloadAlignment: 16)
        XCTAssertNotNil(noritoDecodeFrame(invalid))
        XCTAssertThrowsError(try KagemushaEnrolledOpenAccountChallengeV1.decodeCanonicalExact(invalid))
      }
      for invalid in [canonical + Data([0]), Data("/state/wallet".utf8), Data("{}".utf8),
        noritoEncode(typeName: "iroha.kagemusha.v1.enrolled-open-selector", payload: frame.payload,
          flags: NoritoHeader.compactLen, payloadAlignment: 16),
        Data(repeating: 0, count: 16 * 1024 + 1)] {
        XCTAssertThrowsError(try KagemushaEnrolledOpenAccountChallengeV1.decodeCanonicalExact(invalid))
      }
    }
  }

  func testAuthorityDigestBytesAreOwnedAfterMutableCallerChanges() throws {
    let mutable = NSMutableData(data: digest(22))
    let value = try challenge(source: .initialCertificate(certificateDigest: Data(referencing: mutable)))
    let before = try value.canonicalBytes(), message = value.accountSigningMessage()
    mutable.resetBytes(in: NSRange(location: 0, length: 32))
    XCTAssertEqual(try value.canonicalBytes(), before)
    XCTAssertEqual(value.accountSigningMessage(), message)
  }

  func testChecksumValidRecoveryRequiresCompleteExactCheckpoint() throws {
    let canonical = try recovered().canonicalBytes()
    let frame = try XCTUnwrap(noritoDecodeFrame(canonical))
    let original = try fields(frame.payload)
    var enumReader = CanonicalNoritoReader(data: original[5])
    XCTAssertEqual(try enumReader.readUInt32LE(), 1)
    let source = try fields(enumReader.readBytes(enumReader.remaining()))
    let statement = try fields(source[0])
    XCTAssertEqual(statement.count, 11)
    for index in 0..<6 {
      var changedStatement = statement
      switch index {
      case 0: changedStatement[1] = Data([2, 0])
      case 1: changedStatement.append(Data([1]))
      case 2: changedStatement[0] = Data(repeating: 1, count: 15)
      case 3: changedStatement[3] = digest(1) // DigestV1 requires compact byte fields.
      case 4: changedStatement[10] = writeFields(Array(repeating: Data([0]), count: 32))
      default: break
      }
      let checkpoint = index == 5 ? digest(1) : writeFields(changedStatement)
      var changedEnum = CompactNoritoWriter()
      changedEnum.writeUInt32LE(1)
      changedEnum.writeField(checkpoint)
      changedEnum.writeField(source[1])
      var changed = original
      changed[5] = changedEnum.data
      let invalid = noritoEncode(typeName: schema, payload: writeFields(changed),
        flags: NoritoHeader.compactLen, payloadAlignment: 16)
      XCTAssertNotNil(noritoDecodeFrame(invalid), "Valid checksum must reach checkpoint decoder")
      XCTAssertThrowsError(try KagemushaEnrolledOpenAccountChallengeV1.decodeCanonicalExact(invalid),
        "checkpoint mutation \(index)")
    }
  }

  func testChecksumValidArchivesRejectEveryInvalidChallengeField() throws {
    let canonical = try challenge().canonicalBytes()
    let frame = try XCTUnwrap(noritoDecodeFrame(canonical))
    let original = try fields(frame.payload)
    for index in 0..<10 {
      var changed = original
      switch index {
      case 0: changed[index] = Data([2, 0])
      case 1: changed[index] = CompactNorito.encodeString("invalid-domain")
      case 2: changed[index] = digest(99)
      case 3: changed[index][changed[index].count - 1] ^= 1
      case 5: changed[index][0] = 2
      case 9: changed[index] = CompactNorito.encodeUInt64(119_999)
      default: changed[index] = digest(0)
      }
      let invalid = noritoEncode(typeName: schema, payload: writeFields(changed),
        flags: NoritoHeader.compactLen, payloadAlignment: 16)
      XCTAssertNotNil(noritoDecodeFrame(invalid))
      XCTAssertThrowsError(try KagemushaEnrolledOpenAccountChallengeV1.decodeCanonicalExact(invalid),
        "challenge field \(index)")
    }
  }

  private func fields(_ payload: Data) throws -> [Data] {
    var reader = CanonicalNoritoReader(data: payload), result: [Data] = []
    while reader.remaining() > 0 { result.append(try reader.readCompactField()) }
    return result
  }

  private func writeFields(_ values: [Data]) -> Data {
    var writer = CompactNoritoWriter()
    values.forEach { writer.writeField($0) }
    return writer.data
  }

  func testRustInitialFixtureCanonicalHashAndEd25519Signature() throws {
    try verifyRustFixture("initial")
  }

  func testRustRecoveryFixtureCanonicalHashSignatureAndFullWidthCheckpoint() throws {
    try verifyRustFixture("recovery")
  }

  private func verifyRustFixture(_ name: String) throws {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    var fixture: [String: Any]?
    while directory.path != "/" {
      let path = directory.appendingPathComponent("fixtures/offline/kagemusha_enrolled_open_challenge_v1.json")
      if FileManager.default.fileExists(atPath: path.path) {
        fixture = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: path)) as? [String: Any])
        break
      }
      directory.deleteLastPathComponent()
    }
    let values = try XCTUnwrap(fixture)
    func bytes(_ field: String) throws -> Data {
      try XCTUnwrap(Data(hexString: XCTUnwrap(values[field] as? String)))
    }
    func wide(bit: Int, low: UInt8) throws -> KagemushaUInt128V1 {
      var bytes = Data(repeating: 0, count: 16)
      bytes[0] = low
      bytes[bit / 8] = 1 << (bit % 8)
      return try .init(littleEndianBytes: bytes)
    }
    let canonical = try bytes(name + "_challenge_canonical_hex")
    let actual = try KagemushaEnrolledOpenAccountChallengeV1.decodeCanonicalExact(canonical)
    XCTAssertEqual(canonical.count, name == "initial" ? 569 : 1191)
    XCTAssertEqual(try actual.canonicalBytes(), canonical)
    XCTAssertEqual(actual.accountSigningMessage(), try bytes(name + "_account_signing_message_hex"))
    let bare = try XCTUnwrap(noritoDecodeFrame(canonical)).payload
    XCTAssertEqual(bare, try bytes(name + "_challenge_payload_hex"))
    XCTAssertEqual(try fields(bare)[5], try bytes(name + "_authority_source_payload_hex"))

    let owner = try KagemushaEnrolledOpenSelectorV1.decodeCanonicalExact(bytes("selector_canonical_hex")).owner
    let source: KagemushaEnrolledOpenAuthoritySourceProjectionV1
    if name == "initial" {
      source = .initialCertificate(certificateDigest: digest(77))
    } else {
      let statement = try KagemushaDurabilityAnchorStatementProjectionV1(
        metadataRevision: wide(bit: 80, low: 7),
        lane: .init(networkID: owner.runtime.networkID, deviceLaneID: owner.laneID,
          asset: owner.runtime.asset, scale: owner.runtime.scale),
        stateCommitment: digest(81),
        hardwareEpoch: .init(generation: wide(bit: 72, low: 1), epochID: digest(82)),
        devicePolicyBinding: .init(deviceKeyReference: digest(83), hardwarePolicyID: digest(84)),
        stateNonceCommitment: digest(85), logicalSequence: wide(bit: 90, low: 19),
        journalRevision: wide(bit: 91, low: 20), inboxRevision: wide(bit: 92, low: 21),
        snapshotCommitment: digest(86))
      source = .recoveryCheckpoint(statement: statement, terminalCertificateDigest: digest(87))
    }
    XCTAssertEqual(actual.authoritySource, source)
    let independent = try KagemushaEnrolledOpenAccountChallengeV1(
      enrollmentID: owner.enrollmentID(), owner: owner, nonce: digest(88), authoritySource: source,
      releaseID: digest(89), hardwarePolicyDigest: digest(84), coreAuthorizationKeyReference: digest(90))
    XCTAssertEqual(try independent.canonicalBytes(), canonical)
    XCTAssertEqual(independent.accountSigningMessage(), actual.accountSigningMessage())

    var account = CanonicalNoritoReader(data: owner.accountID.canonicalPayload)
    XCTAssertEqual(try account.readUInt32LE(), 0)
    var key = CanonicalNoritoReader(data: try account.readCompactField())
    XCTAssertEqual(try key.readUInt64LE(), 33)
    XCTAssertEqual(try key.readCompactField(), Data([0]))
    var publicKey = Data()
    for _ in 0..<32 { publicKey.append(try key.readCompactField()) }
    let verifier = try Curve25519.Signing.PublicKey(rawRepresentation: publicKey)
    let signature = try bytes(name + "_account_signature_hex")
    XCTAssertTrue(verifier.isValidSignature(signature, for: actual.accountSigningMessage()))
    XCTAssertFalse(verifier.isValidSignature(signature, for: IrohaHash.hash(canonical)))
    XCTAssertFalse(verifier.isValidSignature(signature, for: IrohaHash.hash(actual.accountSigningMessage())))
  }
}

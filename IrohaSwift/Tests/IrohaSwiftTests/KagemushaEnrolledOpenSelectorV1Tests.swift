import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaEnrolledOpenSelectorV1Tests: XCTestCase {
  private let schema = "iroha.kagemusha.v1.enrolled-open-selector"

  private func account(_ second: Bool = false) throws -> KagemushaAccountIDV1 {
    let hex = second
      ? "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"
      : "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    return try KagemushaAccountIDV1(canonicalPayload: AccountAddress.fromAccount(
      publicKey: XCTUnwrap(Data(hexString: hex)), algorithm: "ed25519"
    ).compactNoritoAccountControllerPayload())
  }

  private func runtime(
    fiID: String = "mibank", dataspace: UInt64 = 10, namespace: String = "mibank.bpng",
    network: UInt8 = 3, assetByte: UInt8 = 0x2f, incarnation: UInt8 = 5, scale: UInt32 = 2
  ) throws -> KagemushaRetailEnrollmentRuntimeProjectionV1 {
    var uuid: [UInt8] = [
      assetByte, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b,
      0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd, 0xcd, 0x2f,
    ]
    if assetByte == 0 { uuid[6] = 0 }
    return try KagemushaRetailEnrollmentRuntimeProjectionV1(
      fiID: fiID, ledgerDataspaceID: dataspace, authenticationNamespace: namespace,
      networkID: Data(repeating: network, count: 32),
      asset: KagemushaAssetDefinitionIDV1(canonicalPayload: Data(uuid.flatMap { [1, $0] })),
      assetIncarnation: KagemushaAssetIncarnationV1(bytes: Data(repeating: incarnation, count: 32)),
      scale: scale)
  }

  private func owner() throws -> KagemushaRetailEnrollmentOwnerProjectionV1 {
    try .init(accountID: account(), runtime: runtime(), laneID: Data(repeating: 32, count: 32))
  }

  func testCanonicalRoundTripRetainsEntireProjectionAndHandlesDataSlices() throws {
    let original = try KagemushaEnrolledOpenSelectorV1(owner: owner())
    let bytes = try original.canonicalBytes()
    XCTAssertEqual(try KagemushaEnrolledOpenSelectorV1.decodeCanonicalExact(bytes), original)
    XCTAssertEqual(try KagemushaEnrolledOpenSelectorV1.decodeCanonicalExact(
      (Data([0, 0]) + bytes).dropFirst(2)), original)
    XCTAssertEqual(original.enrollmentID, try original.owner.enrollmentID())
    XCTAssertEqual(original.enrollmentID.count, 32)
    XCTAssertLessThanOrEqual(bytes.count, KagemushaEnrolledOpenSelectorV1.maximumCanonicalBytes)
  }

  func testEveryImmutableScopeFieldChangesIdentityAndRejectsOldDigest() throws {
    let original = try owner()
    let digest = try original.enrollmentID()
    let variants: [KagemushaRetailEnrollmentOwnerProjectionV1] = try [
      .init(accountID: account(true), runtime: original.runtime, laneID: original.laneID),
      .init(accountID: original.accountID, runtime: runtime(fiID: "other"), laneID: original.laneID),
      .init(accountID: original.accountID, runtime: runtime(dataspace: 11), laneID: original.laneID),
      .init(accountID: original.accountID, runtime: runtime(namespace: "other.bpng"), laneID: original.laneID),
      .init(accountID: original.accountID, runtime: runtime(network: 7), laneID: original.laneID),
      .init(accountID: original.accountID, runtime: runtime(assetByte: 0x30), laneID: original.laneID),
      .init(accountID: original.accountID, runtime: runtime(incarnation: 9), laneID: original.laneID),
      .init(accountID: original.accountID, runtime: runtime(scale: 3), laneID: original.laneID),
      .init(accountID: original.accountID, runtime: original.runtime, laneID: Data(repeating: 33, count: 32)),
    ]
    for value in variants {
      XCTAssertNotEqual(try value.enrollmentID(), digest)
      XCTAssertThrowsError(try KagemushaEnrolledOpenSelectorV1(
        version: 1, owner: value, enrollmentID: digest))
    }
  }

  func testNamesRequireExactCanonicalUTF8AndRustNameSyntax() throws {
    let invalid = ["", "bank name", "bank@name", "bank#name", "bank$name", "e\u{301}",
      String(repeating: "a", count: 256), String(repeating: "é", count: 128)]
      + [0, 0x1f, 0x7f, 0x85, 0x9f, 0x061c, 0x200e, 0x200f, 0x202a, 0x202e,
         0x2066, 0x2069, 0xa0, 0x1680, 0x2000, 0x200a, 0x2028, 0x2029,
         0x202f, 0x205f, 0x3000].map { "bank" + String(UnicodeScalar($0)!) }
    for name in invalid {
      XCTAssertThrowsError(try runtime(fiID: name), name.debugDescription)
      XCTAssertThrowsError(try runtime(namespace: name), name.debugDescription)
    }
    for name in ["MiBank", "mibank.bpng", "é", "銀行", String(repeating: "a", count: 255)] {
      XCTAssertEqual(try runtime(fiID: name).fiID.utf8.map { $0 }, Array(name.utf8))
    }
    // Swift's ordinary equality would incorrectly treat the rejected spelling as canonical.
    XCTAssertEqual("é", "e\u{301}")
  }

  func testInvalidScopeVersionAndEnrollmentIdentityFailClosed() throws {
    for value: UInt8 in [0, 2] { XCTAssertThrowsError(try runtime(network: value)) }
    XCTAssertThrowsError(try runtime(assetByte: 0))
    XCTAssertThrowsError(try runtime(scale: KagemushaWireV1.maximumAssetScale + 1))
    XCTAssertNoThrow(try runtime(dataspace: 0, scale: KagemushaWireV1.maximumAssetScale))
    let valid = try owner()
    for count in [0, 31, 32, 33] {
      XCTAssertThrowsError(try KagemushaRetailEnrollmentOwnerProjectionV1(
        accountID: valid.accountID, runtime: valid.runtime, laneID: Data(repeating: 0, count: count)))
    }
    for version: UInt16 in [0, 2, .max] {
      XCTAssertThrowsError(try KagemushaEnrolledOpenSelectorV1(
        version: version, owner: valid, enrollmentID: valid.enrollmentID()))
    }
    for id in [Data(), Data(repeating: 0, count: 32), Data(repeating: 9, count: 32)] {
      XCTAssertThrowsError(try KagemushaEnrolledOpenSelectorV1(version: 1, owner: valid, enrollmentID: id))
    }
  }

  func testAccountMustBeSingleEd25519() throws {
    let secp = try AccountAddress.fromAccount(publicKey: XCTUnwrap(Data(hexString:
      "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")),
      algorithm: "secp256k1")
    let account = try KagemushaAccountIDV1(canonicalPayload: secp.compactNoritoAccountControllerPayload())
    XCTAssertThrowsError(try KagemushaRetailEnrollmentOwnerProjectionV1(
      accountID: account, runtime: runtime(), laneID: Data(repeating: 1, count: 32)))

    // A canonical one-member multisig remains an unsupported controller even
    // when its only member is a valid Ed25519 key.
    var single = CanonicalNoritoReader(data: try self.account().canonicalPayload)
    XCTAssertEqual(try single.readUInt32LE(), 0)
    let key = try single.readCompactField()
    var member = CompactNoritoWriter()
    member.writeField(key)
    member.writeField(Data([1, 0]))
    var members = CompactNoritoWriter()
    members.writeUInt64LE(1)
    members.writeField(member.data)
    var policy = CompactNoritoWriter()
    policy.writeField(Data([1]))
    policy.writeField(Data([1, 0]))
    policy.writeField(members.data)
    var multisig = CompactNoritoWriter()
    multisig.writeUInt32LE(1)
    multisig.writeField(policy.data)
    let multiAccount = try KagemushaAccountIDV1(canonicalPayload: multisig.data)
    XCTAssertThrowsError(try KagemushaRetailEnrollmentOwnerProjectionV1(
      accountID: multiAccount, runtime: runtime(), laneID: Data(repeating: 1, count: 32)))
  }

  func testActualRustCanonicalFixtureRoundTripsAndMatchesIndependentIdentity() throws {
    let fixture = try rustFixture()
    func bytes(_ key: String) throws -> Data {
      try XCTUnwrap(Data(hexString: XCTUnwrap(fixture[key] as? String)))
    }
    XCTAssertEqual(fixture["schema"] as? String, schema)
    let canonical = try bytes("selector_canonical_hex")
    let decoded = try KagemushaEnrolledOpenSelectorV1.decodeCanonicalExact(canonical)
    XCTAssertEqual(try decoded.canonicalBytes(), canonical)
    XCTAssertEqual(decoded.enrollmentID, try bytes("enrollment_id_hex"))
    XCTAssertEqual(decoded.owner.accountID.canonicalPayload, try bytes("account_payload_hex"))
    XCTAssertEqual(decoded.owner.runtime.fiID, fixture["fi_id"] as? String)
    XCTAssertEqual(decoded.owner.runtime.ledgerDataspaceID, fixture["ledger_dataspace_id"] as? UInt64)
    XCTAssertEqual(decoded.owner.runtime.authenticationNamespace, fixture["authentication_namespace"] as? String)
    XCTAssertEqual(decoded.owner.runtime.networkID, try bytes("network_id_hex"))
    XCTAssertEqual(decoded.owner.runtime.assetIncarnation.bytes, try bytes("asset_incarnation_hex"))
    XCTAssertEqual(decoded.owner.laneID, try bytes("lane_id_hex"))

    let projectedAccount = try KagemushaAccountIDV1(canonicalPayload: AccountAddress.fromAccount(
      publicKey: bytes("account_public_key_hex")).compactNoritoAccountControllerPayload())
    let projectedRuntime = try KagemushaRetailEnrollmentRuntimeProjectionV1(
      fiID: XCTUnwrap(fixture["fi_id"] as? String),
      ledgerDataspaceID: XCTUnwrap(fixture["ledger_dataspace_id"] as? UInt64),
      authenticationNamespace: XCTUnwrap(fixture["authentication_namespace"] as? String),
      networkID: bytes("network_id_hex"), asset: runtime().asset,
      assetIncarnation: .init(bytes: bytes("asset_incarnation_hex")),
      scale: XCTUnwrap(fixture["scale"] as? UInt32))
    let projected = try KagemushaEnrolledOpenSelectorV1(owner: .init(
      accountID: projectedAccount, runtime: projectedRuntime, laneID: bytes("lane_id_hex")))
    XCTAssertEqual(try projected.canonicalBytes(), canonical)
    XCTAssertEqual(projected.enrollmentID, try bytes("enrollment_id_hex"))
  }

  private func rustFixture() throws -> [String: Any] {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while directory.path != "/" {
      let path = directory.appendingPathComponent("fixtures/offline/kagemusha_enrolled_open_selector_v1.json")
      if FileManager.default.fileExists(atPath: path.path) {
        return try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: path)) as? [String: Any])
      }
      directory.deleteLastPathComponent()
    }
    throw NSError(domain: "Missing Rust enrolled-open selector fixture", code: 1)
  }

  func testRejectsEveryTruncationPathsJSONAlternateSchemaAndOversize() throws {
    let bytes = try KagemushaEnrolledOpenSelectorV1(owner: owner()).canonicalBytes()
    for length in 0..<bytes.count {
      XCTAssertThrowsError(try KagemushaEnrolledOpenSelectorV1.decodeCanonicalExact(bytes.prefix(length)),
        "truncation \(length)")
    }
    let frame = try XCTUnwrap(noritoDecodeFrame(bytes))
    for invalid in [Data("/wallet/state.db".utf8), Data("{\"version\":1}".utf8), bytes + Data([0]),
      Data(repeating: 0, count: KagemushaEnrolledOpenSelectorV1.maximumCanonicalBytes + 1),
      noritoEncode(typeName: "iroha.kagemusha.v1.retail-enrollment-owner", payload: frame.payload,
        flags: NoritoHeader.compactLen, payloadAlignment: 8)] {
      XCTAssertThrowsError(try KagemushaEnrolledOpenSelectorV1.decodeCanonicalExact(invalid))
    }
  }

  func testRejectsChecksumValidVersionDigestUnknownFieldsAndNonminimalLength() throws {
    let bytes = try KagemushaEnrolledOpenSelectorV1(owner: owner()).canonicalBytes()
    let frame = try XCTUnwrap(noritoDecodeFrame(bytes))
    var version = frame.payload
    version[1] = 2
    var digest = frame.payload
    digest[digest.count - 1] ^= 1
    var nonminimal = frame.payload
    nonminimal.replaceSubrange(0..<1, with: [0x82, 0])
    for payload in [version, digest, frame.payload + Data([1, 0]), nonminimal] {
      let invalid = noritoEncode(typeName: schema, payload: payload,
        flags: NoritoHeader.compactLen, payloadAlignment: 8)
      XCTAssertNotNil(noritoDecodeFrame(invalid), "Must exercise model validation after valid framing")
      XCTAssertThrowsError(try KagemushaEnrolledOpenSelectorV1.decodeCanonicalExact(invalid))
    }
  }

  func testCallerMutationCannotChangeRetainedIdentityOrCanonicalBytes() throws {
    let mutableLane = NSMutableData(data: Data(repeating: 32, count: 32))
    let mutableNetwork = NSMutableData(data: Data(repeating: 3, count: 32))
    let base = try runtime()
    let runtime = try KagemushaRetailEnrollmentRuntimeProjectionV1(
      fiID: base.fiID, ledgerDataspaceID: base.ledgerDataspaceID,
      authenticationNamespace: base.authenticationNamespace,
      networkID: Data(referencing: mutableNetwork), asset: base.asset,
      assetIncarnation: base.assetIncarnation, scale: base.scale)
    let value = try KagemushaEnrolledOpenSelectorV1(owner: .init(
      accountID: account(), runtime: runtime, laneID: Data(referencing: mutableLane)))
    let expected = try value.canonicalBytes()
    mutableLane.resetBytes(in: NSRange(location: 0, length: 32))
    mutableNetwork.resetBytes(in: NSRange(location: 0, length: 32))
    var returnedLane = value.owner.laneID
    returnedLane[0] = 99
    var returnedID = value.enrollmentID
    returnedID[0] ^= 1
    XCTAssertEqual(try value.canonicalBytes(), expected)
    XCTAssertEqual(value.owner.laneID, Data(repeating: 32, count: 32))
    XCTAssertEqual(value.owner.runtime.networkID, Data(repeating: 3, count: 32))
  }
}

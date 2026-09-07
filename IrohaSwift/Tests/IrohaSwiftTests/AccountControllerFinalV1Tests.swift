import Foundation
import XCTest
@testable import IrohaSwift

final class AccountControllerFinalV1Tests: XCTestCase {
  func testAccountEncodingAlwaysRequiresCanonicalController() throws {
    let address = try AccountAddress.fromAccount(publicKey: keys(1)[0])
    let literal = try address.toI105(networkPrefix: AccountId.defaultNetworkPrefix)
    XCTAssertEqual(try CanonicalNorito.encodeAccountId(literal), try address.noritoAccountControllerPayload())
    for invalid in ["alice", "alice@wonderland.sora", "0x1234", literal + " ", " " + literal] {
      XCTAssertThrowsError(try CanonicalNorito.encodeAccountId(invalid), invalid)
      XCTAssertThrowsError(try CanonicalNorito.encodeCompactAccountId(invalid), invalid)
    }
  }

  func testAddressRejectsRetiredCountAndNoncanonicalMultisigPolicies() throws {
    let publicKeys = try keys(2)
    let canonical = try addressBytes(threshold: 2, members: [(publicKeys[0], 1), (publicKeys[1], 2)])
    XCTAssertNoThrow(try AccountAddress.fromCanonicalBytes(canonical))
    let invalidMembers: [[(Data, UInt16)]] = [
      [(publicKeys[1], 1), (publicKeys[0], 1)],
      [(publicKeys[0], 1), (publicKeys[0], 2)],
      [(publicKeys[0], 0)],
    ]
    for members in invalidMembers {
      XCTAssertThrowsError(try AccountAddress.fromCanonicalBytes(addressBytes(threshold: 1, members: members)))
    }
    XCTAssertThrowsError(try AccountAddress.fromCanonicalBytes(addressBytes(threshold: 2, members: [(publicKeys[0], 1)])))
    var invalid = canonical; invalid[2] = 2
    XCTAssertThrowsError(try AccountAddress.fromCanonicalBytes(invalid))
    invalid = try addressBytes(threshold: 1, members: [(publicKeys[0], 1)])
    invalid.remove(at: 5)
    XCTAssertThrowsError(try AccountAddress.fromCanonicalBytes(invalid))
    // A forged large count must fail before reserving the advertised vector.
    XCTAssertThrowsError(try AccountAddress.fromCanonicalBytes(Data([0x0a, 1, 1, 0, 1, 255, 255])))
  }

  func testFullU16CountRetainsAll256Members() throws {
    let publicKeys = try keys(256)
    let members = publicKeys.map { ($0, UInt16(1)) }
    let canonical = try addressBytes(threshold: 256, members: members)
    let address = try AccountAddress.fromCanonicalBytes(canonical)
    XCTAssertEqual(Array(canonical[5..<7]), [1, 0])
    let compact = try address.compactNoritoAccountControllerPayload()
    XCTAssertTrue(AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(compact))
    var outer = CanonicalNoritoReader(data: compact)
    XCTAssertEqual(try outer.readUInt32LE(), 1)
    var policy = CanonicalNoritoReader(data: try outer.readCompactField())
    _ = try policy.readCompactField(); _ = try policy.readCompactField()
    var encodedMembers = CanonicalNoritoReader(data: try policy.readCompactField())
    XCTAssertEqual(try encodedMembers.readUInt64LE(), 256)
    let builder = MultisigPolicyBuilder().setThreshold(256)
    for key in publicKeys.reversed() { builder.addMember(algorithm: .ed25519, weight: 1, publicKey: key) }
    let built = try builder.build()
    XCTAssertEqual(built.members.map(\.publicKey), publicKeys)
    XCTAssertEqual(built.threshold, 256)
  }

  func testBuilderRejectsInvalidPolicyAndCanonicalizesOnlyInputOrder() throws {
    let publicKeys = try keys(2)
    let build: (UInt8, UInt16, [(Data, UInt16)]) throws -> MultisigPolicy = { version, threshold, members in
      let builder = MultisigPolicyBuilder().setVersion(version).setThreshold(threshold)
      for (key, weight) in members { builder.addMember(algorithm: .ed25519, weight: weight, publicKey: key) }
      return try builder.build()
    }
    let invalidPolicies: [(UInt8, UInt16, [(Data, UInt16)])] = [
      (2, 1, [(publicKeys[0], 1)]), (1, 0, [(publicKeys[0], 1)]),
      (1, 2, [(publicKeys[0], 1)]), (1, 1, [(publicKeys[0], 0)]),
      (1, 1, [(publicKeys[0], 1), (publicKeys[0], 2)]),
    ]
    for (version, threshold, members) in invalidPolicies { XCTAssertThrowsError(try build(version, threshold, members)) }
    let first = try build(1, 2, [(publicKeys[0], 1), (publicKeys[1], 2)])
    let second = try build(1, 2, [(publicKeys[1], 2), (publicKeys[0], 1)])
    XCTAssertEqual(first.ctap2Cbor, second.ctap2Cbor)
    XCTAssertEqual(first.digestBlake2b256, second.digestBlake2b256)
    XCTAssertNotEqual(first.ctap2Cbor, try build(1, 1, [(publicKeys[0], 1), (publicKeys[1], 2)]).ctap2Cbor)
    XCTAssertNotEqual(first.ctap2Cbor, try build(1, 2, [(publicKeys[0], 2), (publicKeys[1], 1)]).ctap2Cbor)
  }

  private func keys(_ count: Int) throws -> [Data] {
    try (1...count).map { index in
      var seed = Data(repeating: 0, count: 32)
      seed[0] = UInt8(index & 255); seed[1] = UInt8(index >> 8)
      return try Keypair(privateKeyBytes: seed).publicKey
    }.sorted { $0.lexicographicallyPrecedes($1) }
  }

  private func addressBytes(threshold: UInt16, members: [(Data, UInt16)]) throws -> Data {
    var bytes = Data([0x0a, 1, 1])
    func append(_ value: UInt16) { bytes.append(UInt8(value >> 8)); bytes.append(UInt8(value & 255)) }
    append(threshold); append(UInt16(members.count))
    for (key, weight) in members {
      bytes.append(1); append(weight); append(UInt16(key.count)); bytes.append(key)
    }
    return bytes
  }
}

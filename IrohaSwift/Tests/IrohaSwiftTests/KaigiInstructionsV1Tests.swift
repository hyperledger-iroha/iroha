import Foundation
import XCTest

@testable import IrohaSwift

final class KaigiInstructionsV1Tests: XCTestCase {
  func testAllNineInstructionsUseCanonicalNativeFrames() throws {
    let callID = try KaigiIdV1(domainID: "meetings.universal", callName: "standup")
    let host = try account(1)
    let participant = try account(2)
    let manifest = try relayManifest()
    let call = try NewKaigiV1(
      id: callID,
      host: host,
      title: "Daily standup",
      description: "Engineering sync",
      maxParticipants: UInt32.max,
      gasRatePerMinute: UInt64.max,
      metadata: ["zeta": .bool(true), "alpha": .string("first")],
      scheduledStartMs: UInt64.max,
      billingAccount: host,
      roomPolicy: .authenticated,
      relayManifest: manifest
    )
    let registration = try KaigiRelayRegistrationV1(
      relayID: manifest.hops[0].relayID,
      hpkePublicKey: manifest.hops[0].hpkePublicKey,
      bandwidthClass: UInt8.max
    )

    let instructions: [any KaigiInstructionV1] = [
      try CreateKaigiInstructionV1(call: call),
      try JoinKaigiInstructionV1(callID: callID, participant: participant),
      try LeaveKaigiInstructionV1(callID: callID, participant: participant),
      EndKaigiInstructionV1(callID: callID, endedAtMs: UInt64.max),
      try RecordKaigiUsageInstructionV1(
        callID: callID,
        durationMs: UInt64.max,
        billedGas: UInt64.max
      ),
      SetKaigiRelayManifestInstructionV1(callID: callID, relayManifest: manifest),
      RegisterKaigiRelayInstructionV1(relay: registration),
      try UnregisterKaigiRelayInstructionV1(relayID: manifest.hops[0].relayID),
      try ReportKaigiRelayHealthInstructionV1(
        callID: callID,
        relayID: manifest.hops[0].relayID,
        status: .degraded,
        reportedAtMs: UInt64.max,
        notes: "packet loss"
      ),
    ]

    XCTAssertEqual(
      instructions.map(\.wireID),
      [
        "iroha.instruction.v1::kaigi::CreateKaigi",
        "iroha.instruction.v1::kaigi::JoinKaigi",
        "iroha.instruction.v1::kaigi::LeaveKaigi",
        "iroha.instruction.v1::kaigi::EndKaigi",
        "iroha.instruction.v1::kaigi::RecordKaigiUsage",
        "iroha.instruction.v1::kaigi::SetKaigiRelayManifest",
        "iroha.instruction.v1::kaigi::RegisterKaigiRelay",
        "iroha.instruction.v1::kaigi::UnregisterKaigiRelay",
        "iroha.instruction.v1::kaigi::ReportKaigiRelayHealth",
      ]
    )

    for instruction in instructions {
      let bare = try instruction.barePayload()
      let transactionFrame = try instruction.transactionInstructionFrame()
      XCTAssertEqual(transactionFrame.wireName, instruction.wireID)
      let concrete = try XCTUnwrap(noritoDecodeFrame(transactionFrame.framedPayload))
      XCTAssertEqual(concrete.header.flags, NoritoHeader.compactLen)
      XCTAssertEqual(
        concrete.header.schema,
        noritoSchemaHash(forTypeName: instruction.concreteSchemaName)
      )
      XCTAssertEqual(concrete.payload, bare)
      XCTAssertEqual(concrete.paddingLength, 0)

      let standalone = try instruction.standaloneInstructionBoxFrame()
      let instructionBox = try XCTUnwrap(noritoDecodeFrame(standalone))
      XCTAssertEqual(
        instructionBox.header.schema,
        noritoSchemaHash(
          forTypeName: "(alloc::string::String, alloc::vec::Vec<u8>)"
        )
      )
      XCTAssertEqual(
        instructionBox.payload,
        try transactionFrame.compactInstructionBoxPayload()
      )
    }
  }

  func testUInt64MaximumUsesExactEightByteFields() throws {
    let callID = try KaigiIdV1(domainID: "meetings.universal", callName: "limits")
    let usage = try RecordKaigiUsageInstructionV1(
      callID: callID,
      durationMs: UInt64.max,
      billedGas: UInt64.max
    )
    var usageReader = CanonicalNoritoReader(data: try usage.barePayload())
    _ = try usageReader.readCompactField()
    XCTAssertEqual(try usageReader.readCompactField(), Data(repeating: 0xFF, count: 8))
    XCTAssertEqual(try usageReader.readCompactField(), Data(repeating: 0xFF, count: 8))

    let report = try ReportKaigiRelayHealthInstructionV1(
      callID: callID,
      relayID: try account(3),
      status: .unavailable,
      reportedAtMs: UInt64.max
    )
    var reportReader = CanonicalNoritoReader(data: try report.barePayload())
    _ = try reportReader.readCompactField()
    _ = try reportReader.readCompactField()
    XCTAssertEqual(try reportReader.readCompactField(), Data([2, 0, 0, 0]))
    XCTAssertEqual(try reportReader.readCompactField(), Data(repeating: 0xFF, count: 8))
    XCTAssertEqual(try reportReader.readCompactField(), Data([0]))
    XCTAssertEqual(reportReader.remaining(), 0)
  }

  func testRelayHealthBarePayloadMatchesRustOwnedEncoding() throws {
    let relayID = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"
    let report = try ReportKaigiRelayHealthInstructionV1(
      callID: KaigiIdV1(domainID: "meetings.universal", callName: "limits"),
      relayID: relayID,
      status: .unavailable,
      reportedAtMs: UInt64.max
    )
    let expected = try XCTUnwrap(
      Data(
        hexString:
          "1e1509086d656574696e67730a09756e6976657273616c07066c696d6974734f000000004a21000000000000000100011f0185017f01e901800152014a012e01e401fe016501e501d3014601f701aa01ad01cb0163016a0164010f011d0119011d011c016e01150186010701ba011e040200000008ffffffffffffffff0100"
      )
    )
    XCTAssertEqual(try report.barePayload(), expected)
  }

  func testLeaveAlwaysEncodesReservedPrivacyFieldsAsNone() throws {
    let leave = try LeaveKaigiInstructionV1(
      callID: KaigiIdV1(domainID: "meetings.universal", callName: "transparent"),
      participant: account(4)
    )
    var reader = CanonicalNoritoReader(data: try leave.barePayload())
    _ = try reader.readCompactField()
    _ = try reader.readCompactField()
    for _ in 0..<4 {
      XCTAssertEqual(try reader.readCompactField(), Data([0]))
    }
    XCTAssertEqual(reader.remaining(), 0)
  }

  func testPrivacyArtifactsFixReservedIdentityHintsToSafeValues() throws {
    let commitment = KaigiParticipantCommitmentV1(commitment: try hash(0x11))
    let nullifier = try KaigiParticipantNullifierV1(digest: hash(0x22))
    let privacy = try KaigiPrivacyArtifactsV1(
      commitment: commitment,
      nullifier: nullifier,
      rosterRoot: hash(0x33),
      proof: Data([0xAA, 0xBB])
    )
    let call = try NewKaigiV1(
      id: KaigiIdV1(domainID: "meetings.universal", callName: "private"),
      host: account(5),
      privacyMode: .zkRosterV1
    )
    let create = try CreateKaigiInstructionV1(call: call, privacyArtifacts: privacy)
    var outer = CanonicalNoritoReader(data: try create.barePayload())
    _ = try outer.readCompactField()

    var commitmentOption = CanonicalNoritoReader(data: try outer.readCompactField())
    XCTAssertEqual(try commitmentOption.readUInt8(), 1)
    var encodedCommitment = CanonicalNoritoReader(
      data: try commitmentOption.readCompactField()
    )
    XCTAssertEqual(try encodedCommitment.readCompactField(), commitment.commitment.bytes)
    XCTAssertEqual(try encodedCommitment.readCompactField(), Data([0]))
    XCTAssertEqual(encodedCommitment.remaining(), 0)

    var nullifierOption = CanonicalNoritoReader(data: try outer.readCompactField())
    XCTAssertEqual(try nullifierOption.readUInt8(), 1)
    var encodedNullifier = CanonicalNoritoReader(data: try nullifierOption.readCompactField())
    XCTAssertEqual(try encodedNullifier.readCompactField(), nullifier.digest.bytes)
    XCTAssertEqual(try encodedNullifier.readCompactField(), Data(repeating: 0, count: 8))
    XCTAssertEqual(encodedNullifier.remaining(), 0)
  }

  func testManifestSnapshotsInputsAndRejectsWireDuplicateAccounts() throws {
    var key = Data([1, 2, 3])
    var hops = [
      try KaigiRelayHopV1(relayID: account(10), hpkePublicKey: key, weight: 1),
      try KaigiRelayHopV1(relayID: account(11), hpkePublicKey: Data([4]), weight: 2),
      try KaigiRelayHopV1(relayID: account(12), hpkePublicKey: Data([5]), weight: 3),
    ]
    let manifest = try KaigiRelayManifestV1(hops: hops, expiryMs: UInt64.max)
    key[0] = 9
    hops.removeAll()
    XCTAssertEqual(manifest.hops.count, 3)
    XCTAssertEqual(manifest.hops[0].hpkePublicKey, Data([1, 2, 3]))

    let address = try AccountAddress.fromAccount(
      publicKey: Keypair(privateKeyBytes: Data(repeating: 0x41, count: 32)).publicKey
    )
    let mainnet = try address.toI105(networkPrefix: AccountId.defaultNetworkPrefix)
    let alternate = try address.toI105(networkPrefix: 42)
    XCTAssertNotEqual(mainnet, alternate)
    let duplicates = [
      try KaigiRelayHopV1(relayID: mainnet, hpkePublicKey: Data([1]), weight: 1),
      try KaigiRelayHopV1(relayID: alternate, hpkePublicKey: Data([2]), weight: 1),
      try KaigiRelayHopV1(relayID: account(13), hpkePublicKey: Data([3]), weight: 1),
    ]
    XCTAssertThrowsError(try KaigiRelayManifestV1(hops: duplicates, expiryMs: 1))
  }

  func testRelayManifestAndHPKEKeyV1Boundaries() throws {
    XCTAssertEqual(KaigiRelayBoundsV1.maxManifestHops, 8)
    XCTAssertEqual(KaigiRelayBoundsV1.maxHPKEPublicKeyBytes, 4_096)

    let hops = try (0..<KaigiRelayBoundsV1.maxManifestHops).map { index in
      try KaigiRelayHopV1(
        relayID: account(UInt8(60 + index)),
        hpkePublicKey: Data(
          repeating: 0xA5,
          count: index == 0 ? KaigiRelayBoundsV1.maxHPKEPublicKeyBytes : 1
        ),
        weight: 1
      )
    }
    let manifest = try KaigiRelayManifestV1(hops: hops, expiryMs: 1)
    XCTAssertEqual(manifest.hops.count, 8)
    XCTAssertEqual(manifest.hops[0].hpkePublicKey.count, 4_096)
    XCTAssertThrowsError(
      try KaigiRelayManifestV1(hops: hops + [hops[0]], expiryMs: 1)
    )

    XCTAssertThrowsError(
      try KaigiRelayHopV1(
        relayID: account(70),
        hpkePublicKey: Data(repeating: 0xA5, count: 4_097),
        weight: 1
      )
    )
    let registration = try KaigiRelayRegistrationV1(
      relayID: account(70),
      hpkePublicKey: Data(repeating: 0xA5, count: 4_096),
      bandwidthClass: 1
    )
    XCTAssertEqual(registration.hpkePublicKey.count, 4_096)
    XCTAssertThrowsError(
      try KaigiRelayRegistrationV1(
        relayID: account(70),
        hpkePublicKey: Data(repeating: 0xA5, count: 4_097),
        bandwidthClass: 1
      )
    )
  }

  func testValidationMatchesStatelessRustAdmissionRules() throws {
    let callID = try KaigiIdV1(domainID: "meetings.universal", callName: "validation")
    let host = try account(20)
    XCTAssertThrowsError(
      try NewKaigiV1(id: callID, host: host, maxParticipants: 0)
    )
    XCTAssertThrowsError(
      try NewKaigiV1(id: callID, host: host, billingAccount: account(21))
    )
    XCTAssertThrowsError(
      try NewKaigiV1(id: callID, host: host, metadata: ["bad key": .bool(true)])
    )
    XCTAssertThrowsError(
      try NewKaigiV1(id: callID, host: host, metadata: ["value": .number(.nan)])
    )
    XCTAssertThrowsError(
      try RecordKaigiUsageInstructionV1(callID: callID, durationMs: 0, billedGas: 0)
    )
    XCTAssertThrowsError(
      try KaigiRelayHopV1(relayID: host, hpkePublicKey: Data(), weight: 1)
    )
    XCTAssertThrowsError(
      try KaigiRelayHopV1(relayID: host, hpkePublicKey: Data([1]), weight: 0)
    )
    XCTAssertThrowsError(
      try KaigiRelayRegistrationV1(
        relayID: host,
        hpkePublicKey: Data([1]),
        bandwidthClass: 0
      )
    )

    let privacy = try KaigiPrivacyArtifactsV1(
      commitment: KaigiParticipantCommitmentV1(commitment: hash(0x31)),
      nullifier: KaigiParticipantNullifierV1(digest: hash(0x32)),
      rosterRoot: hash(0x33),
      proof: Data([1])
    )
    let transparent = try NewKaigiV1(id: callID, host: host)
    XCTAssertThrowsError(
      try CreateKaigiInstructionV1(call: transparent, privacyArtifacts: privacy)
    )
    XCTAssertThrowsError(
      try KaigiPrivacyArtifactsV1(
        commitment: privacy.commitment,
        nullifier: privacy.nullifier,
        rosterRoot: privacy.rosterRoot,
        proof: Data()
      )
    )
    XCTAssertThrowsError(
      try KaigiParticipantNullifierV1(
        digest: KaigiHashV1(bytes: Data(repeating: 0, count: 31) + Data([1]))
      )
    )
  }

  func testRelayHealthNotesUseRustUnicodeScalarCount() throws {
    let callID = try KaigiIdV1(domainID: "meetings.universal", callName: "health")
    let relayID = try account(30)
    let twoScalarGrapheme = "e\u{301}"
    XCTAssertEqual(twoScalarGrapheme.count, 1)
    XCTAssertEqual(twoScalarGrapheme.unicodeScalars.count, 2)
    XCTAssertNoThrow(
      try ReportKaigiRelayHealthInstructionV1(
        callID: callID,
        relayID: relayID,
        status: .healthy,
        reportedAtMs: 0,
        notes: String(repeating: twoScalarGrapheme, count: 256)
      )
    )
    XCTAssertThrowsError(
      try ReportKaigiRelayHealthInstructionV1(
        callID: callID,
        relayID: relayID,
        status: .healthy,
        reportedAtMs: 0,
        notes: String(repeating: twoScalarGrapheme, count: 257)
      )
    )
  }

  func testMetadataEncodingIsIndependentOfDictionaryInsertionOrder() throws {
    let callID = try KaigiIdV1(domainID: "meetings.universal", callName: "metadata")
    let host = try account(40)
    var first: [String: ToriiJSONValue] = [:]
    first["zeta"] = .number(2)
    first["alpha"] = .number(1)
    var second: [String: ToriiJSONValue] = [:]
    second["alpha"] = .number(1)
    second["zeta"] = .number(2)
    let firstCall = try NewKaigiV1(id: callID, host: host, metadata: first)
    let secondCall = try NewKaigiV1(id: callID, host: host, metadata: second)
    XCTAssertEqual(
      try CreateKaigiInstructionV1(call: firstCall).barePayload(),
      try CreateKaigiInstructionV1(call: secondCall).barePayload()
    )
  }

  private func relayManifest() throws -> KaigiRelayManifestV1 {
    try KaigiRelayManifestV1(
      hops: [
        KaigiRelayHopV1(
          relayID: account(50),
          hpkePublicKey: Data([0xA1, 0xA2]),
          weight: 1
        ),
        KaigiRelayHopV1(
          relayID: account(51),
          hpkePublicKey: Data([0xB1, 0xB2]),
          weight: 2
        ),
        KaigiRelayHopV1(
          relayID: account(52),
          hpkePublicKey: Data([0xC1, 0xC2]),
          weight: 3
        ),
      ],
      expiryMs: UInt64.max
    )
  }

  private func account(_ byte: UInt8) throws -> String {
    let publicKey = try Keypair(
      privateKeyBytes: Data(repeating: byte, count: 32)
    ).publicKey
    return try AccountId.makeI105(publicKey: publicKey)
  }

  private func hash(_ byte: UInt8) throws -> KaigiHashV1 {
    var bytes = Data(repeating: byte, count: KaigiHashV1.byteCount)
    bytes[bytes.index(before: bytes.endIndex)] |= 1
    return try KaigiHashV1(bytes: bytes)
  }
}

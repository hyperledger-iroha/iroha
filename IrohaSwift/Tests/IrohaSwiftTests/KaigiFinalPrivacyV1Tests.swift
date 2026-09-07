import Foundation
import XCTest
@testable import IrohaSwift

final class KaigiFinalPrivacyV1Tests: XCTestCase {
  private let modulus = Data(hexString: "01000000ed302d991bf94c09fc98462200000000000000000000000000000040")!

  func testCanonicalScalarBoundariesPreserveAllBitsAndCopies() throws {
    var maximum = modulus
    maximum[0] = 0
    for bytes in [Data(repeating: 0, count: 32), maximum, Data(repeating: 0x24, count: 32)] {
      XCTAssertEqual(try KaigiAuthorizationScalarV1(bytes: bytes).bytes, bytes)
    }
    for bytes in [Data(), Data(repeating: 0, count: 31), Data(repeating: 0, count: 33), modulus,
                  Data(repeating: 255, count: 32)] {
      XCTAssertThrowsError(try KaigiAuthorizationScalarV1(bytes: bytes))
    }
    var bytes = Data(repeating: 0, count: 32)
    let low = try KaigiAuthorizationScalarV1(bytes: bytes)
    bytes[31] = 1
    let high = try KaigiAuthorizationScalarV1(bytes: bytes)
    XCTAssertNotEqual(low, high)
    bytes[0] = 7
    XCTAssertEqual(high.bytes[0], 0)
    XCTAssertEqual(low.bytes, Data(repeating: 0, count: 32))
  }

  func testAllFinalPrivateActionsMatchRustOwnedInstructionBoxes() throws {
    let id = try KaigiIdV1(domainID: "wonderland.sora", callName: "weekly-sync")
    let host = KaigiFinalWireFixturesV1.accounts[0]
    var maximum = modulus
    maximum[0] = 0
    let scalar = try KaigiAuthorizationScalarV1(bytes: maximum)
    let artifacts = try KaigiPrivacyArtifactsV1(
      commitment: .init(commitment: KaigiAuthorizationScalarV1(bytes: Data(repeating: 0x22, count: 32))),
      nullifier: .init(digest: scalar),
      rosterRoot: KaigiHashV1(bytes: Data(repeating: 0x55, count: 32)),
      proof: Data("wire-fixture-only".utf8))
    let call = try NewKaigiV1(id: id, host: host, privacyMode: .zkRosterV1)
    let cases: [(String, any KaigiInstructionV1)] = [
      ("PrivateCreateKaigi", try CreateKaigiInstructionV1(call: call, privacyArtifacts: artifacts)),
      ("PrivateJoinKaigi", try JoinKaigiInstructionV1(callID: id, participant: host, privacyArtifacts: artifacts)),
      ("PrivateLeaveKaigi", try LeaveKaigiInstructionV1(callID: id, participant: host, privacyArtifacts: artifacts)),
      ("PrivateEndKaigi", EndKaigiInstructionV1(callID: id, privacyArtifacts: artifacts)),
      ("PrivateRecordKaigiUsage", try RecordKaigiUsageInstructionV1(callID: id, durationMs: 1, billedGas: 0,
        privacy: KaigiUsagePrivacyV1(commitment: scalar, proof: Data("wire-fixture-only".utf8)))),
    ]
    for (name, instruction) in cases {
      XCTAssertEqual(try instruction.standaloneInstructionBoxFrame(),
                     Data(base64Encoded: KaigiFinalWireFixturesV1.instructionBoxes[name]!), name)
    }
    XCTAssertThrowsError(try CreateKaigiInstructionV1(call: call))
  }

  func testAllTransparentActionsRetainExactRustOwnedWire() throws {
    let id = try KaigiIdV1(domainID: "wonderland.sora", callName: "weekly-sync")
    let host = KaigiFinalWireFixturesV1.accounts[0]
    let cases: [(String, any KaigiInstructionV1)] = [
      ("CreateKaigi", try CreateKaigiInstructionV1(call: NewKaigiV1(id: id, host: host))),
      ("JoinKaigi", try JoinKaigiInstructionV1(callID: id, participant: host)),
      ("LeaveKaigi", try LeaveKaigiInstructionV1(callID: id, participant: host)),
      ("EndKaigi", EndKaigiInstructionV1(callID: id)),
      ("RecordKaigiUsage", try RecordKaigiUsageInstructionV1(callID: id, durationMs: 1, billedGas: 2)),
      ("SetKaigiRelayManifest", SetKaigiRelayManifestInstructionV1(callID: id, relayManifest: nil)),
      ("RegisterKaigiRelay", RegisterKaigiRelayInstructionV1(relay: try KaigiRelayRegistrationV1(
        relayID: host, hpkePublicKey: Data("key".utf8), bandwidthClass: 1))),
      ("UnregisterKaigiRelay", try UnregisterKaigiRelayInstructionV1(relayID: host)),
      ("ReportKaigiRelayHealth", try ReportKaigiRelayHealthInstructionV1(
        callID: id, relayID: host, status: .healthy, reportedAtMs: 3)),
    ]
    for (name, instruction) in cases {
      XCTAssertEqual(try instruction.standaloneInstructionBoxFrame(),
                     Data(base64Encoded: KaigiFinalWireFixturesV1.instructionBoxes[name]!), name)
    }
  }

  func testComplexPrivateCreateMatchesFullWidthRustFixture() throws {
    let accounts = KaigiFinalWireFixturesV1.accounts
    let manifest = try KaigiRelayManifestV1(hops: [
      KaigiRelayHopV1(relayID: accounts[0], hpkePublicKey: Data([0x10, 0x20]), weight: 1),
      KaigiRelayHopV1(relayID: accounts[1], hpkePublicKey: Data([0x30]), weight: 2),
      KaigiRelayHopV1(relayID: accounts[2], hpkePublicKey: Data([0x40, 0x50, 0x60]), weight: 255),
    ], expiryMs: 1 << 63)
    let call = try NewKaigiV1(
      id: KaigiIdV1(domainID: "wonderland.sora", callName: "weekly-sync"), host: accounts[0],
      title: "Roadmap 🛰", description: "exact", maxParticipants: 7,
      gasRatePerMinute: 9_007_199_254_740_993,
      metadata: ["z": .array([.bool(true), .null, .number(7)]), "a": .object(["nested": .string("value")])],
      scheduledStartMs: 1_234_567_890_123, billingAccount: accounts[0],
      privacyMode: .zkRosterV1, roomPolicy: .public, relayManifest: manifest)
    let artifacts = try KaigiPrivacyArtifactsV1(
      commitment: .init(commitment: KaigiAuthorizationScalarV1(bytes: Data(repeating: 0x44, count: 31) + Data([4]))),
      nullifier: .init(digest: KaigiAuthorizationScalarV1(bytes: Data(repeating: 0x55, count: 31) + Data([5]))),
      rosterRoot: KaigiHashV1(bytes: Data(repeating: 0x66, count: 31) + Data([0x67])),
      proof: Data([1, 2, 3]))
    XCTAssertEqual(try CreateKaigiInstructionV1(call: call, privacyArtifacts: artifacts).standaloneInstructionBoxFrame(),
                   Data(base64Encoded: KaigiFinalWireFixturesV1.instructionBoxes["ComplexCreateKaigi"]!))
  }

  func testCanonicalRecordPreservesOriginalSubjectAndFullWidthSequence() throws {
    let record = try KaigiPrivacyStateV1.decodeCanonicalRecordJSON(KaigiFinalWireFixturesV1.recordJSON)
    XCTAssertEqual(record.originalHost, KaigiFinalWireFixturesV1.accounts[0])
    XCTAssertEqual(record.privateParticipation[0].originalAccount, KaigiFinalWireFixturesV1.accounts[1])
    XCTAssertEqual(record.privateParticipation[0].sequence, 1)
    XCTAssertEqual(record.rosterCommitments[0].commitment, record.privateParticipation[0].activeCommitment)
    XCTAssertEqual(record.usageCommitments[0].bytes[0], 3)
    XCTAssertEqual(record.nullifierLog.count, 2)
    XCTAssertEqual(record.segmentsRecorded, 1)
    let exact = String(data: KaigiFinalWireFixturesV1.recordJSON, encoding: .utf8)!
      .replacingOccurrences(of: "\"sequence\":1", with: "\"sequence\":18446744073709551614")
    XCTAssertEqual(try KaigiPrivacyStateV1.decodeCanonicalRecordJSON(Data(exact.utf8))
      .privateParticipation[0].sequence, UInt64.max - 1)
  }

  func testRecordRejectsRetiredHintsMissingOwnershipAndMalformedScalars() throws {
    let original = try JSONSerialization.jsonObject(with: KaigiFinalWireFixturesV1.recordJSON) as! [String: Any]
    let reject: ([String: Any]) throws -> Void = { object in
      XCTAssertThrowsError(try KaigiPrivacyStateV1.decodeCanonicalRecordJSON(
        JSONSerialization.data(withJSONObject: object)))
    }
    var missing = original; missing.removeValue(forKey: "private_participation"); try reject(missing)
    var hint = original
    var c = hint["host_commitment"] as! [String: Any]; c["alias_tag"] = NSNull()
    hint["host_commitment"] = c; try reject(hint)
    var n = original["nullifier_log"] as! [[String: Any]]; n[0]["issued_at_ms"] = 0
    hint = original; hint["nullifier_log"] = n; try reject(hint)
    for raw in [Array(modulus), [UInt8](repeating: 0, count: 31), [UInt8](repeating: 0, count: 33)] {
      hint = original; hint["usage_commitments"] = [raw]; try reject(hint)
    }
    let text = String(data: KaigiFinalWireFixturesV1.recordJSON, encoding: .utf8)!
    for sequence in ["0", "18446744073709551615", "18446744073709551616", "1.0", "1e0", "true", "\"1\""] {
      XCTAssertThrowsError(try KaigiPrivacyStateV1.decodeCanonicalRecordJSON(Data(
        text.replacingOccurrences(of: "\"sequence\":1", with: "\"sequence\":\(sequence)").utf8)), sequence)
    }
    for scalar in ["3.0", "3e0", "true", "\"3\""] {
      XCTAssertThrowsError(try KaigiPrivacyStateV1.decodeCanonicalRecordJSON(Data(
        text.replacingOccurrences(of: "\"usage_commitments\":[[3,", with: "\"usage_commitments\":[[\(scalar),").utf8)), scalar)
    }
    XCTAssertThrowsError(try KaigiPrivacyStateV1.decodeCanonicalRecordJSON(Data(
      text.replacingOccurrences(of: "\"sequence\":1", with: "\"sequence\":1,\"sequence\":2").utf8)))
    var ledger = original["private_participation"] as! [String: Any]
    var entries = ledger["entries"] as! [[String: Any]]
    entries.append(entries[0]); ledger["entries"] = entries
    hint = original; hint["private_participation"] = ledger; try reject(hint)
    hint = original; hint["roster_commitments"] = []; try reject(hint)
    hint = original; hint["roster_root"] = "hash:" + String(repeating: "0", count: 64) + "#0000"; try reject(hint)
  }
}

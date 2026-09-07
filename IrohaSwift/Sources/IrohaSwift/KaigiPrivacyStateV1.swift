import Foundation

/// One retained original subject and its current or next participation sequence.
public struct KaigiPrivateParticipationV1: Equatable, Sendable {
  public let originalAccount: String
  public let sequence: UInt64
  public let activeCommitment: KaigiAuthorizationScalarV1?
}

/// Privacy-relevant projection of the final canonical `KaigiRecord` JSON.
///
/// This requires the full record, including retained original participation.
/// The app-facing Torii call view omits this information and cannot be used here.
/// Decoding establishes shape and roster ownership consistency, not authenticated
/// state, account lineage, network identity, proof validity or ledger execution.
public struct KaigiPrivacyStateV1: Equatable, Sendable {
  /// Local read-resource bound; not a new consensus record-size limit.
  public static let maximumJSONBytes = 8 * 1024 * 1024
  public let callID: KaigiIdV1
  public let originalHost: String
  public let privacyMode: KaigiPrivacyModeV1
  public let hostCommitment: KaigiParticipantCommitmentV1?
  public let rosterRoot: KaigiHashV1
  public let rosterCommitments: [KaigiParticipantCommitmentV1]
  public let privateParticipation: [KaigiPrivateParticipationV1]
  public let nullifierLog: [KaigiParticipantNullifierV1]
  public let usageCommitments: [KaigiAuthorizationScalarV1]
  public let segmentsRecorded: UInt32
  public let totalDurationMs: UInt64
  public let totalBilledGas: UInt64

  /// Decode only the final model shape; retired hint fields are rejected.
  public static func decodeCanonicalRecordJSON(_ data: Data) throws -> Self {
    guard data.count <= maximumJSONBytes else {
      throw KaigiV1Error.invalidValue("Kaigi record exceeds the SDK JSON read bound.")
    }
    // This bounded-depth pass rejects duplicate keys and non-integer scalar /
    // sequence spellings before Foundation's typed decoding can normalize them.
    try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(
      in: data,
      integerKeys: ["sequence", "segments_recorded", "total_duration_ms", "total_billed_gas"],
      integerArrayKeys: ["commitment", "digest", "active_commitment"],
      integerMatrixKeys: ["usage_commitments"]
    )
    return try JSONDecoder().decode(KaigiRecordProjectionJSONV1.self, from: data).value
  }
}

private struct KaigiJSONKeyV1: CodingKey {
  let stringValue: String
  var intValue: Int? { nil }
  init(_ string: String) { stringValue = string }
  init?(stringValue: String) { self.init(stringValue) }
  init?(intValue: Int) { return nil }
}

private func kaigiJSONFieldsV1(
  _ decoder: Decoder, _ expected: Set<String>
) throws -> KeyedDecodingContainer<KaigiJSONKeyV1> {
  let container = try decoder.container(keyedBy: KaigiJSONKeyV1.self)
  guard Set(container.allKeys.map(\.stringValue)) == expected else {
    throw KaigiV1Error.invalidValue("Kaigi JSON has missing, unknown or retired fields.")
  }
  return container
}

private struct KaigiScalarJSONV1: Decodable {
  let value: KaigiAuthorizationScalarV1
  init(from decoder: Decoder) throws {
    var container = try decoder.unkeyedContainer()
    guard container.count == 32 else {
      throw KaigiV1Error.invalidValue("Kaigi scalar JSON must contain exactly 32 bytes.")
    }
    var bytes = Data()
    bytes.reserveCapacity(32)
    for _ in 0..<32 { bytes.append(try container.decode(UInt8.self)) }
    guard container.isAtEnd else { throw KaigiV1Error.invalidValue("Trailing Kaigi scalar bytes.") }
    value = try KaigiAuthorizationScalarV1(bytes: bytes)
  }
}

private struct KaigiCommitmentJSONV1: Decodable {
  let value: KaigiParticipantCommitmentV1
  init(from decoder: Decoder) throws {
    let c = try kaigiJSONFieldsV1(decoder, ["commitment"])
    value = KaigiParticipantCommitmentV1(
      commitment: try c.decode(KaigiScalarJSONV1.self, forKey: .init("commitment")).value)
  }
}

private struct KaigiNullifierJSONV1: Decodable {
  let value: KaigiParticipantNullifierV1
  init(from decoder: Decoder) throws {
    let c = try kaigiJSONFieldsV1(decoder, ["digest"])
    value = KaigiParticipantNullifierV1(
      digest: try c.decode(KaigiScalarJSONV1.self, forKey: .init("digest")).value)
  }
}

private struct KaigiParticipationJSONV1: Decodable {
  let value: KaigiPrivateParticipationV1
  init(from decoder: Decoder) throws {
    let c = try kaigiJSONFieldsV1(decoder, ["original_account", "sequence", "active_commitment"])
    let account = try kaigiCanonicalAccountID(
      c.decode(String.self, forKey: .init("original_account")), field: "original_account")
    let sequence = try c.decode(UInt64.self, forKey: .init("sequence"))
    let active = try c.decodeIfPresent(KaigiScalarJSONV1.self, forKey: .init("active_commitment"))?.value
    guard sequence > 0, sequence != UInt64.max || active == nil else {
      throw KaigiV1Error.invalidValue("Invalid retained Kaigi participation sequence.")
    }
    value = KaigiPrivateParticipationV1(
      originalAccount: account, sequence: sequence, activeCommitment: active)
  }
}

private struct KaigiParticipationLedgerJSONV1: Decodable {
  let entries: [KaigiPrivateParticipationV1]
  init(from decoder: Decoder) throws {
    let c = try kaigiJSONFieldsV1(decoder, ["entries"])
    var list = try c.nestedUnkeyedContainer(forKey: .init("entries"))
    guard let count = list.count, count <= Int(NewKaigiV1.maxParticipantsV1) else {
      throw KaigiV1Error.invalidValue("Kaigi retained subject count exceeds V1 bounds.")
    }
    var values: [KaigiPrivateParticipationV1] = []
    var accounts = Set<Data>()
    var commitments = Set<KaigiAuthorizationScalarV1>()
    while !list.isAtEnd {
      let item = try list.decode(KaigiParticipationJSONV1.self).value
      let identity = try CanonicalNorito.encodeCompactAccountId(item.originalAccount)
      guard accounts.insert(identity).inserted,
        item.activeCommitment.map({ commitments.insert($0).inserted }) ?? true
      else { throw KaigiV1Error.invalidValue("Duplicate Kaigi subject or active commitment.") }
      values.append(item)
    }
    entries = values
  }
}

private struct KaigiRecordProjectionJSONV1: Decodable {
  let value: KaigiPrivacyStateV1
  init(from decoder: Decoder) throws {
    let c = try kaigiJSONFieldsV1(decoder, [
      "id", "host", "billing_account", "title", "description", "max_participants",
      "gas_rate_per_minute", "metadata", "scheduled_start_ms", "privacy_mode", "room_policy",
      "relay_manifest", "host_commitment", "roster_root", "roster_commitments",
      "private_participation", "nullifier_log", "usage_commitments", "status", "created_at_ms",
      "ended_at_ms", "total_duration_ms", "total_billed_gas", "segments_recorded", "participants",
      "participant_metadata",
    ])
    let id = try c.superDecoder(forKey: .init("id"))
    let fields = try kaigiJSONFieldsV1(id, ["domain_id", "call_name"])
    let callID = try KaigiIdV1(
      domainID: fields.decode(String.self, forKey: .init("domain_id")),
      callName: fields.decode(String.self, forKey: .init("call_name")))
    let host = try kaigiCanonicalAccountID(c.decode(String.self, forKey: .init("host")), field: "host")
    let mode = try kaigiJSONFieldsV1(c.superDecoder(forKey: .init("privacy_mode")), ["mode", "state"])
    guard try mode.decodeNil(forKey: .init("state")) else {
      throw KaigiV1Error.invalidValue("Kaigi privacy mode must have unit state.")
    }
    let privacyMode: KaigiPrivacyModeV1
    switch try mode.decode(String.self, forKey: .init("mode")) {
    case "Transparent": privacyMode = .transparent
    case "ZkRosterV1": privacyMode = .zkRosterV1
    default: throw KaigiV1Error.invalidValue("Unknown Kaigi privacy mode.")
    }
    let ledger = try c.decode(KaigiParticipationLedgerJSONV1.self, forKey: .init("private_participation"))
    let roster = try c.decode([KaigiCommitmentJSONV1].self, forKey: .init("roster_commitments")).map(\.value)
    let active = Set(ledger.entries.compactMap(\.activeCommitment))
    guard roster.count <= Int(NewKaigiV1.maxParticipantsV1),
      Set(roster.map(\.commitment)).count == roster.count,
      active == Set(roster.map(\.commitment)) else {
      throw KaigiV1Error.invalidValue("Kaigi roster and retained ownership differ.")
    }
    value = KaigiPrivacyStateV1(
      callID: callID, originalHost: host, privacyMode: privacyMode,
      hostCommitment: try c.decodeIfPresent(KaigiCommitmentJSONV1.self, forKey: .init("host_commitment"))?.value,
      rosterRoot: try kaigiRosterHashLiteralV1(c.decode(String.self, forKey: .init("roster_root"))),
      rosterCommitments: roster, privateParticipation: ledger.entries,
      nullifierLog: try c.decode([KaigiNullifierJSONV1].self, forKey: .init("nullifier_log")).map(\.value),
      usageCommitments: try c.decode([KaigiScalarJSONV1].self, forKey: .init("usage_commitments")).map(\.value),
      segmentsRecorded: try c.decode(UInt32.self, forKey: .init("segments_recorded")),
      totalDurationMs: try c.decode(UInt64.self, forKey: .init("total_duration_ms")),
      totalBilledGas: try c.decode(UInt64.self, forKey: .init("total_billed_gas")))
  }
}

private func kaigiRosterHashLiteralV1(_ literal: String) throws -> KaigiHashV1 {
  let bytes = Array(literal.utf8)
  let upperHex: (UInt8) -> Bool = { (48...57).contains($0) || (65...70).contains($0) }
  guard bytes.count == 74, Array(bytes.prefix(5)) == Array("hash:".utf8), bytes[69] == 35,
    bytes[5..<69].allSatisfy(upperHex), bytes[70..<74].allSatisfy(upperHex),
    let checksum = UInt16(String(decoding: bytes[70..<74], as: UTF8.self), radix: 16),
    let hash = Data(hexString: String(decoding: bytes[5..<69], as: UTF8.self))
  else { throw KaigiV1Error.invalidValue("Invalid canonical Kaigi roster hash literal.") }
  var crc: UInt16 = 0xffff
  for byte in bytes[..<69] {
    crc ^= UInt16(byte) << 8
    for _ in 0..<8 { crc = crc & 0x8000 != 0 ? (crc << 1) ^ 0x1021 : crc << 1 }
  }
  guard crc == checksum else { throw KaigiV1Error.invalidValue("Invalid Kaigi roster hash checksum.") }
  return try KaigiHashV1(bytes: hash)
}

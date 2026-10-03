// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import CryptoKit
import Foundation

/// Bounded transport shaping/correlation only. All authenticated issuer authority is
/// admitted by the existing genuine Native C21/E/FI holders after these checks.
enum OrdinaryEnrollmentWire {
  static func json(_ fields: [String: Any]) throws -> Data {
    let data = try JSONSerialization.data(withJSONObject: fields, options: [.sortedKeys])
    guard (1...262144).contains(data.count) else { throw invalid() }
    return data
  }
  static func hex(_ bytes: Data) -> String { bytes.map { String(format: "%02x", $0) }.joined() }
  static func bytes(_ hex: String) throws -> Data {
    guard hex.count == 64, hex.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }) else { throw invalid() }
    let chars = Array(hex.utf8)
    func digit(_ c: UInt8) -> UInt8 { c <= 57 ? c - 48 : c - 87 }
    return Data(stride(from: 0, to: 64, by: 2).map { digit(chars[$0]) * 16 + digit(chars[$0 + 1]) })
  }
  static func prepare(_ reservation: KagemushaNativeReservedOrdinaryAppIdentityV1) throws -> Data {
    try json(["account_id": reservation.accountID(), "client_nonce_hex": hex(reservation.clientNonce()),
      "release_id_hex": hex(reservation.releaseID()), "profile_id_hex": hex(reservation.hardwareProfileID()),
      "lane_id_hex": hex(reservation.laneID()), "financial_authority_commitment_hex": hex(reservation.financialAuthorityCommitment())])
  }
  static func preparation(_ reply: Data) throws -> Data {
    let p = try object(reply)
    let original = try base64(string(p, "signed_preparation_base64"), maximum: 515, exact: 515)
    let challenge = try KagemushaOrdinaryAppIdentityPreparedProjectionV1.challenge(transport: original)
    let digest = Data(SHA256.hash(data: challenge.canonicalSigningBytes))
    guard try string(p, "operation_id") == hex(digest),
      try base64(string(p, "attestation_challenge_base64"), maximum: 32, exact: 32) == digest,
      try expiry(reply) == challenge.expiresAtMS else { throw invalid() }
    return original
  }
  static func raw(preparation: Data, collected: KagemushaNativeCollectedAppIdentityOriginalV1) throws -> Data {
    let challenge = try KagemushaOrdinaryAppIdentityPreparedProjectionV1.challenge(transport: preparation)
    return try json(["schema": "iroha.kagemusha.ordinary-app-raw-admission-request.v1", "operation": "issue",
      "operation_id": hex(Data(SHA256.hash(data: challenge.canonicalSigningBytes))),
      "signed_preparation_base64": preparation.base64EncodedString(),
      "attested_public_key_sec1_base64": collected.publicKeyX963.base64EncodedString(),
      "raw_attestation_base64": collected.rawAttestation.base64EncodedString()])
  }
  static func certificate(preparation: Data, admitted: KagemushaNativeRawAppIdentityAdmissionV1,
    consumed: KagemushaNativeConsumedAppEnrollmentPossessionOriginalV1) throws -> Data {
    let challenge = try KagemushaOrdinaryAppIdentityPreparedProjectionV1.challenge(transport: preparation)
    let id = Data(SHA256.hash(data: challenge.canonicalSigningBytes))
    guard consumed.receipt.enrollmentChallengeHash == id, consumed.receipt.keyAlias == admitted.keyReference else { throw invalid() }
    return try json(["schema": "iroha.kagemusha.ordinary-app-credential-request.v1", "operation": "issue",
      "operation_id": hex(id), "signed_preparation_base64": preparation.base64EncodedString(),
      "attested_public_key_sec1_base64": admitted.publicKeyX963.base64EncodedString(),
      "raw_attestation_base64": admitted.rawAttestation.base64EncodedString(),
      "app_possession": ["platform": "apple_app_attest", "raw_assertion_base64": consumed.rawAssertion.base64EncodedString()],
      "play_integrity_token": NSNull()])
  }
  static func original(_ reply: Data, field: String, digestField: String, exact: Int?, maximum: Int) throws -> Data {
    let p = try object(reply)
    let original = try base64(string(p, field), maximum: maximum, exact: exact)
    guard try string(p, digestField) == hex(Data(SHA256.hash(data: original))) else { throw invalid() }
    return original
  }
  /// Strict DATA correlation of the complete Model-owned Start original.
  /// Native supplies all seven fields; this checker never creates missing evidence.
  static func requireRetailStartOriginal(_ raw: Data, signedPreparation: Data,
    credential: Data) throws -> Data {
    _ = try KagemushaOrdinaryAppIdentityPreparedProjectionV1.challenge(transport: signedPreparation)
    guard (1...16384).contains(credential.count), (1...262144).contains(raw.count),
      let text = String(data: raw, encoding: .utf8), Data(text.utf8) == raw else { throw invalid() }
    try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(in: raw)
    guard let fields = try JSONSerialization.jsonObject(with: raw) as? [String: Any],
      Set(fields.keys) == Set(["wallet", "signed_preparation_base64", "raw_admission_original_base64",
        "platform_original_base64", "core_possession_original_base64", "app_certificate_base64", "selected_integrity"]),
      fields["selected_integrity"] is NSNull else { throw invalid() }
    let wallet = try string(fields, "wallet")
    guard (1...4096).contains(wallet.utf8.count), wallet.utf8.allSatisfy({ (0x21...0x7e).contains($0) }) else {
      throw invalid()
    }
    // Canonical account/C binding is authenticated by the genuine Model-backed exporter.
    // Swift's detached decoder preserves text; it cannot select an admitted account.
    guard try startBase64(string(fields, "signed_preparation_base64"), maximum: 515, exact: 515) == signedPreparation,
      try startBase64(string(fields, "app_certificate_base64"), maximum: 16384, exact: nil) == credential else {
      throw invalid()
    }
    _ = try startBase64(string(fields, "raw_admission_original_base64"), maximum: 314, exact: 314)
    _ = try startBase64(string(fields, "platform_original_base64"), maximum: 131072, exact: nil)
    _ = try startBase64(string(fields, "core_possession_original_base64"), maximum: 5120, exact: nil)
    return Data(raw)
  }

  /// Join exact Native phase15 originals; hashes and public metadata cannot fill gaps.
  static func retailStartOriginalChunks(_ chunks: [[Data]], signedPreparation: Data,
    credential: Data, pendingScope: Data, credentialDigest: Data, nativeTicket: Data) throws -> Data {
    guard (1...4).contains(chunks.count), let first = chunks.first,
      KagemushaAppPlatformPreparedProjectionV1.digest(pendingScope),
      KagemushaAppPlatformPreparedProjectionV1.digest(credentialDigest) else { throw invalid() }
    for (index, fields) in chunks.enumerated() {
      let request = [KagemushaCoreCoordinatorFrameV1.u32(15), nativeTicket,
        KagemushaCoreCoordinatorFrameV1.u32(UInt32(index))]
      try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession, request, fields)
      guard fields[2] == first[2], fields[3] == first[3],
        fields[4] == pendingScope, fields[5] == credentialDigest else { throw invalid() }
    }
    let total = Int(KagemushaAppPlatformPreparedProjectionV1.u32(first[3]))
    guard chunks.count == (total + 65535) / 65536 else { throw invalid() }
    var body = Data(); body.reserveCapacity(total)
    for fields in chunks { body.append(fields[1]) }
    guard body.count == total, Data(SHA256.hash(data: body)) == first[2] else { throw invalid() }
    // credentialDigest is the actual Model domain digest returned by Native phase8.
    // It is deliberately not replaced by SHA256(credential transport bytes).
    return try requireRetailStartOriginal(body, signedPreparation: signedPreparation, credential: credential)
  }

  private static func startBase64(_ value: String, maximum: Int, exact: Int?) throws -> Data {
    guard value.utf8.count <= ((maximum + 2) / 3) * 4 else { throw invalid() }
    return try base64(value, maximum: maximum, exact: exact)
  }
  static func retailChallenge(_ reply: Data, operation: Data) throws -> (challenge: Data, message: Data) {
    guard operation.count == 32 else { throw invalid() }
    let p = try object(reply)
    guard try string(p, "challenge_id") == hex(operation), try expiry(reply) > 0 else { throw invalid() }
    let challenge = try base64(string(p, "canonical_challenge_base64"), maximum: 32768, exact: nil)
    let message = try base64(string(p, "account_signing_message_base64"), maximum: 32, exact: 32)
    guard message.contains(where: { $0 != 0 }) else { throw invalid() }
    return (challenge, message)
  }
  static func retailCertificate(_ reply: Data, operation: Data) throws -> (certificate: Data, enrollmentID: Data) {
    guard operation.count == 32 else { throw invalid() }
    let p = try object(reply)
    guard try string(p, "challenge_id") == hex(operation) else { throw invalid() }
    let enrollmentID = try bytes(string(p, "enrollment_id_hex"))
    guard enrollmentID.contains(where: { $0 != 0 }) else { throw invalid() }
    return try (base64(string(p, "canonical_certificate_base64"), maximum: 16384, exact: nil), enrollmentID)
  }
  private static func object(_ reply: Data) throws -> [String: Any] {
    guard (1...524288).contains(reply.count), let object = try JSONSerialization.jsonObject(with: reply) as? [String: Any] else { throw invalid() }
    return object
  }
  private static func string(_ p: [String: Any], _ field: String) throws -> String {
    guard let value = p[field] as? String else { throw invalid() }; return value
  }
  private static func expiry(_ originalReply: Data) throws -> UInt64 {
    // Decode directly from the original response, preserving UInt64 width rather
    // than coercing the JSON through NSNumber/Double. Native authenticates time.
    struct Reply: Decodable {
      let expiresAtMS: UInt64
      enum CodingKeys: String, CodingKey { case expiresAtMS = "expires_at_ms" }
    }
    return try JSONDecoder().decode(Reply.self, from: originalReply).expiresAtMS
  }
  private static func base64(_ value: String, maximum: Int, exact: Int?) throws -> Data {
    guard let bytes = Data(base64Encoded: value), bytes.base64EncodedString() == value,
      (1...maximum).contains(bytes.count), exact == nil || bytes.count == exact else { throw invalid() }
    return bytes
  }
  private static func invalid() -> KagemushaCoreCoordinatorErrorV1 { .invalidFrame("unverified ordinary FI transport original differs") }
}

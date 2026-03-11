import Foundation
import CryptoKit
#if canImport(IrohaSwift)
import IrohaSwift
#endif

struct PosProvisionManifest: Decodable {
  let manifestId: String
  let sequence: Int
  let publishedAtMs: UInt64
  let validFromMs: UInt64
  let validUntilMs: UInt64
  let rotationHintMs: UInt64?
  let operatorId: String
  let backendRoots: [PosBackendRoot]
  let metadata: [String: String]?
  let payloadBase64: String
  let operatorSignature: String?

  enum CodingKeys: String, CodingKey {
    case manifestId = "manifest_id"
    case sequence
    case publishedAtMs = "published_at_ms"
    case validFromMs = "valid_from_ms"
    case validUntilMs = "valid_until_ms"
    case rotationHintMs = "rotation_hint_ms"
    case operatorId = "operator"
    case backendRoots = "backend_roots"
    case metadata
    case payloadBase64 = "payload_base64"
    case operatorSignature = "operator_signature"
  }
}

struct PosBackendRoot: Decodable {
  let label: String
  let role: String
  let publicKey: String
  let validFromMs: UInt64
  let validUntilMs: UInt64
  let metadata: [String: String]?

  enum CodingKeys: String, CodingKey {
    case label
    case role
    case publicKey = "public_key"
    case validFromMs = "valid_from_ms"
    case validUntilMs = "valid_until_ms"
    case metadata
  }
}

struct PosManifestStatus: Identifiable {
  struct BackendRootStatus: Identifiable {
    var id: String { label }
    let label: String
    let role: String
    let statusLabel: String
    let active: Bool
  }

  var id: String { manifestId }
  let manifestId: String
  let sequence: Int
  let operatorId: String
  let validWindowLabel: String
  let rotationLabel: String
  let dualStatusLabel: String
  let dualStatusHealthy: Bool
  let warnings: [String]
  let backendRoots: [BackendRootStatus]

  static func from(manifest: PosProvisionManifest, now: Date = Date()) -> PosManifestStatus {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    let validWindow = "\(formatter.string(from: manifest.validFromMs.date)) – \(formatter.string(from: manifest.validUntilMs.date))"
    let rotationLabel = manifest.rotationHintMs.map { formatter.string(from: $0.date) } ?? "n/a"
    let backendStatuses = manifest.backendRoots.map { root -> BackendRootStatus in
      let active = now >= root.validFromMs.date && now <= root.validUntilMs.date
      let statusText = backendStatusLabel(root: root, active: active, formatter: formatter, now: now)
      return BackendRootStatus(label: root.label, role: root.role, statusLabel: statusText, active: active)
    }
    let dualStatus = computeDualStatus(roots: backendStatuses, manifest: manifest, now: now)
    let warnings = computeWarnings(manifest: manifest, roots: backendStatuses, dualStatus: dualStatus, now: now, formatter: formatter)
    return PosManifestStatus(
      manifestId: manifest.manifestId,
      sequence: manifest.sequence,
      operatorId: manifest.operatorId,
      validWindowLabel: validWindow,
      rotationLabel: rotationLabel,
      dualStatusLabel: dualStatus.label,
      dualStatusHealthy: dualStatus.healthy,
      warnings: warnings,
      backendRoots: backendStatuses
    )
  }

  static func unavailable(message: String) -> PosManifestStatus {
    return PosManifestStatus(
      manifestId: "unavailable",
      sequence: -1,
      operatorId: "n/a",
      validWindowLabel: "n/a",
      rotationLabel: "n/a",
      dualStatusLabel: message,
      dualStatusHealthy: false,
      warnings: [message],
      backendRoots: []
    )
  }

  private struct DualStatus {
    let healthy: Bool
    let label: String
  }

  private static func backendStatusLabel(root: PosBackendRoot, active: Bool, formatter: ISO8601DateFormatter, now: Date) -> String {
    let expiresIn = root.validUntilMs.date.timeIntervalSince(now)
    let expiresLabel: String
    if expiresIn > 0 {
      expiresLabel = "expires \(formatDuration(milliseconds: expiresIn * 1000))"
    } else {
      expiresLabel = "expired \(formatter.string(from: root.validUntilMs.date))"
    }
    let window = "\(formatter.string(from: root.validFromMs.date)) – \(formatter.string(from: root.validUntilMs.date))"
    return "\(active ? "active" : "inactive") · \(window) (\(expiresLabel))"
  }

  private static func computeDualStatus(roots: [BackendRootStatus], manifest: PosProvisionManifest, now: Date) -> DualStatus {
    let activeRoles = Set(roots.filter(\.active).map { $0.role })
    let required: Set<String> = ["offline_admission_signer", "offline_allowance_witness"]
    let missing = required.subtracting(activeRoles)
    if missing.isEmpty {
      let remaining = manifest.validUntilMs.date.timeIntervalSince(now)
      return DualStatus(healthy: true, label: "admission+witness for \(formatDuration(milliseconds: remaining * 1000))")
    }
    return DualStatus(healthy: false, label: "missing \(missing.joined(separator: \", \"))")
  }

  private static func computeWarnings(
    manifest: PosProvisionManifest,
    roots: [BackendRootStatus],
    dualStatus: DualStatus,
    now: Date,
    formatter: ISO8601DateFormatter
  ) -> [String] {
    var warnings: [String] = []
    if now < manifest.validFromMs.date {
      warnings.append("manifest not active until \(formatter.string(from: manifest.validFromMs.date))")
    }
    let manifestExpires = manifest.validUntilMs.date.timeIntervalSince(now)
    if manifestExpires <= PosManifestLoader.manifestWarningWindow {
      warnings.append("manifest expires in \(formatDuration(milliseconds: manifestExpires * 1000))")
    }
    if let hint = manifest.rotationHintMs {
      let delta = hint.date.timeIntervalSince(now)
      if delta <= 0 {
        warnings.append("rotation hint passed \(formatDuration(milliseconds: -delta * 1000)) ago")
      } else if delta <= PosManifestLoader.rotationWarningWindow {
        warnings.append("rotation hint in \(formatDuration(milliseconds: delta * 1000))")
      }
    }
    roots.filter { !$0.active }.forEach { root in
      warnings.append("\(root.label) inactive")
    }
    if !dualStatus.healthy {
      warnings.append(dualStatus.label)
    }
    return warnings
  }

  private static func formatDuration(milliseconds: Double) -> String {
    if milliseconds <= 0 {
      return "0s"
    }
    var seconds = Int(milliseconds / 1000)
    let days = seconds / (24 * 3600)
    seconds %= 24 * 3600
    let hours = seconds / 3600
    seconds %= 3600
    let minutes = seconds / 60
    let parts = [
      days > 0 ? "\(days)d" : nil,
      hours > 0 ? "\(hours)h" : nil,
      minutes > 0 ? "\(minutes)m" : nil
    ].compactMap { $0 }
    if parts.isEmpty {
      return "\(Int(milliseconds / 1000))s"
    }
    return parts.joined(separator: " ")
  }
}

enum PosManifestLoader {
  static let manifestWarningWindow: TimeInterval = 7 * 24 * 3600
  static let rotationWarningWindow: TimeInterval = 3 * 24 * 3600

  static func loadManifest(bundle: Bundle = .main) throws -> PosProvisionManifest {
    guard let url = bundle.url(forResource: "pos_manifest", withExtension: "json") else {
      throw NSError(domain: "PosManifestLoader", code: 1, userInfo: [NSLocalizedDescriptionKey: "pos_manifest.json missing from bundle"])
    }
    let data = try Data(contentsOf: url)
    return try parse(data: data)
  }

  static func parse(data: Data) throws -> PosProvisionManifest {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .useDefaultKeys
    let manifest = try decoder.decode(PosProvisionManifest.self, from: data)
    try verifySignature(manifest: manifest)
    return manifest
  }

  private static func verifySignature(manifest: PosProvisionManifest) throws {
    guard let operatorSignature = manifest.operatorSignature,
          let signature = Data(hexString: operatorSignature) else {
      throw NSError(
        domain: "PosManifestLoader",
        code: 2,
        userInfo: [NSLocalizedDescriptionKey: "manifest missing operator signature"]
      )
    }
    guard let payload = Data(base64Encoded: manifest.payloadBase64) else {
      throw NSError(
        domain: "PosManifestLoader",
        code: 3,
        userInfo: [NSLocalizedDescriptionKey: "manifest payload_base64 invalid"]
      )
    }
    let operatorKey = try decodeOperatorPublicKey(manifest.operatorId)
    let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: operatorKey)
    guard publicKey.isValidSignature(signature, for: payload) else {
      throw NSError(
        domain: "PosManifestLoader",
        code: 4,
        userInfo: [NSLocalizedDescriptionKey: "manifest signature verification failed"]
      )
    }
  }

  private static func decodeOperatorPublicKey(_ operatorId: String) throws -> Data {
    let address = operatorId.split(separator: "@", maxSplits: 1, omittingEmptySubsequences: false).first.map(String.init) ?? operatorId
    let normalized = address.trimmingCharacters(in: .whitespacesAndNewlines)
    if normalized.lowercased().hasPrefix("ed01") {
      guard let raw = Data(hexString: normalized) else {
        throw NSError(domain: "PosManifestLoader", code: 5, userInfo: [NSLocalizedDescriptionKey: "invalid operator key encoding"])
      }
      guard raw.count > 3, raw[0] == 0xED, raw[1] == 0x01 else {
        throw NSError(domain: "PosManifestLoader", code: 6, userInfo: [NSLocalizedDescriptionKey: "unsupported operator key encoding"])
      }
      let declaredLen = Int(raw[2])
      guard declaredLen == raw.count - 3 else {
        throw NSError(domain: "PosManifestLoader", code: 7, userInfo: [NSLocalizedDescriptionKey: "unexpected operator key length"])
      }
      return Data(raw.dropFirst(3))
    }
#if canImport(IrohaSwift)
    let account = try AccountAddress.fromI105(normalized, expectedPrefix: nil)
    let canonical = try account.canonicalBytes()
    return try extractSingleSignatoryKey(canonical)
#else
    throw NSError(domain: "PosManifestLoader", code: 8, userInfo: [NSLocalizedDescriptionKey: "I105 decoding requires IrohaSwift"])
#endif
  }

#if canImport(IrohaSwift)
  private static func extractSingleSignatoryKey(_ canonical: Data) throws -> Data {
    guard canonical.count >= 4 else {
      throw NSError(domain: "PosManifestLoader", code: 9, userInfo: [NSLocalizedDescriptionKey: "invalid canonical address length"])
    }
    var cursor = canonical.startIndex
    let header = canonical[cursor]
    cursor += 1
    let extensionFlag = header & 0x01
    let classBits = (header >> 3) & 0x03
    guard extensionFlag == 0 else {
      throw NSError(domain: "PosManifestLoader", code: 10, userInfo: [NSLocalizedDescriptionKey: "address extension flag set"])
    }
    guard classBits == 0 || classBits == 1 else {
      throw NSError(domain: "PosManifestLoader", code: 11, userInfo: [NSLocalizedDescriptionKey: "unknown address class"])
    }
    guard cursor < canonical.endIndex else {
      throw NSError(domain: "PosManifestLoader", code: 12, userInfo: [NSLocalizedDescriptionKey: "invalid canonical address length"])
    }
    let domainTag = canonical[cursor]
    cursor += 1
    switch domainTag {
    case 0x00:
      break
    case 0x01:
      cursor += 12
    case 0x02:
      cursor += 4
    default:
      throw NSError(domain: "PosManifestLoader", code: 13, userInfo: [NSLocalizedDescriptionKey: "unknown domain tag \(domainTag)"])
    }
    guard cursor < canonical.endIndex else {
      throw NSError(domain: "PosManifestLoader", code: 14, userInfo: [NSLocalizedDescriptionKey: "invalid canonical address length"])
    }
    let controllerTag = canonical[cursor]
    cursor += 1
    guard controllerTag == 0x00 else {
      throw NSError(domain: "PosManifestLoader", code: 15, userInfo: [NSLocalizedDescriptionKey: "unsupported controller tag \(controllerTag)"])
    }
    guard cursor + 2 <= canonical.endIndex else {
      throw NSError(domain: "PosManifestLoader", code: 16, userInfo: [NSLocalizedDescriptionKey: "invalid canonical address length"])
    }
    let curveId = canonical[cursor]
    cursor += 1
    guard curveId == 0x01 else {
      throw NSError(domain: "PosManifestLoader", code: 17, userInfo: [NSLocalizedDescriptionKey: "unsupported signing algorithm \(curveId)"])
    }
    let keyLen = Int(canonical[cursor])
    cursor += 1
    let end = cursor + keyLen
    guard end == canonical.count else {
      throw NSError(domain: "PosManifestLoader", code: 18, userInfo: [NSLocalizedDescriptionKey: "unexpected trailing bytes in address payload"])
    }
    return Data(canonical[cursor..<end])
  }
#endif
}
}

private extension UInt64 {
  var date: Date { Date(timeIntervalSince1970: TimeInterval(self) / 1000.0) }
}

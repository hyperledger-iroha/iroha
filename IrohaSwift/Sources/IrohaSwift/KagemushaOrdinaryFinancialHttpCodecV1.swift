import CryptoKit
import Foundation

/// Fixed service envelopes only. Full signature, currentness, finality and proof admission is Native.
enum KagemushaOrdinaryFinancialHttpCodecV1 {
  enum Kind: Equatable, Sendable { case currentControl, lineage }
  static func maximumResponse(_ kind: Kind) -> Int {
    let maxima = kind == .currentControl ? [65_536, 134_217_728] : [32_768, 131_072, 134_217_728]
    return maxima.reduce(1024) { $0 + (($1 + 2) / 3) * 4 }
  }
  static func requestBody(_ kind: Kind, request: Data, signature: Data, proof: Data = Data()) throws -> Data {
    guard (1...(kind == .currentControl ? 8192 : 196_608)).contains(request.count), signature.count == 64,
      kind == .currentControl ? proof.isEmpty : (1...67_108_864).contains(proof.count) else {
      throw invalid("financial HTTP original request differs")
    }
    var object = ["schema": kind == .currentControl ?
      "iroha.kagemusha.ordinary-current-fi-control-request.v1" : "iroha.kagemusha.ordinary-lineage-cas-request.v1",
      "canonical_request_base64": request.base64EncodedString(),
      "account_signature_base64": signature.base64EncodedString()]
    if kind == .lineage { object["proof_bundle_original_base64"] = proof.base64EncodedString() }
    return try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
  }
  static func requestID(_ kind: Kind, request: Data) throws -> String {
    guard (1...(kind == .currentControl ? 8192 : 196_608)).contains(request.count) else {
      throw invalid("financial HTTP request ID has no bounded original")
    }
    let domain = kind == .currentControl ? "iroha:kagemusha:v1:ordinary-current-fi-control-http\0" :
      "iroha:kagemusha:v1:ordinary-lineage-http\0"
    var bytes = Array(SHA256.hash(data: Data(domain.utf8) + request).prefix(16))
    // Same maintained request-correlation UUID derivation as the shared Kotlin codec.
    bytes[6] = (bytes[6] & 0x0f) | 0x40
    bytes[8] = (bytes[8] & 0x3f) | 0x80
    return UUID(uuid: (bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
      bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15])).uuidString.lowercased()
  }
  static func responseOriginals(_ kind: Kind, raw: Data) throws -> [Data] {
    guard (1...maximumResponse(kind)).contains(raw.count), let text = String(data: raw, encoding: .utf8),
      Data(text.utf8) == raw else { throw invalid("financial HTTP original response exceeds bounds") }
    try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(in: raw)
    guard let object = try JSONSerialization.jsonObject(with: raw) as? [String: Any] else {
      throw invalid("financial HTTP response must be an object")
    }
    let keys = kind == .currentControl ? ["signed_control_original_base64", "authority_original_base64"] :
      ["signed_result_original_base64", "data_record_original_base64", "authority_original_base64"]
    let limits = kind == .currentControl ? [65_536, 134_217_728] : [32_768, 131_072, 134_217_728]
    guard Set(object.keys) == Set(keys) else { throw invalid("financial HTTP original keys differ") }
    return try zip(keys, limits).map { key, limit in
      guard let encoded = object[key] as? String, encoded.utf8.count <= ((limit + 2) / 3) * 4,
        let original = Data(base64Encoded: encoded), (1...limit).contains(original.count),
        original.base64EncodedString() == encoded else {
        throw invalid("financial HTTP original is not exact canonical Base64")
      }
      return original
    }
  }
  private static func invalid(_ message: String) -> KagemushaCoreCoordinatorErrorV1 { .invalidFrame(message) }
}

/// Product transport must authenticate its genuine session and bound the response while reading.
/// Public construction cannot supply a signing subject, account signature or service proof.
public protocol KagemushaOrdinaryCurrentControlOriginalTransportV1: Sendable {
  func exchange(_ original: KagemushaOrdinaryCurrentControlHttpOriginalV1) async throws -> Data
}
public protocol KagemushaOrdinaryLineageOriginalTransportV1: Sendable {
  func exchange(_ original: KagemushaOrdinaryLineageHttpOriginalV1) async throws -> Data
}

public final class KagemushaOrdinaryCurrentControlHttpOriginalV1: @unchecked Sendable {
  public let path = "/v1/kagemusha/enrollment/ordinary/current-control"
  public let maximumResponseBytes = KagemushaOrdinaryFinancialHttpCodecV1.maximumResponse(.currentControl)
  public let requestID: String
  private let originalBody: Data
  private let requireOriginal: @Sendable () throws -> Void
  init(request: Data, signature: Data, requireOriginal: @escaping @Sendable () throws -> Void) throws {
    requestID = try KagemushaOrdinaryFinancialHttpCodecV1.requestID(.currentControl, request: request)
    originalBody = try KagemushaOrdinaryFinancialHttpCodecV1.requestBody(.currentControl, request: request, signature: signature)
    self.requireOriginal = requireOriginal
  }
  public func requireCurrent() throws { try requireOriginal() }
  public func body() throws -> Data { try requireOriginal(); let body = Data(originalBody); try requireOriginal(); return body }
}
public final class KagemushaOrdinaryLineageHttpOriginalV1: @unchecked Sendable {
  public let path = "/v1/kagemusha/enrollment/ordinary/lineage-cas"
  public let maximumResponseBytes = KagemushaOrdinaryFinancialHttpCodecV1.maximumResponse(.lineage)
  public let requestID: String
  private let originalBody: Data
  private let requireOriginal: @Sendable () throws -> Void
  init(fields: [Data], requireOriginal: @escaping @Sendable () throws -> Void) throws {
    try KagemushaOrdinaryOutgoingFrameV1.requireResponse(6, fields)
    requestID = try KagemushaOrdinaryFinancialHttpCodecV1.requestID(.lineage, request: fields[1])
    originalBody = try KagemushaOrdinaryFinancialHttpCodecV1.requestBody(.lineage,
      request: fields[1], signature: fields[2], proof: fields[3])
    self.requireOriginal = requireOriginal
  }
  public func requireCurrent() throws { try requireOriginal() }
  public func body() throws -> Data { try requireOriginal(); let body = Data(originalBody); try requireOriginal(); return body }
}

/// Synchronous lifetime of one exact HTTP original across suspension. It grants no authority.
final class KagemushaOrdinaryHttpLifetimeV1: @unchecked Sendable {
  private let lock = NSLock()
  private var open = true
  private let session: KagemushaOrdinaryNativeAccountSessionV1
  private let binding: KagemushaOrdinaryNativeBindingV1
  init(session: KagemushaOrdinaryNativeAccountSessionV1, binding: KagemushaOrdinaryNativeBindingV1) {
    self.session = session; self.binding = binding
  }
  func requireCurrent() throws {
    lock.lock(); defer { lock.unlock() }
    guard open else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    try session.requireCurrent(); try binding.requireOpen(); try session.requireCurrent()
  }
  func retire() { lock.lock(); defer { lock.unlock() }; open = false }
}

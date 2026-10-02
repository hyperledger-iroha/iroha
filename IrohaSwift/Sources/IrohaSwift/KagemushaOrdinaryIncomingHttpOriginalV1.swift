import CryptoKit
import Foundation

/// Product transport streams exact Native-signed incoming CAS originals and bounds its response.
public protocol KagemushaOrdinaryIncomingOriginalTransportV1: Sendable {
  func exchange(_ original: KagemushaOrdinaryIncomingHttpOriginalV1) async throws -> Data
}

/// Closed-origin data carrier. HTTP retry reuses every original byte and request identity.
/// Constructing or sending this data cannot admit a global result or advance Native State.
public final class KagemushaOrdinaryIncomingHttpOriginalV1: @unchecked Sendable {
  public let path = "/v1/kagemusha/enrollment/ordinary/lineage-cas"
  public let maximumResponseBytes = KagemushaOrdinaryFinancialHttpCodecV1.maximumResponse(.lineage)
  public let requestID: String
  private let request: Data
  private let signature: Data
  private let proof: Data
  let requestOriginalDigest: Data
  private let lifetime: KagemushaOrdinaryIncomingHttpLifetimeV1
  init(fields: [Data], commit: Bool, lifetime: KagemushaOrdinaryIncomingHttpLifetimeV1) throws {
    let phase: KagemushaOrdinaryIncomingPhaseV1 = commit ? .commitTransport : .reserveTransport
    // The maintained canonical frame grammar authenticates structural byte counts/hash only.
    try KagemushaOrdinaryIncomingFrameV1.requireResponse(phase, fields: fields)
    guard fields[0] == Data([0]) else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("incoming HTTP original is already acknowledged") }
    request = Data(fields[1]); signature = Data(fields[2]); proof = Data(fields[3])
    requestOriginalDigest = Data(fields[4]); self.lifetime = lifetime
    var bytes = Array(SHA256.hash(data: Data("iroha:kagemusha:v1:ordinary-incoming-lineage-http\0".utf8) + request).prefix(16))
    bytes[6] = (bytes[6] & 0x0f) | 0x40; bytes[8] = (bytes[8] & 0x3f) | 0x80
    requestID = UUID(uuid: (bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
      bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15])).uuidString.lowercased()
  }
  public func requireCurrent() throws { try lifetime.requireCurrent() }
  /// Streams bounded Base64 chunks; it neither opens nor closes the caller-owned stream.
  public func writeBody(to output: OutputStream) throws {
    try requireCurrent()
    func write(_ data: Data) throws {
      try data.withUnsafeBytes { raw in
        guard let start = raw.bindMemory(to: UInt8.self).baseAddress else { return }
        var offset = 0
        while offset < data.count {
          try requireCurrent()
          let count = output.write(start.advanced(by: offset), maxLength: data.count - offset)
          guard count > 0, count <= data.count - offset else {
            throw output.streamError ?? KagemushaCoreCoordinatorErrorV1.invalidFrame("incoming HTTP output did not accept its original")
          }
          offset += count
        }
      }
    }
    func text(_ value: String) throws { try write(Data(value.utf8)) }
    func base64(_ data: Data) throws {
      // All nonfinal chunks are divisible by three, so concatenation is canonical Base64.
      let width = 49_152
      var offset = 0
      while offset < data.count {
        let end = min(offset + width, data.count)
        try write(data.subdata(in: offset..<end).base64EncodedData())
        offset = end
      }
    }
    try text("{\"schema\":\"iroha.kagemusha.ordinary-lineage-cas-request.v1\",\"canonical_request_base64\":\"")
    try base64(request); try text("\",\"account_signature_base64\":\"")
    try base64(signature); try text("\",\"proof_bundle_original_base64\":\"")
    try base64(proof); try text("\"}")
    try requireCurrent()
  }
}

/// Synchronous lifetime for the exact carrier across an async transport suspension.
final class KagemushaOrdinaryIncomingHttpLifetimeV1: @unchecked Sendable {
  private let lock = NSLock()
  private var open = true
  private let native: any KagemushaIncomingWorkflowNativeV1
  init(native: any KagemushaIncomingWorkflowNativeV1) { self.native = native }
  func requireCurrent() throws {
    lock.lock(); defer { lock.unlock() }
    guard open else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    try native.requireOpen()
  }
  func retire() { lock.lock(); defer { lock.unlock() }; open = false }
}

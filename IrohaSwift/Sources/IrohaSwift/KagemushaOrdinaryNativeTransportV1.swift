import Foundation
#if canImport(Darwin)
import Darwin
#endif

/// Exact first-release Norito transport. These parsers provide DATA, never authority.
enum KagemushaOrdinaryNativeArchiveV1 {
  static let outgoingRequest = "connect_norito_bridge::KagemushaOrdinaryNativeOutgoingRequestV1"
  static let outgoingResponse = "connect_norito_bridge::KagemushaOrdinaryNativeOutgoingResponseV1"
  static let currentResponse = "connect_norito_bridge::KagemushaOrdinaryNativeCurrentControlResponseV1"
  static let startupResponse = "connect_norito_bridge::KagemushaOrdinaryNativeStartupResponseV1"
  // These four Rust structs contain u64 and Vec fields. Their archived alignment is eight.
  static let archivedAlignment = 8
  static let maximumArchive = 128 * 1024 * 1024 + 192 * 1024 + 4096

  struct Response {
    let phase: UInt8
    let id: UInt64
    let fields: [Data]
  }

  static func encode(schema: String, phase: UInt8, id: UInt64, fields: [Data]) -> Data {
    var payload = prefixed(littleEndian(UInt16(1)))
    payload += prefixed(Data([phase]))
    payload += prefixed(littleEndian(id))
    var vector = littleEndian(UInt64(fields.count))
    for field in fields {
      vector += prefixed(littleEndian(UInt64(field.count)) + field)
    }
    payload += prefixed(vector)
    return noritoEncode(typeName: schema, payload: payload,
      flags: NoritoHeader.compactLen, payloadAlignment: archivedAlignment)
  }

  static func decode(_ raw: Data, schema: String, phase: UInt8, id: UInt64?) throws -> Response {
    guard (40...maximumArchive).contains(raw.count), let frame = noritoDecodeFrame(raw),
      frame.header.schema == noritoSchemaHash(forTypeName: schema),
      frame.header.flags == NoritoHeader.compactLen, frame.paddingLength == 0 else {
      throw invalid("noncanonical ordinary Native archive")
    }
    var reader = Reader(frame.payload)
    guard try reader.field() == littleEndian(UInt16(1)),
      try reader.field() == Data([phase]) else { throw invalid("ordinary Native version or phase differs") }
    let returnedID = try unsigned(try reader.field())
    if let id, returnedID != id { throw invalid("ordinary Native descriptor differs") }
    var vector = Reader(try reader.field())
    try reader.end()
    let count = try unsigned(vector.take(8))
    guard count <= 14 else { throw invalid("ordinary Native field census differs") }
    var fields = [Data]()
    for _ in 0..<Int(count) {
      var element = Reader(try vector.field())
      let size = try unsigned(element.take(8))
      guard size <= UInt64(maximumArchive) else { throw invalid("ordinary Native field exceeds bound") }
      fields.append(try element.take(Int(size)))
      try element.end()
    }
    try vector.end()
    guard encode(schema: schema, phase: phase, id: returnedID, fields: fields) == raw else {
      throw invalid("ordinary Native archive has alternate encoding")
    }
    return Response(phase: phase, id: returnedID, fields: fields)
  }

  static func littleEndian<T: FixedWidthInteger>(_ number: T) -> Data {
    var original = number.littleEndian
    return Swift.withUnsafeBytes(of: &original) { Data($0) }
  }
  static func unsigned(_ bytes: Data) throws -> UInt64 {
    guard bytes.count == 8 else { throw invalid("ordinary Native scalar width differs") }
    return bytes.enumerated().reduce(UInt64(0)) { $0 | UInt64($1.element) << ($1.offset * 8) }
  }
  static func invalid(_ message: String) -> KagemushaCoreCoordinatorErrorV1 { .invalidFrame(message) }

  private static func prefixed(_ field: Data) -> Data { compact(UInt64(field.count)) + field }
  private static func compact(_ value: UInt64) -> Data {
    var remaining = value, result = Data()
    repeat {
      let next = UInt8(remaining & 127)
      remaining >>= 7
      result.append(next | (remaining == 0 ? 0 : 128))
    } while remaining != 0
    return result
  }
  private struct Reader {
    let bytes: Data
    var offset = 0
    init(_ bytes: Data) { self.bytes = Data(bytes) }
    mutating func take(_ size: Int) throws -> Data {
      guard size >= 0, offset <= bytes.count, size <= bytes.count - offset else {
        throw KagemushaOrdinaryNativeArchiveV1.invalid("truncated ordinary Native archive")
      }
      defer { offset += size }
      return Data(bytes[offset..<(offset + size)])
    }
    mutating func field() throws -> Data {
      let begin = offset
      var value: UInt64 = 0, shift = 0
      while true {
        let byte = try take(1)[0]
        guard shift < 64, shift != 63 || byte <= 1 else {
          throw KagemushaOrdinaryNativeArchiveV1.invalid("ordinary Native length overflow")
        }
        value |= UInt64(byte & 127) << shift
        if byte & 128 == 0 { break }
        shift += 7
      }
      guard KagemushaOrdinaryNativeArchiveV1.compact(value) == Data(bytes[begin..<offset]),
        value <= UInt64(KagemushaOrdinaryNativeArchiveV1.maximumArchive) else {
        throw KagemushaOrdinaryNativeArchiveV1.invalid("noncanonical ordinary Native length")
      }
      return try take(Int(value))
    }
    func end() throws {
      guard offset == bytes.count else { throw KagemushaOrdinaryNativeArchiveV1.invalid("trailing ordinary Native bytes") }
    }
  }
}

/// Sole linked endpoint. Neither a public callback nor a deserialized object can supply it.
final class KagemushaOrdinaryNativeEndpointV1: @unchecked Sendable {
  #if canImport(Darwin)
  private typealias OutgoingFn = @convention(c) (UnsafePointer<UInt8>?, Int,
    UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<Int>?) -> Int32
  private typealias StartupFn = @convention(c) (UInt8, UInt64, UnsafePointer<UInt8>?, Int,
    UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<Int>?) -> Int32
  private typealias CurrentFn = @convention(c) (UInt8, UInt64, UnsafePointer<UInt8>?, Int,
    UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<Int>?) -> Int32
  private typealias FreeFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
  private let outgoingFunction: OutgoingFn
  private let startupFunction: StartupFn
  private let currentFunction: CurrentFn
  private let freeFunction: FreeFn
  private init(outgoing: @escaping OutgoingFn, startup: @escaping StartupFn,
    current: @escaping CurrentFn, free: @escaping FreeFn) {
    outgoingFunction = outgoing; startupFunction = startup
    currentFunction = current; freeFunction = free
  }
  #endif

  static func linked() throws -> KagemushaOrdinaryNativeEndpointV1 {
    #if canImport(Darwin)
    let (image, _) = NoritoBridgeLoader.openHandle()
    guard let image,
      let outgoing = dlsym(image, "connect_norito_kagemusha_ordinary_outgoing_v1"),
      let startup = dlsym(image, "connect_norito_kagemusha_ordinary_runtime_startup_v1"),
      let current = dlsym(image, "connect_norito_kagemusha_ordinary_current_control_v1"),
      let free = dlsym(image, "connect_norito_free") else {
      throw KagemushaCoreCoordinatorErrorV1.unavailable
    }
    return KagemushaOrdinaryNativeEndpointV1(outgoing: unsafeBitCast(outgoing, to: OutgoingFn.self),
      startup: unsafeBitCast(startup, to: StartupFn.self), current: unsafeBitCast(current, to: CurrentFn.self),
      free: unsafeBitCast(free, to: FreeFn.self))
    #else
    throw KagemushaCoreCoordinatorErrorV1.unavailable
    #endif
  }

  func outgoing(_ request: Data) throws -> Data {
    #if canImport(Darwin)
    return try output { pointer, length in
      request.withUnsafeBytes { outgoingFunction($0.bindMemory(to: UInt8.self).baseAddress, $0.count, pointer, length) }
    }
    #else
    throw KagemushaCoreCoordinatorErrorV1.unavailable
    #endif
  }
  func startup(phase: UInt8, id: UInt64) throws -> Data {
    #if canImport(Darwin)
    return try output { startupFunction(phase, id, nil, 0, $0, $1) }
    #else
    throw KagemushaCoreCoordinatorErrorV1.unavailable
    #endif
  }
  func current(phase: UInt8, handle: UInt64, signed: Data, authority: Data) throws -> Data {
    #if canImport(Darwin)
    return try output { pointer, length in
      signed.withUnsafeBytes { s in authority.withUnsafeBytes { a in
        currentFunction(phase, handle, s.bindMemory(to: UInt8.self).baseAddress, s.count,
          a.bindMemory(to: UInt8.self).baseAddress, a.count, pointer, length)
      } }
    }
    #else
    throw KagemushaCoreCoordinatorErrorV1.unavailable
    #endif
  }
  #if canImport(Darwin)
  private func output(_ action: (UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>,
    UnsafeMutablePointer<Int>) -> Int32) throws -> Data {
    var pointer: UnsafeMutablePointer<UInt8>?, length = 0
    let status = action(&pointer, &length)
    defer { if let pointer { freeFunction(UnsafeMutableRawPointer(pointer)) } }
    guard status == 0 else { throw KagemushaCoreCoordinatorErrorV1.nativeFailure(status) }
    guard let pointer, (40...KagemushaOrdinaryNativeArchiveV1.maximumArchive).contains(length) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("ordinary Native output exceeds bound")
    }
    return Data(bytes: pointer, count: length)
  }
  #endif
}

/// Retain the same bridge descriptor and close policy. No handle escapes the module.
final class KagemushaOrdinaryNativeBindingV1: @unchecked Sendable {
  private let bridge: KagemushaCoreCoordinatorBridgeV1
  private let endpoint: KagemushaOrdinaryNativeEndpointV1
  init(bridge: KagemushaCoreCoordinatorBridgeV1) throws {
    self.bridge = bridge; endpoint = try .linked()
  }
  func requireOpen() throws { try bridge.requireOrdinaryDescriptorOpen() }
  func revoke() throws { try bridge.close() }
  func outgoing(_ phase: UInt8, fields: [Data] = []) throws -> [Data] {
    try KagemushaOrdinaryOutgoingFrameV1.requireRequest(phase, fields)
    return try bridge.withOrdinaryDescriptor { handle in
      let raw = try endpoint.outgoing(KagemushaOrdinaryNativeArchiveV1.encode(
        schema: KagemushaOrdinaryNativeArchiveV1.outgoingRequest, phase: phase, id: handle, fields: fields))
      let response = try KagemushaOrdinaryNativeArchiveV1.decode(raw,
        schema: KagemushaOrdinaryNativeArchiveV1.outgoingResponse, phase: phase, id: handle)
      try KagemushaOrdinaryOutgoingFrameV1.requireResponse(phase, response.fields)
      return response.fields
    }
  }
  func current(_ phase: UInt8, signed: Data = Data(), authority: Data = Data()) throws -> [Data] {
    guard (phase == 1 || phase == 2) && signed.isEmpty && authority.isEmpty ||
      phase == 3 && (1...65_536).contains(signed.count) && (1...134_217_728).contains(authority.count) else {
      throw KagemushaOrdinaryNativeArchiveV1.invalid("ordinary current-control request differs")
    }
    return try bridge.withOrdinaryDescriptor { handle in
      let raw = try endpoint.current(phase: phase, handle: handle, signed: signed, authority: authority)
      let fields = try KagemushaOrdinaryNativeArchiveV1.decode(raw,
        schema: KagemushaOrdinaryNativeArchiveV1.currentResponse, phase: phase, id: handle).fields
      if phase == 3 {
        guard fields.isEmpty else { throw KagemushaOrdinaryNativeArchiveV1.invalid("current-control intake response differs") }
      } else {
        guard fields.count == 2, (1...8192).contains(fields[0].count) else {
          throw KagemushaOrdinaryNativeArchiveV1.invalid("current-control original response differs")
        }
        if phase == 1 {
          guard fields[1] == Data("iroha:kagemusha:v1:ordinary-current-fi-control-request\0".utf8) + fields[0] else {
            throw KagemushaOrdinaryNativeArchiveV1.invalid("current-control signing message differs")
          }
        } else if fields[1].count != 64 {
          throw KagemushaOrdinaryNativeArchiveV1.invalid("current-control retained account signature differs")
        }
      }
      return fields
    }
  }
  func startup(_ phase: UInt8, id: UInt64) throws -> KagemushaOrdinaryNativeArchiveV1.Response {
    guard phase == 1 && id == 0 || phase == 6 && id != 0 else {
      throw KagemushaOrdinaryNativeArchiveV1.invalid("ordinary current-wallet read phase differs")
    }
    return try bridge.withOrdinaryDescriptor { _ in
      let response = try KagemushaOrdinaryNativeArchiveV1.decode(endpoint.startup(phase: phase, id: id),
        schema: KagemushaOrdinaryNativeArchiveV1.startupResponse, phase: phase, id: nil)
      guard response.id != 0 else { throw KagemushaOrdinaryNativeArchiveV1.invalid("ordinary current-wallet read ID absent") }
      if phase == 1 {
        guard response.fields.count == 3, KagemushaOrdinaryOutgoingFrameV1.digest(response.fields[0]),
          response.fields[1...2].allSatisfy({ (1...4096).contains($0.count) }) else {
          throw KagemushaOrdinaryNativeArchiveV1.invalid("ordinary current-wallet read originals differ")
        }
      } else if !response.fields.isEmpty {
        throw KagemushaOrdinaryNativeArchiveV1.invalid("ordinary current-wallet consume response differs")
      }
      return response
    }
  }
}

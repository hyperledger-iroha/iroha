import Foundation

/// Closed native coordinator methods; schema 2 is the sole V1 protocol frame.
public enum KagemushaCoreCoordinatorMethodV1: UInt8, CaseIterable, Sendable {
  case reserveOperationID = 1, acceptQualification, acceptAuthenticatedReply
  case beginSenderTransition, provePreparedSenderTransition, buildTerminalEnvelope
  case acceptInstalledTerminal, recoverSender, recoverTerminalEnvelope, releaseOutbox
}

/// Framing errors grant no native coordinator or monetary authority.
public enum KagemushaCoreCoordinatorErrorV1: Error, Equatable, Sendable {
  case invalidFrame(String)
  case unavailable
  case nativeFailure(Int32)
}

/// Exact ABI framing and request/response correlation. Embedded Norito preparation, candidate,
/// and recovery archives remain opaque; qualified native Core must authenticate their semantics.
public enum KagemushaCoreCoordinatorFrameV1 {
  public static let schemaVersion: UInt16 = 2
  public static let maximumFields = 16
  public static let maximumFieldBytes = 64 * 1024
  public static let maximumRequestBytes = 256 * 1024
  public static let maximumResponseBytes = 128 * 1024
  private static let magic = Data("IKGMCOR1".utf8)

  /// Encode only a complete method-specific request using the current native schema.
  public static func encodeRequest(_ method: KagemushaCoreCoordinatorMethodV1, fields: [Data]) throws -> Data {
    try validateRequest(method, fields)
    return try encode(fields, maximum: maximumRequestBytes)
  }

  /// Decode exact request bytes, rejecting retired schemas, tails and invalid method fields.
  public static func decodeRequest(_ method: KagemushaCoreCoordinatorMethodV1, frame: Data) throws -> [Data] {
    let fields = try decode(frame, maximum: maximumRequestBytes)
    try validateRequest(method, fields)
    return fields
  }

  /// Decode a response after correlating its operation, terminal and envelope to the request.
  public static func decodeResponse(
    _ method: KagemushaCoreCoordinatorMethodV1, requestFrame: Data, responseFrame: Data
  ) throws -> [Data] {
    let request = try decodeRequest(method, frame: requestFrame)
    let response = try decode(responseFrame, maximum: maximumResponseBytes)
    try validateResponse(method, request, response)
    return response
  }

  /// Encode a native field-array response with the same request correlation as the C ABI.
  public static func encodeResponse(
    _ method: KagemushaCoreCoordinatorMethodV1, requestFrame: Data, fields: [Data]
  ) throws -> Data {
    try validateResponse(method, decodeRequest(method, frame: requestFrame), fields)
    return try encode(fields, maximum: maximumResponseBytes)
  }

  /// Canonical little-endian native discriminant for method fields.
  public static func u32(_ value: UInt32) -> Data {
    var value = value.littleEndian
    return withUnsafeBytes(of: &value) { Data($0) }
  }

  private static func encode(_ fields: [Data], maximum: Int) throws -> Data {
    try require(fields.count <= maximumFields, "too many fields")
    var count = 16
    for field in fields {
      try require(field.count <= maximumFieldBytes, "oversized field")
      count += 4 + field.count
    }
    try require(count <= maximum, "oversized frame")
    var result = magic
    result.append(contentsOf: [UInt8(schemaVersion), 0, UInt8(fields.count), 0, 0, 0, 0, 0])
    for field in fields {
      result.append(u32(UInt32(field.count)))
      result.append(field)
    }
    return result
  }

  private static func decode(_ frame: Data, maximum: Int) throws -> [Data] {
    try require((16...maximum).contains(frame.count), "invalid frame size")
    let frame = Data(frame)
    try require(frame.prefix(8) == magic && frame[8] == schemaVersion && frame[9] == 0, "invalid schema")
    let count = Int(frame[10]) | (Int(frame[11]) << 8)
    try require(count <= maximumFields && frame[12..<16].allSatisfy { $0 == 0 }, "invalid header")
    var cursor = 16
    var result: [Data] = []
    for _ in 0..<count {
      try require(frame.count - cursor >= 4, "truncated field length")
      let length = Int(readU32(Data(frame[cursor..<cursor + 4])))
      cursor += 4
      try require(length <= maximumFieldBytes && length <= frame.count - cursor, "invalid field length")
      result.append(Data(frame[cursor..<cursor + length]))
      cursor += length
    }
    try require(cursor == frame.count, "trailing bytes")
    return result
  }

  private static func validateRequest(_ method: KagemushaCoreCoordinatorMethodV1, _ fields: [Data]) throws {
    switch method {
    case .reserveOperationID:
      try count(fields, 3); try operation(fields, 0); try digest(fields, 1); try nonempty(fields, 2)
    case .acceptQualification:
      try count(fields, 6); try qualification(fields, 0); try digest(fields, 5)
    case .acceptAuthenticatedReply:
      try count(fields, 9); try operation(fields, 0); try digest(fields, 1)
      try nonempty(fields, 2); try nonempty(fields, 3); try qualification(fields, 4)
    case .beginSenderTransition:
      try digest(fields, 0)
      let end = try senderInputs(fields, 1)
      try count(fields, end + 5); try qualification(fields, end)
    case .provePreparedSenderTransition, .buildTerminalEnvelope, .recoverTerminalEnvelope:
      try count(fields, 2); try nonempty(fields, 0); try nonempty(fields, 1)
    case .acceptInstalledTerminal:
      try count(fields, 5)
      for index in fields.indices { try nonempty(fields, index) }
    case .recoverSender:
      try count(fields, 8)
      try require(fields[0] == Data([0]) || fields[0] == Data([1]), "invalid recovery selector")
      try digest(fields, 1); _ = try kind(fields, 2); try qualification(fields, 3)
    case .releaseOutbox:
      try digest(fields, 0)
      let end = try senderInputs(fields, 1)
      try count(fields, end + 7); try nonempty(fields, end)
      let receipt = try field(fields, end + 1)
      try require(receipt.count > 4 && receipt.prefix(4) == field(fields, 1), "invalid terminal receipt")
      try qualification(fields, end + 2)
    }
  }

  private static func validateResponse(_ method: KagemushaCoreCoordinatorMethodV1, _ request: [Data], _ response: [Data]) throws {
    switch method {
    case .reserveOperationID:
      try count(response, 1); try digest(response, 0); try equal(response, 0, request, 1)
    case .acceptQualification, .acceptAuthenticatedReply:
      try count(response, 0)
    case .beginSenderTransition:
      try count(response, 2); try digest(response, 0); try nonempty(response, 1); try equal(response, 0, request, 0)
    case .provePreparedSenderTransition, .buildTerminalEnvelope, .recoverTerminalEnvelope:
      try count(response, 1); try nonempty(response, 0)
    case .acceptInstalledTerminal:
      try count(response, 2); try nonempty(response, 0); try nonempty(response, 1); try equal(response, 0, request, 1)
    case .recoverSender:
      if response.isEmpty { return }
      try count(response, 3); try digest(response, 0); try digest(response, 1); try nonempty(response, 2)
      try equal(response, request[0] == Data([0]) ? 1 : 0, request, 1)
    case .releaseOutbox:
      try count(response, 5); try digest(response, 0); try nonempty(response, 1); try digest(response, 2)
      try nonempty(response, 3); try nonempty(response, 4)
      try equal(response, 3, request, senderInputs(request, 1))
    }
  }

  private static func require(_ condition: Bool, _ message: String) throws {
    guard condition else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame(message) }
  }

  private static func field(_ fields: [Data], _ index: Int) throws -> Data {
    try require(fields.indices.contains(index), "missing field")
    return fields[index]
  }

  private static func count(_ fields: [Data], _ expected: Int) throws {
    try require(fields.count == expected, "invalid field count")
  }

  private static func nonempty(_ fields: [Data], _ index: Int) throws {
    try require(!field(fields, index).isEmpty, "empty field")
  }

  private static func digest(_ fields: [Data], _ index: Int) throws {
    let bytes = try field(fields, index)
    try require(bytes.count == 32 && bytes.contains { $0 != 0 }, "invalid digest")
  }

  private static func readU32(_ bytes: Data) -> UInt32 {
    bytes.enumerated().reduce(UInt32(0)) { $0 | (UInt32($1.element) << (8 * $1.offset)) }
  }

  private static func number(_ fields: [Data], _ index: Int) throws -> UInt32 {
    let bytes = try field(fields, index)
    try require(bytes.count == 4, "invalid u32")
    return readU32(bytes)
  }

  private static func operation(_ fields: [Data], _ index: Int) throws {
    try require((1...22).contains(number(fields, index)), "invalid device operation")
  }

  private static func kind(_ fields: [Data], _ index: Int) throws -> UInt32 {
    let value = try number(fields, index)
    try require(value <= 1, "invalid sender kind")
    return value
  }

  private static func senderInputs(_ fields: [Data], _ start: Int) throws -> Int {
    if try kind(fields, start) == 0 {
      try nonempty(fields, start + 1)
      return start + 2
    }
    let amount = try field(fields, start + 1)
    try require(amount.count == 16 && amount.contains { $0 != 0 }, "invalid positive u128")
    try nonempty(fields, start + 2)
    return start + 3
  }

  private static func qualification(_ fields: [Data], _ start: Int) throws {
    try require(number(fields, start) == 1, "invalid protocol version")
    try digest(fields, start + 1); try nonempty(fields, start + 2); try nonempty(fields, start + 3)
    try require(number(fields, start + 4) == 0xffff, "incomplete capabilities")
  }

  private static func equal(_ left: [Data], _ li: Int, _ right: [Data], _ ri: Int) throws {
    try require(field(left, li) == field(right, ri), "response substituted request binding")
  }
}

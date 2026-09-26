import Foundation

/// A testnet value-admission request failed before or at the native durable owner.
public enum KagemushaTestnetValueAdmissionErrorV1: Error, Equatable, Sendable {
  case invalidOperationID
  case bridgeUnavailable
  case ownerUnavailable
  case nativeRejected(Int32)
  case invalidArchive
}

/// Copyable evidence from one native-verified experimental testnet mint.
///
/// The copied archive is inspectable and forgeable. A testnet ledger must call the native
/// admission boundary itself and deduplicate the operation and credit IDs. This value cannot
/// qualify hardware or authorize production money, payments, or redemption.
public struct KagemushaTestnetValueAdmissionV1: Equatable, Sendable {
  public let canonicalArchive: Data
  public var testnetOnly: Bool { true }
  public var hardwareQualified: Bool { false }
  public var productionMonetaryAuthorized: Bool { false }

  fileprivate init(canonicalArchive: Data) {
    self.canonicalArchive = canonicalArchive
  }
}

protocol KagemushaTestnetValueAdmissionEndpointV1: AnyObject {
  func admit(operationID: Data) -> (status: Int32, archive: Data)
}

/// Ask the durable native owner to admit one retained Applied top-up and paired MintFold proof.
///
/// Only the operation ID crosses this boundary. The signed Experimental release, original
/// private reservation, verified finality context, and paired proof remain in native custody.
public enum KagemushaTestnetValueAdmissionBridgeV1 {
  public static let maximumArchiveBytes = 768
  private static let archiveSchema =
    "connect_norito_bridge::KagemushaTestnetValueAdmissionArchiveV1"

  public static func admit(operationID: Data) throws -> KagemushaTestnetValueAdmissionV1 {
    try validate(operationID: operationID)
    guard let endpoint = NativeEndpoint.create() else {
      throw KagemushaTestnetValueAdmissionErrorV1.bridgeUnavailable
    }
    return try admit(operationID: operationID, endpoint: endpoint)
  }

  static func admit(
    operationID: Data,
    endpoint: any KagemushaTestnetValueAdmissionEndpointV1
  ) throws -> KagemushaTestnetValueAdmissionV1 {
    try validate(operationID: operationID)
    let result = endpoint.admit(operationID: Data(operationID))
    switch result.status {
    case 0:
      return try decode(result.archive, operationID: operationID)
    case -312:
      throw KagemushaTestnetValueAdmissionErrorV1.ownerUnavailable
    default:
      throw KagemushaTestnetValueAdmissionErrorV1.nativeRejected(result.status)
    }
  }

  private static func validate(operationID: Data) throws {
    guard operationID.count == 32, operationID.contains(where: { $0 != 0 }) else {
      throw KagemushaTestnetValueAdmissionErrorV1.invalidOperationID
    }
  }

  private static func decode(
    _ archive: Data, operationID: Data
  ) throws -> KagemushaTestnetValueAdmissionV1 {
    guard !archive.isEmpty, archive.count <= maximumArchiveBytes,
      let frame = noritoDecodeFrame(archive),
      frame.header.compression == .none,
      frame.header.flags == NoritoHeader.compactLen,
      frame.header.schema == noritoSchemaHash(forTypeName: archiveSchema),
      frame.paddingLength == noritoHeaderPaddingLength(payloadAlignment: 16)
    else { throw KagemushaTestnetValueAdmissionErrorV1.invalidArchive }
    do {
      var reader = CanonicalNoritoReader(data: frame.payload)
      var fields = [Data]()
      fields.reserveCapacity(17)
      for length in [2, 1, 32, 32, 32, 32, 32, 4, 32, 32, 32, 16,
                     32, 32, 32, 8, 32] {
        let field = try reader.readCompactField()
        guard field.count == length else { throw KagemushaTestnetValueAdmissionErrorV1.invalidArchive }
        fields.append(field)
      }
      guard reader.remaining() == 0 else { throw KagemushaTestnetValueAdmissionErrorV1.invalidArchive }
      var canonical = CompactNoritoWriter()
      for field in fields { canonical.writeField(field) }
      guard noritoEncode(
        typeName: archiveSchema, payload: canonical.data,
        flags: NoritoHeader.compactLen, payloadAlignment: 16) == archive
      else { throw KagemushaTestnetValueAdmissionErrorV1.invalidArchive }

      var versionReader = CanonicalNoritoReader(data: fields[0])
      let version = try versionReader.readUInt16LE()
      var scaleReader = CanonicalNoritoReader(data: fields[7])
      let assetScale = try scaleReader.readUInt32LE()
      let amount = try KagemushaUInt128V1(littleEndianBytes: fields[11])
      var heightReader = CanonicalNoritoReader(data: fields[15])
      let blockHeight = try heightReader.readUInt64LE()
      let nonzero: (Data) -> Bool = { $0.contains(where: { $0 != 0 }) }
      guard version == 1, fields[1][fields[1].startIndex] == 0,
        assetScale <= 28, blockHeight > 0, !amount.isZero,
        [fields[2], fields[3], fields[4], fields[5], fields[6], fields[8],
         fields[9], fields[10], fields[12], fields[13], fields[14], fields[16]].allSatisfy(nonzero),
        fields[2] != fields[3], fields[2] != fields[4], fields[3] != fields[4],
        fields[5] != fields[8], fields[9] == operationID
      else { throw KagemushaTestnetValueAdmissionErrorV1.invalidArchive }
      return KagemushaTestnetValueAdmissionV1(canonicalArchive: Data(archive))
    } catch {
      throw KagemushaTestnetValueAdmissionErrorV1.invalidArchive
    }
  }

  private final class NativeEndpoint: KagemushaTestnetValueAdmissionEndpointV1 {
    #if canImport(Darwin)
    private typealias AdmitFn = @convention(c) (
      UnsafePointer<UInt8>?, Int,
      UnsafeMutablePointer<UInt8>?, Int, UnsafeMutablePointer<Int>?
    ) -> Int32
    private let function: AdmitFn

    private init(function: @escaping AdmitFn) { self.function = function }

    static func create() -> NativeEndpoint? {
      guard NoritoNativeBridge.shared.isAvailable,
        let function: AdmitFn = NoritoNativeBridge.shared.resolveNativeSymbol(
          "connect_norito_kagemusha_testnet_value_admit_v1", as: AdmitFn.self)
      else { return nil }
      return NativeEndpoint(function: function)
    }

    func admit(operationID: Data) -> (status: Int32, archive: Data) {
      var output = [UInt8](repeating: 0,
        count: KagemushaTestnetValueAdmissionBridgeV1.maximumArchiveBytes)
      var written = 0
      let status = operationID.withUnsafeBytes { operation in
        output.withUnsafeMutableBufferPointer { result in
          function(operation.bindMemory(to: UInt8.self).baseAddress, operationID.count,
                   result.baseAddress, result.count, &written)
        }
      }
      guard status == 0,
        (1...KagemushaTestnetValueAdmissionBridgeV1.maximumArchiveBytes).contains(written)
      else { return (status == 0 ? -311 : status, Data()) }
      return (0, Data(output[..<written]))
    }
    #else
    static func create() -> NativeEndpoint? { nil }
    func admit(operationID: Data) -> (status: Int32, archive: Data) { (-312, Data()) }
    #endif
  }
}

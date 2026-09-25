import Foundation

/// A native experimental testnet mint-credit request failed.
public enum KagemushaTestnetValueCreditErrorV1: Error, Equatable, Sendable {
  case invalidOperationID
  case bridgeUnavailable
  case ledgerUnavailable
  case nativeRejected(Int32)
  case invalidArchive
}

/// Copyable inspection bytes for one credit counted by the native testnet ledger.
///
/// The bytes can be copied or forged after return. Only the successful live native call
/// acknowledges a durable ledger credit; this value is not a spend or hardware credential.
public struct KagemushaTestnetValueCreditV1: Equatable, Sendable {
  public let canonicalArchive: Data
  public var testnetOnly: Bool { true }
  public var hardwareQualified: Bool { false }
  public var productionMonetaryAuthorized: Bool { false }

  fileprivate init(canonicalArchive: Data) {
    self.canonicalArchive = canonicalArchive
  }
}

protocol KagemushaTestnetValueCreditEndpointV1: AnyObject {
  func credit(operationID: Data) -> (status: Int32, archive: Data)
}

/// Count an already-admitted, proof-backed top-up in the installed native testnet ledger.
///
/// Only the operation ID crosses this boundary. The native owner retains the signed
/// Experimental release, private reservation, finality context, and paired proof.
public enum KagemushaTestnetValueCreditBridgeV1 {
  public static let maximumArchiveBytes = 512
  private static let archiveSchema =
    "connect_norito_bridge::KagemushaTestnetMintLedgerCreditArchiveV1"

  public static func credit(operationID: Data) throws -> KagemushaTestnetValueCreditV1 {
    try validate(operationID: operationID)
    guard let endpoint = NativeEndpoint.create() else {
      throw KagemushaTestnetValueCreditErrorV1.bridgeUnavailable
    }
    return try credit(operationID: operationID, endpoint: endpoint)
  }

  static func credit(
    operationID: Data,
    endpoint: any KagemushaTestnetValueCreditEndpointV1
  ) throws -> KagemushaTestnetValueCreditV1 {
    try validate(operationID: operationID)
    let result = endpoint.credit(operationID: Data(operationID))
    switch result.status {
    case 0:
      return try decode(result.archive)
    case -312:
      throw KagemushaTestnetValueCreditErrorV1.ledgerUnavailable
    default:
      throw KagemushaTestnetValueCreditErrorV1.nativeRejected(result.status)
    }
  }

  private static func validate(operationID: Data) throws {
    guard operationID.count == 32, operationID.contains(where: { $0 != 0 }) else {
      throw KagemushaTestnetValueCreditErrorV1.invalidOperationID
    }
  }

  private static func decode(_ archive: Data) throws -> KagemushaTestnetValueCreditV1 {
    guard !archive.isEmpty, archive.count <= maximumArchiveBytes,
      let frame = noritoDecodeFrame(archive),
      frame.header.compression == .none,
      frame.header.flags == NoritoHeader.compactLen,
      frame.header.schema == noritoSchemaHash(forTypeName: archiveSchema),
      !frame.payload.isEmpty
    else { throw KagemushaTestnetValueCreditErrorV1.invalidArchive }
    return KagemushaTestnetValueCreditV1(canonicalArchive: Data(archive))
  }

  private final class NativeEndpoint: KagemushaTestnetValueCreditEndpointV1 {
    #if canImport(Darwin)
    private typealias CreditFn = @convention(c) (
      UnsafePointer<UInt8>?, Int,
      UnsafeMutablePointer<UInt8>?, Int, UnsafeMutablePointer<Int>?
    ) -> Int32
    private let function: CreditFn

    private init(function: @escaping CreditFn) { self.function = function }

    static func create() -> NativeEndpoint? {
      guard NoritoNativeBridge.shared.isAvailable,
        let function: CreditFn = NoritoNativeBridge.shared.resolveNativeSymbol(
          "connect_norito_kagemusha_testnet_value_credit_v1", as: CreditFn.self)
      else { return nil }
      return NativeEndpoint(function: function)
    }

    func credit(operationID: Data) -> (status: Int32, archive: Data) {
      var output = [UInt8](repeating: 0,
        count: KagemushaTestnetValueCreditBridgeV1.maximumArchiveBytes)
      var written = 0
      let status = operationID.withUnsafeBytes { operation in
        output.withUnsafeMutableBufferPointer { result in
          function(operation.bindMemory(to: UInt8.self).baseAddress, operationID.count,
                   result.baseAddress, result.count, &written)
        }
      }
      guard status == 0,
        (1...KagemushaTestnetValueCreditBridgeV1.maximumArchiveBytes).contains(written)
      else { return (status == 0 ? -311 : status, Data()) }
      return (0, Data(output[..<written]))
    }
    #else
    static func create() -> NativeEndpoint? { nil }
    func credit(operationID: Data) -> (status: Int32, archive: Data) { (-312, Data()) }
    #endif
  }
}

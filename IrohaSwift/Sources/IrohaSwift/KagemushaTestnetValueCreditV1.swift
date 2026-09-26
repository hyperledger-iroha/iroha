import Foundation

/// A native experimental testnet mint-credit request failed.
public enum KagemushaTestnetValueCreditErrorV1: Error, Equatable, Sendable {
  case invalidOperationID
  case bridgeUnavailable
  case ledgerUnavailable
  case nativeRejected(Int32)
  case invalidArchive
}

/// Exact signed-release, network, asset, and liability-pool scope of a testnet credit.
public struct KagemushaTestnetValueCreditScopeV1: Equatable, Sendable {
  public let networkID: Data
  public let releaseID: Data
  public let releaseAttestationDigest: Data
  public let assetIdentityDigest: Data
  public let assetIncarnation: Data
  public let assetScale: UInt32
  public let liabilityPoolID: Data
}

/// Copyable inspection facts for one credit counted by the native testnet ledger.
///
/// These facts and archive can be copied or forged after return. Only the successful live
/// native call acknowledges a durable ledger credit; neither is a spend or hardware credential.
public struct KagemushaTestnetValueCreditV1: Equatable, Sendable {
  public let canonicalArchive: Data
  public let version: UInt16
  public let scope: KagemushaTestnetValueCreditScopeV1
  public let operationID: Data
  public let creditID: Data
  /// Positive atomic asset units counted for this finalized top-up.
  public let amount: KagemushaUInt128V1
  /// Cumulative atomic asset units in the installed testnet ledger.
  public let totalAdmitted: KagemushaUInt128V1
  public var testnetOnly: Bool { true }
  /// Always false for the current Experimental release; true is rejected on decode.
  public let hardwareQualified: Bool
  public var productionMonetaryAuthorized: Bool { false }

  fileprivate init(
    canonicalArchive: Data, version: UInt16, scope: KagemushaTestnetValueCreditScopeV1,
    operationID: Data, creditID: Data, amount: KagemushaUInt128V1,
    totalAdmitted: KagemushaUInt128V1, hardwareQualified: Bool
  ) {
    self.canonicalArchive = canonicalArchive
    self.version = version
    self.scope = scope
    self.operationID = operationID
    self.creditID = creditID
    self.amount = amount
    self.totalAdmitted = totalAdmitted
    self.hardwareQualified = hardwareQualified
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
      return try decode(result.archive, operationID: operationID)
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

  private static func decode(
    _ archive: Data, operationID: Data
  ) throws -> KagemushaTestnetValueCreditV1 {
    guard !archive.isEmpty, archive.count <= maximumArchiveBytes,
      let frame = noritoDecodeFrame(archive),
      frame.header.compression == .none,
      frame.header.flags == NoritoHeader.compactLen,
      frame.header.schema == noritoSchemaHash(forTypeName: archiveSchema),
      frame.paddingLength == noritoHeaderPaddingLength(payloadAlignment: 16)
    else { throw KagemushaTestnetValueCreditErrorV1.invalidArchive }
    do {
      var reader = CanonicalNoritoReader(data: frame.payload)
      var fields = [Data]()
      fields.reserveCapacity(13)
      for length in [2, 1, 32, 32, 32, 32, 32, 4, 32, 32, 32, 16, 16] {
        let field = try reader.readCompactField()
        guard field.count == length else { throw KagemushaTestnetValueCreditErrorV1.invalidArchive }
        fields.append(field)
      }
      guard reader.remaining() == 0 else { throw KagemushaTestnetValueCreditErrorV1.invalidArchive }
      var canonical = CompactNoritoWriter()
      for field in fields { canonical.writeField(field) }
      guard noritoEncode(
        typeName: archiveSchema, payload: canonical.data,
        flags: NoritoHeader.compactLen, payloadAlignment: 16) == archive
      else { throw KagemushaTestnetValueCreditErrorV1.invalidArchive }

      var versionReader = CanonicalNoritoReader(data: fields[0])
      let version = try versionReader.readUInt16LE()
      let hardwareQualified = fields[1][fields[1].startIndex]
      var scaleReader = CanonicalNoritoReader(data: fields[7])
      let assetScale = try scaleReader.readUInt32LE()
      let amount = try KagemushaUInt128V1(littleEndianBytes: fields[11])
      let totalAdmitted = try KagemushaUInt128V1(littleEndianBytes: fields[12])
      let nonzero: (Data) -> Bool = { $0.contains(where: { $0 != 0 }) }
      guard version == 1, hardwareQualified == 0, assetScale <= 28,
        [fields[2], fields[3], fields[4], fields[5], fields[6], fields[8],
         fields[9], fields[10]].allSatisfy(nonzero),
        fields[2] != fields[3], fields[2] != fields[4], fields[3] != fields[4],
        fields[5] != fields[8], fields[9] == operationID,
        !amount.isZero, amount.isLessThanOrEqual(to: totalAdmitted)
      else { throw KagemushaTestnetValueCreditErrorV1.invalidArchive }
      return KagemushaTestnetValueCreditV1(
        canonicalArchive: Data(archive), version: version,
        scope: KagemushaTestnetValueCreditScopeV1(
          networkID: fields[2], releaseID: fields[3],
          releaseAttestationDigest: fields[4], assetIdentityDigest: fields[5],
          assetIncarnation: fields[6], assetScale: assetScale, liabilityPoolID: fields[8]),
        operationID: fields[9], creditID: fields[10], amount: amount,
        totalAdmitted: totalAdmitted, hardwareQualified: false)
    } catch {
      throw KagemushaTestnetValueCreditErrorV1.invalidArchive
    }
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

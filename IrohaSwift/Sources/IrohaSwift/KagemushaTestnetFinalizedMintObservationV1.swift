import Foundation

/// A finalized-mint diagnostic cannot authorize money or qualify hardware.
public enum KagemushaTestnetFinalizedMintObservationErrorV1: Error, Equatable, Sendable {
  case invalidInput
  case bridgeUnavailable
  case ownerUnavailable
  case nativeRejected(Int32)
  case invalidObservation
}

/// Opaque, unsigned inspection data from a native-verified testnet mint and paired State proof.
public struct KagemushaTestnetFinalizedMintObservationV1: Equatable, Sendable {
  public let canonicalArchive: Data

  /// Testnet proof inspection is not a hardware qualification.
  public var hardwareQualified: Bool { false }
  /// This copyable archive is not a monetary admission or payment receipt.
  public var monetaryAuthorized: Bool { false }

  fileprivate init(canonicalArchive: Data) {
    self.canonicalArchive = canonicalArchive
  }
}

protocol KagemushaTestnetFinalizedMintObservationEndpointV1: AnyObject {
  func observe(
    operationID: Data, originalStatusJSON: Data,
    trustAnchor: KagemushaFinalityTrustAnchorV1,
    publicInputsArchive: Data, pairedProofArchive: Data
  ) -> (status: Int32, archive: Data)
}

/// Inspects an actual Applied top-up and paired MintFold proof through the native owner.
///
/// The owner must already have retained the private pre-send reservation and pinned the
/// independently authenticated finality anchor in Rust. The supplied coordinates only
/// select that exact pin; this Swift API cannot create or replace either authority.
public enum KagemushaTestnetFinalizedMintObservationBridgeV1 {
  public static let maximumStatusJSONBytes = 16_777_216
  public static let maximumPublicInputsBytes = 4_096
  public static let maximumPairedProofBytes = 6_528
  public static let maximumObservationBytes = 512
  private static let archiveSchema =
    "connect_norito_bridge::KagemushaTestnetFinalizedMintObservationArchiveV1"

  /// Passes the original Torii response bytes to native verification without re-encoding them.
  public static func observe(
    operationID: Data, originalStatusJSON: Data,
    trustAnchor: KagemushaFinalityTrustAnchorV1,
    publicInputsArchive: Data, pairedProofArchive: Data
  ) throws -> KagemushaTestnetFinalizedMintObservationV1 {
    try validateInputs(
      operationID: operationID, originalStatusJSON: originalStatusJSON,
      trustAnchor: trustAnchor, publicInputsArchive: publicInputsArchive,
      pairedProofArchive: pairedProofArchive)
    guard let endpoint = NativeEndpoint.create() else {
      throw KagemushaTestnetFinalizedMintObservationErrorV1.bridgeUnavailable
    }
    return try observe(
      operationID: operationID, originalStatusJSON: originalStatusJSON,
      trustAnchor: trustAnchor, publicInputsArchive: publicInputsArchive,
      pairedProofArchive: pairedProofArchive, endpoint: endpoint)
  }

  static func observe(
    operationID: Data, originalStatusJSON: Data,
    trustAnchor: KagemushaFinalityTrustAnchorV1,
    publicInputsArchive: Data, pairedProofArchive: Data,
    endpoint: any KagemushaTestnetFinalizedMintObservationEndpointV1
  ) throws -> KagemushaTestnetFinalizedMintObservationV1 {
    try validateInputs(
      operationID: operationID, originalStatusJSON: originalStatusJSON,
      trustAnchor: trustAnchor, publicInputsArchive: publicInputsArchive,
      pairedProofArchive: pairedProofArchive)
    let result = endpoint.observe(
      operationID: operationID, originalStatusJSON: originalStatusJSON,
      trustAnchor: trustAnchor, publicInputsArchive: publicInputsArchive,
      pairedProofArchive: pairedProofArchive)
    switch result.status {
    case 0:
      return try decodeObservation(result.archive)
    case -312:
      throw KagemushaTestnetFinalizedMintObservationErrorV1.ownerUnavailable
    default:
      throw KagemushaTestnetFinalizedMintObservationErrorV1.nativeRejected(result.status)
    }
  }

  private static func validateInputs(
    operationID: Data, originalStatusJSON: Data,
    trustAnchor: KagemushaFinalityTrustAnchorV1,
    publicInputsArchive: Data, pairedProofArchive: Data
  ) throws {
    guard kagemushaIsDigest(operationID),
      (1...maximumStatusJSONBytes).contains(originalStatusJSON.count),
      trustAnchor.networkID.count == 32,
      trustAnchor.networkID.last.map({ $0 & 1 == 1 }) == true,
      trustAnchor.blockHeight > 0,
      trustAnchor.heightContextID.count == 32,
      trustAnchor.heightContextID.last.map({ $0 & 1 == 1 }) == true,
      (1...maximumPublicInputsBytes).contains(publicInputsArchive.count),
      (1...maximumPairedProofBytes).contains(pairedProofArchive.count)
    else { throw KagemushaTestnetFinalizedMintObservationErrorV1.invalidInput }
  }

  private static func decodeObservation(
    _ archive: Data
  ) throws -> KagemushaTestnetFinalizedMintObservationV1 {
    guard !archive.isEmpty, archive.count <= maximumObservationBytes,
      let frame = noritoDecodeFrame(archive),
      frame.header.flags == NoritoHeader.compactLen,
      frame.header.schema == noritoSchemaHash(forTypeName: archiveSchema)
    else { throw KagemushaTestnetFinalizedMintObservationErrorV1.invalidObservation }
    // The native owner emits this exact canonical type only after verifying the
    // retained mint, finality, and State proof. Do not infer authority from bytes.
    return KagemushaTestnetFinalizedMintObservationV1(canonicalArchive: Data(archive))
  }

  private final class NativeEndpoint: KagemushaTestnetFinalizedMintObservationEndpointV1 {
    #if canImport(Darwin)
    private typealias ObserveFn = @convention(c) (
      UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int,
      UnsafePointer<UInt8>?, Int, UInt64, UnsafePointer<UInt8>?, Int,
      UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int,
      UnsafeMutablePointer<UInt8>?, Int, UnsafeMutablePointer<Int>?
    ) -> Int32
    private let function: ObserveFn

    private init(function: @escaping ObserveFn) { self.function = function }

    static func create() -> NativeEndpoint? {
      guard NoritoNativeBridge.shared.isAvailable,
        let function: ObserveFn = NoritoNativeBridge.shared.resolveNativeSymbol(
          "connect_norito_kagemusha_testnet_finalized_mint_observe_v1", as: ObserveFn.self)
      else { return nil }
      return NativeEndpoint(function: function)
    }

    func observe(
      operationID: Data, originalStatusJSON: Data,
      trustAnchor: KagemushaFinalityTrustAnchorV1,
      publicInputsArchive: Data, pairedProofArchive: Data
    ) -> (status: Int32, archive: Data) {
      var output = [UInt8](
        repeating: 0, count: KagemushaTestnetFinalizedMintObservationBridgeV1.maximumObservationBytes)
      var written = 0
      let status = operationID.withUnsafeBytes { operation in
        originalStatusJSON.withUnsafeBytes { response in
          trustAnchor.networkID.withUnsafeBytes { network in
            trustAnchor.heightContextID.withUnsafeBytes { context in
              publicInputsArchive.withUnsafeBytes { publicInputs in
                pairedProofArchive.withUnsafeBytes { proof in
                  output.withUnsafeMutableBufferPointer { result in
                    function(
                      operation.bindMemory(to: UInt8.self).baseAddress, operationID.count,
                      response.bindMemory(to: UInt8.self).baseAddress, originalStatusJSON.count,
                      network.bindMemory(to: UInt8.self).baseAddress, trustAnchor.networkID.count,
                      trustAnchor.blockHeight,
                      context.bindMemory(to: UInt8.self).baseAddress, trustAnchor.heightContextID.count,
                      publicInputs.bindMemory(to: UInt8.self).baseAddress, publicInputsArchive.count,
                      proof.bindMemory(to: UInt8.self).baseAddress, pairedProofArchive.count,
                      result.baseAddress, result.count, &written)
                  }
                }
              }
            }
          }
        }
      }
      guard status == 0,
        (1...KagemushaTestnetFinalizedMintObservationBridgeV1.maximumObservationBytes).contains(written)
      else { return (status, Data()) }
      return (0, Data(output[..<written]))
    }
    #else
    static func create() -> NativeEndpoint? { nil }
    func observe(
      operationID: Data, originalStatusJSON: Data,
      trustAnchor: KagemushaFinalityTrustAnchorV1,
      publicInputsArchive: Data, pairedProofArchive: Data
    ) -> (status: Int32, archive: Data) { (-312, Data()) }
    #endif
  }
}

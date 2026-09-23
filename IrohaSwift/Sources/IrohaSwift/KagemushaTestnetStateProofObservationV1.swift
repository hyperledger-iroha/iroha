import Foundation

/// The testnet observer cannot authorize a payment, enrollment, or hardware profile.
public enum KagemushaTestnetStateProofObservationErrorV1: Error, Equatable, Sendable {
  case invalidInput
  case bridgeUnavailable
  case ownerUnavailable
  case nativeRejected(Int32)
  case invalidObservation
}

/// Unsigned inspection data from one native-verified paired State proof.
///
/// This value is copyable and is never a monetary receipt or hardware attestation.
public struct KagemushaTestnetStateProofObservationV1: Equatable, Sendable {
  public let canonicalArchive: Data

  /// The testnet observer deliberately makes no hardware qualification claim.
  public var hardwareQualified: Bool { false }
  /// Native proof inspection never admits or transfers monetary value.
  public var monetaryAuthorized: Bool { false }

  fileprivate init(canonicalArchive: Data) {
    self.canonicalArchive = canonicalArchive
  }
}

protocol KagemushaTestnetStateProofObservationEndpointV1: AnyObject {
  func observe(publicInputsArchive: Data, pairedProofArchive: Data) -> (status: Int32, archive: Data)
}

/// Calls the release-pinned native verifier's diagnostic observation endpoint.
///
/// A stock bridge without a Rust-installed owner returns `ownerUnavailable`.
/// Installing an owner requires an independently authenticated release and
/// operator-pinned scope in Rust; this Swift API cannot install or replace one.
public enum KagemushaTestnetStateProofObservationBridgeV1 {
  public static let maximumPublicInputsBytes = 4_096
  public static let maximumPairedProofBytes = 6_528
  public static let maximumObservationBytes = 256
  private static let archiveSchema =
    "connect_norito_bridge::KagemushaTestnetStateObservationArchiveV1"

  /// Verify and append exactly one canonical paired State proof to a testnet trial.
  public static func observe(
    publicInputsArchive: Data, pairedProofArchive: Data
  ) throws -> KagemushaTestnetStateProofObservationV1 {
    try validateInputs(publicInputsArchive, pairedProofArchive)
    guard let endpoint = NativeEndpoint.create() else {
      throw KagemushaTestnetStateProofObservationErrorV1.bridgeUnavailable
    }
    return try observe(
      publicInputsArchive: publicInputsArchive, pairedProofArchive: pairedProofArchive,
      endpoint: endpoint)
  }

  static func observe(
    publicInputsArchive: Data, pairedProofArchive: Data,
    endpoint: any KagemushaTestnetStateProofObservationEndpointV1
  ) throws -> KagemushaTestnetStateProofObservationV1 {
    try validateInputs(publicInputsArchive, pairedProofArchive)
    let result = endpoint.observe(
      publicInputsArchive: publicInputsArchive, pairedProofArchive: pairedProofArchive)
    switch result.status {
    case 0:
      return try decodeObservation(result.archive)
    case -312:
      throw KagemushaTestnetStateProofObservationErrorV1.ownerUnavailable
    default:
      throw KagemushaTestnetStateProofObservationErrorV1.nativeRejected(result.status)
    }
  }

  private static func validateInputs(_ publicInputs: Data, _ proof: Data) throws {
    guard (1...maximumPublicInputsBytes).contains(publicInputs.count),
      (1...maximumPairedProofBytes).contains(proof.count)
    else { throw KagemushaTestnetStateProofObservationErrorV1.invalidInput }
  }

  private static func decodeObservation(
    _ archive: Data
  ) throws -> KagemushaTestnetStateProofObservationV1 {
    guard !archive.isEmpty, archive.count <= maximumObservationBytes,
      let frame = noritoDecodeFrame(Data(archive)),
      frame.header.flags == NoritoHeader.compactLen,
      frame.header.schema == noritoSchemaHash(forTypeName: archiveSchema)
    else { throw KagemushaTestnetStateProofObservationErrorV1.invalidObservation }
    // The native owner encodes the exact canonical Norito struct before it
    // advances its trial. Keep that payload opaque until a cross-language
    // fixture pins the Swift decoder's complete field layout.
    return KagemushaTestnetStateProofObservationV1(canonicalArchive: Data(archive))
  }

  private final class NativeEndpoint: KagemushaTestnetStateProofObservationEndpointV1 {
    #if canImport(Darwin)
    private typealias ObserveFn = @convention(c) (
      UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int,
      UnsafeMutablePointer<UInt8>?, Int, UnsafeMutablePointer<Int>?
    ) -> Int32
    private let function: ObserveFn

    private init(function: @escaping ObserveFn) { self.function = function }

    static func create() -> NativeEndpoint? {
      guard NoritoNativeBridge.shared.isAvailable,
        let function: ObserveFn = NoritoNativeBridge.shared.resolveNativeSymbol(
          "connect_norito_kagemusha_testnet_state_proof_observe_v1", as: ObserveFn.self)
      else { return nil }
      return NativeEndpoint(function: function)
    }

    func observe(publicInputsArchive: Data, pairedProofArchive: Data)
      -> (status: Int32, archive: Data)
    {
      var output = [UInt8](
        repeating: 0, count: KagemushaTestnetStateProofObservationBridgeV1.maximumObservationBytes)
      var written = 0
      let status = output.withUnsafeMutableBufferPointer { result in
        publicInputsArchive.withUnsafeBytes { publicInputs in
          pairedProofArchive.withUnsafeBytes { proof in
            function(
              publicInputs.bindMemory(to: UInt8.self).baseAddress, publicInputsArchive.count,
              proof.bindMemory(to: UInt8.self).baseAddress, pairedProofArchive.count,
              result.baseAddress, result.count, &written)
          }
        }
      }
      guard status == 0,
        (1...KagemushaTestnetStateProofObservationBridgeV1.maximumObservationBytes).contains(written)
      else {
        return (status, Data())
      }
      return (0, Data(output[..<written]))
    }
    #else
    static func create() -> NativeEndpoint? { nil }
    func observe(publicInputsArchive: Data, pairedProofArchive: Data)
      -> (status: Int32, archive: Data)
    { (-312, Data()) }
    #endif
  }
}

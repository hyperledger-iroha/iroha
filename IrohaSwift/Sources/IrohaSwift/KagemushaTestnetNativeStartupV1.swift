import Foundation

/// A signed Experimental deployment could not activate its native testnet owner.
public enum KagemushaTestnetNativeStartupErrorV1: Error, Equatable, Sendable {
  case invalidBootstrapLength
  case bridgeUnavailable
  case contractMismatch
  case nativeContextUnavailable
  case nativeRejected(Int32)
}

protocol KagemushaTestnetNativeStartupEndpointV1: AnyObject {
  func contract() -> [UInt32]?
  func activate(signedBootstrap: Data) -> Int32
}

/// Activate the durable Experimental owner using a threshold-signed deployment checkpoint.
///
/// The application transports the signed package. Rust independently supplies the authority
/// policy, deployment pins, trusted time, replay floor, signed release and private storage.
/// Success installs the testnet owner; it neither credits a top-up nor qualifies offline
/// spending. An uncertain installation requires recovery in a new process.
public enum KagemushaTestnetNativeStartupV1 {
  public static let maximumBootstrapBytes = 1024 * 1024
  private static let expectedContract: [UInt32] = [1, UInt32(maximumBootstrapBytes)]

  public static func activate(signedBootstrap: Data) throws {
    try validate(signedBootstrap)
    guard let endpoint = NativeEndpoint.create() else {
      throw KagemushaTestnetNativeStartupErrorV1.bridgeUnavailable
    }
    try activate(signedBootstrap: signedBootstrap, endpoint: endpoint)
  }

  static func activate(
    signedBootstrap: Data, endpoint: any KagemushaTestnetNativeStartupEndpointV1
  ) throws {
    try validate(signedBootstrap)
    guard endpoint.contract() == expectedContract else {
      throw KagemushaTestnetNativeStartupErrorV1.contractMismatch
    }
    switch endpoint.activate(signedBootstrap: Data(signedBootstrap)) {
    case 0: return
    case -312: throw KagemushaTestnetNativeStartupErrorV1.nativeContextUnavailable
    case let status: throw KagemushaTestnetNativeStartupErrorV1.nativeRejected(status)
    }
  }

  private static func validate(_ signedBootstrap: Data) throws {
    guard (1...maximumBootstrapBytes).contains(signedBootstrap.count) else {
      throw KagemushaTestnetNativeStartupErrorV1.invalidBootstrapLength
    }
  }

  private final class NativeEndpoint: KagemushaTestnetNativeStartupEndpointV1 {
    #if canImport(Darwin)
    private typealias ContractFn = @convention(c) (UnsafeMutablePointer<UInt32>?, Int) -> Int32
    private typealias ActivateFn = @convention(c) (UnsafePointer<UInt8>?, Int) -> Int32
    private let contractFunction: ContractFn
    private let activateFunction: ActivateFn

    private init(contract: @escaping ContractFn, activate: @escaping ActivateFn) {
      contractFunction = contract
      activateFunction = activate
    }

    static func create() -> NativeEndpoint? {
      guard NoritoNativeBridge.shared.isAvailable,
        let contract: ContractFn = NoritoNativeBridge.shared.resolveNativeSymbol(
          "connect_norito_kagemusha_testnet_native_startup_contract_v1", as: ContractFn.self),
        let activate: ActivateFn = NoritoNativeBridge.shared.resolveNativeSymbol(
          "connect_norito_kagemusha_testnet_native_startup_activate_v1", as: ActivateFn.self)
      else { return nil }
      return NativeEndpoint(contract: contract, activate: activate)
    }

    func contract() -> [UInt32]? {
      var words = [UInt32](repeating: 0, count: 2)
      let written = words.withUnsafeMutableBufferPointer {
        contractFunction($0.baseAddress, $0.count)
      }
      return written == 2 ? words : nil
    }

    func activate(signedBootstrap: Data) -> Int32 {
      signedBootstrap.withUnsafeBytes {
        activateFunction($0.bindMemory(to: UInt8.self).baseAddress, signedBootstrap.count)
      }
    }
    #else
    static func create() -> NativeEndpoint? { nil }
    func contract() -> [UInt32]? { nil }
    func activate(signedBootstrap: Data) -> Int32 { -312 }
    #endif
  }
}

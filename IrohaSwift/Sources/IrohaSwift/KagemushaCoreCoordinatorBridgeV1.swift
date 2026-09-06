import Foundation
#if canImport(Darwin)
import Darwin
#endif

/// C ABI endpoint. Test endpoints do not qualify a monetary provider.
protocol KagemushaCoreCoordinatorEndpointV1: AnyObject {
  func contract() throws -> [UInt32]
  func open(storagePath: Data) throws -> UInt64
  func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data
}

/// Serialized transport to the process-owned native coordinator, without a software backend.
/// Contract matching proves ABI compatibility only; native Core must admit its qualified hardware.
/// Returned Norito archives remain opaque. The native ABI owns handles for the process lifetime.
public final class KagemushaCoreCoordinatorBridgeV1 {
  private let endpoint: any KagemushaCoreCoordinatorEndpointV1
  private let handle: UInt64
  private let lock = NSLock()
  private static let expectedContract: [UInt32] = [2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff]

  private init(endpoint: any KagemushaCoreCoordinatorEndpointV1, handle: UInt64) {
    self.endpoint = endpoint
    self.handle = handle
  }

  /// Open the exact native ABI. Missing symbols, a mismatched contract or absent backend fails closed.
  public static func open(storagePath: String) throws -> KagemushaCoreCoordinatorBridgeV1 {
    _ = try validatePath(storagePath)
    guard let endpoint = NativeEndpoint.create() else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    return try openEndpoint(storagePath: storagePath, endpoint: endpoint)
  }

  static func openEndpoint(
    storagePath: String, endpoint: any KagemushaCoreCoordinatorEndpointV1
  ) throws -> KagemushaCoreCoordinatorBridgeV1 {
    let encodedPath = try validatePath(storagePath)
    guard try endpoint.contract() == expectedContract else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("native coordinator contract mismatch")
    }
    let handle = try endpoint.open(storagePath: encodedPath)
    guard handle != 0 else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    return KagemushaCoreCoordinatorBridgeV1(endpoint: endpoint, handle: handle)
  }

  /// Validate and invoke one method, rejecting substituted response identities and envelopes.
  public func invoke(_ method: KagemushaCoreCoordinatorMethodV1, fields: [Data]) throws -> [Data] {
    lock.lock()
    defer { lock.unlock() }
    let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(method, fields: fields)
    let response = try endpoint.invoke(handle: handle, method: method.rawValue, request: request)
    return try KagemushaCoreCoordinatorFrameV1.decodeResponse(method, requestFrame: request, responseFrame: response)
  }

  private static func validatePath(_ path: String) throws -> Data {
    let bytes = Data(path.utf8)
    guard !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
      (1...4096).contains(bytes.count), !bytes.contains(0)
    else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid coordinator storage path") }
    return bytes
  }

  private final class NativeEndpoint: KagemushaCoreCoordinatorEndpointV1 {
    #if canImport(Darwin)
    private typealias ContractFn = @convention(c) (UnsafeMutablePointer<UInt32>?, Int) -> Int32
    private typealias OpenFn = @convention(c) (UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<UInt64>?) -> Int32
    private typealias InvokeFn = @convention(c) (
      UInt64, UInt8, UnsafePointer<UInt8>?, Int,
      UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<Int>?
    ) -> Int32
    private typealias FreeFn = @convention(c) (UnsafeMutableRawPointer?) -> Void

    private let contractFunction: ContractFn
    private let openFunction: OpenFn
    private let invokeFunction: InvokeFn
    private let freeFunction: FreeFn

    private init(contract: @escaping ContractFn, open: @escaping OpenFn, invoke: @escaping InvokeFn, free: @escaping FreeFn) {
      contractFunction = contract
      openFunction = open
      invokeFunction = invoke
      freeFunction = free
    }

    static func create() -> NativeEndpoint? {
      let (image, _) = NoritoBridgeLoader.openHandle()
      guard let image,
        let contract = dlsym(image, "connect_norito_kagemusha_core_coordinator_contract_v1"),
        let open = dlsym(image, "connect_norito_kagemusha_core_coordinator_open_v1"),
        let invoke = dlsym(image, "connect_norito_kagemusha_core_coordinator_invoke_v1"),
        let free = dlsym(image, "connect_norito_free")
      else { return nil }
      return NativeEndpoint(
        contract: unsafeBitCast(contract, to: ContractFn.self), open: unsafeBitCast(open, to: OpenFn.self),
        invoke: unsafeBitCast(invoke, to: InvokeFn.self), free: unsafeBitCast(free, to: FreeFn.self))
    }

    func contract() throws -> [UInt32] {
      var words = [UInt32](repeating: 0, count: 10)
      let status = words.withUnsafeMutableBufferPointer { contractFunction($0.baseAddress, $0.count) }
      guard status == 10 else { throw KagemushaCoreCoordinatorErrorV1.nativeFailure(status) }
      return words
    }

    func open(storagePath: Data) throws -> UInt64 {
      var handle: UInt64 = 0
      let status = storagePath.withUnsafeBytes {
        openFunction($0.bindMemory(to: UInt8.self).baseAddress, $0.count, &handle)
      }
      try requireSuccess(status)
      guard handle != 0 else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      return handle
    }

    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
      var pointer: UnsafeMutablePointer<UInt8>?
      var length = 0
      let status = request.withUnsafeBytes {
        invokeFunction(handle, method, $0.bindMemory(to: UInt8.self).baseAddress, $0.count, &pointer, &length)
      }
      defer { if let pointer { freeFunction(UnsafeMutableRawPointer(pointer)) } }
      try requireSuccess(status)
      guard let pointer, (16...KagemushaCoreCoordinatorFrameV1.maximumResponseBytes).contains(length) else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid native response buffer")
      }
      return Data(bytes: pointer, count: length)
    }

    private func requireSuccess(_ status: Int32) throws {
      if status == -312 { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      guard status == 0 else { throw KagemushaCoreCoordinatorErrorV1.nativeFailure(status) }
    }
    #else
    static func create() -> NativeEndpoint? { nil }
    func contract() throws -> [UInt32] { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func open(storagePath: Data) throws -> UInt64 { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    #endif
  }
}

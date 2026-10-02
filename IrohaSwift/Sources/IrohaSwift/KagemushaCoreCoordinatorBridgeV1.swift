import Foundation
#if canImport(Darwin)
import Darwin
#endif

/// C ABI endpoint. Test endpoints do not qualify a monetary provider.
protocol KagemushaCoreCoordinatorEndpointV1: AnyObject {
  func contract() throws -> [UInt32]
  func install(storagePath: Data) throws
  func open(storagePath: Data) throws -> UInt64
  func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data
  func invokeIncoming(request: Data) throws -> Data
  func invokeIntegrity(phase: UInt8, handle: UInt64, original: Data) throws -> Data
  func close(handle: UInt64) throws
}

/// Native Core's exact acknowledgment of one already committed App Attest transition.
/// The initializer remains inside the SDK's validated native coordinator path. Frame
/// correlation does not qualify the backend; stock method 13 remains unavailable.
public struct KagemushaAppAttestCoreCommitAcknowledgmentV1: Equatable, Sendable {
  public let operationID: Data
  public let keyIDDigest: Data
  public let selectionDigest: Data
  public let rawAssertionDigest: Data
  public let committedCounter: UInt32
  public let terminalCertificateDigest: Data
  public let installedEnvelopeDigest: Data

  fileprivate init(_ response: [Data]) {
    operationID = response[0]
    keyIDDigest = response[1]
    selectionDigest = response[2]
    rawAssertionDigest = response[3]
    committedCounter = response[4].enumerated().reduce(UInt32(0)) {
      $0 | (UInt32($1.element) << ($1.offset * 8))
    }
    terminalCertificateDigest = response[5]
    installedEnvelopeDigest = response[6]
  }
}

/// Bounded untrusted projections of one native-owned recovered account attempt.
/// Native Core retains its fresh nonce, original checkpoint and 120-second continuous
/// deadline. This value alone grants neither a recovered session nor monetary authority.
public struct KagemushaEnrolledRecoveryAttemptV1: Equatable, Sendable {
  public let attemptID: Data
  public let challenge: KagemushaEnrolledOpenAccountChallengeV1
  public let canonicalAccountChallenge: Data
  public let accountSigningMessage: Data
  public let canonicalDeviceCommand: Data
  public let deviceRequestID: Data

  /// Validate the exact method-12 phase-9 projection, without admitting its authority.
  public init(nativeFields: [Data]) throws {
    let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(9)])
    _ = try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: request, fields: nativeFields)
    attemptID = Data(nativeFields[0])
    canonicalAccountChallenge = Data(nativeFields[1])
    challenge = try KagemushaEnrolledOpenAccountChallengeV1.decodeCanonicalExact(nativeFields[1])
    accountSigningMessage = Data(nativeFields[2])
    canonicalDeviceCommand = Data(nativeFields[3])
    deviceRequestID = Data(nativeFields[4])
  }
}

/// Serialized transport to the process-owned native coordinator, without a software backend.
/// Contract matching proves ABI compatibility only; native Core must admit its qualified hardware.
/// Returned Norito archives remain opaque. Close and every post-dispatch failure revoke the handle;
/// a new open needs a fresh process. Uncertain monetary state remains owned by the qualified backend.
public final class KagemushaCoreCoordinatorBridgeV1 {
  private let endpoint: any KagemushaCoreCoordinatorEndpointV1
  private var handle: UInt64
  private let lock = NSLock()
  private static let expectedContract: [UInt32] = [2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21]

  private init(endpoint: any KagemushaCoreCoordinatorEndpointV1, handle: UInt64) {
    self.endpoint = endpoint
    self.handle = handle
  }

  deinit { try? close() }

  /// Install the independently provisioned Rust owner, then open the exact native ABI.
  /// Only the storage path crosses this boundary; no caller policies or authority claims
  /// can install a backend. Missing native provisioning or any rejected install fails closed.
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
    try endpoint.install(storagePath: encodedPath)
    let handle = try endpoint.open(storagePath: encodedPath)
    guard handle != 0 else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    return KagemushaCoreCoordinatorBridgeV1(endpoint: endpoint, handle: handle)
  }

  /// Validate and invoke one method, rejecting substituted response identities and envelopes.
  public func invoke(_ method: KagemushaCoreCoordinatorMethodV1, fields: [Data]) throws -> [Data] {
    lock.lock()
    defer { lock.unlock() }
    guard handle != 0 else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(method, fields: fields)
    do {
      let response = try endpoint.invoke(handle: handle, method: method.rawValue, request: request)
      return try KagemushaCoreCoordinatorFrameV1.decodeResponse(method, requestFrame: request, responseFrame: response)
    } catch {
      // Native dispatch or publication may have advanced hardware before the response failed.
      let closing = handle
      handle = 0
      try? endpoint.close(handle: closing)
      throw error
    }
  }

  /// Dispatch the closed ordinary incoming lifecycle on this exact retained descriptor.
  /// Native owns Mint/Receive admission, clock renewal, proofs and durable replay. Invalid
  /// requests never dispatch; uncertain dispatch or response corruption permanently closes.
  public func invokeOrdinaryIncoming(_ phase: KagemushaOrdinaryIncomingPhaseV1,
    originals: [Data] = []) throws -> [Data] {
    lock.lock()
    defer { lock.unlock() }
    guard handle != 0 else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    let request = try KagemushaOrdinaryIncomingFrameV1.encodeRequest(phase,
      handle: handle, originals: originals)
    do {
      let response = try endpoint.invokeIncoming(request: request)
      return try KagemushaOrdinaryIncomingFrameV1.decodeResponse(phase,
        handle: handle, response: response)
    } catch {
      let closing = handle
      handle = 0
      try? endpoint.close(handle: closing)
      throw error
    }
  }

  // Same owned descriptor, input-free phase10 only. Full PI mutation remains Native-owned.
  func completedAppKeyFields() throws -> [Data] {
    lock.lock()
    defer { lock.unlock() }
    guard handle != 0 else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    do {
      let response = try endpoint.invokeIntegrity(phase: 10, handle: handle, original: Data())
      return try KagemushaCompletedAppKeyFrameV1.decodeResponse(handle: handle, response: response)
    } catch {
      let closing = handle
      handle = 0
      try? endpoint.close(handle: closing)
      throw error
    }
  }

  /// Begin possession recovery from the installed native owner's retained enrollment.
  /// The response must be correlated against independently held app selection before signing.
  public func beginEnrolledRecovery() throws -> KagemushaEnrolledRecoveryAttemptV1 {
    try KagemushaEnrolledRecoveryAttemptV1(nativeFields: invoke(.initialEnrollment,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(9)]))
  }

  /// Return the exact account signature and complete original authenticated hardware frame.
  /// Only native Core may publish a recovered lease after authenticating both proofs.
  public func completeEnrolledRecovery(_ attempt: KagemushaEnrolledRecoveryAttemptV1,
    accountSignature: Data, originalDeviceResponse: Data) throws {
    _ = try invoke(.initialEnrollment, fields: [KagemushaCoreCoordinatorFrameV1.u32(10),
      attempt.attemptID, accountSignature, originalDeviceResponse])
  }

  /// Cancel the exact outstanding native attempt without creating another attempt.
  /// Retire a completed recovered lease by closing its original owning handle.
  public func cancelEnrolledRecovery(_ attempt: KagemushaEnrolledRecoveryAttemptV1) throws {
    _ = try invoke(.initialEnrollment, fields: [KagemushaCoreCoordinatorFrameV1.u32(11), attempt.attemptID])
  }

  /// Query native Core's retained committed terminal and original assertion before lane advance.
  /// The qualified backend must independently authenticate all fields from its durable journal.
  public func acknowledgeCommittedAppAttest(
    operationID: Data, keyID: String, binding: KagemushaAppAttestTransitionBindingV1,
    rawAssertion: Data, previousCounter: UInt32,
    terminalCertificateDigest: Data, installedEnvelopeDigest: Data
  ) throws -> KagemushaAppAttestCoreCommitAcknowledgmentV1 {
    let response = try invoke(.acknowledgeCommittedAppAttest, fields: [
      operationID, Data(keyID.utf8), binding.canonicalSelectionSigningBytes,
      rawAssertion, KagemushaCoreCoordinatorFrameV1.u32(previousCounter),
      terminalCertificateDigest, installedEnvelopeDigest,
    ])
    return KagemushaAppAttestCoreCommitAcknowledgmentV1(response)
  }

  /// Internal exact-symbol transports share this same descriptor monitor and close policy.
  /// No public handle, endpoint callback or authority object is introduced.
  func withOrdinaryDescriptor<T>(_ action: (UInt64) throws -> T) throws -> T {
    lock.lock()
    defer { lock.unlock() }
    guard handle != 0 else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    do { return try action(handle) }
    catch {
      let closing = handle
      handle = 0
      try? endpoint.close(handle: closing)
      throw error
    }
  }

  func requireOrdinaryDescriptorOpen() throws {
    lock.lock()
    defer { lock.unlock() }
    guard handle != 0 else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
  }

  /// Revoke locally before delegated teardown; repeated close is harmless.
  public func close() throws {
    lock.lock()
    defer { lock.unlock() }
    let closing = handle
    if closing == 0 { return }
    handle = 0
    try endpoint.close(handle: closing)
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
    private typealias InstallFn = @convention(c) (UnsafePointer<UInt8>?, Int) -> Int32
    private typealias OpenFn = @convention(c) (UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<UInt64>?) -> Int32
    private typealias InvokeFn = @convention(c) (
      UInt64, UInt8, UnsafePointer<UInt8>?, Int,
      UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<Int>?
    ) -> Int32
    private typealias IncomingFn = @convention(c) (
      UnsafePointer<UInt8>?, Int,
      UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<Int>?
    ) -> Int32
    private typealias IntegrityFn = @convention(c) (
      UInt8, UInt64, UnsafePointer<UInt8>?, Int,
      UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<Int>?
    ) -> Int32
    private typealias CloseFn = @convention(c) (UInt64) -> Int32
    private typealias FreeFn = @convention(c) (UnsafeMutableRawPointer?) -> Void

    private let contractFunction: ContractFn
    private let installFunction: InstallFn
    private let openFunction: OpenFn
    private let invokeFunction: InvokeFn
    private let incomingFunction: IncomingFn
    private let integrityFunction: IntegrityFn
    private let closeFunction: CloseFn
    private let freeFunction: FreeFn

    private init(contract: @escaping ContractFn, install: @escaping InstallFn, open: @escaping OpenFn, invoke: @escaping InvokeFn, incoming: @escaping IncomingFn, integrity: @escaping IntegrityFn, close: @escaping CloseFn, free: @escaping FreeFn) {
      contractFunction = contract
      installFunction = install
      openFunction = open
      invokeFunction = invoke
      incomingFunction = incoming
      integrityFunction = integrity
      closeFunction = close
      freeFunction = free
    }

    static func create() -> NativeEndpoint? {
      let (image, _) = NoritoBridgeLoader.openHandle()
      guard let image,
        let contract = dlsym(image, "connect_norito_kagemusha_core_coordinator_contract_v1"),
        let install = dlsym(image, "connect_norito_kagemusha_core_coordinator_install_v1"),
        let open = dlsym(image, "connect_norito_kagemusha_core_coordinator_open_v1"),
        let invoke = dlsym(image, "connect_norito_kagemusha_core_coordinator_invoke_v1"),
        let incoming = dlsym(image, "connect_norito_kagemusha_ordinary_incoming_v1"),
        let integrity = dlsym(image, "connect_norito_kagemusha_ordinary_integrity_refresh_v1"),
        let close = dlsym(image, "connect_norito_kagemusha_core_coordinator_close_v1"),
        let free = dlsym(image, "connect_norito_free")
      else { return nil }
      return NativeEndpoint(
        contract: unsafeBitCast(contract, to: ContractFn.self), install: unsafeBitCast(install, to: InstallFn.self),
        open: unsafeBitCast(open, to: OpenFn.self),
        invoke: unsafeBitCast(invoke, to: InvokeFn.self),
        incoming: unsafeBitCast(incoming, to: IncomingFn.self),
        integrity: unsafeBitCast(integrity, to: IntegrityFn.self), close: unsafeBitCast(close, to: CloseFn.self),
        free: unsafeBitCast(free, to: FreeFn.self))
    }

    func contract() throws -> [UInt32] {
      var words = [UInt32](repeating: 0, count: 12)
      let status = words.withUnsafeMutableBufferPointer { contractFunction($0.baseAddress, $0.count) }
      guard status == 12 else { throw KagemushaCoreCoordinatorErrorV1.nativeFailure(status) }
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

    func install(storagePath: Data) throws {
      let status = storagePath.withUnsafeBytes {
        installFunction($0.bindMemory(to: UInt8.self).baseAddress, $0.count)
      }
      try requireSuccess(status)
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

    func invokeIncoming(request: Data) throws -> Data {
      var pointer: UnsafeMutablePointer<UInt8>?
      var length = 0
      let status = request.withUnsafeBytes {
        incomingFunction($0.bindMemory(to: UInt8.self).baseAddress, $0.count, &pointer, &length)
      }
      defer { if let pointer { freeFunction(UnsafeMutableRawPointer(pointer)) } }
      try requireSuccess(status)
      guard let pointer, (1...KagemushaOrdinaryIncomingFrameV1.frameMaximum).contains(length)
      else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid ordinary incoming Native buffer") }
      return Data(bytes: pointer, count: length)
    }

    func invokeIntegrity(phase: UInt8, handle: UInt64, original: Data) throws -> Data {
      var pointer: UnsafeMutablePointer<UInt8>?
      var length = 0
      let status = original.withUnsafeBytes {
        integrityFunction(phase, handle, $0.bindMemory(to: UInt8.self).baseAddress,
          $0.count, &pointer, &length)
      }
      defer { if let pointer { freeFunction(UnsafeMutableRawPointer(pointer)) } }
      try requireSuccess(status)
      guard let pointer, (1...KagemushaCompletedAppKeyFrameV1.frameMaximum).contains(length)
      else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid completed-key Native buffer") }
      return Data(bytes: pointer, count: length)
    }

    func close(handle: UInt64) throws {
      try requireSuccess(closeFunction(handle))
    }

    private func requireSuccess(_ status: Int32) throws {
      if status == -312 { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      guard status == 0 else { throw KagemushaCoreCoordinatorErrorV1.nativeFailure(status) }
    }
    #else
    static func create() -> NativeEndpoint? { nil }
    func contract() throws -> [UInt32] { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func install(storagePath: Data) throws { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func open(storagePath: Data) throws -> UInt64 { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeIncoming(request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeIntegrity(phase: UInt8, handle: UInt64, original: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func close(handle: UInt64) throws { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    #endif
  }
}

import CryptoKit
import Foundation

/// A native-retained FI ceremony. No public archive initializer creates this holder.
/// Its signatures and certificates remain original bytes; native Core authenticates them.
/// An enrollment confirmation does not qualify State/Guard proofs or enable cash dispatch.
public final class KagemushaNativePreparedRetailEnrollmentV1: @unchecked Sendable {
  private let bridge: KagemushaCoreCoordinatorBridgeV1
  private let possession: KagemushaNativePreparedAppEnrollmentPossessionV1
  private let possessionTicket: Data
  private let original: [Data]
  private let lock = NSRecursiveLock()
  private var signature: Data?
  private var certificate: Data?
  private var confirmation: [Data]?
  private var observedState: KagemushaNativeRetailEnrollmentRecoveryV1.State = .prepared
  private var unusable = false

  private init(bridge: KagemushaCoreCoordinatorBridgeV1,
    possession: KagemushaNativePreparedAppEnrollmentPossessionV1, possessionTicket: Data,
    original: [Data]) {
    self.bridge = bridge; self.possession = possession
    self.possessionTicket = Data(possessionTicket); self.original = original.map { Data($0) }
  }

  static func prepare(bridge: KagemushaCoreCoordinatorBridgeV1,
    possession: KagemushaNativePreparedAppEnrollmentPossessionV1,
    identity: KagemushaNativeOrdinaryAppIdentityConfirmationV1,
    originalChallenge: Data, accountSigningMessage: Data) throws -> KagemushaNativePreparedRetailEnrollmentV1 {
    let challenge = Data(originalChallenge), message = Data(accountSigningMessage)
    // Validate bounded untrusted input before touching the native owner.
    guard (1...32768).contains(challenge.count), KagemushaAppPlatformPreparedProjectionV1.digest(message) else {
      throw invalid()
    }
    do {
      let parent = try possession.recheck()
      guard identity.pendingScope == parent.nativeScope,
        KagemushaAppPlatformPreparedProjectionV1.digest(identity.credentialDigest) else { throw invalid() }
      let fields = try bridge.invoke(.appEnrollmentPossession,
        fields: [KagemushaCoreCoordinatorFrameV1.u32(9), parent.ticket, challenge, message])
      guard fields[3] == identity.pendingScope, fields[4] == identity.credentialDigest else { throw invalid() }
      let result = KagemushaNativePreparedRetailEnrollmentV1(bridge: bridge, possession: possession,
        possessionTicket: parent.ticket, original: fields)
      try result.recheck()
      return result
    } catch { try? bridge.close(); throw error }
  }

  public func originalChallengeBytes() throws -> Data {
    try guarded { try recheck(); return Data(original[1]) }
  }
  public func accountSigningMessage() throws -> Data {
    try guarded { try recheck(); return Data(original[2]) }
  }

  /// Fsync the native invocation fence before signing. Unknown outcomes never permit another signature.
  public func fenceAccountSigning() throws -> KagemushaNativeRetailEnrollmentWalletActionV1 {
    try guarded {
      try recheck()
      let before = try readRecovery()
      // Recovery state1 records a previous invocation, even when no original was
      // returned. Only phase10's transition from prepared may authorize signing.
      guard before.state != .invocationUnknown else { throw Self.invalid() }
      let fields = try call(10)
      if fields[0] == Data([1]) {
        guard before.state == .prepared, observedState == .prepared,
          signature == nil, certificate == nil else { throw Self.invalid() }
        observedState = .invocationUnknown
        try recheck()
        return .signOriginalMessage(Data(original[2]))
      }
      try remember(signature: fields[1], certificate: nil)
      let recovered = try readRecovery()
      guard recovered.state.rawValue >= 2, recovered.accountSignature == fields[1] else { throw Self.invalid() }
      try recheck()
      return .retainedOriginalSignature(Data(fields[1]))
    }
  }

  /// Return the original wallet signature to the fenced native owner before issuer transport.
  public func retainOriginalAccountSignature(_ bytes: Data) throws {
    let offered = Data(bytes)
    guard offered.count == 64 else { throw Self.invalid() }
    try guarded {
      try recheck()
      if let signature, signature != offered { throw Self.invalid() }
      _ = try call(11, raw: offered)
      let recovered = try readRecovery()
      guard recovered.state.rawValue >= 2, recovered.accountSignature == offered else { throw Self.invalid() }
      try recheck()
    }
  }

  /// Read retained originals without invoking a signer or issuer. A retained certificate must
  /// still be returned through phase12 to complete or recover native financial custody.
  public func recoverOriginals() throws -> KagemushaNativeRetailEnrollmentRecoveryV1 {
    try guarded { try recheck(); let result = try readRecovery(); try recheck(); return result }
  }

  /// Offer the complete issuer original. Native verifies FI/wallet/app binding and retains
  /// the financial witness. This confirmation grants no app-level cash or provider readiness.
  public func acceptOriginalEnrollmentCertificate(_ bytes: Data) throws
    -> KagemushaNativeRetailEnrollmentConfirmationV1 {
    let offered = Data(bytes)
    guard (1...16384).contains(offered.count) else { throw Self.invalid() }
    return try guarded {
      try recheck()
      if let certificate, certificate != offered { throw Self.invalid() }
      let accepted = try call(12, raw: offered)
      guard accepted[1] == original[3], confirmation == nil || confirmation == accepted else { throw Self.invalid() }
      let recovered = try readRecovery()
      guard recovered.state == .certificateRetained, recovered.certificateOriginal == offered else { throw Self.invalid() }
      confirmation = accepted.map { Data($0) }
      try recheck()
      return KagemushaNativeRetailEnrollmentConfirmationV1(enrollmentID: accepted[0],
        pendingScope: accepted[1], credentialDigest: original[4])
    }
  }

  /// Prepare the same native zero-state Bootstrap after FI has retained its certificate.
  /// The selector cannot construct S, W, a captured approval or monetary authority.
  public func prepareBootstrapAppApproval() throws -> KagemushaNativePreparedBootstrapAppApprovalV1 {
    return try guarded {
      _ = try recheckForBootstrap()
      guard let certificate else { throw Self.invalid() }
      let operationID = Data(SHA256.hash(data:
        Data("iroha:kagemusha:v1:ordinary-bootstrap-operation-id\0".utf8) + certificate))
      return try KagemushaNativePreparedBootstrapAppApprovalV1.prepare(bridge: bridge,
        retail: self, operationID: operationID)
    }
  }

  func recheckForBootstrap() throws -> (enrollmentID: Data, credentialDigest: Data,
    app: KagemushaAppPlatformPreparedProjectionV1) {
    try guarded {
      try recheck()
      guard let confirmation, confirmation.count == 2, certificate != nil,
        try readRecovery().state == .certificateRetained else { throw Self.invalid() }
      return (Data(confirmation[0]), Data(original[4]), try possession.recheck())
    }
  }

  /// Native permits cancellation only before a wallet invocation. Cancelled originals cannot be reused.
  public func cancel() throws {
    try guarded {
      try recheck()
      guard try readRecovery().state == .prepared else { throw Self.invalid() }
      _ = try call(14)
      unusable = true
    }
  }

  private func readRecovery() throws -> KagemushaNativeRetailEnrollmentRecoveryV1 {
    let fields = try call(13)
    guard let state = KagemushaNativeRetailEnrollmentRecoveryV1.State(rawValue: fields[0][0]),
      state.rawValue >= observedState.rawValue else { throw Self.invalid() }
    // A previously returned original may not disappear or change across native reads.
    if let signature, signature != fields[1] { throw Self.invalid() }
    if let certificate, certificate != fields[2] { throw Self.invalid() }
    if state.rawValue >= 2 { try remember(signature: fields[1], certificate: state == .certificateRetained ? fields[2] : nil) }
    observedState = state
    return KagemushaNativeRetailEnrollmentRecoveryV1(state: state, signature: fields[1], certificate: fields[2])
  }
  private func remember(signature offeredSignature: Data, certificate offeredCertificate: Data?) throws {
    if let signature, signature != offeredSignature { throw Self.invalid() }
    if let offeredCertificate {
      if let certificate, certificate != offeredCertificate { throw Self.invalid() }
      certificate = Data(offeredCertificate)
    }
    signature = Data(offeredSignature)
  }
  private func recheck() throws {
    let parent = try possession.recheck()
    guard parent.ticket == possessionTicket, parent.nativeScope == original[3] else { throw Self.invalid() }
    let fields = try bridge.invoke(.appEnrollmentPossession,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(9), possessionTicket, original[1], original[2]])
    guard fields == original else { throw Self.invalid() }
    _ = try possession.recheck()
  }
  private func call(_ phase: UInt32, raw: Data? = nil) throws -> [Data] {
    var fields = [KagemushaCoreCoordinatorFrameV1.u32(phase), original[0]]
    if let raw { fields.append(raw) }
    return try bridge.invoke(.appEnrollmentPossession, fields: fields)
  }
  private func guarded<T>(_ operation: () throws -> T) throws -> T {
    lock.lock(); defer { lock.unlock() }
    guard !unusable else { throw Self.invalid() }
    do { return try operation() }
    catch { unusable = true; try? bridge.close(); throw error }
  }
  private static func invalid() -> KagemushaCoreCoordinatorErrorV1 {
    .invalidFrame("native retail enrollment original differs")
  }
}

/// A native invocation fence result, never a signature or monetary authority factory.
public enum KagemushaNativeRetailEnrollmentWalletActionV1: Sendable {
  case signOriginalMessage(Data)
  case retainedOriginalSignature(Data)
}

/// Exact native recovery projection. Invoked/unknown cannot return to prepared.
public struct KagemushaNativeRetailEnrollmentRecoveryV1: Sendable {
  public enum State: UInt8, Sendable { case prepared = 0, invocationUnknown, signatureRetained, certificateRetained }
  public let state: State
  public let accountSignature: Data
  public let certificateOriginal: Data
  fileprivate init(state: State, signature: Data, certificate: Data) {
    self.state = state; accountSignature = Data(signature); certificateOriginal = Data(certificate)
  }
}

/// Native acknowledgement of the same retained FI certificate; no cash-readiness claim.
public struct KagemushaNativeRetailEnrollmentConfirmationV1: Sendable {
  public let enrollmentID: Data
  public let pendingScope: Data
  public let credentialDigest: Data
  fileprivate init(enrollmentID: Data, pendingScope: Data, credentialDigest: Data) {
    self.enrollmentID = Data(enrollmentID); self.pendingScope = Data(pendingScope)
    self.credentialDigest = Data(credentialDigest)
  }
}

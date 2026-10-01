import Foundation

/// A process-local reservation retained by native Core. Its carrier contains no financial secret.
/// Only exact signed-original intake on this same holder can obtain a prepared identity.
public final class KagemushaNativeReservedOrdinaryAppIdentityV1: @unchecked Sendable {
  private let bridge: KagemushaCoreCoordinatorBridgeV1
  private let original: KagemushaOrdinaryAppIdentityReservationProjectionV1
  private let lock = NSRecursiveLock()
  private var acceptedOriginal: Data?
  private var unusable = false

  private init(bridge: KagemushaCoreCoordinatorBridgeV1,
    original: KagemushaOrdinaryAppIdentityReservationProjectionV1) {
    self.bridge = bridge; self.original = original
  }

  static func reserveOriginal(bridge: KagemushaCoreCoordinatorBridgeV1) throws
    -> KagemushaNativeReservedOrdinaryAppIdentityV1 {
    do {
      let fields = try bridge.invoke(.preparedOrdinaryAppIdentity,
        fields: [KagemushaCoreCoordinatorFrameV1.u32(12)])
      let result = KagemushaNativeReservedOrdinaryAppIdentityV1(bridge: bridge,
        original: try KagemushaOrdinaryAppIdentityReservationProjectionV1(fields))
      _ = try result.read(0)
      return result
    } catch { try? bridge.close(); throw error }
  }

  public func accountID() throws -> String { String(decoding: try read(1), as: UTF8.self) }
  public func clientNonce() throws -> Data { try read(2) }
  public func releaseID() throws -> Data { try read(3) }
  public func hardwareProfileID() throws -> Data { try read(4) }
  public func laneID() throws -> Data { try read(5) }
  public func financialAuthorityCommitment() throws -> Data { try read(6) }
  public func requestID() throws -> String { String(decoding: try read(7), as: UTF8.self) }

  /// Original native-selected policy bytes, or empty when absent; this grants no verdict.
  public func originalPlayIntegrityPolicyBytes() throws -> Data {
    try guarded {
      try recheck()
      let policy = try bridge.invoke(.preparedOrdinaryAppIdentity,
        fields: [KagemushaCoreCoordinatorFrameV1.u32(14), original.ticket])[0]
      try recheck()
      return Data(policy)
    }
  }

  /// Native Core authenticates this exact signed 515-byte original against its fsynced reservation.
  public func acceptOriginalSignedPreparation(_ bytes: Data) throws
    -> KagemushaNativePreparedOrdinaryAppIdentityV1 {
    let offered = Data(bytes)
    _ = try KagemushaOrdinaryAppIdentityPreparedProjectionV1.challenge(transport: offered)
    return try guarded {
      try recheck()
      if let acceptedOriginal, acceptedOriginal != offered { throw invalid() }
      let fields = try bridge.invoke(.preparedOrdinaryAppIdentity,
        fields: [KagemushaCoreCoordinatorFrameV1.u32(13), original.ticket, offered])
      let prepared = try KagemushaOrdinaryAppIdentityPreparedProjectionV1(fields)
      // The reservation ticket and the returned Attempt ticket are separate native identities.
      guard prepared.signedChallenge == offered, original.matches(prepared.challenge) else {
        throw invalid()
      }
      try recheck()
      let result = try KagemushaNativePreparedOrdinaryAppIdentityV1.fromNative(
        bridge: bridge, original: prepared)
      acceptedOriginal = offered
      return result
    }
  }

  private func read(_ index: Int) throws -> Data {
    try guarded { try recheck(); return Data(original.fields[index]) }
  }
  private func recheck() throws {
    let fields = try bridge.invoke(.preparedOrdinaryAppIdentity,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(12)])
    guard fields == original.fields else { throw invalid() }
  }
  private func guarded<T>(_ operation: () throws -> T) throws -> T {
    lock.lock(); defer { lock.unlock() }
    guard !unusable else { throw invalid() }
    do { return try operation() }
    catch { unusable = true; try? bridge.close(); throw error }
  }
  private func invalid() -> KagemushaCoreCoordinatorErrorV1 {
    .invalidFrame("native preparation substituted the original reservation")
  }
}

extension KagemushaCoreCoordinatorBridgeV1 {
  /// Read an existing native selector without creating an identity or reservation.
  public func originalEnrollmentID() throws -> Data {
    try invoke(.preparedOrdinaryAppIdentity, fields: [KagemushaCoreCoordinatorFrameV1.u32(11)])[0]
  }
  /// Reserve the native-selected account and nonce before requesting the exact issuer original.
  public func reserveOriginalIdentity() throws -> KagemushaNativeReservedOrdinaryAppIdentityV1 {
    try KagemushaNativeReservedOrdinaryAppIdentityV1.reserveOriginal(bridge: self)
  }
}

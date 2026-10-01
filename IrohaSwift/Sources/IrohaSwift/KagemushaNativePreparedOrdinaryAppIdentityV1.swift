import CryptoKit
import Foundation

/// A process-local original C capability obtained only through native signed-original admission.
/// No public byte, verdict or challenge initializer exists. Financial transitions remain separate.
public final class KagemushaNativePreparedOrdinaryAppIdentityV1: @unchecked Sendable {
  private let bridge: KagemushaCoreCoordinatorBridgeV1
  private let original: KagemushaOrdinaryAppIdentityPreparedProjectionV1
  private let lock = NSRecursiveLock()
  private var unusable = false

  private init(bridge: KagemushaCoreCoordinatorBridgeV1,
    original: KagemushaOrdinaryAppIdentityPreparedProjectionV1) {
    self.bridge = bridge; self.original = original
  }
  static func fromNative(bridge: KagemushaCoreCoordinatorBridgeV1,
    original: KagemushaOrdinaryAppIdentityPreparedProjectionV1) throws
    -> KagemushaNativePreparedOrdinaryAppIdentityV1 {
    let result = KagemushaNativePreparedOrdinaryAppIdentityV1(bridge: bridge, original: original)
    _ = try result.recheck()
    return result
  }
  public func originalChallengeSigningBytes() throws -> Data {
    try guarded { _ = try recheck(); return Data(original.signingBytes) }
  }
  public func originalSignedPreparationBytes() throws -> Data {
    try guarded { _ = try recheck(); return Data(original.signedChallenge) }
  }
  public func cancel() throws { try guarded { _ = try call(9); unusable = true } }
  func recheck() throws -> KagemushaOrdinaryAppIdentityPreparedProjectionV1 {
    try guarded {
      let r = try call(8)
      guard r[0] == original.nativeScope, r[1] == original.generationChallenge else { throw Self.invalid() }
      return original
    }
  }
  func recover() throws -> KagemushaOrdinaryAppIdentityRecoveryProjectionV1 {
    try guarded {
      _ = try recheck()
      let result = try KagemushaOrdinaryAppIdentityRecoveryProjectionV1(call(7), original: original)
      _ = try recheck(); return result
    }
  }
  func fenceGeneration() throws -> (fresh: Bool, keyReference: String) {
    try guarded {
      _ = try recheck(); let r = try call(2); _ = try recheck()
      return (r[0] == Data([1]), r[1].isEmpty ? "" : try original.validateKeyReference(r[1]))
    }
  }
  func retainKey(_ keyReference: String) throws {
    try guarded {
      _ = try original.validateKeyReference(Data(keyReference.utf8)); _ = try recheck()
      _ = try call(3, extra: [Data(keyReference.utf8)]); _ = try recheck()
    }
  }
  func fenceAttestation() throws -> Bool {
    try guarded { _ = try recheck(); let r = try call(4); _ = try recheck(); return r[0] == Data([1]) }
  }
  func retainAttestation(point: Data, raw: Data) throws {
    guard (1...131_072).contains(raw.count) else { throw Self.invalid() }
    try guarded {
      let retained = try recover(); guard retained.state == 3 else { throw Self.invalid() }
      _ = try original.validateKeyReference(Data(retained.keyReference.utf8), point: point)
      _ = try recheck()
      _ = try call(5, extra: [point, Data(raw.prefix(65_536)), Data(raw.dropFirst(65_536))])
      _ = try recheck()
    }
  }
  func readRetainedAttestation(_ metadata: KagemushaOrdinaryAppIdentityRecoveryProjectionV1) throws -> Data {
    try guarded {
      guard metadata.state >= 4 else { throw Self.invalid() }; _ = try recheck()
      var raw = Data()
      let chunkCount = (metadata.rawLength + 65_535) / 65_536
      for index in 0..<chunkCount {
        let r = try call(10, extra: [KagemushaCoreCoordinatorFrameV1.u32(UInt32(index))])
        guard r[2] == metadata.rawDigest,
          Int(KagemushaAppPlatformPreparedProjectionV1.u32(r[3])) == metadata.rawLength else { throw Self.invalid() }
        raw.append(r[1])
      }
      guard raw.count == metadata.rawLength, Data(SHA256.hash(data: raw)) == metadata.rawDigest else { throw Self.invalid() }
      _ = try recheck(); return raw
    }
  }

  /// Read the native-retained evidence without key generation, issuer admission or possession.
  public func recoverOriginalAttestation() throws -> KagemushaNativeCollectedAppIdentityOriginalV1? {
    try guarded {
      let retained = try recover()
      guard retained.state >= 4 else { return nil }
      return KagemushaNativeCollectedAppIdentityOriginalV1(reference: retained.keyReference,
        point: retained.point, raw: try readRetainedAttestation(retained))
    }
  }
  /// Read the already admitted exact original without re-invoking phase 6.
  public func recoverOriginalAdmission() throws -> KagemushaNativeRawAppIdentityAdmissionV1? {
    try guarded {
      let retained = try recover()
      guard retained.state == 5 else { return nil }
      return try admission(retained, raw: readRetainedAttestation(retained))
    }
  }
  /// Explicit intake of the complete issuer original. Native verifies its signature and custody.
  public func acceptOriginalRawAdmission(_ bytes: Data) throws -> KagemushaNativeRawAppIdentityAdmissionV1 {
    let offered = Data(bytes)
    guard offered.count == 314 else { throw Self.invalid() }
    return try guarded {
      let retained = try recover()
      guard retained.state >= 4,
        retained.state != 5 || retained.rawAdmission == offered else { throw Self.invalid() }
      let raw = try readRetainedAttestation(retained)
      _ = try KagemushaOrdinaryAppIdentityRawProjectionV1(offered, original: original, retained: retained)
      let accepted = try call(6, extra: [offered]), completed = try recover()
      guard completed.state == 5, completed.keyReference == retained.keyReference,
        completed.point == retained.point, completed.rawDigest == retained.rawDigest,
        completed.rawLength == retained.rawLength, completed.rawAdmission == offered,
        accepted[0] == completed.pendingScope, accepted[1] == Data(SHA256.hash(data: offered)) else {
        throw Self.invalid()
      }
      return try admission(completed, raw: raw)
    }
  }

  /// Separately obtain genuine native E possession after raw admission. This invokes method 20;
  /// it neither completes a final credential nor grants financial authority.
  public func preparePendingAppAttestPossession() throws -> KagemushaNativePendingAppAttestIdentityV1 {
    try guarded {
      guard original.platform == 4 else { throw Self.invalid() }
      let retained = try recover()
      guard retained.state == 5 else { throw Self.invalid() }
      _ = try admission(retained, raw: readRetainedAttestation(retained))
      let a = retained.rawAdmission, c = original.challenge
      let possession = try bridge.prepareAppEnrollmentPossession(
        originalEnrollmentChallengeHash: original.generationChallenge)
      let e = try possession.recheck()
      let start = Data("iroha:kagemusha:v1:app-enrollment-possession\0".utf8).count + 8
      guard e.platform == 4, e.approval == nil, e.credentialDigest.isEmpty,
        e.enrollmentChallenge == original.signingBytes, e.nativeScope == retained.pendingScope,
        e.publicKeyX963 == retained.point, e.keyAlias == retained.keyReference,
        e.generationChallenge == original.generationChallenge, e.appleCounterFloor == 0,
        Data(e.signingBytes[(start + 323)..<(start + 355)]) == retained.rawDigest,
        e.appSigningIdentityDigest == Data(a[198..<230]), c.platform == 4 else { throw Self.invalid() }
      _ = try recheck()
      return KagemushaNativePendingAppAttestIdentityV1(possession: possession, keyReference: retained.keyReference)
    }
  }
  private func admission(_ retained: KagemushaOrdinaryAppIdentityRecoveryProjectionV1,
    raw: Data) throws -> KagemushaNativeRawAppIdentityAdmissionV1 {
    let joined = try KagemushaOrdinaryAppIdentityRawProjectionV1(retained.rawAdmission,
      original: original, retained: retained)
    guard retained.state == 5, joined.pendingScope(nativeScope: original.nativeScope) == retained.pendingScope else {
      throw Self.invalid()
    }
    return KagemushaNativeRawAppIdentityAdmissionV1(reference: retained.keyReference,
      point: retained.point, raw: raw, admission: retained.rawAdmission, scope: retained.pendingScope)
  }
  private func call(_ phase: UInt32, extra: [Data] = []) throws -> [Data] {
    try bridge.invoke(.preparedOrdinaryAppIdentity,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(phase), original.ticket] + extra)
  }
  private func guarded<T>(_ operation: () throws -> T) throws -> T {
    lock.lock(); defer { lock.unlock() }
    guard !unusable else { throw Self.invalid() }
    do { return try operation() }
    catch { unusable = true; try? bridge.close(); throw error }
  }
  private static func invalid() -> KagemushaCoreCoordinatorErrorV1 {
    .invalidFrame("native ordinary identity original differs")
  }
}

/// Read-only exact retained platform evidence, before issuer admission.
public struct KagemushaNativeCollectedAppIdentityOriginalV1: Sendable {
  public let keyReference: String
  public let publicKeyX963, rawAttestation: Data
  fileprivate init(reference: String, point: Data, raw: Data) {
    keyReference = reference; publicKeyX963 = Data(point); rawAttestation = Data(raw)
  }
}

/// Native raw-identity readback. Final credential and financial authority remain separate.
public struct KagemushaNativeRawAppIdentityAdmissionV1: Sendable {
  public let keyReference: String
  public let publicKeyX963, rawAttestation, signedAdmission, pendingScope: Data
  fileprivate init(reference: String, point: Data, raw: Data, admission: Data, scope: Data) {
    keyReference = reference; publicKeyX963 = Data(point); rawAttestation = Data(raw)
    signedAdmission = Data(admission); pendingScope = Data(scope)
  }
}

/// Native-admitted raw Apple enrollment and its genuine pending E capability.
/// It is not a final identity credential, a financial owner or a spending lease.
public struct KagemushaNativePendingAppAttestIdentityV1: Sendable {
  public let possession: KagemushaNativePreparedAppEnrollmentPossessionV1
  private let keyReference: String
  fileprivate init(possession: KagemushaNativePreparedAppEnrollmentPossessionV1, keyReference: String) {
    self.possession = possession; self.keyReference = keyReference
  }
  /// Establish a new assertion journal only while native E is uninvoked and its counter floor is zero.
  /// Existing or interrupted journal files are never reset or repaired here.
  public func bootstrapNewAssertionJournal(directoryURL: URL) throws -> KagemushaAppAttestFileIntentStoreV1 {
    let p = try possession.recheck(), r = try possession.recover()
    guard p.platform == 4, p.keyAlias == keyReference, p.appleCounterFloor == 0, r.state == 0 else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    let store = try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(directoryURL: directoryURL, keyID: keyReference)
    _ = try possession.recheck(); return store
  }
}

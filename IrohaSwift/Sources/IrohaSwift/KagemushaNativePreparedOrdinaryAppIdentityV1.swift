import CryptoKit
import Foundation

/// A process-local original C capability created only by the genuine native owner.
/// No public byte, verdict or challenge initializer exists. This is ordinary app
/// enrollment authority; it cannot authorize a financial state transition.
public final class KagemushaNativePreparedOrdinaryAppIdentityV1: @unchecked Sendable {
  private let bridge: KagemushaCoreCoordinatorBridgeV1
  private let original: KagemushaOrdinaryAppIdentityPreparedProjectionV1
  private let originalID: Data

  private init(bridge: KagemushaCoreCoordinatorBridgeV1,
    original: KagemushaOrdinaryAppIdentityPreparedProjectionV1, id: Data) {
    self.bridge=bridge;self.original=original;originalID=id
  }
  static func prepareOriginal(bridge: KagemushaCoreCoordinatorBridgeV1) throws
    -> KagemushaNativePreparedOrdinaryAppIdentityV1 {
    // Phase11 reads an already-reserved native selector. It does not generate C,
    // an ID, nonces, an account or a pending authority from application fields.
    let id=try bridge.invoke(.preparedOrdinaryAppIdentity,
      fields:[KagemushaCoreCoordinatorFrameV1.u32(11)])[0]
    let fields=try bridge.invoke(.preparedOrdinaryAppIdentity,
      fields:[KagemushaCoreCoordinatorFrameV1.u32(1),id])
    let p=try KagemushaOrdinaryAppIdentityPreparedProjectionV1(fields,enrollmentID:id)
    let result=KagemushaNativePreparedOrdinaryAppIdentityV1(bridge:bridge,original:p,id:id)
    _ = try result.recheck()
    guard try bridge.invoke(.preparedOrdinaryAppIdentity,
      fields:[KagemushaCoreCoordinatorFrameV1.u32(11)])[0] == id else { throw invalid() }
    return result
  }
  public func cancel() throws { _ = try call(9) }
  func recheck() throws -> KagemushaOrdinaryAppIdentityPreparedProjectionV1 {
    let r=try call(8)
    guard r[0] == original.nativeScope,r[1] == original.generationChallenge else { throw Self.invalid() }
    return original
  }
  func recover() throws -> KagemushaOrdinaryAppIdentityRecoveryProjectionV1 {
    _ = try recheck()
    let result=try KagemushaOrdinaryAppIdentityRecoveryProjectionV1(call(7),original:original)
    _ = try recheck();return result
  }
  func fenceGeneration() throws -> (fresh:Bool,keyReference:String) {
    _ = try recheck();let r=try call(2);_ = try recheck()
    return (r[0] == Data([1]),r[1].isEmpty ? "" : try original.validateKeyReference(r[1]))
  }
  func retainKey(_ keyReference:String) throws {
    _ = try original.validateKeyReference(Data(keyReference.utf8));_ = try recheck()
    _ = try call(3,extra:[Data(keyReference.utf8)]);_ = try recheck()
  }
  func fenceAttestation() throws -> Bool {
    _ = try recheck();let r=try call(4);_ = try recheck();return r[0] == Data([1])
  }
  func retainAttestation(point:Data,raw:Data) throws {
    guard (1...131_072).contains(raw.count) else { throw Self.invalid() }
    let retained=try recover();guard retained.state == 3 else { throw Self.invalid() }
    _ = try original.validateKeyReference(Data(retained.keyReference.utf8),point:point)
    _ = try recheck()
    _ = try call(5,extra:[point,Data(raw.prefix(65_536)),Data(raw.dropFirst(65_536))])
    _ = try recheck()
  }
  func readRetainedAttestation(_ metadata:KagemushaOrdinaryAppIdentityRecoveryProjectionV1) throws -> Data {
    guard metadata.state >= 4 else { throw Self.invalid() };_ = try recheck()
    var raw=Data()
    let chunkCount=(metadata.rawLength+65_535)/65_536
    for index in 0..<chunkCount {
      let r=try call(10,extra:[KagemushaCoreCoordinatorFrameV1.u32(UInt32(index))])
      guard r[2] == metadata.rawDigest,
        Int(KagemushaAppPlatformPreparedProjectionV1.u32(r[3])) == metadata.rawLength else { throw Self.invalid() }
      raw.append(r[1])
    }
    guard raw.count == metadata.rawLength,Data(SHA256.hash(data:raw)) == metadata.rawDigest else { throw Self.invalid() }
    _ = try recheck();return raw
  }
  func acceptOriginalRawAdmission() throws -> KagemushaNativePendingAppAttestIdentityV1 {
    _ = try recheck()
    // No raw admission bytes or verdict enter this request. Native fetches the
    // held independent issuer original and authenticates it under current policy.
    let accepted=try call(6), retained=try recover()
    guard retained.state == 5,accepted[0] == retained.pendingScope,
      accepted[1] == Data(SHA256.hash(data:retained.rawAdmission)) else { throw Self.invalid() }
    let a=retained.rawAdmission,c=original.challenge
    guard try KagemushaOrdinaryAppIdentityRawProjectionV1(a,original:original,
      retained:retained).pendingScope(nativeScope:original.nativeScope) == retained.pendingScope else { throw Self.invalid() }
    let possession=try bridge.prepareAppEnrollmentPossession(originalEnrollmentChallengeHash:original.generationChallenge)
    let e=try possession.recheck()
    let start=Data("iroha:kagemusha:v1:app-enrollment-possession\0".utf8).count+8
    guard e.platform == 4,e.approval == nil,e.credentialDigest.isEmpty,
      e.enrollmentChallenge == original.signingBytes,e.nativeScope == retained.pendingScope,
      e.publicKeyX963 == retained.point,e.keyAlias == retained.keyReference,
      e.generationChallenge == original.generationChallenge,e.appleCounterFloor == 0,
      Data(e.signingBytes[(start+323)..<(start+355)]) == retained.rawDigest,
      e.appSigningIdentityDigest == Data(a[198..<230]),c.platform == 4 else { throw Self.invalid() }
    _ = try recheck()
    return KagemushaNativePendingAppAttestIdentityV1(possession:possession,keyReference:retained.keyReference)
  }
  private func call(_ phase:UInt32,extra:[Data]=[]) throws -> [Data] {
    try bridge.invoke(.preparedOrdinaryAppIdentity,
      fields:[KagemushaCoreCoordinatorFrameV1.u32(phase),original.ticket]+extra)
  }
  private static func invalid()->KagemushaCoreCoordinatorErrorV1 { .invalidFrame("native ordinary identity original differs") }
}

/// Native-admitted raw Apple enrollment and its genuine pending E capability.
/// It is not a final identity credential, a financial owner or a spending lease.
public struct KagemushaNativePendingAppAttestIdentityV1: Sendable {
  public let possession: KagemushaNativePreparedAppEnrollmentPossessionV1
  private let keyReference:String
  fileprivate init(possession:KagemushaNativePreparedAppEnrollmentPossessionV1,keyReference:String) {
    self.possession=possession;self.keyReference=keyReference
  }
  /// Explicitly establish a new local assertion journal only while the native E
  /// original is uninvoked and its attested key counter floor remains zero.
  /// Existing or interrupted journal files are never reset or repaired here.
  public func bootstrapNewAssertionJournal(directoryURL:URL) throws -> KagemushaAppAttestFileIntentStoreV1 {
    let p=try possession.recheck(),r=try possession.recover()
    guard p.platform == 4,p.keyAlias == keyReference,p.appleCounterFloor == 0,r.state == 0 else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    let store=try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(directoryURL:directoryURL,keyID:keyReference)
    _ = try possession.recheck();return store
  }
}

extension KagemushaCoreCoordinatorBridgeV1 {
  /// Read and resolve the independently installed native original enrollment.
  /// The application never supplies enrollment ID, C, nonces or raw-admission authority.
  public func prepareOriginalIdentity() throws -> KagemushaNativePreparedOrdinaryAppIdentityV1 {
    try KagemushaNativePreparedOrdinaryAppIdentityV1.prepareOriginal(bridge:self)
  }
}

import CryptoKit
import Foundation

/// Called Apple key-generation and raw-attestation path for a genuine native C21
/// original only. It cannot choose C, renew its interval or submit a verdict.
public actor KagemushaAppAttestOrdinaryIdentityProviderV1 {
  private let service:any KagemushaAppAttestServiceV1
  private let verifier:KagemushaAppAttestEnrollmentVerifierV1
  private var invocationInFlight=false
  private var locallyUncertain=false
  public init(service:any KagemushaAppAttestServiceV1,verifier:KagemushaAppAttestEnrollmentVerifierV1) {
    self.service=service;self.verifier=verifier
  }
  /// Fence each device call in native WAL and retain the complete original for issuer transport.
  /// Explicit raw-admission intake, E possession and credential issuance remain separate steps.
  public func collect(_ identity:KagemushaNativePreparedOrdinaryAppIdentityV1) async throws
    -> KagemushaNativeCollectedAppIdentityOriginalV1 {
    guard service.isSupported else { throw KagemushaAppAttestEvidenceErrorV1.unsupportedDevice }
    guard !invocationInFlight else { throw KagemushaAppAttestEvidenceErrorV1.assertionAlreadyInFlight }
    guard !locallyUncertain else { throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown }
    invocationInFlight=true;defer { invocationInFlight=false }
    do {
      let original=try apple(identity)
      var retained=try identity.recover()
      if try KagemushaOrdinaryAppIdentityRecoveryActionV1.classify(retained.state) == .generate {
        let fence=try identity.fenceGeneration()
        if fence.fresh {
          _ = try identity.recheck()
          let key:String
          do { key=try await service.generateKey() }
          catch { throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown }
          try identity.retainKey(key)
        }
        retained=try identity.recover()
      }
      let action=try KagemushaOrdinaryAppIdentityRecoveryActionV1.classify(retained.state)
      guard action != .generate else { throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown }
      if action == .attest {
        if try identity.fenceAttestation() {
          _ = try identity.recheck()
          let raw:Data
          // The native original supplies SHA(full C) once. No SHA(E), alternate
          // client challenge or extra generation/assertion call is substituted.
          do { raw=try await service.attestKey(retained.keyReference,
            clientDataHash:original.generationChallenge) }
          catch { throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown }
          let checked=try evidence(raw,key:retained.keyReference,original:original)
          try identity.retainAttestation(point:checked.publicKeyX963,raw:raw)
        }
        retained=try identity.recover()
      }
      return try finish(identity,original:original,retained:retained)
    } catch { locallyUncertain=true;throw error }
  }
  /// Recover only the native-retained original; no Apple device API is called.
  /// An invoked generation or attestation without its original stays unavailable.
  public func recover(_ identity:KagemushaNativePreparedOrdinaryAppIdentityV1) throws
    -> KagemushaNativeCollectedAppIdentityOriginalV1 {
    guard !invocationInFlight else { throw KagemushaAppAttestEvidenceErrorV1.assertionAlreadyInFlight }
    invocationInFlight=true;defer { invocationInFlight=false }
    do {
      let result=try finish(identity,original:apple(identity),retained:identity.recover())
      locallyUncertain=false;return result
    } catch { locallyUncertain=true;throw error }
  }
  private func apple(_ identity:KagemushaNativePreparedOrdinaryAppIdentityV1) throws
    -> KagemushaOrdinaryAppIdentityPreparedProjectionV1 {
    let p=try identity.recheck()
    guard p.platform == 4,p.originalAlias.isEmpty,p.androidLevelsMask == 0 else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("ordinary App Attest requires original Apple C")
    }
    return p
  }
  private func evidence(_ raw:Data,key:String,original:KagemushaOrdinaryAppIdentityPreparedProjectionV1) throws
    -> KagemushaAppAttestEnrollmentEvidenceV1 {
    guard (1...131_072).contains(raw.count) else { throw KagemushaAppAttestEvidenceErrorV1.emptyRawObject }
    // Local certificate/AppID/counter checks supply the actual point only. Native
    // and issuer independently authenticate release, raw admission and current time.
    let result=try verifier.verify(rawAttestation:raw,keyID:key,
      clientDataHash:original.generationChallenge)
    _ = try original.validateKeyReference(Data(key.utf8),point:result.publicKeyX963)
    return result
  }
  private func finish(_ identity:KagemushaNativePreparedOrdinaryAppIdentityV1,
    original:KagemushaOrdinaryAppIdentityPreparedProjectionV1,
    retained:KagemushaOrdinaryAppIdentityRecoveryProjectionV1) throws
    -> KagemushaNativeCollectedAppIdentityOriginalV1 {
    guard try KagemushaOrdinaryAppIdentityRecoveryActionV1.classify(retained.state) == .complete else {
      throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown
    }
    let raw=try identity.readRetainedAttestation(retained)
    let checked=try evidence(raw,key:retained.keyReference,original:original)
    guard checked.publicKeyX963 == retained.point else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("retained App Attest original point differs")
    }
    guard let result = try identity.recoverOriginalAttestation(),
      result.publicKeyX963 == retained.point, result.rawAttestation == raw,
      result.keyReference == retained.keyReference else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("retained App Attest original differs")
    }
    locallyUncertain=false;return result
  }
}

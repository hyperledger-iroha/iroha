// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import CryptoKit
import Foundation
@testable import IrohaSwift

#if os(iOS) && canImport(DeviceCheck) && !targetEnvironment(simulator)
/// Callable physical iOS support, without a test runner or an installed-owner factory.
/// Observations are detached comparison data. They cannot reopen Native, admit an issuer,
/// construct a credential, or establish State/Guard, Current, or financial readiness.
///
/// TODO: connect an admitted product-host XCTest entry to genuine installed runtime/account
/// acquisition and supervised fault injection. This file contains no executable test case.
@available(iOS 15.0, *)
enum OrdinaryPersistentIdentityPhysicalHarnessV1 {
  /// Product-supplied public configuration from the authenticated release, not from evidence.
  /// Construct the existing verifier through its public pinned-root initializer, with its
  /// actual verification clock. No test-clock initializer or mock service is selected here.
  struct ProductAppAttestInputs {
    let rootCertificateDER: Data
    let expectedAppIDHash: Data
    let expectedRelease: KagemushaAppAttestExpectedReleaseV1
    let environment: KagemushaAppAttestEnvironmentV1

    fileprivate func verifier() throws -> KagemushaAppAttestEnrollmentVerifierV1 {
      try KagemushaAppAttestEnrollmentVerifierV1(rootCertificateDER: rootCertificateDER,
        expectedAppIDHash: expectedAppIDHash, expectedRelease: expectedRelease,
        environment: environment)
    }
  }

  /// Collect through the real DCAppAttestService adapter. Native chooses fresh versus
  /// retained-original recovery and fences generation and attestation before invocation.
  static func collectNativeOriginal(identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
    configuration: ProductAppAttestInputs) async throws -> CollectedObservation {
    let c = try requireApplePreparation(identity)
    let provider = KagemushaAppAttestOrdinaryIdentityProviderV1(
      service: KagemushaAppleAppAttestServiceV1(), verifier: try configuration.verifier())
    let original = try await provider.collect(identity)
    let observed = CollectedObservation(signedPreparation: c.signedChallenge,
      challengeTranscript: c.signingBytes, keyReference: original.keyReference,
      publicKeyX963: original.publicKeyX963, rawAttestation: original.rawAttestation)
    try requireCollectedOriginal(identity, configuration: configuration, expected: observed)
    return observed
  }

  /// The existing production method derives E from this very identity's admitted raw original
  /// and same coordinator before any Apple assertion. It may establish the genuine pending
  /// Native E on the first call; repeated calls select its retained original.
  /// The concrete file journal must already be durably anchored and bootstrapped for this key.
  /// This helper never creates, resets or repairs that journal, nor issues a credential.
  static func proveAndObserveBoundOriginal(
    identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
    collected: CollectedObservation,
    assertionStore: KagemushaAppAttestFileIntentStoreV1,
    configuration: ProductAppAttestInputs) async throws
    -> (possession: KagemushaNativePreparedAppEnrollmentPossessionV1,
        observed: PossessionObservation) {
    try requireCollectedOriginal(identity, configuration: configuration, expected: collected)
    let admission = try requireAdmissionOriginal(identity, collected: collected)
    // No caller-supplied E, ticket, signing subject, counter or challenge is accepted.
    let pending = try identity.preparePendingAppAttestPossession()
    let operation = pending.possession
    let e = try requireBoundPossession(identity, operation: operation, admission: admission)
    let provider = KagemushaAppAttestEnrollmentPossessionProviderV1(
      service: KagemushaAppleAppAttestServiceV1(), intentStore: assertionStore,
      expectedRelease: configuration.expectedRelease)
    let receipt = try await provider.completePossession(operation)
    let consumed = try operation.recoverOriginalConsumedAssertion()
    try same(receipt.canonicalReceipt, consumed.receipt.canonicalReceipt, "Native E receipt")
    let observed = PossessionObservation(collected: collected,
      signedAdmission: admission.signedAdmission, pendingScope: admission.pendingScope,
      enrollmentPossession: e.signingBytes, rawAssertion: consumed.rawAssertion,
      receipt: consumed.receipt)
    try requireRecoveredOriginal(identity, operation: operation,
      configuration: configuration, expected: observed)
    return (operation, observed)
  }

  /// Compare genuinely reacquired C/E holders with the first retained originals after a lost
  /// result or process restart. Recovery uses the existing production reconciliation path;
  /// it may consume an already retained assertion and advance its matching local journal.
  /// It never generates a key, attests, invokes an Apple assertion, reserves E, or calls HTTP.
  /// Missing native originals or journal files remain errors; reinstall permits no replacement.
  static func assertRecoveredOriginal(identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
    possession: KagemushaNativePreparedAppEnrollmentPossessionV1,
    assertionStore: KagemushaAppAttestFileIntentStoreV1,
    configuration: ProductAppAttestInputs, expected: PossessionObservation) async throws {
    try requireCollectedOriginal(identity, configuration: configuration, expected: expected.collected)
    let admission = try requireAdmissionOriginal(identity, collected: expected.collected)
    _ = try requireBoundPossession(identity, operation: possession, admission: admission)
    let provider = KagemushaAppAttestEnrollmentPossessionProviderV1(
      service: KagemushaAppleAppAttestServiceV1(), intentStore: assertionStore,
      expectedRelease: configuration.expectedRelease)
    let recovered = try await provider.recoverPossession(possession)
    try same(recovered.canonicalReceipt, expected.canonicalReceipt, "recovered Native E receipt")
    try requireRecoveredOriginal(identity, operation: possession,
      configuration: configuration, expected: expected)
  }

  /// Call the already opened shared enrollment actor. The product host must have supplied its
  /// actual protected journal/HTTP, installed owner, App Attest adapter and existing wallet key.
  /// This return acknowledges enrollment only; it grants no financial-ready result.
  static func beginOrResumeInstalledEnrollment(service: KagemushaAppAttestOrdinaryEnrollmentV1)
    async throws -> KagemushaOrdinaryEnrollmentOriginalsV1 {
    try await service.beginOrResume()
  }

  /// Compare a genuinely recovered completed retail holder. The production completed-original
  /// path reads/re-admits its retained certificate without HTTP, device/wallet signing or E selection.
  static func assertSameRetainedEnrollment(retail: KagemushaNativePreparedRetailEnrollmentV1,
    requireOriginalOwner: @Sendable () async throws -> Void,
    expected: KagemushaOrdinaryEnrollmentOriginalsV1) async throws {
    let recovered = try await KagemushaAppAttestOrdinaryEnrollmentV1.recoverCompletedOriginals(
      retail: retail, requireOriginalOwner: requireOriginalOwner)
    try same(expected.confirmation.enrollmentID, recovered.confirmation.enrollmentID, "FI enrollment ID")
    try same(expected.confirmation.pendingScope, recovered.confirmation.pendingScope, "FI pending scope")
    try same(expected.confirmation.credentialDigest, recovered.confirmation.credentialDigest, "app credential digest")
    try same(expected.originalRetailCertificate, recovered.originalRetailCertificate, "FI certificate")
  }

  private static func requireApplePreparation(_ identity: KagemushaNativePreparedOrdinaryAppIdentityV1)
    throws -> KagemushaOrdinaryAppIdentityPreparedProjectionV1 {
    let c = try identity.recheck()
    guard c.platform == 4, c.originalAlias.isEmpty, c.androidLevelsMask == 0,
      c.signedChallenge.count == 515, c.signingBytes.count == 512 else { throw invalid("Apple C required") }
    // Existing bounded SDK parser only; Native already authenticates the signed original.
    let challenge = try KagemushaOrdinaryAppIdentityPreparedProjectionV1.challenge(transport: c.signedChallenge)
    try same(challenge.canonicalSigningBytes, c.signingBytes, "full C signing transcript")
    try same(identity.originalSignedPreparationBytes(), c.signedChallenge, "Native signed C")
    try same(identity.originalChallengeSigningBytes(), c.signingBytes, "Native C transcript")
    return c
  }

  private static func requireCollectedOriginal(_ identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
    configuration: ProductAppAttestInputs, expected: CollectedObservation) throws {
    let c = try requireApplePreparation(identity)
    try same(c.signedChallenge, expected.signedPreparation, "signed C515")
    try same(c.signingBytes, expected.challengeTranscript, "C transcript")
    guard let retained = try identity.recoverOriginalAttestation(),
      retained.keyReference == expected.keyReference else { throw invalid("Native App Attest original unavailable") }
    try same(retained.publicKeyX963, expected.publicKeyX963, "full original P-256 point")
    try same(retained.rawAttestation, expected.rawAttestation, "complete raw App Attest original")
    // Corroborate the untouched full evidence through the existing pinned-root verifier.
    // This does not assert present device-key usability or a server fraud/risk verdict.
    let checked = try configuration.verifier().verify(rawAttestation: retained.rawAttestation,
      keyID: retained.keyReference, clientDataHash: c.generationChallenge)
    try same(checked.publicKeyX963, retained.publicKeyX963, "verified original point")
    _ = try requireApplePreparation(identity)
  }

  private static func requireAdmissionOriginal(_ identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
    collected: CollectedObservation) throws -> KagemushaNativeRawAppIdentityAdmissionV1 {
    guard let admitted = try identity.recoverOriginalAdmission(),
      admitted.keyReference == collected.keyReference,
      admitted.signedAdmission.count == 314,
      KagemushaOrdinaryAppIdentityPreparedProjectionV1.digest(admitted.pendingScope) else {
      throw invalid("genuine Native raw314 admission required")
    }
    try same(admitted.publicKeyX963, collected.publicKeyX963, "admitted point")
    try same(admitted.rawAttestation, collected.rawAttestation, "admitted raw original")
    return admitted
  }

  private static func requireBoundPossession(_ identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
    operation: KagemushaNativePreparedAppEnrollmentPossessionV1,
    admission: KagemushaNativeRawAppIdentityAdmissionV1) throws -> KagemushaAppPlatformPreparedProjectionV1 {
    let c = try requireApplePreparation(identity), e = try operation.recheck()
    guard e.platform == 4, e.approval == nil, e.bootstrapApproval == nil,
      e.credentialDigest.isEmpty, e.financialSubject.isEmpty, e.appleCounterFloor == 0,
      e.signingBytes.count == 424, e.keyAlias == admission.keyReference else { throw invalid("original Apple E required") }
    try same(e.enrollmentChallenge, c.signingBytes, "C bound to E")
    try same(e.generationChallenge, c.generationChallenge, "original attestation challenge")
    try same(operation.originalEnrollmentChallengeHash(), c.generationChallenge, "E selector")
    try same(e.nativeScope, admission.pendingScope, "E pending scope")
    try same(e.publicKeyX963, admission.publicKeyX963, "E original point")
    try same(operation.signingBytes(), e.signingBytes, "Native original E424")
    return e
  }

  private static func requireRecoveredOriginal(_ identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
    operation: KagemushaNativePreparedAppEnrollmentPossessionV1,
    configuration: ProductAppAttestInputs, expected: PossessionObservation) throws {
    try requireCollectedOriginal(identity, configuration: configuration, expected: expected.collected)
    let admission = try requireAdmissionOriginal(identity, collected: expected.collected)
    try same(admission.signedAdmission, expected.signedAdmission, "signed raw314")
    try same(admission.pendingScope, expected.pendingScope, "pending scope")
    let e = try requireBoundPossession(identity, operation: operation, admission: admission)
    try same(e.signingBytes, expected.enrollmentPossession, "original E424")
    let consumed = try operation.recoverOriginalConsumedAssertion()
    try same(consumed.rawAssertion, expected.rawAssertion, "original Apple E assertion")
    try same(consumed.receipt.canonicalReceipt, expected.canonicalReceipt, "original Native E receipt")
    guard consumed.receipt.keyAlias == expected.collected.keyReference,
      consumed.receipt.observedCounter == expected.observedCounter else { throw invalid("original key or counter changed") }
    try same(consumed.receipt.enrollmentChallengeHash, e.generationChallenge, "receipt C selector")
    // Existing SDK verifier/CBOR/DER/equation implementation; no test-side parser is added.
    let checked = try KagemushaAppAttestEnrollmentPossessionOriginalV1(
      rawAssertion: consumed.rawAssertion, nativeProjection: e,
      expectedRelease: configuration.expectedRelease)
    guard checked.observedCounter == consumed.receipt.observedCounter else { throw invalid("assertion counter differs") }
    _ = try requireApplePreparation(identity)
    try same(operation.signingBytes(), e.signingBytes, "current retained E")
  }

  private static func same(_ first: Data, _ second: Data, _ label: String) throws {
    guard first == second else { throw invalid(label + " changed") }
  }
  private static func invalid(_ message: String) -> KagemushaCoreCoordinatorErrorV1 {
    .invalidFrame("physical original comparison: " + message)
  }

  /// Read-only, detached comparison DATA; no serialization, provider receipt or owner admission.
  struct CollectedObservation: Sendable {
    let signedPreparation, challengeTranscript: Data
    let keyReference: String
    let publicKeyX963, rawAttestation: Data
    fileprivate init(signedPreparation: Data, challengeTranscript: Data,
      keyReference: String, publicKeyX963: Data, rawAttestation: Data) {
      self.signedPreparation = Data(signedPreparation); self.challengeTranscript = Data(challengeTranscript)
      self.keyReference = keyReference; self.publicKeyX963 = Data(publicKeyX963)
      self.rawAttestation = Data(rawAttestation)
    }
  }

  /// Read-only expected originals. Copying these values cannot reconstruct a C/E capability.
  struct PossessionObservation: Sendable {
    let collected: CollectedObservation
    let signedAdmission, pendingScope, enrollmentPossession, rawAssertion, canonicalReceipt: Data
    let observedCounter: UInt32
    fileprivate init(collected: CollectedObservation, signedAdmission: Data, pendingScope: Data,
      enrollmentPossession: Data, rawAssertion: Data,
      receipt: KagemushaNativeAppEnrollmentPossessionReceiptV1) {
      self.collected = collected; self.signedAdmission = Data(signedAdmission)
      self.pendingScope = Data(pendingScope); self.enrollmentPossession = Data(enrollmentPossession)
      self.rawAssertion = Data(rawAssertion); self.canonicalReceipt = Data(receipt.canonicalReceipt)
      observedCounter = receipt.observedCounter
    }
  }
}
#endif

// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import CryptoKit
import Foundation

/// Product-owned protected FI HTTP only. The request already contains the SDK's exact
/// original body and business identities. Fresh HTTP proof headers are separate.
public protocol KagemushaOrdinaryEnrollmentOriginalTransportV1: Sendable {
  func exchangeOriginal(_ request: KagemushaOrdinaryEnrollmentOriginalRequestV1) async throws -> Data
}

/// A protected durable compare-and-swap store for detached transport originals.
/// Successful retain must mean the complete value is atomically durable before returning.
/// A missing/corrupt/changed existing record must never be replaced with a new ceremony.
/// This store creates no Native owner, key, assertion journal or authority capability.
public protocol KagemushaOrdinaryEnrollmentOriginalStoringV1: Sendable {
  func loadOriginal() async throws -> Data?
  func retainOriginal(_ next: Data, expecting previous: Data?) async throws
}

/// Explicit durable-store lifecycle. Opening an existing ceremony never bootstraps a missing record.
public enum KagemushaOrdinaryEnrollmentJournalModeV1: Equatable, Sendable { case createNew, openExisting }

/// The selected existing wallet Ed25519 role, separate from the App Attest app key.
/// Implementations must pin the actual key generation and forbid key creation/replacement.
public protocol KagemushaOrdinaryEnrollmentWalletSignerV1: Sendable {
  var accountID: String { get }
  var publicKey: Data { get }
  func requireOriginalOwner() async throws
  func signExistingOriginal(_ message: Data) async throws -> Data
}

/// Read-only transport bytes. Only this SDK workflow can create a request.
/// None of these public bytes constructs C21, E20, a final identity or a financial owner.
public struct KagemushaOrdinaryEnrollmentOriginalRequestV1: Sendable {
  public let path: String
  public let requestID: String
  public let idempotencyKey: String
  private let originalBody: Data
  private let recheckOriginal: @Sendable () throws -> Void
  fileprivate init(stage: String, value: OrdinaryEnrollmentRequestRecord,
    recheckOriginal: @escaping @Sendable () throws -> Void) {
    path = "/v1/kagemusha/enrollment/ordinary/" + stage
    requestID = value.requestID; idempotencyKey = value.idempotencyKey
    originalBody = Data(value.body); self.recheckOriginal = recheckOriginal
  }
  /// Every HTTP dispatch/redirect/retry must obtain these exact bytes immediately
  /// before sending. Only this workflow installs the held Native original check.
  public func body() throws -> Data {
    try recheckOriginal(); let result = Data(originalBody); try recheckOriginal(); return result
  }
  public func requireCurrent() throws { try recheckOriginal() }
}

/// Enrollment acknowledgement from actual Native FI acceptance, not cash readiness.
public struct KagemushaOrdinaryEnrollmentOriginalsV1: Sendable {
  public let confirmation: KagemushaNativeRetailEnrollmentConfirmationV1
  public let originalRetailCertificate: Data
  fileprivate init(_ confirmation: KagemushaNativeRetailEnrollmentConfirmationV1, certificate: Data) {
    self.confirmation = confirmation; originalRetailCertificate = Data(certificate)
  }
}

/// Shared Apple ordinary enrollment on one already admitted Native reservation.
/// The app supplies protected persistence/HTTP and an existing wallet signer. Native
/// supplies every C/E/FI authority holder, device/wallet invocation fence and original.
/// Keep this actor for retries. A process restart must reacquire the same genuine Native
/// owner and the same durable store; the saved public record cannot recreate that owner.
public actor KagemushaAppAttestOrdinaryEnrollmentV1 {
  private let reservation: KagemushaNativeReservedOrdinaryAppIdentityV1
  private let transport: any KagemushaOrdinaryEnrollmentOriginalTransportV1
  private let journal: any KagemushaOrdinaryEnrollmentOriginalStoringV1
  private let signer: any KagemushaOrdinaryEnrollmentWalletSignerV1
  private let service: any KagemushaAppAttestServiceV1
  private let expectedRelease: KagemushaAppAttestExpectedReleaseV1
  private let collector: KagemushaAppAttestOrdinaryIdentityProviderV1
  private let requireOwner: @Sendable () async throws -> Void
  private var record: OrdinaryEnrollmentRecord
  private var serialized: Data
  private var prepared: KagemushaNativePreparedOrdinaryAppIdentityV1?
  private var pending: KagemushaNativePendingAppAttestIdentityV1?
  private var possessionProvider: KagemushaAppAttestEnrollmentPossessionProviderV1?
  private var assertionStore: KagemushaAppAttestFileIntentStoreV1?
  private var identity: KagemushaNativeOrdinaryAppIdentityConfirmationV1?
  private var retail: KagemushaNativePreparedRetailEnrollmentV1?
  private var completed: KagemushaOrdinaryEnrollmentOriginalsV1?
  private var inFlight = false
  private var locallyUncertain = false
  private var unusable = false

  /// Requires a real existing reservation and selected account/FI/release owner.
  /// Directory selection is anchored durably before a new assertion journal may be created.
  public static func open(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
    transport: any KagemushaOrdinaryEnrollmentOriginalTransportV1,
    journal: any KagemushaOrdinaryEnrollmentOriginalStoringV1,
    journalMode: KagemushaOrdinaryEnrollmentJournalModeV1,
    walletSigner: any KagemushaOrdinaryEnrollmentWalletSignerV1,
    service: any KagemushaAppAttestServiceV1,
    verifier: KagemushaAppAttestEnrollmentVerifierV1,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1,
    assertionJournalDirectory: URL,
    requireOriginalOwner: @escaping @Sendable () async throws -> Void) async throws -> KagemushaAppAttestOrdinaryEnrollmentV1 {
    try await requireOriginalOwner(); try await walletSigner.requireOriginalOwner()
    let owner = try OrdinaryEnrollmentRecord.owner(reservation, signer: walletSigner)
    guard assertionJournalDirectory.isFileURL,
      assertionJournalDirectory.path.hasPrefix("/"),
      assertionJournalDirectory.standardizedFileURL.path == assertionJournalDirectory.path else { throw invalid() }
    let prior = try await journal.loadOriginal()
    guard (journalMode == .createNew && prior == nil) || (journalMode == .openExisting && prior != nil),
      prior == nil || prior!.count <= 4194304 else { throw invalid() }
    let record: OrdinaryEnrollmentRecord
    if let prior {
      record = try JSONDecoder().decode(OrdinaryEnrollmentRecord.self, from: prior)
      try record.validate()
      guard record.owner == owner, record.assertionDirectory == assertionJournalDirectory.path else { throw invalid() }
    } else {
      record = OrdinaryEnrollmentRecord(owner: owner, assertionDirectory: assertionJournalDirectory.path)
    }
    let encoded = try prior ?? record.encoded()
    if prior == nil { try await journal.retainOriginal(encoded, expecting: nil) }
    guard try await journal.loadOriginal() == encoded,
      try OrdinaryEnrollmentRecord.owner(reservation, signer: walletSigner) == owner else { throw invalid() }
    try await requireOriginalOwner(); try await walletSigner.requireOriginalOwner()
    return KagemushaAppAttestOrdinaryEnrollmentV1(reservation: reservation, transport: transport,
      journal: journal, signer: walletSigner, service: service, verifier: verifier,
      expectedRelease: expectedRelease, requireOwner: requireOriginalOwner, record: record, serialized: encoded)
  }

  /// Completed FI recovery after C expiry requires a real already recovered Native
  /// retail holder from the admitted startup owner. Public saved bytes cannot create it.
  /// This method invokes no device/signer/HTTP and re-admits the retained certificate.
  public static func recoverCompletedOriginals(
    retail: KagemushaNativePreparedRetailEnrollmentV1,
    requireOriginalOwner: @Sendable () async throws -> Void) async throws -> KagemushaOrdinaryEnrollmentOriginalsV1 {
    try await requireOriginalOwner()
    let original = try retail.recoverOriginals()
    guard original.state == .certificateRetained else { throw unknown() }
    let accepted = try retail.acceptOriginalEnrollmentCertificate(original.certificateOriginal)
    let checked = try retail.recoverOriginals()
    guard checked.state == .certificateRetained,
      checked.accountSignature == original.accountSignature,
      checked.certificateOriginal == original.certificateOriginal else { throw invalid() }
    try await requireOriginalOwner()
    return KagemushaOrdinaryEnrollmentOriginalsV1(accepted, certificate: original.certificateOriginal)
  }

  private init(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
    transport: any KagemushaOrdinaryEnrollmentOriginalTransportV1,
    journal: any KagemushaOrdinaryEnrollmentOriginalStoringV1,
    signer: any KagemushaOrdinaryEnrollmentWalletSignerV1, service: any KagemushaAppAttestServiceV1,
    verifier: KagemushaAppAttestEnrollmentVerifierV1, expectedRelease: KagemushaAppAttestExpectedReleaseV1,
    requireOwner: @escaping @Sendable () async throws -> Void, record: OrdinaryEnrollmentRecord, serialized: Data) {
    self.reservation = reservation; self.transport = transport; self.journal = journal; self.signer = signer
    self.service = service; self.expectedRelease = expectedRelease; self.requireOwner = requireOwner
    self.collector = KagemushaAppAttestOrdinaryIdentityProviderV1(service: service, verifier: verifier)
    self.record = record; self.serialized = serialized
  }

  /// Complete/resume only known originals. Ambiguous device/wallet invocations require explicit recovery.
  public func beginOrResume() async throws -> KagemushaOrdinaryEnrollmentOriginalsV1 {
    try await run(recoveryOnly: false)
  }
  /// No key generation, attestation, E assertion or wallet signing occurs in this method.
  /// HTTP may recover only the exact durably saved request, including after a lost reply.
  public func recoverOriginals() async throws -> KagemushaOrdinaryEnrollmentOriginalsV1 {
    try await run(recoveryOnly: true)
  }

  /// Forward the same native-completed FI ceremony to its genuine Bootstrap holder.
  /// This invokes no App Attest signer and supplies no State/Guard publication authority.
  public func prepareBootstrapAppApproval() async throws -> KagemushaNativePreparedBootstrapAppApprovalV1 {
    guard !inFlight, !unusable, !locallyUncertain, completed != nil, let retail else { throw Self.invalid() }
    inFlight = true; defer { inFlight = false }
    do {
      try await current()
      let retained = try retail.recoverOriginals()
      guard retained.state == .certificateRetained,
        retained.certificateOriginal == completed?.originalRetailCertificate else { throw Self.invalid() }
      let acknowledged = try retail.acceptOriginalEnrollmentCertificate(retained.certificateOriginal)
      guard acknowledged.enrollmentID == completed?.confirmation.enrollmentID else { throw Self.invalid() }
      let held = try retail.prepareBootstrapAppApproval()
      try await current(); return held
    } catch { unusable = true; throw error }
  }

  private func run(recoveryOnly: Bool) async throws -> KagemushaOrdinaryEnrollmentOriginalsV1 {
    guard !inFlight else { throw KagemushaAppAttestEvidenceErrorV1.assertionAlreadyInFlight }
    guard !unusable else { throw Self.invalid() }
    guard recoveryOnly || !locallyUncertain else { throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown }
    inFlight = true; defer { inFlight = false }
    do {
      let result = try await finish(recoveryOnly: recoveryOnly)
      locallyUncertain = false; return result
    } catch let retry as OrdinaryEnrollmentTransportFailure {
      throw retry.underlying
    } catch {
      // Re-entry permits only explicit recovery of this same retained attempt.
      locallyUncertain = true; throw error
    }
  }

  private func finish(recoveryOnly: Bool) async throws -> KagemushaOrdinaryEnrollmentOriginalsV1 {
    try await current()
    if let completed, let retail {
      let recovered = try retail.recoverOriginals()
      guard recovered.state == .certificateRetained,
        recovered.certificateOriginal == completed.originalRetailCertificate else { throw Self.invalid() }
      let checked = try retail.acceptOriginalEnrollmentCertificate(recovered.certificateOriginal)
      guard checked.enrollmentID == completed.confirmation.enrollmentID,
        checked.pendingScope == completed.confirmation.pendingScope,
        checked.credentialDigest == completed.confirmation.credentialDigest else { throw Self.invalid() }
      try await current(); return completed
    }
    if prepared == nil {
      if recoveryOnly && record.requests["prepare"] == nil { throw Self.unknown() }
      let reply = try await exchange("prepare", body: try OrdinaryEnrollmentWire.prepare(reservation))
      let original = try OrdinaryEnrollmentWire.preparation(reply)
      prepared = try reservation.acceptOriginalSignedPreparation(original)
      try await current()
    }
    guard let prepared else { throw Self.invalid() }
    let collected: KagemushaNativeCollectedAppIdentityOriginalV1
    if let original = try prepared.recoverOriginalAttestation() { collected = original }
    else if recoveryOnly || record.collectionInvoked { collected = try await collector.recover(prepared) }
    else {
      try await update { $0.collectionInvoked = true }
      collected = try await collector.collect(prepared)
      try await current()
    }
    let preparation = try prepared.originalSignedPreparationBytes()
    let admitted: KagemushaNativeRawAppIdentityAdmissionV1
    if let original = try prepared.recoverOriginalAdmission() { admitted = original }
    else {
      let reply = try await exchange("raw-attestation", body: try OrdinaryEnrollmentWire.raw(
        preparation: preparation, collected: collected))
      admitted = try prepared.acceptOriginalRawAdmission(OrdinaryEnrollmentWire.original(reply,
        field: "raw_admission_base64", digestField: "raw_admission_sha256_hex", exact: 314, maximum: 314))
    }
    guard admitted.keyReference == collected.keyReference, admitted.publicKeyX963 == collected.publicKeyX963,
      admitted.rawAttestation == collected.rawAttestation else { throw Self.invalid() }
    if pending == nil { pending = try prepared.preparePendingAppAttestPossession() }
    guard let pending else { throw Self.invalid() }
    if assertionStore == nil {
      let directory = URL(fileURLWithPath: record.assertionDirectory)
      if !record.assertionJournalCreationInvoked {
        guard !recoveryOnly else { throw Self.unknown() }
        try await update { $0.assertionJournalCreationInvoked = true }
        assertionStore = try pending.bootstrapNewAssertionJournal(directoryURL: directory)
      } else {
        // Anchored path never falls back to create/reset after missing files or a lost reply.
        assertionStore = try KagemushaAppAttestFileIntentStoreV1(directoryURL: directory)
      }
    }
    guard let assertionStore else { throw Self.invalid() }
    _ = try assertionStore.load(keyID: admitted.keyReference)
    if possessionProvider == nil {
      possessionProvider = KagemushaAppAttestEnrollmentPossessionProviderV1(service: service,
        intentStore: assertionStore, expectedRelease: expectedRelease)
    }
    guard let possessionProvider else { throw Self.invalid() }
    let receipt: KagemushaNativeAppEnrollmentPossessionReceiptV1
    if recoveryOnly || record.possessionInvoked {
      receipt = try await possessionProvider.recoverPossession(pending.possession)
    } else {
      try await update { $0.possessionInvoked = true }
      receipt = try await possessionProvider.completePossession(pending.possession)
    }
    try await current()
    let consumed = try pending.possession.recoverOriginalConsumedAssertion()
    guard consumed.receipt.canonicalReceipt == receipt.canonicalReceipt,
      consumed.receipt.keyAlias == admitted.keyReference,
      receipt.enrollmentChallengeHash == (try pending.possession.originalEnrollmentChallengeHash()) else { throw Self.invalid() }
    let appReply = try await exchange("certificate", body: try OrdinaryEnrollmentWire.certificate(
      preparation: preparation, admitted: admitted, consumed: consumed))
    let appCertificate = try OrdinaryEnrollmentWire.original(appReply, field: "certificate_base64",
      digestField: "certificate_sha256_hex", exact: nil, maximum: 16384)
    let confirmed = try pending.possession.acceptOriginalFinalCredential(appCertificate)
    if let identity {
      guard identity.pendingScope == confirmed.pendingScope, identity.credentialDigest == confirmed.credentialDigest else { throw Self.invalid() }
    }
    identity = confirmed
    try await current()
    let operation = try pending.possession.originalEnrollmentChallengeHash()
    if retail == nil {
      let originalStart = try pending.possession.financialStartOriginal(originalSignedPreparation: preparation)
      let start = try await exchange("start", body: originalStart)
      let challenge = try OrdinaryEnrollmentWire.retailChallenge(start, operation: operation)
      retail = try pending.possession.prepareRetailEnrollment(originalChallenge: challenge.challenge,
        accountSigningMessage: challenge.message, identity: confirmed)
    }
    guard let retail else { throw Self.invalid() }
    let recovered = try retail.recoverOriginals()
    switch recovered.state {
    case .prepared:
      guard !recoveryOnly, !record.walletSigningInvoked else { throw Self.unknown() }
      let action = try retail.fenceAccountSigning()
      switch action {
      case .signOriginalMessage(let message):
        try await update { $0.walletSigningInvoked = true }
        try await signer.requireOriginalOwner(); try await current()
        let signature = try await signer.signExistingOriginal(message)
        try await signer.requireOriginalOwner(); try await current()
        guard signature.count == 64,
          try Curve25519.Signing.PublicKey(rawRepresentation: signer.publicKey).isValidSignature(signature, for: message) else { throw Self.invalid() }
        try retail.retainOriginalAccountSignature(signature)
      case .retainedOriginalSignature(let signature):
        try retail.retainOriginalAccountSignature(signature)
      }
    case .invocationUnknown: throw Self.unknown()
    case .signatureRetained, .certificateRetained: break
    }
    let originals = try retail.recoverOriginals()
    guard originals.state == .signatureRetained || originals.state == .certificateRetained else { throw Self.unknown() }
    let certificate: Data
    let advertisedID: Data?
    if originals.state == .certificateRetained { certificate = originals.certificateOriginal; advertisedID = nil }
    else {
      let reply = try await exchange("finish", body: try OrdinaryEnrollmentWire.json([
        "challenge_id": OrdinaryEnrollmentWire.hex(operation), "account_signature_base64": originals.accountSignature.base64EncodedString()]))
      let issued = try OrdinaryEnrollmentWire.retailCertificate(reply, operation: operation)
      certificate = issued.certificate; advertisedID = issued.enrollmentID
    }
    let final = try retail.acceptOriginalEnrollmentCertificate(certificate)
    guard final.pendingScope == confirmed.pendingScope, final.credentialDigest == confirmed.credentialDigest,
      advertisedID == nil || advertisedID == final.enrollmentID else { throw Self.invalid() }
    let result = KagemushaOrdinaryEnrollmentOriginalsV1(final, certificate: certificate)
    try await current(); completed = result; return result
  }

  private func exchange(_ stage: String, body: Data) async throws -> Data {
    try await current()
    if let existing = record.requests[stage] { guard existing.body == body else { throw Self.invalid() } }
    else {
      let requestID = stage == "prepare" ? try reservation.requestID() : UUID().uuidString.lowercased()
      try await update { $0.requests[stage] = OrdinaryEnrollmentRequestRecord(requestID: requestID,
        idempotencyKey: UUID().uuidString.lowercased(), body: body) }
    }
    guard let request = record.requests[stage] else { throw Self.invalid() }
    let dispatch = try originalDispatch(stage: stage, request: request)
    try dispatch.requireCurrent()
    if let reply = record.responses[stage] { try dispatch.requireCurrent(); return Data(reply) }
    let reply: Data
    do {
      try dispatch.requireCurrent()
      reply = try await transport.exchangeOriginal(dispatch)
      try dispatch.requireCurrent()
    }
    catch { try await current(); throw OrdinaryEnrollmentTransportFailure(underlying: error) }
    try await current()
    guard (1...524288).contains(reply.count) else { throw Self.invalid() }
    try await update { $0.responses[stage] = Data(reply) }
    return reply
  }

  private func originalDispatch(stage: String, request: OrdinaryEnrollmentRequestRecord) throws
    -> KagemushaOrdinaryEnrollmentOriginalRequestV1 {
    let reservation = self.reservation
    let possession = pending?.possession
    let preparationOwner = prepared
    let retailOwner = retail
    let recheck: @Sendable () throws -> Void
    if stage == "start" {
      guard let possession, let preparationOwner else { throw Self.invalid() }
      let preparation = try preparationOwner.originalSignedPreparationBytes()
      let body = Data(request.body)
      recheck = {
        _ = try reservation.accountID()
        guard try preparationOwner.originalSignedPreparationBytes() == preparation,
          try possession.financialStartOriginal(originalSignedPreparation: preparation) == body else {
          throw KagemushaCoreCoordinatorErrorV1.invalidFrame("financial Start dispatch original changed")
        }
      }
    } else {
      recheck = {
        _ = try reservation.accountID()
        if let possession { _ = try possession.signingBytes() }
        else if let preparationOwner { _ = try preparationOwner.originalSignedPreparationBytes() }
        if let retailOwner { _ = try retailOwner.recoverOriginals() }
      }
    }
    return .init(stage: stage, value: request, recheckOriginal: recheck)
  }

  private func current() async throws {
    guard !unusable else { throw Self.invalid() }
    do {
      try await requireOwner(); try await signer.requireOriginalOwner()
      guard try await journal.loadOriginal() == serialized,
        try OrdinaryEnrollmentRecord.owner(reservation, signer: signer) == record.owner else { throw Self.invalid() }
      if let pending { _ = try pending.possession.signingBytes() }
      // Once a final identity is admitted, its current Native E/credential gate replaces
      // fresh C time. A saved response alone never selects this transition.
      if identity == nil, let prepared { _ = try prepared.originalSignedPreparationBytes() }
    } catch { unusable = true; throw error }
  }
  private func update(_ change: (inout OrdinaryEnrollmentRecord) -> Void) async throws {
    try await current()
    var next = record; change(&next); try next.validate()
    let encoded = try next.encoded()
    try await journal.retainOriginal(encoded, expecting: serialized)
    guard try await journal.loadOriginal() == encoded else { unusable = true; throw Self.invalid() }
    record = next; serialized = encoded; try await current()
  }
  private static func invalid() -> KagemushaCoreCoordinatorErrorV1 { .invalidFrame("ordinary enrollment original owner changed") }
  private static func unknown() -> KagemushaAppAttestEvidenceErrorV1 { .assertionOutcomeUnknown }
}

private struct OrdinaryEnrollmentTransportFailure: Error { let underlying: Error }

struct OrdinaryEnrollmentRequestRecord: Codable {
  let requestID, idempotencyKey: String
  let body: Data
}
struct OrdinaryEnrollmentRecord: Codable {
  let owner: [String: Data]
  let assertionDirectory: String
  var requests: [String: OrdinaryEnrollmentRequestRecord] = [:]
  var responses: [String: Data] = [:]
  var collectionInvoked = false
  var assertionJournalCreationInvoked = false
  var possessionInvoked = false
  var walletSigningInvoked = false
  static func owner(_ reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
    signer: any KagemushaOrdinaryEnrollmentWalletSignerV1) throws -> [String: Data] {
    let account = try reservation.accountID()
    guard signer.accountID == account, signer.publicKey.count == 32 else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("existing wallet signer differs") }
    return try ["account": Data(account.utf8), "walletKey": Data(signer.publicKey),
      "nonce": reservation.clientNonce(), "release": reservation.releaseID(),
      "profile": reservation.hardwareProfileID(), "lane": reservation.laneID(),
      "financialCommitment": reservation.financialAuthorityCommitment(), "requestID": Data(reservation.requestID().utf8)]
  }
  func encoded() throws -> Data {
    let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
    return try encoder.encode(self)
  }
  func validate() throws {
    guard !owner.isEmpty, assertionDirectory.hasPrefix("/"), Set(responses.keys).isSubset(of: Set(requests.keys)),
      Set(requests.values.map(\.requestID)).count == requests.count,
      Set(requests.values.map(\.idempotencyKey)).count == requests.count,
      Set(requests.keys).isSubset(of: ["prepare", "raw-attestation", "certificate", "start", "finish"]) else { throw invalid() }
    for request in requests.values {
      guard UUID(uuidString: request.requestID)?.uuidString.lowercased() == request.requestID,
        UUID(uuidString: request.idempotencyKey)?.uuidString.lowercased() == request.idempotencyKey,
        (1...262144).contains(request.body.count) else { throw invalid() }
    }
    for response in responses.values { guard (1...524288).contains(response.count) else { throw invalid() } }
  }
  private func invalid() -> KagemushaCoreCoordinatorErrorV1 { .invalidFrame("invalid detached enrollment record") }
}

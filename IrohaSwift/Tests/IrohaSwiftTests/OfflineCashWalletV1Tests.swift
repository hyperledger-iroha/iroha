import Foundation
import XCTest

@testable import IrohaSwift

final class OfflineCashWalletV1Tests: XCTestCase {
  func testAuthorizationPrecedesCapacityReservationAndTicketIssuance() throws {
    let fixture = try Fixture()
    let provider = FakeProvider(fixture: fixture)
    let wallet = try OfflineCashWalletV1.open(provider: provider)

    let authorization = try wallet.prepareAcceptanceIntentAuthorization(request: fixture.request)
    XCTAssertFalse(provider.didReserveInbox)

    let ticket = try wallet.issueAcceptanceTicket(
      request: fixture.request, authorization: authorization)
    XCTAssertTrue(provider.didPrepareAuthorization)
    XCTAssertTrue(provider.didReserveInbox)
    XCTAssertEqual(ticket.exactAmount, fixture.amount)
    XCTAssertEqual(ticket.recipientOneTimeKey, fixture.ticket.recipientOneTimeKey)
  }

  func testReceiveFoldConsumesExactlyOneCredit() throws {
    let fixture = try Fixture()
    let provider = FakeProvider(fixture: fixture)
    provider.pendingCreditWatermarkValue = OfflineCashUInt128V1(1)
    provider.foldResults = [try fixture.foldedStateBytes(sequence: 1, commitmentByte: 54)]
    let wallet = try OfflineCashWalletV1.open(provider: provider)
    let result = try XCTUnwrap(wallet.foldNextReceive())
    XCTAssertEqual(result.aggregateState.sequence, OfflineCashUInt128V1(1))
  }

  func testCommitPaymentSynchronouslyDrainsEveryPendingCreditBeforeCommit() throws {
    let fixture = try Fixture()
    let provider = FakeProvider(fixture: fixture)
    provider.pendingCreditWatermarkValue = OfflineCashUInt128V1(39)
    provider.foldResults = try (1...39).map {
      try fixture.foldedStateBytes(sequence: UInt64($0), commitmentByte: UInt8(54 + $0))
    }
    let wallet = try OfflineCashWalletV1.open(provider: provider)

    XCTAssertThrowsError(
      try wallet.commitPayment(
        request: fixture.request, authorization: fixture.authorization, ticket: fixture.ticket))
    XCTAssertEqual(provider.foldCount, 39)
    XCTAssertEqual(provider.events.last, "commit")
    XCTAssertEqual(
      provider.observedWatermarks,
      Array(repeating: OfflineCashUInt128V1(39), count: 40))
  }

  func testNonSuccessorFoldStateFailsInsteadOfSpinning() throws {
    let fixture = try Fixture()
    let provider = FakeProvider(fixture: fixture)
    provider.pendingCreditWatermarkValue = OfflineCashUInt128V1(1)
    provider.foldResults = [fixture.stateBytes]
    let wallet = try OfflineCashWalletV1.open(provider: provider)

    XCTAssertThrowsError(try wallet.drainStagedCredits())
    XCTAssertEqual(provider.events, ["fold"])
  }

  func testStageMintCreditUsesTheAuthorizationSealedCreditID() throws {
    let fixture = try Fixture()
    let provider = FakeProvider(fixture: fixture)
    let wallet = try OfflineCashWalletV1.open(provider: provider)

    XCTAssertNotEqual(
      fixture.mintAuthorization.statement.creditID, Data(repeating: 0, count: 32))
    XCTAssertEqual(
      fixture.mintCredit.statement.lifecycle.creditID,
      fixture.mintAuthorization.statement.creditID)
    XCTAssertEqual(
      try wallet.stageMintCredit(
        authorization: fixture.mintAuthorization, credit: fixture.mintCredit),
      .staged)
    XCTAssertTrue(provider.didStageMintCredit)
  }

  func testHardwareEpochRotationDrainsWholeWatermarkBeforeRotating() throws {
    let fixture = try Fixture()
    let provider = FakeProvider(fixture: fixture)
    provider.pendingCreditWatermarkValue = OfflineCashUInt128V1(17)
    provider.foldResults = try (1...17).map {
      try fixture.foldedStateBytes(sequence: UInt64($0), commitmentByte: UInt8(57 + $0))
    }
    let rotation = try fixture.rotation(generation: 2)
    provider.rotationQualification = rotation.qualification
    provider.rotationStateBytes = rotation.stateBytes
    let wallet = try OfflineCashWalletV1.open(provider: provider)

    let result = try wallet.rotateHardwareEpoch()

    XCTAssertEqual(provider.foldCount, 17)
    XCTAssertEqual(provider.events.last, "rotate")
    XCTAssertTrue(result.sequence.isZero)
    XCTAssertEqual(result.hardwareEpochID, rotation.qualification.credential.hardwareEpochID)
    XCTAssertEqual(
      wallet.qualification().credential.hardwareEpochGeneration, 2)
  }

  func testHardwareEpochRotationRejectsSkippedGeneration() throws {
    let fixture = try Fixture()
    let provider = FakeProvider(fixture: fixture)
    let rotation = try fixture.rotation(generation: 3)
    provider.rotationQualification = rotation.qualification
    provider.rotationStateBytes = rotation.stateBytes
    let wallet = try OfflineCashWalletV1.open(provider: provider)

    XCTAssertThrowsError(try wallet.rotateHardwareEpoch())
    XCTAssertEqual(wallet.aggregateState(), fixture.state)
  }
}

private final class FakeProvider: OfflineCashHardwareProviderV1 {
  let fixture: Fixture
  var qualificationValue: OfflineCashHardwareQualificationV1
  var didPrepareAuthorization = false
  var didReserveInbox = false
  var didStageMintCredit = false
  var pendingCreditWatermarkValue = OfflineCashUInt128V1.zero
  var foldResults: [Data] = []
  var foldCount = 0
  var observedWatermarks: [OfflineCashUInt128V1] = []
  var events: [String] = []
  var rotationQualification: OfflineCashHardwareQualificationV1?
  var rotationStateBytes: Data?

  init(fixture: Fixture) {
    self.fixture = fixture
    qualificationValue = fixture.qualification
  }

  func qualification() throws -> OfflineCashHardwareQualificationV1 { qualificationValue }
  func recoverAggregateState() throws -> Data { fixture.stateBytes }

  func createPaymentRequest(
    recipient: OfflineCashAccountIDV1, amount: OfflineCashUInt128V1,
    validityWindowMS: UInt64
  ) throws -> Data {
    try OfflineCashNoritoV1.encodePaymentRequestShape(fixture.request)
  }

  func prepareAcceptanceIntentAuthorization(
    canonicalRequest: Data, exactAmount: OfflineCashUInt128V1
  ) throws -> Data {
    didPrepareAuthorization = true
    XCTAssertEqual(
      canonicalRequest, try OfflineCashNoritoV1.encodePaymentRequestShape(fixture.request))
    XCTAssertEqual(exactAmount, fixture.amount)
    return try OfflineCashNoritoV1.encodeAcceptanceIntentAuthorizationShape(fixture.authorization)
  }

  func verifyAuthorizationReserveInboxAndIssueTicket(
    canonicalRequest: Data, canonicalAuthorization: Data
  ) throws -> Data {
    XCTAssertTrue(didPrepareAuthorization)
    didReserveInbox = true
    XCTAssertEqual(
      canonicalAuthorization,
      try OfflineCashNoritoV1.encodeAcceptanceIntentAuthorizationShape(fixture.authorization))
    return try OfflineCashNoritoV1.encodeAcceptanceTicketShape(fixture.ticket)
  }

  func prepareProveCommitPayment(
    canonicalRequest: Data, canonicalAuthorization: Data, canonicalTicket: Data
  ) throws -> Data {
    events.append("commit")
    throw OfflineCashWalletErrorV1.invalidHardwareResult("unused")
  }

  func recoverPayment(acceptanceTicketID: Data) throws -> Data? { nil }

  func verifyAndStageInboundPayment(
    canonicalRequest: Data, canonicalAuthorization: Data, canonicalTicket: Data,
    canonicalPayment: Data
  ) throws -> (OfflineCashHardwareStageDispositionV1, Data) {
    throw OfflineCashWalletErrorV1.invalidHardwareResult("unused")
  }

  func releasePaymentOutbox(
    canonicalRequest: Data, canonicalPayment: Data, canonicalAcknowledgement: Data
  ) throws {}

  func prepareMintAuthorization(
    operationID: Data, amount: OfflineCashUInt128V1, payer: OfflineCashAccountIDV1,
    recipient: OfflineCashAccountIDV1
  ) throws -> Data { throw OfflineCashWalletErrorV1.invalidHardwareResult("unused") }

  func verifyAuthorizationAndStageMintCredit(
    canonicalAuthorization: Data, canonicalMintCredit: Data
  ) throws -> OfflineCashHardwareStageDispositionV1 {
    XCTAssertEqual(
      canonicalAuthorization,
      try OfflineCashNoritoV1.encodeMintAuthorizationShape(fixture.mintAuthorization))
    XCTAssertEqual(
      canonicalMintCredit,
      try OfflineCashNoritoV1.encodeMintCreditShape(fixture.mintCredit))
    didStageMintCredit = true
    return .staged
  }

  func pendingCreditWatermark() throws -> OfflineCashUInt128V1 {
    pendingCreditWatermarkValue
  }

  func foldNextReceive(
    upToInclusive watermark: OfflineCashUInt128V1
  ) throws -> Data? {
    observedWatermarks.append(watermark)
    guard !foldResults.isEmpty else {
      events.append("fold:none")
      return nil
    }
    let result = foldResults.removeFirst()
    foldCount += 1
    events.append("fold")
    return result
  }

  func prepareProveCommitRedemption(
    amount: OfflineCashUInt128V1, beneficiary: OfflineCashAccountIDV1
  ) throws -> Data { throw OfflineCashWalletErrorV1.invalidHardwareResult("unused") }

  func recoverRedemption(redemptionID: Data) throws -> Data? { nil }

  func rotateHardwareEpoch() throws -> Data {
    events.append("rotate")
    guard let rotationQualification, let rotationStateBytes else {
      throw OfflineCashWalletErrorV1.invalidHardwareResult("unused")
    }
    qualificationValue = rotationQualification
    return rotationStateBytes
  }
}

private struct Fixture {
  let amount = OfflineCashUInt128V1(42)
  let qualification: OfflineCashHardwareQualificationV1
  let state: OfflineCashAggregateStateCommitmentV1
  let stateBytes: Data
  let request: OfflineCashPaymentRequestV1
  let authorization: OfflineCashAcceptanceIntentAuthorizationV1
  let ticket: OfflineCashAcceptanceTicketV1
  let mintAuthorization: OfflineCashMintAuthorizationV1
  let mintCredit: OfflineCashMintCreditV1

  init() throws {
    let networkID = Fixture.digest(1)
    let releaseID = Fixture.digest(2)
    let profileID = Fixture.digest(3)
    let suiteID = Fixture.digest(4)
    let publicKey = try OfflineCashDevicePublicKeyV1(sec1Bytes: Fixture.publicKeyBytes())
    let signature = try Fixture.signature()
    let profile = try OfflineCashHardwareProfileV1(
      hardwareProfileID: profileID, providerID: Fixture.digest(5),
      platformClass: .appleOEMService, productClassDigest: Fixture.digest(6),
      firmwarePolicyDigest: Fixture.digest(7),
      enrollmentAttestationVerifierDigest: Fixture.digest(8),
      attestationTrustRootsDigest: Fixture.digest(9),
      allowedSuiteCommitment: Fixture.digest(10), policyEpoch: 1,
      governanceCredentialPublicKey: publicKey, capabilityMask: 0xffff,
      qualificationReportDigest: Fixture.digest(11), validFromMS: 1,
      expiresAtMS: 10_000)
    let credential = try OfflineCashHardwareCredentialV1(
      credentialID: Fixture.digest(12), networkID: networkID,
      hardwareProfileID: profileID, suiteID: suiteID,
      firmwarePolicyDigest: profile.firmwarePolicyDigest, policyEpoch: 1,
      laneCommitment: Fixture.digest(13), hardwareEpochID: Fixture.digest(14),
      hardwareEpochGeneration: 1, devicePublicKey: publicKey,
      deviceKeyReference: Fixture.digest(15), issuedAtMS: 10, expiresAtMS: 9_000,
      governanceSignature: signature)
    let hardwarePolicyDigest = Fixture.digest(53)
    qualification = try OfflineCashHardwareQualificationV1(
      releaseID: releaseID, hardwarePolicyDigest: hardwarePolicyDigest,
      profile: profile, credential: credential)

    let asset = try OfflineCashAssetDefinitionIDV1("6TEAJqbb8oEPmLncoNiMRbLEK6tw")
    let incarnation = try OfflineCashAssetIncarnationV1(bytes: Fixture.digest(16))
    state = try OfflineCashAggregateStateCommitmentV1(
      releaseID: releaseID, networkID: networkID, asset: asset,
      assetIncarnation: incarnation, scale: 4, liabilityPoolID: Fixture.digest(17),
      laneID: Fixture.digest(18), hardwareEpochID: credential.hardwareEpochID,
      keyReference: credential.deviceKeyReference, hardwarePolicyID: hardwarePolicyDigest,
      sequence: OfflineCashUInt128V1(0), stateCommitment: Fixture.digest(19))
    stateBytes = try OfflineCashNoritoV1.encodeAggregateStateShape(state)

    let recipient = try OfflineCashAccountIDV1(
      "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
    let requestDigest = Fixture.digest(20)
    request = try OfflineCashPaymentRequestV1(
      releaseID: releaseID, networkID: networkID, asset: asset,
      assetIncarnation: incarnation, scale: 4, liabilityPoolID: state.liabilityPoolID,
      recipient: recipient, amount: amount, hardwareCredential: credential,
      requestID: Fixture.digest(21), issuedAtMS: 100, expiresAtMS: 200,
      signature: signature)
    let intent = try OfflineCashAcceptanceIntentV1(
      requestDigest: requestDigest, intentID: Fixture.digest(22), exactAmount: amount,
      senderOneTimeCommitment: Fixture.digest(23))
    let proof = try OfflineCashPairedProofV1(
      eqProtocolDigest: Fixture.digest(24), epProtocolDigest: Fixture.digest(25),
      semanticDigest: Fixture.digest(26), guardEqCredentialAudit: Fixture.digest(27),
      guardEpCredentialAudit: Fixture.digest(28), eqDeferredAudit: Fixture.digest(29),
      epDeferredAudit: Fixture.digest(30), eqProof: Data([1]), epProof: Data([2]),
      eqHistory: Data(repeating: 3, count: 544), epHistory: Data(repeating: 4, count: 544))
    authorization = try OfflineCashAcceptanceIntentAuthorizationV1(
      statement: OfflineCashAcceptanceIntentAuthorizationStatementV1(
        intent: intent, releaseID: releaseID, suiteID: suiteID,
        vkDigest: Fixture.digest(31), artifactManifestDigest: Fixture.digest(32)),
      proof: proof)
    ticket = try OfflineCashAcceptanceTicketV1(
      networkID: networkID, requestID: request.requestID, requestDigest: requestDigest,
      acceptanceTicketID: Fixture.digest(33), asset: asset, assetIncarnation: incarnation,
      scale: 4, intentDigest: Fixture.digest(34), exactAmount: amount,
      reservedInboxBytes: 8_960,
      recipientOneTimeKey: OfflineCashX25519PublicKeyV1(rawBytes: Fixture.digest(35)),
      hardwareProfileID: profileID, policyEpoch: 1, issuedAtMS: 110,
      expiresAtMS: 190, signature: signature)

    let creditID = Fixture.digest(36)
    let recipientCredentialCommitment = Fixture.digest(37)
    let issuanceCommitment = Fixture.digest(38)
    let artifactManifestDigest = Fixture.digest(39)
    let context = try OfflineCashMintAuthorizationContextV1(
      operationID: Fixture.digest(40), releaseID: releaseID, suiteID: suiteID,
      vkDigest: Fixture.digest(41), artifactManifestDigest: artifactManifestDigest,
      networkID: networkID, asset: asset, assetIncarnation: incarnation, scale: 4,
      liabilityPoolID: state.liabilityPoolID, amount: amount, payer: recipient,
      recipient: recipient, hardwareCredentialID: credential.credentialID,
      hardwareProfileID: profileID, policyEpoch: 1,
      recipientCredentialCommitment: recipientCredentialCommitment,
      creditCommitment: Fixture.digest(42),
      recipientOneTimeKey: try OfflineCashX25519PublicKeyV1(rawBytes: Fixture.digest(43)))
    mintAuthorization = try OfflineCashMintAuthorizationV1(
      statement: try OfflineCashMintAuthorizationStatementV1(
        context: context, issuanceCommitment: issuanceCommitment, creditID: creditID,
        ciphertextDigest: Fixture.digest(44)),
      proof: proof)
    let lifecycle = try OfflineCashLifecycleBindingV1(
      networkID: networkID, suiteID: suiteID, vkDigest: context.vkDigest,
      releaseID: releaseID, asset: asset, assetIncarnation: incarnation, scale: 4,
      liabilityPoolID: state.liabilityPoolID, hardwareProfileID: profileID, policyEpoch: 1,
      operationKind: .mintFold, requestID: Data(repeating: 0, count: 32),
      acceptanceTicketID: Data(repeating: 0, count: 32), creditID: creditID,
      ciphertextDigest: Fixture.digest(44))
    let envelope = try OfflineCashEncryptedCreditEnvelopeV1(
      ephemeralX25519PublicKey: OfflineCashX25519PublicKeyV1(
        rawBytes: Fixture.digest(45)),
      nonce: Data(repeating: 46, count: OfflineCashWireV1.xchachaNonceBytes),
      ciphertextAndTag: Data(repeating: 47, count: OfflineCashWireV1.xchachaTagBytes + 1))
    mintCredit = try OfflineCashMintCreditV1(
      statement: try OfflineCashMintCreditStatementV1(
        lifecycle: lifecycle,
        recipientCredentialCommitment: recipientCredentialCommitment,
        authorizationContextDigest: Fixture.digest(48),
        mintAuthorizationDigest: Fixture.digest(49), amount: amount,
        issuanceCommitment: issuanceCommitment, recipient: recipient,
        creditCommitment: context.creditCommitment, mintedAtMS: 120),
      proof: proof, finalityCertificateBinding: Fixture.digest(50),
      finalityAuthorityHead: Fixture.digest(51),
      finalityGenesisRosterID: Fixture.digest(52),
      finalityProofBindingDigest: Fixture.digest(53),
      encryptedCredit: envelope,
      artifactManifestDigest: artifactManifestDigest)
  }

  func foldedStateBytes(sequence: UInt64, commitmentByte: UInt8) throws -> Data {
    try OfflineCashNoritoV1.encodeAggregateStateShape(
      OfflineCashAggregateStateCommitmentV1(
        releaseID: state.releaseID, networkID: state.networkID, asset: state.asset,
        assetIncarnation: state.assetIncarnation, scale: state.scale,
        liabilityPoolID: state.liabilityPoolID, laneID: state.laneID,
        hardwareEpochID: state.hardwareEpochID, keyReference: state.keyReference,
        hardwarePolicyID: state.hardwarePolicyID,
        sequence: OfflineCashUInt128V1(sequence),
        stateCommitment: Fixture.digest(commitmentByte)))
  }

  func rotation(generation: UInt64) throws -> (
    qualification: OfflineCashHardwareQualificationV1, stateBytes: Data
  ) {
    let previous = qualification.credential
    let rotatedCredential = try OfflineCashHardwareCredentialV1(
      credentialID: Fixture.digest(60), networkID: previous.networkID,
      hardwareProfileID: previous.hardwareProfileID, suiteID: previous.suiteID,
      firmwarePolicyDigest: previous.firmwarePolicyDigest, policyEpoch: previous.policyEpoch,
      laneCommitment: previous.laneCommitment, hardwareEpochID: Fixture.digest(61),
      hardwareEpochGeneration: generation, devicePublicKey: previous.devicePublicKey,
      deviceKeyReference: Fixture.digest(62), issuedAtMS: previous.issuedAtMS,
      expiresAtMS: previous.expiresAtMS, governanceSignature: previous.governanceSignature)
    let rotatedQualification = try OfflineCashHardwareQualificationV1(
      releaseID: qualification.releaseID,
      hardwarePolicyDigest: qualification.hardwarePolicyDigest,
      profile: qualification.profile, credential: rotatedCredential)
    let rotatedState = try OfflineCashAggregateStateCommitmentV1(
      releaseID: state.releaseID, networkID: state.networkID, asset: state.asset,
      assetIncarnation: state.assetIncarnation, scale: state.scale,
      liabilityPoolID: state.liabilityPoolID, laneID: state.laneID,
      hardwareEpochID: rotatedCredential.hardwareEpochID,
      keyReference: rotatedCredential.deviceKeyReference,
      hardwarePolicyID: qualification.hardwarePolicyDigest,
      sequence: OfflineCashUInt128V1.zero, stateCommitment: Fixture.digest(63))
    return (
      rotatedQualification,
      try OfflineCashNoritoV1.encodeAggregateStateShape(rotatedState)
    )
  }

  static func digest(_ byte: UInt8) -> Data {
    var value = Data(repeating: byte, count: 32)
    value[31] |= 1
    return value
  }

  static func signature() throws -> OfflineCashDeviceSignatureV1 {
    var scalar = Data(repeating: 0, count: 32)
    scalar[31] = 1
    return try OfflineCashDeviceSignatureV1(rawBytes: scalar + scalar)
  }

  static func publicKeyBytes() -> Data {
    Data([
      0x04,
      0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47,
      0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40, 0xf2,
      0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0,
      0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98, 0xc2, 0x96,
      0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b,
      0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16,
      0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce,
      0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5,
    ])
  }
}

import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

/// Scripted endpoints verify orchestration only; they provide no proof or hardware qualification.
final class KagemushaNativeCoreCoordinatorAdapterV1Tests: XCTestCase {
  func testOutgoingStateProofExportReturnsOneBoundedPairForOriginalOperation() throws {
    let endpoint = Endpoint()
    let core = try adapter(endpoint)
    let operationID = Data(repeating: 0x66, count: 32)
    let publicInputs = Data([0x81])
    let pairedProof = Data([0x82])
    endpoint.expect(.exportOutgoingStateProof, [operationID],
      [operationID, publicInputs, pairedProof])
    let pair = try core.exportOutgoingStateProof(operationID: operationID)
    XCTAssertEqual(pair.operationID, operationID)
    XCTAssertEqual(pair.publicInputsArchive, publicInputs)
    XCTAssertEqual(pair.pairedProofArchive, pairedProof)
    XCTAssertThrowsError(try KagemushaOutgoingStateProofArchivesV1(
      operationID: Data(repeating: 0, count: 32),
      publicInputsArchive: publicInputs, pairedProofArchive: pairedProof))
    XCTAssertThrowsError(try KagemushaOutgoingStateProofArchivesV1(
      operationID: operationID, publicInputsArchive: Data(), pairedProofArchive: pairedProof))
    XCTAssertThrowsError(try KagemushaOutgoingStateProofArchivesV1(
      operationID: operationID, publicInputsArchive: publicInputs,
      pairedProofArchive: Data(repeating: 1, count: 6_529)))
    endpoint.expect(.exportOutgoingStateProof, [operationID],
      [Data(repeating: 0x67, count: 32), publicInputs, pairedProof])
    XCTAssertThrowsError(try core.exportOutgoingStateProof(operationID: operationID))
  }

  func testCloseRevokesTypedAdapterBeforeMonetaryDispatch() throws {
    let endpoint = Endpoint()
    let core = try adapter(endpoint)
    try core.close()
    try core.close()
    XCTAssertEqual(endpoint.closeCalls, 1)
    XCTAssertThrowsError(try core.reserveOperationID(
      operation: 5, operationID: Data(repeating: 7, count: 32), publicBinding: Data([1])))
    XCTAssertEqual(endpoint.calls, 0)
  }

  func testReleaseAcceptanceForwardsOriginalFrameAndRejectsMissingOrSubstitutedOriginal() throws {
    let f = try Fixture()
    let endpoint = Endpoint()
    let core = try adapter(endpoint)
    let id = f.archive.preparation.operationID
    let signature = try KagemushaNoritoV1.decodePaymentRequestShapeExact(f.archive.paymentRequest).hardwareCredential.governanceSignature.rawBytes
    let reply = Data([0x12])
    let original = try testSignedDeviceResponseFrame(operation: 12, status: .success,
      requestID: id, payload: reply, authenticator: signature)
    XCTAssertThrowsError(try core.acceptAuthenticatedDeviceReply(operation: 12, requestID: id,
      canonicalCommand: Data([1]), canonicalReply: reply, responseAuthenticator: signature,
      originalResponse: nil, qualification: f.qualification))
    XCTAssertThrowsError(try core.acceptAuthenticatedDeviceReply(operation: 12, requestID: id,
      canonicalCommand: Data([1]), canonicalReply: Data([2]), responseAuthenticator: signature,
      originalResponse: original, qualification: f.qualification))
    let wrongOperation = try testSignedDeviceResponseFrame(operation: 8, status: .success,
      requestID: id, payload: reply, authenticator: signature)
    let wrongRequest = try testSignedDeviceResponseFrame(operation: 12, status: .success,
      requestID: Data(repeating: 0x7f, count: 32), payload: reply, authenticator: signature)
    let uncertain = try testSignedDeviceResponseFrame(operation: 12, status: .unavailable,
      requestID: id, payload: reply, authenticator: signature)
    var tampered = original
    tampered[tampered.count - signature.count - 1] ^= 1
    for substituted in [wrongOperation, wrongRequest, uncertain, tampered] {
      XCTAssertThrowsError(try core.acceptAuthenticatedDeviceReply(operation: 12, requestID: id,
        canonicalCommand: Data([1]), canonicalReply: reply, responseAuthenticator: signature,
        originalResponse: substituted, qualification: f.qualification))
    }
    var anotherScalar = Data(repeating: 0, count: 32); anotherScalar[31] = 2
    let differentAuthenticator = anotherScalar + anotherScalar
    XCTAssertThrowsError(try core.acceptAuthenticatedDeviceReply(operation: 12, requestID: id,
      canonicalCommand: Data([1]), canonicalReply: reply, responseAuthenticator: differentAuthenticator,
      originalResponse: original, qualification: f.qualification))
    XCTAssertEqual(endpoint.calls, 0)
    endpoint.expect(.acceptAuthenticatedReply,
      [u32(12), id, Data([1]), reply, signature] + (try f.qualificationFields) + [original], [])
    try core.acceptAuthenticatedDeviceReply(operation: 12, requestID: id, canonicalCommand: Data([1]),
      canonicalReply: reply, responseAuthenticator: signature, originalResponse: original, qualification: f.qualification)
    XCTAssertEqual(endpoint.calls, 1)
  }

  func testWalletTransitionMethodsMapExactNativeFields() throws {
    let f = try Fixture()
    let endpoint = Endpoint()
    let core = try adapter(endpoint)
    let id = f.archive.preparation.operationID
    let prep = try f.archive.bytes("preparation")
    let candidate = try f.archive.bytes("candidate")
    let responseAuthenticator = try KagemushaNoritoV1.decodePaymentRequestShapeExact(f.archive.paymentRequest).hardwareCredential.governanceSignature.rawBytes
    endpoint.expect(.reserveOperationID, [u32(5), id, Data([9])], [id])
    XCTAssertEqual(try core.reserveOperationID(operation: 5, operationID: id, publicBinding: Data([9])), id)
    try endpoint.expect(.acceptQualification, f.qualificationFields + [f.qualification.hardwarePolicyDigest], [])
    try core.acceptQualification(f.qualification, hardwarePolicyDigest: f.qualification.hardwarePolicyDigest)
    try endpoint.expect(.acceptAuthenticatedReply, [u32(5), id, Data([7]), Data([8]), responseAuthenticator] + f.qualificationFields, [])
    try core.acceptAuthenticatedDeviceReply(operation: 5, requestID: id, canonicalCommand: Data([7]),
      canonicalReply: Data([8]), responseAuthenticator: responseAuthenticator, originalResponse: nil,
      qualification: f.qualification)
    try endpoint.expect(.beginSenderTransition, [id, u32(0), f.archive.paymentRequest] + f.qualificationFields, [id, prep])
    XCTAssertEqual(try core.beginSenderTransition(operationID: id, inputs: f.archive.inputs, qualification: f.qualification), f.archive.preparation)
    endpoint.expect(.provePreparedSenderTransition, [prep, Data([5])], [candidate])
    XCTAssertEqual(try core.provePreparedSenderTransition(preparation: f.archive.preparation, authenticatedPreparationReply: Data([5])), f.archive.candidate)
    let originalCommit = try testSignedDeviceResponseFrame(operation: 7, status: .success,
      requestID: id, payload: Data([7]), authenticator: responseAuthenticator)
    endpoint.expect(.buildTerminalEnvelope, [candidate, originalCommit], [f.archive.payment])
    XCTAssertEqual(try core.terminalEnvelope(candidate: f.archive.candidate,
      originalSignedCommitResponse: originalCommit), f.archive.payment)
    try endpoint.expect(.acceptInstalledTerminal, [candidate, f.archive.payment, Data([9]), Data([10]), Data([21])], [f.archive.payment, f.aggregate])
    XCTAssertEqual(try core.acceptInstalledTerminal(candidate: f.archive.candidate, canonicalEnvelope: f.archive.payment,
      authenticatedInstallReply: Data([9]), authenticatedInstalledReply: Data([10]), authenticatedWalletSnapshotReply: Data([21])).aggregateState, try f.aggregate)
    let recovery = try f.recovery()
    let recoveryBytes = try KagemushaCoreCoordinatorArchiveV1.encodeRecoveryShape(recovery)
    try endpoint.expect(.recoverSender, [Data([0]), f.terminalID, u32(0)] + f.qualificationFields, [id, f.terminalID, recoveryBytes])
    XCTAssertEqual(try core.senderRecovery(kind: .payment, terminalID: f.terminalID, qualification: f.qualification), recovery)
    try endpoint.expect(.recoverSender, [Data([1]), id, u32(0)] + f.qualificationFields, [])
    XCTAssertNil(try core.senderRecoveryByOperationID(kind: .payment, operationID: id, qualification: f.qualification))
    endpoint.expect(.recoverTerminalEnvelope, [recoveryBytes, Data([10])], [f.archive.payment])
    XCTAssertEqual(try core.recoverTerminalEnvelope(recovery: recovery, authenticatedInstalledReply: Data([10])), f.archive.payment)
    try endpoint.expect(.releaseOutbox,
      [f.terminalID, u32(0), f.archive.paymentRequest, f.archive.payment, u32(0) + f.archive.acknowledgement] + f.qualificationFields,
      [id, prep, f.envelopeDigest, f.archive.payment, Data([12])])
    let release = try core.outboxRelease(creditID: f.terminalID, inputs: f.archive.inputs, canonicalPayment: f.archive.payment,
      terminalReceipt: .paymentAcknowledgement(canonicalAcknowledgement: f.archive.acknowledgement), qualification: f.qualification)
    XCTAssertEqual(release.operationID, id)
    XCTAssertEqual(release.envelopeDigest, try f.envelopeDigest)
    XCTAssertEqual(release.hardwareReleaseAuthorization, Data([12]))
    let observation = try KagemushaDeviceOperationCodecV1.encodeControlCommand(.recoverWalletSnapshot)
    endpoint.expect(.beginObservation, [u32(21), observation], [id])
    XCTAssertEqual(try core.beginObservation(operation: 21, canonicalCommand: observation), id)
    XCTAssertEqual(endpoint.calls, 13)
  }

  func testBeginPreservesNativeProviderRootDistinctFromHardwareManifestDigest() throws {
    let f = try Fixture(), qualification = try f.qualification
    let preparation = f.archive.preparation
    XCTAssertNotEqual(preparation.context.devicePolicyBinding.hardwarePolicyID,
      qualification.hardwarePolicyDigest)
    let endpoint = Endpoint(), core = try adapter(endpoint)
    try endpoint.expect(.acceptQualification,
      f.qualificationFields + [qualification.hardwarePolicyDigest], [])
    try core.acceptQualification(qualification,
      hardwarePolicyDigest: qualification.hardwarePolicyDigest)
    try endpoint.expect(.beginSenderTransition,
      [preparation.operationID, u32(0), f.archive.paymentRequest] + f.qualificationFields,
      [preparation.operationID, f.archive.bytes("preparation")])
    XCTAssertEqual(try core.beginSenderTransition(operationID: preparation.operationID,
      inputs: f.archive.inputs, qualification: qualification), preparation)
    XCTAssertEqual(endpoint.calls, 3)
  }

  func testBeginRejectsSubstitutedNativeProviderRootOrHardwareManifestPin() throws {
    let f = try Fixture(), qualification = try f.qualification
    let preparation = f.archive.preparation
    let root = preparation.context.devicePolicyBinding.hardwarePolicyID
    XCTAssertNotEqual(root, qualification.hardwarePolicyDigest)
    for changed in 0..<3 {
      let endpoint = Endpoint()
      endpoint.policyFields = [changed == 0 ? digest(91) : qualification.releaseID,
        changed == 1 ? digest(92) : qualification.hardwarePolicyDigest,
        changed == 2 ? digest(93) : root]
      try endpoint.expect(.beginSenderTransition, nil,
        [preparation.operationID, f.archive.bytes("preparation")])
      XCTAssertThrowsError(try adapter(endpoint).beginSenderTransition(
        operationID: preparation.operationID, inputs: f.archive.inputs,
        qualification: qualification))
    }
  }

  func testProviderDispatchesDistinctProviderRootWithoutChangingHardwareManifestPin() throws {
    // This scripted route stops at unavailable hardware op5; it creates no proof or funds.
    let f = try Fixture(), qualification = try f.qualification
    let preparation = f.archive.preparation
    let qualificationBytes = try qualificationReply(qualification)
    XCTAssertNotEqual(preparation.context.devicePolicyBinding.hardwarePolicyID,
      qualification.hardwarePolicyDigest)
    let endpoint = Endpoint()
    endpoint.responseHandler = { method, fields in
      switch method {
      case .reserveOperationID: return [fields[1]]
      case .beginObservation: return [digest(0x45)]
      case .acceptQualification:
        XCTAssertEqual(fields.last, qualification.hardwarePolicyDigest)
        return []
      case .acceptAuthenticatedReply: return []
      case .beginSenderTransition:
        return try [preparation.operationID, f.archive.bytes("preparation")]
      default: throw TestError.unexpectedCall
      }
    }
    var operations: [UInt8] = []
    let transport = Transport(qualification: qualification) { operation, requestID, command, key in
      operations.append(operation)
      if operation == 1 {
        return try testAuthenticatedDeviceResponse(operation: operation, status: .success,
          canonicalReply: qualificationBytes, authenticator: responseSignature(operation),
          requestID: requestID)
      }
      XCTAssertEqual(operation, 5)
      XCTAssertEqual(key, qualification.credential.devicePublicKey.sec1Bytes)
      let decoded = try KagemushaDeviceOperationCodecV1.decodeSenderCommand(
        operation: operation, requestID: requestID, canonicalBytes: command)
      XCTAssertEqual(decoded.context, preparation.context)
      return try testAuthenticatedDeviceResponse(operation: operation, status: .unavailable,
        canonicalReply: Data(), authenticator: Data(), requestID: requestID)
    }
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport,
      core: try adapter(endpoint), intentOwner: testOperationIntentOwner(), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    XCTAssertThrowsError(try provider.prepareProveCommitPayment(
      operationID: preparation.operationID, canonicalRequest: f.archive.paymentRequest)) { error in
      XCTAssertEqual(error as? KagemushaAuthenticatedHardwareProviderErrorV1,
        .operationFailed(operation: 5, status: .unavailable))
    }
    XCTAssertEqual(operations, [1, 5])
  }

  func testWalletOpenPreservesProviderRootAndRejectsOtherQualificationBindings() throws {
    // Scripted observations test Swift correlation only; they establish no native authority.
    let f = try Fixture(), qualification = try f.qualification
    let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(f.aggregate)
    XCTAssertNotEqual(state.hardwarePolicyID, qualification.hardwarePolicyDigest)
    let qualificationBytes = try qualificationReply(qualification)
    func provider(aggregate: Data) throws -> KagemushaAuthenticatedHardwareProviderV1 {
      var length = UInt64(aggregate.count).littleEndian
      let vector = withUnsafeBytes(of: &length) { Data($0) } + aggregate
      let snapshot = replyArchive("wallet-recovery-snapshot-reply", [Data([1, 0]), Data([21]),
        Data([1]) + compactField(vector), Data(repeating: 0, count: 16),
        Data(repeating: 0, count: 16), Data(repeating: 0, count: 16)], alignment: 16)
      let endpoint = Endpoint()
      var next: UInt8 = 0x45
      endpoint.responseHandler = { method, _ in
        switch method {
        case .beginObservation:
          next += 1
          return [digest(next)]
        case .acceptQualification, .acceptAuthenticatedReply: return []
        default: throw TestError.unexpectedCall
        }
      }
      let transport = Transport(qualification: qualification) { operation, requestID, _, _ in
        XCTAssertTrue(operation == 1 || operation == 21)
        return try testAuthenticatedDeviceResponse(operation: operation, status: .success,
          canonicalReply: operation == 1 ? qualificationBytes : snapshot,
          authenticator: responseSignature(operation), requestID: requestID)
      }
      return KagemushaAuthenticatedHardwareProviderV1(transport: transport,
        core: try adapter(endpoint), intentOwner: testOperationIntentOwner(), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    }
    let wallet = try KagemushaWalletV1.open(provider: provider(aggregate: f.aggregate),
      allowBootstrap: { false })
    XCTAssertEqual(wallet.aggregateState(), state)
    for changed in 0..<4 {
      let invalid = try KagemushaAggregateStateCommitmentV1(
        releaseID: changed == 0 ? digest(91) : state.releaseID,
        networkID: changed == 1 ? digest(92) : state.networkID,
        asset: state.asset, assetIncarnation: state.assetIncarnation, scale: state.scale,
        liabilityPoolID: state.liabilityPoolID, laneID: state.laneID,
        hardwareEpochID: changed == 2 ? digest(93) : state.hardwareEpochID,
        keyReference: changed == 3 ? digest(94) : state.keyReference,
        hardwarePolicyID: state.hardwarePolicyID, sequence: state.sequence,
        stateCommitment: state.stateCommitment)
      XCTAssertThrowsError(try KagemushaWalletV1.open(
        provider: provider(aggregate: KagemushaNoritoV1.encodeAggregateStateShape(invalid)),
        allowBootstrap: { false })) { error in
        XCTAssertEqual(error as? KagemushaWalletErrorV1,
          .invalidHardwareResult("aggregate state qualification"))
      }
    }
  }

  func testBeginRejectsCanonicalArchiveWithDifferentOperationInputOrContext() throws {
    let f = try Fixture()
    let p = f.archive.preparation
    let invalid = try [
      KagemushaNativeSenderPreparationV1(operationID: digest(90), context: p.context, inputsDigest: p.inputsDigest),
      KagemushaNativeSenderPreparationV1(operationID: p.operationID, context: p.context, inputsDigest: digest(91)),
      KagemushaNativeSenderPreparationV1(operationID: p.operationID, context: f.context(coreKey: digest(92)), inputsDigest: p.inputsDigest),
    ]
    for preparation in invalid {
      let endpoint = Endpoint()
      endpoint.expect(.beginSenderTransition, nil, [p.operationID, try KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(preparation)])
      XCTAssertThrowsError(try adapter(endpoint).beginSenderTransition(operationID: p.operationID, inputs: f.archive.inputs, qualification: f.qualification))
    }
  }

  func testProveRejectsDifferentNestedPreparation() throws {
    let f = try Fixture()
    let c = f.archive.candidate
    let preparation = try KagemushaNativeSenderPreparationV1(operationID: digest(99), context: c.preparation.context, inputsDigest: c.preparation.inputsDigest)
    let candidate = try KagemushaNativeSenderCandidateV1(preparation: preparation, selector: c.selector,
      candidateDigest: c.candidateDigest, hardwareCommitAuthorization: c.hardwareCommitAuthorization)
    let endpoint = Endpoint()
    endpoint.expect(.provePreparedSenderTransition, nil, [try KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(candidate)])
    XCTAssertThrowsError(try adapter(endpoint).provePreparedSenderTransition(preparation: c.preparation, authenticatedPreparationReply: Data([5])))
  }

  func testRecoveryRejectsEmbeddedIdentityAndScopeChangesButPermitsRotation() throws {
    let f = try Fixture()
    let endpoint = Endpoint()
    let p = f.archive.preparation
    let invalid = try [f.recovery(operationID: digest(9)), f.recovery(terminalID: digest(9)),
      f.recovery(context: f.context(lane: digest(9))), f.recovery(context: f.context(generation: 2))]
    for recovery in invalid {
      try endpoint.expect(.recoverSender, nil, [p.operationID, f.terminalID, try KagemushaCoreCoordinatorArchiveV1.encodeRecoveryShape(recovery)])
      XCTAssertThrowsError(try adapter(endpoint).senderRecovery(kind: .payment, terminalID: f.terminalID, qualification: f.qualification))
    }
    let recovery = try f.recovery()
    try endpoint.expect(.recoverSender, nil, [p.operationID, f.terminalID, try KagemushaCoreCoordinatorArchiveV1.encodeRecoveryShape(recovery)])
    XCTAssertEqual(try adapter(endpoint).senderRecoveryByOperationID(kind: .payment, operationID: p.operationID,
      qualification: f.makeQualification(generation: 2)), recovery)
  }

  func testInstalledTerminalRejectsDifferentAggregateLaneAndPool() throws {
    let f = try Fixture()
    for state in try [f.aggregate(lane: digest(9)), f.aggregate(pool: digest(9)),
      f.aggregate(policy: digest(9))] {
      let endpoint = Endpoint()
      endpoint.expect(.acceptInstalledTerminal, nil, [f.archive.payment, state])
      XCTAssertThrowsError(try adapter(endpoint).acceptInstalledTerminal(candidate: f.archive.candidate,
        canonicalEnvelope: f.archive.payment, authenticatedInstallReply: Data([9]), authenticatedInstalledReply: Data([10]),
        authenticatedWalletSnapshotReply: Data([21])))
    }
  }

  func testReleaseRejectsOperationInputDigestAndEnvelopeDigestSubstitutions() throws {
    let f = try Fixture()
    let p = f.archive.preparation
    let invalid = try KagemushaNativeSenderPreparationV1(operationID: p.operationID, context: p.context, inputsDigest: digest(9))
    let prep = try f.archive.bytes("preparation")
    let responses = try [
      [digest(9), prep, f.envelopeDigest, f.archive.payment, Data([12])],
      [p.operationID, KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(invalid), f.envelopeDigest, f.archive.payment, Data([12])],
      [p.operationID, prep, digest(9), f.archive.payment, Data([12])],
    ]
    for response in responses {
      let endpoint = Endpoint()
      endpoint.expect(.releaseOutbox, nil, response)
      XCTAssertThrowsError(try adapter(endpoint).outboxRelease(creditID: f.terminalID, inputs: f.archive.inputs,
        canonicalPayment: f.archive.payment, terminalReceipt: .paymentAcknowledgement(canonicalAcknowledgement: f.archive.acknowledgement),
        qualification: f.qualification))
    }
  }

  func testWrongPolicyCreditOrReceiptKindFailsBeforeNativeCall() throws {
    let f = try Fixture()
    let endpoint = Endpoint()
    let core = try adapter(endpoint)
    XCTAssertThrowsError(try core.acceptQualification(f.qualification, hardwarePolicyDigest: digest(9)))
    XCTAssertThrowsError(try core.outboxRelease(creditID: digest(9), inputs: f.archive.inputs, canonicalPayment: f.archive.payment,
      terminalReceipt: .paymentAcknowledgement(canonicalAcknowledgement: f.archive.acknowledgement), qualification: f.qualification))
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(f.archive.paymentRequest)
    XCTAssertThrowsError(try core.outboxRelease(creditID: f.terminalID,
      inputs: .redeemSplit(amount: .init(7), beneficiary: request.recipient), canonicalPayment: f.archive.payment,
      terminalReceipt: .paymentAcknowledgement(canonicalAcknowledgement: f.archive.acknowledgement), qualification: f.qualification))
    XCTAssertEqual(endpoint.calls, 0)
  }

  func testProviderPreservesOriginalAuthenticatorsForQualificationAndNormalAdmission() throws {
    let f = try Fixture()
    let qualification = try f.makeQualification(requestCredential: true)
    let qualificationReply = try qualificationReply(qualification)
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(f.archive.paymentRequest)
    var length = UInt64(f.archive.paymentRequest.count).littleEndian
    let vector = withUnsafeBytes(of: &length) { Data($0) } + f.archive.paymentRequest
    let requestReply = replyArchive("signed-payment-request-reply", [Data([1, 0]), Data([22]), vector])
    var admitted = [(UInt8, Data)]()
    let endpoint = Endpoint()
    endpoint.responseHandler = { method, fields in
      switch method {
      case .reserveOperationID: return [fields[1]]
      case .beginObservation: return [Data((0..<32).map { _ in UInt8.random(in: 1...255) })]
      case .acceptQualification: return []
      case .acceptAuthenticatedReply:
        admitted.append((fields[0][0], fields[4]))
        return []
      case .exportOutgoingStateProof: return [fields[0], Data([0x81]), Data([0x82])]
      default: throw TestError.unexpectedCall
      }
    }
    let transport = Transport(qualification: qualification) { operation, requestID, _, acceptedKey in
      if operation == 1 { XCTAssertNil(acceptedKey) }
      else { XCTAssertEqual(acceptedKey, qualification.credential.devicePublicKey.sec1Bytes) }
      return try testAuthenticatedDeviceResponse(operation: operation, status: .success,
        canonicalReply: operation == 1 ? qualificationReply : requestReply, authenticator: responseSignature(operation), requestID: requestID)
    }
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint), intentOwner: testOperationIntentOwner(), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    XCTAssertEqual(try provider.createPaymentRequest(operationID: request.requestID, recipient: request.recipient,
      amount: request.amount, validityWindowMS: request.expiresAtMS - request.issuedAtMS), f.archive.paymentRequest)
    XCTAssertEqual(admitted.map { $0.0 }, [1, 22])
    for (operation, bytes) in admitted { XCTAssertEqual(bytes, responseSignature(operation)) }
    let exported = try provider.exportOutgoingStateProof(operationID: request.requestID)
    XCTAssertEqual(exported.operationID, request.requestID)
    XCTAssertEqual(exported.publicInputsArchive, Data([0x81]))
    XCTAssertEqual(exported.pairedProofArchive, Data([0x82]))
    let callCount = endpoint.calls
    XCTAssertThrowsError(try provider.exportOutgoingStateProof(operationID: Data(repeating: 0, count: 32)))
    XCTAssertEqual(endpoint.calls, callCount)
  }

  func testAcknowledgementAfterRotationReachesHardwareWithOriginalCreationContext() throws {
    let f = try Fixture()
    let qualification = try f.makeQualification(generation: 2, policy: digest(98), coreKey: digest(99))
    let qualificationReply = try qualificationReply(qualification)
    let endpoint = Endpoint()
    endpoint.responseHandler = { method, fields in
      switch method {
      case .reserveOperationID: return [fields[1]]
      case .beginObservation: return [Data((0..<32).map { _ in UInt8.random(in: 1...255) })]
      case .acceptQualification, .acceptAuthenticatedReply: return []
      case .releaseOutbox:
        return try [f.archive.preparation.operationID, f.archive.bytes("preparation"), f.envelopeDigest, f.archive.payment, Data([12])]
      default: throw TestError.unexpectedCall
      }
    }
    var sawHistoricalRelease = false
    let transport = Transport(qualification: qualification) { operation, requestID, command, acceptedKey in
      if operation == 1 {
        return try testAuthenticatedDeviceResponse(operation: operation, status: .success,
          canonicalReply: qualificationReply, authenticator: responseSignature(operation), requestID: requestID)
      }
      XCTAssertEqual(operation, 12)
      XCTAssertEqual(acceptedKey, qualification.credential.devicePublicKey.sec1Bytes)
      let decoded = try KagemushaDeviceOperationCodecV1.decodeSenderCommand(operation: operation, requestID: requestID, canonicalBytes: command)
      XCTAssertEqual(decoded.context, f.archive.preparation.context)
      XCTAssertEqual(decoded.operationID, f.archive.preparation.operationID)
      sawHistoricalRelease = true
      // Stop at the actual hardware boundary: this test does not fabricate a monetary success.
      return try testAuthenticatedDeviceResponse(operation: operation, status: .unavailable,
        canonicalReply: Data(), authenticator: Data(), requestID: requestID)
    }
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint), intentOwner: testOperationIntentOwner(), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    XCTAssertThrowsError(try provider.recordAcknowledgement(creditID: f.terminalID,
      canonicalRequest: f.archive.paymentRequest, canonicalPayment: f.archive.payment,
      canonicalAcknowledgement: f.archive.acknowledgement)) { error in
        XCTAssertEqual(error as? KagemushaAuthenticatedHardwareProviderErrorV1, .operationFailed(operation: 12, status: .unavailable))
      }
    XCTAssertTrue(sawHistoricalRelease)
  }

  func testLostRotationReplyReopensWithOriginalIntentKeyAndFreshCurrentSnapshot() throws {
    let f = try Fixture()
    let old = try f.qualification
    let newKey = try KagemushaDevicePublicKeyV1(sec1Bytes:
      P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 5, count: 32)).publicKey.x963Representation)
    let current = try f.makeQualification(generation: 2, devicePublicKey: newKey)
    let oldReply = try qualificationReply(old), currentReply = try qualificationReply(current)
    let previous = try KagemushaNoritoV1.decodeAggregateStateShapeExact(f.aggregate)
    let installed = try KagemushaNoritoV1.encodeAggregateStateShape(
      KagemushaAggregateStateCommitmentV1(releaseID: previous.releaseID, networkID: previous.networkID,
        asset: previous.asset, assetIncarnation: previous.assetIncarnation, scale: previous.scale,
        liabilityPoolID: previous.liabilityPoolID, laneID: previous.laneID,
        hardwareEpochID: current.credential.hardwareEpochID, keyReference: previous.keyReference,
        hardwarePolicyID: previous.hardwarePolicyID, sequence: .init(0), stateCommitment: digest(95)))
    var length = UInt64(installed.count).littleEndian
    let vector = withUnsafeBytes(of: &length) { Data($0) } + installed
    let rotationReply = replyArchive("rotate-hardware-epoch-reply", [Data([1, 0]), Data([19]), vector])
    let snapshotReply = replyArchive("wallet-recovery-snapshot-reply",
      [Data([1, 0]), Data([21]), Data([1]) + compactField(vector), Data(repeating: 0, count: 16),
       Data(repeating: 0, count: 16), Data(repeating: 0, count: 16)], alignment: 16)
    let store = TestOperationIntentStore()
    var rotated = false
    var rejectRetainedMutation = false
    var rotationRequests: [Data] = [], snapshots: [Data] = []
    let endpoint = Endpoint()
    endpoint.responseHandler = { method, fields in
      switch method {
      case .reserveOperationID: return [fields[1]]
      case .beginObservation: return [Data((0..<32).map { _ in UInt8.random(in: 1...255) })]
      case .acceptQualification: return []
      case .acceptAuthenticatedReply:
        if fields[0] == u32(19) {
          XCTAssertEqual(fields[8], try KagemushaNoritoV1.encodeHardwareCredentialShape(old.credential))
          let intent = try XCTUnwrap(store.records.values.first { $0.operation == 19 })
          XCTAssertEqual(fields[1], intent.operationID)
          XCTAssertEqual(fields[2], intent.canonicalCommand)
          XCTAssertEqual(fields[3], rotationReply)
          XCTAssertEqual(fields[4], responseSignature(19))
          if rejectRetainedMutation { throw TestError.unexpectedCall }
        }
        return []
      default: throw TestError.unexpectedCall
      }
    }
    let transport = Transport(qualification: current) { operation, id, _, acceptedKey in
      switch operation {
      case 1:
        XCTAssertNil(acceptedKey)
        return try testAuthenticatedDeviceResponse(operation: 1, status: .success,
          canonicalReply: rotated ? currentReply : oldReply, authenticator: responseSignature(1), requestID: id)
      case 19:
        rotationRequests.append(id)
        XCTAssertEqual(acceptedKey, old.credential.devicePublicKey.sec1Bytes)
        if !rotated { rotated = true; throw TestError.unexpectedCall }
        return try testAuthenticatedDeviceResponse(operation: 19, status: .success,
          canonicalReply: rotationReply, authenticator: responseSignature(19), requestID: id)
      case 21:
        snapshots.append(id)
        XCTAssertEqual(acceptedKey, current.credential.devicePublicKey.sec1Bytes)
        return try testAuthenticatedDeviceResponse(operation: 21, status: .success,
          canonicalReply: snapshotReply, authenticator: responseSignature(21), requestID: id)
      default: throw TestError.unexpectedCall
      }
    }
    let original = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
      intentOwner: KagemushaOperationIntentOwnerV1(store: store), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    XCTAssertThrowsError(try original.rotateHardwareEpoch())
    let restarted = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
      intentOwner: KagemushaOperationIntentOwnerV1(store: store), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    XCTAssertEqual(try restarted.rotateHardwareEpoch(), installed)
    XCTAssertEqual(rotationRequests.count, 2)
    XCTAssertEqual(rotationRequests[0], rotationRequests[1])
    rejectRetainedMutation = true
    XCTAssertThrowsError(try restarted.recover(), "Saved mutation evidence must be re-admitted by native Core")
    XCTAssertFalse(try XCTUnwrap(store.records.values.first { $0.operation == 19 }).acknowledged)
    XCTAssertNil(store.records.values.first { $0.operation == 19 }?.authenticatedSnapshotEvidence)
    rejectRetainedMutation = false
    let callsBeforeReopen = endpoint.calls
    let snapshotsBeforeReopen = snapshots.count
    XCTAssertThrowsError(try restarted.recover()) { error in
      XCTAssertEqual(error as? KagemushaCoreCoordinatorErrorV1, .unavailable)
    }
    XCTAssertEqual(endpoint.calls, callsBeforeReopen, "Revoked owner cannot redispatch")
    XCTAssertEqual(snapshots.count, snapshotsBeforeReopen)
    XCTAssertEqual(endpoint.closeCalls, 1)
    // Reopen a fresh scripted native process over the original durable intents.
    let reopenedEndpoint = Endpoint()
    reopenedEndpoint.responseHandler = endpoint.responseHandler
    let reopened = KagemushaAuthenticatedHardwareProviderV1(transport: transport,
      core: try adapter(reopenedEndpoint), intentOwner: KagemushaOperationIntentOwnerV1(store: store), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    store.failAfterSave = true
    XCTAssertThrowsError(try reopened.recover())
    let interrupted = try XCTUnwrap(store.records.values.first { $0.operation == 19 })
    XCTAssertNotNil(interrupted.authenticatedSnapshotEvidence)
    XCTAssertFalse(interrupted.acknowledged, "Snapshot evidence must be durable before acknowledgement")
    store.failAfterSave = false
    XCTAssertEqual(try reopened.recover().aggregateState, installed)
    XCTAssertEqual(try reopened.recover().aggregateState, installed)
    XCTAssertEqual(Set(snapshots).count, 4)
    XCTAssertEqual(store.records.values.first { $0.operation == 19 }?.authenticatedSnapshotEvidence,
      interrupted.authenticatedSnapshotEvidence, "Retain original accepted historical evidence")
    XCTAssertTrue(store.records.values.allSatisfy { ![UInt8(1), 13, 18, 21].contains($0.operation) })
    XCTAssertTrue(try store.pending(operation: 19, purpose: "internal-19", qualificationScope: nil).isEmpty)
    XCTAssertEqual(store.records.values.filter { $0.operation == 19 }.count, 1)
  }

  func testRecoverAutomaticallyReplaysLostRotationUnderOriginalQualification() throws {
    for operation: UInt8 in [19] {
      let f = try Fixture(), originalQualification = try f.qualification
      let currentQualification = try f.makeQualification(generation: operation == 19 ? 2 : 1,
        devicePublicKey: operation == 19 ? KagemushaDevicePublicKeyV1(sec1Bytes:
          P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 5, count: 32)).publicKey.x963Representation) : nil)
      let oldReply = try qualificationReply(originalQualification), currentReply = try qualificationReply(currentQualification)
      let previous = try KagemushaNoritoV1.decodeAggregateStateShapeExact(f.aggregate)
      let installed = try KagemushaNoritoV1.encodeAggregateStateShape(KagemushaAggregateStateCommitmentV1(
        releaseID: previous.releaseID, networkID: previous.networkID, asset: previous.asset,
        assetIncarnation: previous.assetIncarnation, scale: previous.scale, liabilityPoolID: previous.liabilityPoolID,
        laneID: previous.laneID, hardwareEpochID: currentQualification.credential.hardwareEpochID,
        keyReference: previous.keyReference, hardwarePolicyID: previous.hardwarePolicyID,
        sequence: .init(operation == 19 ? 0 : 2), stateCommitment: digest(96)))
      let selector = try KagemushaPendingCreditSelectorV1(kind: .receive, creditID: f.terminalID)
      var length = UInt64(installed.count).littleEndian
      let vector = withUnsafeBytes(of: &length) { Data($0) } + installed
      let mutationReply = replyArchive(operation == 19 ? "rotate-hardware-epoch-reply" : "fold-receive-credit-reply",
        [Data([1, 0]), Data([operation])] + (operation == 17 ? [u32(selector.kind.rawValue), selector.creditID] : []) + [vector], alignment: operation == 17 ? 16 : 8)
      let snapshotReply = replyArchive("wallet-recovery-snapshot-reply", [Data([1, 0]), Data([21]),
        Data([1]) + compactField(vector), Data(repeating: 0, count: 16), Data(repeating: 0, count: 16),
        Data(repeating: 0, count: 16)], alignment: 16)
      let store = TestOperationIntentStore()
      var mutated = false
      var commands: [Data] = [], ids: [Data] = [], events: [String] = []
      let endpoint = Endpoint()
      endpoint.responseHandler = { method, fields in
        switch method {
        case .reserveOperationID: return [fields[1]]
        case .beginObservation: return [Data((0..<32).map { _ in UInt8.random(in: 1...255) })]
        case .acceptQualification: return []
        case .acceptAuthenticatedReply:
          if fields[0] == u32(UInt32(operation)) {
            XCTAssertEqual(fields[8], try KagemushaNoritoV1.encodeHardwareCredentialShape(originalQualification.credential))
            XCTAssertEqual(fields[3], mutationReply)
            events.append("accept-original")
          }
          return []
        default: throw TestError.unexpectedCall
        }
      }
      let transport = Transport(qualification: currentQualification) { observed, id, command, key in
        let bytes: Data
        switch observed {
        case 1: bytes = mutated ? currentReply : oldReply
        case operation:
          XCTAssertEqual(key, originalQualification.credential.devicePublicKey.sec1Bytes)
          commands.append(command); ids.append(id)
          if !mutated { mutated = true; throw TestError.unexpectedCall }
          bytes = mutationReply
        case 21:
          XCTAssertEqual(key, currentQualification.credential.devicePublicKey.sec1Bytes)
          events.append("snapshot"); bytes = snapshotReply
        default: throw TestError.unexpectedCall
        }
        return try testAuthenticatedDeviceResponse(operation: observed, status: .success,
          canonicalReply: bytes, authenticator: responseSignature(observed), requestID: id)
      }
      let original = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
        intentOwner: KagemushaOperationIntentOwnerV1(store: store), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
      if operation == 19 { XCTAssertThrowsError(try original.rotateHardwareEpoch()) }
      else { XCTAssertThrowsError(try original.foldPendingCredit(selector: selector)) }
      XCTAssertNil(store.records.values.first?.canonicalReply)
      let restarted = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
        intentOwner: KagemushaOperationIntentOwnerV1(store: store), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
      XCTAssertEqual(try restarted.recover().aggregateState, installed)
      XCTAssertEqual(commands.count, 2)
      XCTAssertEqual(commands[0], commands[1])
      XCTAssertEqual(ids[0], ids[1])
      let firstAcceptance = try XCTUnwrap(events.firstIndex(of: "accept-original"))
      XCTAssertTrue(events.dropFirst(firstAcceptance + 1).contains("snapshot"))
      let record = try XCTUnwrap(store.records.values.first)
      XCTAssertTrue(record.acknowledged)
      XCTAssertNotNil(record.authenticatedSnapshotEvidence)
      XCTAssertEqual(record.canonicalReply, mutationReply)
    }
  }

  func testLostRequestReplyAfterRotationReusesOriginalCreationCredentialAndResultAck() throws {
    let f = try Fixture(), old = try f.makeQualification(requestCredential: true)
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(f.archive.paymentRequest)
    let current = try f.makeQualification(generation: 2,
      devicePublicKey: KagemushaDevicePublicKeyV1(sec1Bytes:
        P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 6, count: 32)).publicKey.x963Representation), requestCredential: true)
    let oldReply = try qualificationReply(old), currentReply = try qualificationReply(current)
    var length = UInt64(f.archive.paymentRequest.count).littleEndian
    let vector = withUnsafeBytes(of: &length) { Data($0) } + f.archive.paymentRequest
    let requestReply = replyArchive("signed-payment-request-reply", [Data([1, 0]), Data([22]), vector])
    var rotated = false, requests = 0
    let store = TestOperationIntentStore(), endpoint = Endpoint()
    endpoint.responseHandler = { method, fields in
      switch method {
      case .reserveOperationID: return [fields[1]]
      case .beginObservation: return [Data((0..<32).map { _ in UInt8.random(in: 1...255) })]
      case .acceptQualification: return []
      case .acceptAuthenticatedReply:
        if fields[0] == u32(22) {
          XCTAssertEqual(fields[8], try KagemushaNoritoV1.encodeHardwareCredentialShape(old.credential))
        }
        return []
      default: throw TestError.unexpectedCall
      }
    }
    let transport = Transport(qualification: old) { operation, id, _, acceptedKey in
      if operation == 1 {
        return try testAuthenticatedDeviceResponse(operation: 1, status: .success,
          canonicalReply: rotated ? currentReply : oldReply, authenticator: responseSignature(1), requestID: id)
      }
      XCTAssertEqual(operation, 22); XCTAssertEqual(id, request.requestID)
      XCTAssertEqual(acceptedKey, old.credential.devicePublicKey.sec1Bytes)
      requests += 1
      if !rotated { rotated = true; throw TestError.unexpectedCall }
      return try testAuthenticatedDeviceResponse(operation: 22, status: .success,
        canonicalReply: requestReply, authenticator: responseSignature(22), requestID: id)
    }
    func provider() throws -> KagemushaAuthenticatedHardwareProviderV1 {
      KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
        intentOwner: KagemushaOperationIntentOwnerV1(store: store), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    }
    XCTAssertThrowsError(try provider().createPaymentRequest(operationID: request.requestID,
      recipient: request.recipient, amount: request.amount,
      validityWindowMS: request.expiresAtMS - request.issuedAtMS))
    let restarted = try provider()
    let result = try restarted.createPaymentRequest(operationID: request.requestID,
      recipient: request.recipient, amount: request.amount,
      validityWindowMS: request.expiresAtMS - request.issuedAtMS)
    XCTAssertEqual(result, f.archive.paymentRequest); XCTAssertEqual(requests, 2)
    let saved = try XCTUnwrap(store.load(operation: 22, operationID: request.requestID))
    XCTAssertFalse(saved.acknowledged)
    XCTAssertThrowsError(try restarted.acknowledgeDurableResult(operationID: request.requestID, canonicalResult: Data([1])))
    try restarted.acknowledgeDurableResult(operationID: request.requestID, canonicalResult: result)
    XCTAssertTrue(try XCTUnwrap(store.load(operation: 22, operationID: request.requestID)).acknowledged)
  }

  func testLostReadReplyAndRecreatedNativeOwnerRequireFreshNonceAndRejectOldResponse() throws {
    let f = try Fixture(), qualification = try f.qualification
    let credentialReply = try qualificationReply(qualification)
    let snapshotReply = replyArchive("wallet-recovery-snapshot-reply", [Data([1, 0]), Data([21]),
      Data([0]), Data(repeating: 0, count: 16), Data(repeating: 0, count: 16),
      Data(repeating: 0, count: 16)], alignment: 16)
    let store = TestOperationIntentStore()
    var requests: [Data] = [], loseFirst = true, replayOld = false
    var oldAuthenticator: Data?
    let transport = Transport(qualification: qualification) { operation, nonce, _, _ in
      let signature = responseSignature(nonce[0])
      if operation == 21 {
        requests.append(nonce)
        if loseFirst { loseFirst = false; throw TestError.unexpectedCall }
        if replayOld {
          replayOld = false
          return try testAuthenticatedDeviceResponse(operation: operation, status: .success,
            canonicalReply: snapshotReply, authenticator: oldAuthenticator!, requestID: nonce)
        }
        oldAuthenticator = signature
      }
      return try testAuthenticatedDeviceResponse(operation: operation, status: .success,
        canonicalReply: operation == 1 ? credentialReply : snapshotReply, authenticator: signature, requestID: nonce)
    }
    func endpoint(seed: UInt8) -> Endpoint {
      let endpoint = Endpoint()
      var next = seed
      var pending: [UInt8: (Data, Data)] = [:]
      endpoint.responseHandler = { method, fields in
        switch method {
        case .beginObservation:
          next += 1
          let nonce = digest(next)
          pending[fields[0][0]] = (nonce, fields[1])
          return [nonce]
        case .acceptQualification: return []
        case .acceptAuthenticatedReply:
          let op = fields[0][0]
          guard let challenge = pending[op], challenge.0 == fields[1], challenge.1 == fields[2],
            fields[4] == responseSignature(challenge.0[0]) else { throw TestError.unexpectedCall }
          pending[op] = nil
          return []
        default: throw TestError.unexpectedCall
        }
      }
      return endpoint
    }
    let original = KagemushaAuthenticatedHardwareProviderV1(transport: transport,
      core: try adapter(endpoint(seed: 40)), intentOwner: KagemushaOperationIntentOwnerV1(store: store), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    XCTAssertThrowsError(try original.recover())
    XCTAssertNil(try original.recover().aggregateState)
    XCTAssertTrue(store.records.isEmpty, "Reads never allocate durable host intents")
    replayOld = true
    let staleResponseEndpoint = endpoint(seed: 80)
    let recreated = KagemushaAuthenticatedHardwareProviderV1(transport: transport,
      core: try adapter(staleResponseEndpoint), intentOwner: KagemushaOperationIntentOwnerV1(store: store), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    XCTAssertThrowsError(try recreated.recover(), "Prior-owner signature cannot satisfy the new nonce")
    let callsBeforeReopen = staleResponseEndpoint.calls
    let requestsBeforeReopen = requests.count
    XCTAssertThrowsError(try recreated.recover()) { error in
      XCTAssertEqual(error as? KagemushaCoreCoordinatorErrorV1, .unavailable)
    }
    XCTAssertEqual(staleResponseEndpoint.calls, callsBeforeReopen)
    XCTAssertEqual(staleResponseEndpoint.closeCalls, 1)
    XCTAssertEqual(requests.count, requestsBeforeReopen, "Revoked owner cannot issue another device read")
    let reopened = KagemushaAuthenticatedHardwareProviderV1(transport: transport,
      core: try adapter(endpoint(seed: 120)), intentOwner: KagemushaOperationIntentOwnerV1(store: store), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    XCTAssertNil(try reopened.recover().aggregateState)
    XCTAssertEqual(Set(requests).count, 4)
    XCTAssertTrue(store.records.isEmpty)
  }

  func testUncertainPrepareRetriesExactOp5AndPassesOriginalSuccessToCore() throws {
    try exerciseExactOp5Retry(firstStatus: .recoveryRequired, secondStatus: .success)
  }

  func testConcurrentPrepareRetriesExactOp5WithoutChangingItsReservation() throws {
    try exerciseExactOp5Retry(firstStatus: .staleOrConcurrent, secondStatus: .success)
  }

  func testUnavailableExactPrepareRetryKeepsOriginalIntentWithoutProofAdmission() throws {
    try exerciseExactOp5Retry(firstStatus: .recoveryRequired, secondStatus: .unavailable)
  }

  private func exerciseExactOp5Retry(firstStatus: KagemushaDeviceLifecycleStatusV1,
    secondStatus: KagemushaDeviceLifecycleStatusV1) throws {
    // This scripted transport/Core route stops at method5. Its public response specimens
    // provide no authenticated hardware, recursive proof, fund mutation or installed terminal.
    let fixture = try Fixture(), q = try fixture.qualification
    let id = fixture.archive.preparation.operationID
    let store = TestOperationIntentStore(), endpoint = Endpoint()
    let qualification = try qualificationReply(q)
    let preparation = try fixture.archive.bytes("preparation")
    let context = try thirdCompactField(try XCTUnwrap(noritoDecodeFrame(preparation)).payload)
    var operations: [UInt8] = [], prepareCommands: [Data] = [], preparedReplies: [Data] = []
    var fullSuccess: Data?, successfulReply: Data?
    endpoint.responseHandler = { method, fields in
      switch method {
      case .reserveOperationID:
        XCTAssertEqual(fields[1], id); return [fields[1]]
      case .beginObservation: return [self.digestForRetry(0x46)]
      case .acceptQualification, .acceptAuthenticatedReply: return []
      case .beginSenderTransition: return [id, preparation]
      case .provePreparedSenderTransition:
        preparedReplies.append(fields[1]); throw TestError.unexpectedCall
      default: throw TestError.unexpectedCall
      }
    }
    let transport = Transport(qualification: q) { operation, requestID, command, acceptedKey in
      operations.append(operation)
      if operation == 1 {
        return try testAuthenticatedDeviceResponse(operation: operation, status: .success,
          canonicalReply: qualification, authenticator: responseSignature(operation), requestID: requestID)
      }
      XCTAssertEqual(operation, 5)
      XCTAssertEqual(requestID, id)
      XCTAssertEqual(acceptedKey, q.credential.devicePublicKey.sec1Bytes)
      prepareCommands.append(command)
      let status = prepareCommands.count == 1 ? firstStatus : secondStatus
      let payload = self.replyArchive("sender-reply", [Data([1, 0]), Data([operation]), requestID,
        context, Data(repeating: 0, count: 16), u32(0) + self.compactField(Data([0]))], alignment: 16)
      let response = try testAuthenticatedDeviceResponse(operation: operation, status: status,
        canonicalReply: status == .success ? payload : Data(),
        authenticator: status == .success ? responseSignature(operation) : Data(), requestID: requestID)
      if status == .success { fullSuccess = response.canonicalResponseFrame; successfulReply = payload }
      return response
    }
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
      intentOwner: KagemushaOperationIntentOwnerV1(store: store), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    XCTAssertThrowsError(try provider.prepareProveCommitPayment(operationID: id,
      canonicalRequest: fixture.archive.paymentRequest))
    XCTAssertEqual(operations, [1, 5, 5])
    XCTAssertEqual(prepareCommands.count, 2)
    XCTAssertEqual(prepareCommands[0], prepareCommands[1])
    XCTAssertNil(try store.load(operation: 6, operationID: id))
    let persisted = try XCTUnwrap(store.load(operation: 5, operationID: id))
    XCTAssertEqual(persisted.canonicalCommand, prepareCommands[0])
    XCTAssertFalse(persisted.acknowledged)
    if secondStatus == .success {
      XCTAssertEqual(preparedReplies, [try XCTUnwrap(successfulReply)])
      XCTAssertEqual(persisted.originalResponse, try XCTUnwrap(fullSuccess))
    } else {
      XCTAssertTrue(preparedReplies.isEmpty)
      XCTAssertNil(persisted.originalResponse)
    }
  }

  func testUncertainCommitRetriesExactOp7AndPassesOriginalSuccessToCore() throws {
    try exerciseExactOp7Retry(secondStatus: .success)
  }

  func testUnavailableExactCommitRetryKeepsOriginalIntentWithoutTerminalAdmission() throws {
    try exerciseExactOp7Retry(secondStatus: .unavailable)
  }

  private func exerciseExactOp7Retry(secondStatus: KagemushaDeviceLifecycleStatusV1) throws {
    // This scripted route stops at method6. It provides no authenticated hardware,
    // proof, fund mutation or installed terminal; it tests exact transport ownership.
    let fixture = try Fixture(), q = try fixture.qualification
    let id = fixture.archive.preparation.operationID
    let store = TestOperationIntentStore(), endpoint = Endpoint()
    let qualification = try qualificationReply(q)
    let preparation = try fixture.archive.bytes("preparation")
    let candidate = try fixture.archive.bytes("candidate")
    var operations: [UInt8] = [], commitCommands: [Data] = [], terminalOriginals: [Data] = []
    var fullSuccess: Data?
    endpoint.responseHandler = { method, fields in
      switch method {
      case .reserveOperationID: return [fields[1]]
      case .beginObservation: return [self.digestForRetry(0x45)]
      case .acceptQualification, .acceptAuthenticatedReply: return []
      case .beginSenderTransition: return [id, preparation]
      case .provePreparedSenderTransition: return [candidate]
      case .buildTerminalEnvelope:
        terminalOriginals.append(fields[1]); throw TestError.unexpectedCall
      default: throw TestError.unexpectedCall
      }
    }
    let context = try thirdCompactField(try XCTUnwrap(noritoDecodeFrame(preparation)).payload)
    let transport = Transport(qualification: q) { operation, requestID, command, _ in
      operations.append(operation)
      if operation == 1 {
        return try testAuthenticatedDeviceResponse(operation: operation, status: .success,
          canonicalReply: qualification, authenticator: responseSignature(operation), requestID: requestID)
      }
      XCTAssertTrue(operation == 5 || operation == 7)
      let payload = self.replyArchive("sender-reply", [Data([1, 0]), Data([operation]), requestID,
        context, Data(repeating: 0, count: 16), u32(0) + self.compactField(Data([0]))], alignment: 16)
      var status = KagemushaDeviceLifecycleStatusV1.success
      if operation == 7 {
        XCTAssertEqual(requestID, id); commitCommands.append(command)
        status = commitCommands.count == 1 ? .recoveryRequired : secondStatus
      }
      let response = try testAuthenticatedDeviceResponse(operation: operation, status: status,
        canonicalReply: status == .success ? payload : Data(),
        authenticator: status == .success ? responseSignature(operation) : Data(), requestID: requestID)
      if operation == 7 && status == .success { fullSuccess = response.canonicalResponseFrame }
      return response
    }
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
      intentOwner: KagemushaOperationIntentOwnerV1(store: store), incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    XCTAssertThrowsError(try provider.prepareProveCommitPayment(operationID: id,
      canonicalRequest: fixture.archive.paymentRequest))
    XCTAssertEqual(operations, [1, 5, 7, 7])
    XCTAssertEqual(commitCommands.count, 2)
    XCTAssertEqual(commitCommands[0], commitCommands[1])
    let persisted = try XCTUnwrap(store.load(operation: 7, operationID: id))
    XCTAssertEqual(persisted.canonicalCommand, commitCommands[0])
    XCTAssertFalse(persisted.acknowledged)
    if secondStatus == .success {
      XCTAssertEqual(terminalOriginals, [try XCTUnwrap(fullSuccess)])
      XCTAssertEqual(persisted.originalResponse, fullSuccess)
    } else {
      XCTAssertTrue(terminalOriginals.isEmpty)
      XCTAssertNil(persisted.originalResponse)
    }
  }

  private func digestForRetry(_ byte: UInt8) -> Data { Data(repeating: byte, count: 32) }

  private func thirdCompactField(_ payload: Data) throws -> Data {
    let bytes = [UInt8](payload); var cursor = 0
    for index in 0..<3 {
      var length = 0, shift = 0
      while true {
        guard cursor < bytes.count && shift < 64 else { throw TestError.unexpectedCall }
        let byte = bytes[cursor]; cursor += 1; length |= Int(byte & 0x7f) << shift
        if byte < 128 { break }; shift += 7
      }
      guard length <= bytes.count - cursor else { throw TestError.unexpectedCall }
      let field = Data(bytes[cursor..<(cursor + length)]); cursor += length
      if index == 2 { return field }
    }
    throw TestError.unexpectedCall
  }

  private func compactField(_ value: Data) -> Data {
    var size = value.count, bytes = Data()
    repeat { let byte = UInt8(size & 0x7f); size >>= 7; bytes.append(size == 0 ? byte : byte | 0x80) } while size != 0
    return bytes + value
  }

  private func qualificationReply(_ q: KagemushaHardwareQualificationV1) throws -> Data {
    let profile = try XCTUnwrap(noritoDecodeFrame(KagemushaNoritoV1.encodeHardwareProfileShape(q.profile)))
    let credential = try XCTUnwrap(noritoDecodeFrame(KagemushaNoritoV1.encodeHardwareCredentialShape(q.credential)))
    return replyArchive("active-hardware-credential-reply",
      [Data([1, 0]), Data([1]), q.releaseID, q.hardwarePolicyDigest, q.coreAuthorizationKeyReference, profile.payload, credential.payload])
  }

  private func replyArchive(_ schema: String, _ fields: [Data], alignment: Int = 8) -> Data {
    var payload = Data()
    for field in fields {
      var size = field.count
      repeat {
        let byte = UInt8(size & 0x7f)
        size >>= 7
        payload.append(size == 0 ? byte : byte | 0x80)
      } while size != 0
      payload.append(field)
    }
    return noritoEncode(typeName: "iroha.kagemusha.device.v1." + schema, payload: payload,
      flags: NoritoHeader.compactLen, payloadAlignment: alignment)
  }

  private func adapter(_ endpoint: Endpoint) throws -> KagemushaNativeCoreCoordinatorAdapterV1 {
    try KagemushaNativeCoreCoordinatorAdapterV1(bridge: .openEndpoint(storagePath: "/test/store", endpoint: endpoint))
  }

  func testIncomingAdapterPreservesExactNativeProofAndPhysicalCompletionOriginals() throws {
    let endpoint = Endpoint(), core = try adapter(endpoint)
    let selector = try KagemushaPendingCreditSelectorV1(kind: .receive, creditID: digest(41))
    let fields = try incomingFields(selector.creditID)
    endpoint.expect(.prepareIncomingFold, [u32(1), selector.creditID], fields)
    let work = try core.prepareIncomingFold(selector: selector)
    let evidence = try KagemushaOriginalIncomingFoldEvidenceV1(canonicalHardwareCertificate: Data([11]),
      deviceRootSelectionSignature: responseSignature(1))
    endpoint.expect(.completeIncomingFold, [work.historyID, fields[9], Data([11]), responseSignature(1)], [work.historyID])
    try core.completeIncomingFold(work: work, evidence: evidence)
    endpoint.expect(.stageIncomingOriginal, [u32(2), selector.creditID], [selector.creditID])
    try core.stageIncomingOriginal(kind: .stagePeer, creditID: selector.creditID)
    XCTAssertEqual(endpoint.calls, 3)
  }

  func testRefusingIncomingOriginalOwnerCannotCompletePreparedWorkOrDispatchDevice() throws {
    final class RefusingOriginals: KagemushaIncomingFoldEvidenceProviderV1 {
      var checks = 0, acquisitions = 0
      func recheckOriginals(for work: KagemushaNativeIncomingFoldWorkV1) throws {
        checks += 1
        throw KagemushaCoreCoordinatorErrorV1.unavailable
      }
      func originalEvidence(for work: KagemushaNativeIncomingFoldWorkV1) throws -> KagemushaOriginalIncomingFoldEvidenceV1 {
        acquisitions += 1
        throw KagemushaCoreCoordinatorErrorV1.unavailable
      }
    }
    let endpoint = Endpoint(), f = try Fixture(), evidence = RefusingOriginals()
    let selector = try KagemushaPendingCreditSelectorV1(kind: .receive, creditID: digest(41))
    let fields = try incomingFields(selector.creditID)
    var preparations = 0, completions = 0, dispatches = 0
    endpoint.responseHandler = { method, _ in
      if method == .prepareIncomingFold { preparations += 1; return fields }
      if method == .completeIncomingFold { completions += 1 }
      throw TestError.unexpectedCall
    }
    let transport = Transport(qualification: try f.qualification) { _, _, _, _ in
      dispatches += 1; throw TestError.unexpectedCall
    }
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport,
      core: try adapter(endpoint), intentOwner: testOperationIntentOwner(), incomingFoldEvidenceProvider: evidence)
    for _ in 0..<2 {
      XCTAssertThrowsError(try provider.foldPendingCredit(selector: selector)) {
        XCTAssertEqual($0 as? KagemushaCoreCoordinatorErrorV1, .unavailable)
      }
    }
    XCTAssertEqual(preparations, 1); XCTAssertEqual(endpoint.calls, 1)
    XCTAssertEqual(evidence.checks, 2); XCTAssertEqual(evidence.acquisitions, 0)
    XCTAssertEqual(completions, 0); XCTAssertEqual(dispatches, 0)
  }

  func testIncomingFoldRetainsNativeProofAndOriginalEvidenceThroughUncertainCompletionAndSnapshot() throws {
    let f = try Fixture(), endpoint = Endpoint()
    let selector = try KagemushaPendingCreditSelectorV1(kind: .receive, creditID: digest(41))
    let fields = try incomingFields(selector.creditID), q = try f.qualification
    let qualification = try qualificationReply(q), aggregate = try f.aggregate
    var length = UInt64(aggregate.count).littleEndian
    let vector = withUnsafeBytes(of: &length) { Data($0) } + aggregate
    let snapshot = replyArchive("wallet-recovery-snapshot-reply", [Data([1, 0]), Data([21]),
      Data([1]) + compactField(vector), Data(repeating: 0, count: 16), Data(repeating: 0, count: 16),
      Data(repeating: 0, count: 16)], alignment: 16)
    let evidenceProvider = IncomingEvidence()
    var preparations = 0, completions: [[Data]] = [], snapshots = 0, operations: [UInt8] = []
    endpoint.responseHandler = { method, request in
      switch method {
      case .prepareIncomingFold: preparations += 1; return fields
      case .completeIncomingFold:
        completions.append(request)
        if completions.count == 1 { throw TestError.unexpectedCall }
        return [fields[0]]
      case .beginObservation: return [digest(UInt8(endpoint.calls + 20))]
      case .acceptQualification, .acceptAuthenticatedReply: return []
      default: throw TestError.unexpectedCall
      }
    }
    let transport = Transport(qualification: q) { operation, requestID, _, _ in
      operations.append(operation)
      if operation == 21 { snapshots += 1; if snapshots == 1 { throw TestError.unexpectedCall } }
      guard operation == 1 || operation == 21 else { throw TestError.unexpectedCall }
      return try testAuthenticatedDeviceResponse(operation: operation, status: .success,
        canonicalReply: operation == 1 ? qualification : snapshot, authenticator: responseSignature(operation),
        requestID: requestID)
    }
    // The SDK revokes a native handle after an uncertain endpoint return. The test
    // recovery owner explicitly supplies a fresh handle to the same scripted durable
    // store; it never reopens the closed adapter or grants native proof authority.
    let core = try IncomingRecoveryCore(endpoint: endpoint)
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport,
      core: core, intentOwner: testOperationIntentOwner(), incomingFoldEvidenceProvider: evidenceProvider)
    XCTAssertThrowsError(try provider.foldPendingCredit(selector: selector))
    let other = try KagemushaPendingCreditSelectorV1(kind: .mint, creditID: selector.creditID)
    XCTAssertThrowsError(try provider.foldPendingCredit(selector: other))
    XCTAssertEqual(preparations, 1); XCTAssertEqual(completions.count, 1)
    XCTAssertThrowsError(try provider.foldPendingCredit(selector: selector))
    XCTAssertEqual(completions.count, 2); XCTAssertEqual(completions[0], completions[1])
    XCTAssertEqual(try provider.foldPendingCredit(selector: selector).aggregateState, aggregate)
    XCTAssertEqual(preparations, 1); XCTAssertEqual(completions.count, 2)
    XCTAssertEqual(evidenceProvider.acquisitions, 1)
    XCTAssertEqual(core.reopenedHandles, 1); XCTAssertEqual(endpoint.closeCalls, 1)
    XCTAssertTrue(core.closedHandleRefusedWithoutRedispatch)
    XCTAssertTrue(evidenceProvider.works.allSatisfy { $0.nativePairedProof == fields[9] })
    XCTAssertFalse(operations.contains(17)); XCTAssertEqual(snapshots, 2)
  }

  func testRetiredPersistedDirectFoldIntentCannotReplayOrDispatchOnRecovery() throws {
    let f = try Fixture(), endpoint = Endpoint(), store = TestOperationIntentStore()
    let owner = KagemushaOperationIntentOwnerV1(store: store), id = digest(41)
    let prior = try KagemushaOperationIntentV1(applicationScope: store.applicationScope,
      qualificationScope: Data([1]), operation: 17, operationID: id, purpose: "internal-17",
      arguments: Data([1]), publicBinding: Data([2]), canonicalCommand: Data([3]))
    try store.save(prior)
    var dispatches = 0
    let transport = Transport(qualification: try f.qualification) { _, _, _, _ in
      dispatches += 1; throw TestError.unexpectedCall
    }
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport,
      core: try adapter(endpoint), intentOwner: owner, incomingFoldEvidenceProvider: testRequiredIncomingOwner())
    XCTAssertThrowsError(try provider.recover()) {
      XCTAssertEqual($0 as? KagemushaCoreCoordinatorErrorV1, .unavailable)
    }
    XCTAssertEqual(endpoint.calls, 0); XCTAssertEqual(dispatches, 0)
    XCTAssertEqual(try store.load(operation: 17, operationID: id), prior)
  }

  private func incomingFields(_ credit: Data) throws -> [Data] {
    [digest(42), credit, Data([3]), digest(4), digest(5), Data([6]), digest(7),
      KagemushaUInt128V1(8).littleEndianBytes, digest(9), try testIncomingCanonicalPairedProof()]
  }

  // Only this deterministic test owner recreates an adapter after proving the
  // prior handle is closed. Production authorization and proof verification remain
  // inside the real native coordinator, never inside this scripted fixture.
  private final class IncomingRecoveryCore: KagemushaNativeIncomingCoreCoordinatorV1 {
    private let endpoint: Endpoint
    private var active: KagemushaNativeCoreCoordinatorAdapterV1
    private(set) var reopenedHandles = 0
    private(set) var closedHandleRefusedWithoutRedispatch = false

    init(endpoint: Endpoint) throws {
      self.endpoint = endpoint
      active = try .init(bridge: .openEndpoint(storagePath: "/test/store", endpoint: endpoint))
    }

    func prepareIncomingFold(selector: KagemushaPendingCreditSelectorV1) throws -> KagemushaNativeIncomingFoldWorkV1 {
      try active.prepareIncomingFold(selector: selector)
    }
    func completeIncomingFold(work: KagemushaNativeIncomingFoldWorkV1,
      evidence: KagemushaOriginalIncomingFoldEvidenceV1) throws {
      do {
        try active.completeIncomingFold(work: work, evidence: evidence)
      } catch {
        let calls = endpoint.calls
        XCTAssertThrowsError(try active.completeIncomingFold(work: work, evidence: evidence)) {
          XCTAssertEqual($0 as? KagemushaCoreCoordinatorErrorV1, .unavailable)
        }
        XCTAssertEqual(endpoint.calls, calls)
        closedHandleRefusedWithoutRedispatch = endpoint.calls == calls
        active = try .init(bridge: .openEndpoint(storagePath: "/test/store", endpoint: endpoint))
        reopenedHandles += 1
        throw error
      }
    }
    func stageIncomingOriginal(kind: KagemushaNativeIncomingStageKindV1, creditID: Data) throws {
      try active.stageIncomingOriginal(kind: kind, creditID: creditID)
    }
    func reserveOperationID(operation: UInt8, operationID: Data, publicBinding: Data) throws -> Data {
      try active.reserveOperationID(operation: operation, operationID: operationID, publicBinding: publicBinding)
    }
    func beginObservation(operation: UInt8, canonicalCommand: Data) throws -> Data {
      try active.beginObservation(operation: operation, canonicalCommand: canonicalCommand)
    }
    func acceptQualification(_ qualification: KagemushaHardwareQualificationV1, hardwarePolicyDigest: Data) throws {
      try active.acceptQualification(qualification, hardwarePolicyDigest: hardwarePolicyDigest)
    }
    func acceptAuthenticatedDeviceReply(operation: UInt8, requestID: Data, canonicalCommand: Data,
      canonicalReply: Data, responseAuthenticator: Data, originalResponse: Data?, qualification: KagemushaHardwareQualificationV1) throws {
      try active.acceptAuthenticatedDeviceReply(operation: operation, requestID: requestID,
        canonicalCommand: canonicalCommand, canonicalReply: canonicalReply,
        responseAuthenticator: responseAuthenticator, originalResponse: originalResponse, qualification: qualification)
    }
    func beginSenderTransition(operationID: Data, inputs: KagemushaDeviceSenderPublicInputsV1,
      qualification: KagemushaHardwareQualificationV1) throws -> KagemushaNativeSenderPreparationV1 {
      try active.beginSenderTransition(operationID: operationID, inputs: inputs, qualification: qualification)
    }
    func provePreparedSenderTransition(preparation: KagemushaNativeSenderPreparationV1,
      authenticatedPreparationReply: Data) throws -> KagemushaNativeSenderCandidateV1 {
      try active.provePreparedSenderTransition(preparation: preparation,
        authenticatedPreparationReply: authenticatedPreparationReply)
    }
    func terminalEnvelope(candidate: KagemushaNativeSenderCandidateV1,
      originalSignedCommitResponse: Data) throws -> Data {
      try active.terminalEnvelope(candidate: candidate, originalSignedCommitResponse: originalSignedCommitResponse)
    }
    func acceptInstalledTerminal(candidate: KagemushaNativeSenderCandidateV1, canonicalEnvelope: Data,
      authenticatedInstallReply: Data, authenticatedInstalledReply: Data,
      authenticatedWalletSnapshotReply: Data) throws -> KagemushaHardwareTerminalResultV1 {
      try active.acceptInstalledTerminal(candidate: candidate, canonicalEnvelope: canonicalEnvelope,
        authenticatedInstallReply: authenticatedInstallReply, authenticatedInstalledReply: authenticatedInstalledReply,
        authenticatedWalletSnapshotReply: authenticatedWalletSnapshotReply)
    }
    func senderRecovery(kind: KagemushaNativeSenderKindV1, terminalID: Data,
      qualification: KagemushaHardwareQualificationV1) throws -> KagemushaNativeSenderRecoveryV1? {
      try active.senderRecovery(kind: kind, terminalID: terminalID, qualification: qualification)
    }
    func senderRecoveryByOperationID(kind: KagemushaNativeSenderKindV1, operationID: Data,
      qualification: KagemushaHardwareQualificationV1) throws -> KagemushaNativeSenderRecoveryV1? {
      try active.senderRecoveryByOperationID(kind: kind, operationID: operationID, qualification: qualification)
    }
    func recoverTerminalEnvelope(recovery: KagemushaNativeSenderRecoveryV1,
      authenticatedInstalledReply: Data) throws -> Data {
      try active.recoverTerminalEnvelope(recovery: recovery, authenticatedInstalledReply: authenticatedInstalledReply)
    }
    func outboxRelease(creditID: Data, inputs: KagemushaDeviceSenderPublicInputsV1, canonicalPayment: Data,
      terminalReceipt: KagemushaDeviceSenderTerminalReceiptV1,
      qualification: KagemushaHardwareQualificationV1) throws -> KagemushaNativeOutboxReleaseV1 {
      try active.outboxRelease(creditID: creditID, inputs: inputs, canonicalPayment: canonicalPayment,
        terminalReceipt: terminalReceipt, qualification: qualification)
    }
  }

  private final class IncomingEvidence: KagemushaIncomingFoldEvidenceProviderV1 {
    var acquisitions = 0
    var works: [KagemushaNativeIncomingFoldWorkV1] = []
    func recheckOriginals(for work: KagemushaNativeIncomingFoldWorkV1) { works.append(work) }
    func originalEvidence(for work: KagemushaNativeIncomingFoldWorkV1) throws -> KagemushaOriginalIncomingFoldEvidenceV1 {
      acquisitions += 1
      return try KagemushaOriginalIncomingFoldEvidenceV1(canonicalHardwareCertificate: Data([11]),
        deviceRootSelectionSignature: responseSignature(1))
    }
  }

  private final class Endpoint: KagemushaCoreCoordinatorEndpointV1 {
    var calls = 0
    var closeCalls = 0
    var policyFields: [Data]?
    var responseHandler: ((KagemushaCoreCoordinatorMethodV1, [Data]) throws -> [Data])?
    private var method: KagemushaCoreCoordinatorMethodV1 = .reserveOperationID
    private var expected: [Data]?
    private var response = [Data]()
    func expect(_ method: KagemushaCoreCoordinatorMethodV1, _ request: [Data]?, _ response: [Data]) {
      self.method = method; expected = request; self.response = response
    }
    func contract() -> [UInt32] { [2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21] }
    func install(storagePath: Data) throws {}
    func open(storagePath: Data) -> UInt64 { 1 }
    func invokeIntegrity(phase: UInt8, handle: UInt64, original: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeMintFunding(request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeIncoming(request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func close(handle: UInt64) { XCTAssertEqual(handle, 1); closeCalls += 1 }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
      calls += 1
      if method == KagemushaCoreCoordinatorMethodV1.authenticatedHardwarePolicy.rawValue {
        let f = try Fixture(), q = try f.qualification
        return try KagemushaCoreCoordinatorFrameV1.encodeResponse(.authenticatedHardwarePolicy,
          requestFrame: request, fields: policyFields ?? [q.releaseID, q.hardwarePolicyDigest,
            f.archive.preparation.context.devicePolicyBinding.hardwarePolicyID])
      }
      if let responseHandler, let selected = KagemushaCoreCoordinatorMethodV1(rawValue: method) {
        let fields = try KagemushaCoreCoordinatorFrameV1.decodeRequest(selected, frame: request)
        return try KagemushaCoreCoordinatorFrameV1.encodeResponse(selected, requestFrame: request, fields: responseHandler(selected, fields))
      }
      XCTAssertEqual(method, self.method.rawValue)
      if let expected { XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.decodeRequest(self.method, frame: request), expected) }
      return try KagemushaCoreCoordinatorFrameV1.encodeResponse(self.method, requestFrame: request, fields: response)
    }
  }

  private enum TestError: Error { case unexpectedCall }

  private final class Transport: KagemushaNativeAuthenticatedDeviceTransportV1 {
    let qualification: KagemushaHardwareQualificationV1
    let response: (UInt8, Data, Data, Data?) throws -> KagemushaAuthenticatedDeviceResponseV1
    init(qualification: KagemushaHardwareQualificationV1,
      response: @escaping (UInt8, Data, Data, Data?) throws -> KagemushaAuthenticatedDeviceResponseV1) {
      self.qualification = qualification; self.response = response
    }
    func hardwarePolicyID() -> Data { qualification.hardwarePolicyDigest }
    func qualificationReportDigest() -> Data { qualification.profile.qualificationReportDigest }
    func executeAndVerify(operation: UInt8, requestID: Data, canonicalCommand: Data,
      acceptedDevicePublicKey: Data?) throws -> KagemushaAuthenticatedDeviceResponseV1 {
      try response(operation, requestID, canonicalCommand, acceptedDevicePublicKey)
    }
  }

  private typealias Fixture = AuthenticatedProviderFixtureV1
}

private func digest(_ byte: UInt8) -> Data { Data(repeating: byte, count: 32) }
private func u32(_ value: UInt32) -> Data { KagemushaCoreCoordinatorFrameV1.u32(value) }
private func responseSignature(_ operation: UInt8) -> Data {
  var bytes = Data(repeating: 0, count: 64)
  bytes[31] = 1; bytes[63] = operation
  return bytes
}

struct AuthenticatedProviderFixtureV1 {
    let archive: CoordinatorArchiveFixtureV1
    var qualification: KagemushaHardwareQualificationV1 { get throws { try makeQualification() } }
    var qualificationFields: [Data] { get throws {
      let q = try qualification
      return try [u32(1), q.releaseID, KagemushaNoritoV1.encodeHardwareProfileShape(q.profile),
        KagemushaNoritoV1.encodeHardwareCredentialShape(q.credential), u32(0xffff)]
    } }
    var aggregate: Data { get throws { try aggregate() } }
    var terminalID: Data { get throws {
      let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(archive.paymentRequest)
      return try KagemushaNoritoV1.decodePaymentShapeExact(archive.payment, against: request).output.creditID
    } }
    var envelopeDigest: Data { get throws { try KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(archive.payment) } }

    init() throws { archive = try CoordinatorArchiveFixtureV1() }

    func makeQualification(generation: UInt64 = 1, policy: Data? = nil, coreKey: Data? = nil,
      devicePublicKey: KagemushaDevicePublicKeyV1? = nil, requestCredential: Bool = false) throws -> KagemushaHardwareQualificationV1 {
      let c = archive.preparation.context
      let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(archive.paymentRequest)
      let seed = request.hardwareCredential
      let selectedProfileID = requestCredential ? seed.hardwareProfileID : c.release.hardwareProfileID
      let selectedEpoch = requestCredential ? seed.policyEpoch : c.release.policyEpoch
      let profile = try KagemushaHardwareProfileV1(hardwareProfileID: selectedProfileID,
        providerID: digest(1), platformClass: .appleOEMService, productClassDigest: digest(2), firmwarePolicyDigest: seed.firmwarePolicyDigest,
        enrollmentAttestationVerifierDigest: digest(4), attestationTrustRootsDigest: digest(5), allowedSuiteCommitment: digest(6),
        policyEpoch: selectedEpoch, governanceCredentialPublicKey: seed.devicePublicKey, capabilityMask: 0xffff,
        qualificationReportDigest: digest(8), validFromMS: 0, expiresAtMS: seed.expiresAtMS + 1,
        appAttestationAuthorityPolicyDigest: digest(9))
      let credential = try KagemushaHardwareCredentialV1(credentialID: requestCredential ? seed.credentialID : c.credentialID,
        networkID: requestCredential ? seed.networkID : c.lane.networkID,
        hardwareProfileID: selectedProfileID, suiteID: requestCredential ? seed.suiteID : c.release.suiteID,
        firmwarePolicyDigest: seed.firmwarePolicyDigest, policyEpoch: selectedEpoch,
        laneCommitment: requestCredential ? seed.laneCommitment : c.lane.deviceLaneID,
        hardwareEpochID: generation == 1 ? (requestCredential ? seed.hardwareEpochID : c.hardwareEpoch.epochID) : digest(99),
        hardwareEpochGeneration: generation,
        devicePublicKey: devicePublicKey ?? seed.devicePublicKey,
        deviceKeyReference: requestCredential ? seed.deviceKeyReference : c.devicePolicyBinding.deviceKeyReference,
        issuedAtMS: seed.issuedAtMS, expiresAtMS: seed.expiresAtMS,
        appPolicyBindingDigest: seed.appPolicyBindingDigest,
        governanceSignature: seed.governanceSignature)
      return try KagemushaHardwareQualificationV1(releaseID: requestCredential ? request.releaseID : c.release.releaseID,
        hardwarePolicyDigest: policy ?? digest(87), coreAuthorizationKeyReference: coreKey ?? c.coreAuthorizationKeyReference,
        profile: profile, credential: credential)
    }

    func context(lane: Data? = nil, generation: UInt64 = 1, coreKey: Data? = nil) throws -> KagemushaDeviceSenderWalletContextV1 {
      let c = archive.preparation.context
      return try KagemushaDeviceSenderWalletContextV1(
        lane: .init(networkID: c.lane.networkID, deviceLaneID: lane ?? c.lane.deviceLaneID, asset: c.lane.asset, scale: c.lane.scale),
        release: c.release, credentialID: c.credentialID, hardwareEpoch: .init(generation: .init(generation), epochID: c.hardwareEpoch.epochID),
        devicePolicyBinding: c.devicePolicyBinding, coreAuthorizationKeyReference: coreKey ?? c.coreAuthorizationKeyReference)
    }

    func recovery(operationID: Data? = nil, terminalID: Data? = nil,
      context: KagemushaDeviceSenderWalletContextV1? = nil) throws -> KagemushaNativeSenderRecoveryV1 {
      try KagemushaNativeSenderRecoveryV1(operationID: operationID ?? archive.preparation.operationID,
        terminalID: terminalID ?? self.terminalID, context: context ?? archive.preparation.context, inputsDigest: archive.preparation.inputsDigest)
    }

    func aggregate(lane: Data? = nil, pool: Data? = nil, policy: Data? = nil) throws -> Data {
      let c = archive.preparation.context
      let state = try KagemushaAggregateStateCommitmentV1(releaseID: c.release.releaseID, networkID: c.lane.networkID,
        asset: c.lane.asset, assetIncarnation: c.release.assetIncarnation, scale: c.lane.scale,
        liabilityPoolID: pool ?? KagemushaNoritoV1.liabilityPoolID(networkID: c.lane.networkID, asset: c.lane.asset, incarnation: c.release.assetIncarnation),
        laneID: lane ?? c.lane.deviceLaneID, hardwareEpochID: c.hardwareEpoch.epochID, keyReference: c.devicePolicyBinding.deviceKeyReference,
        hardwarePolicyID: policy ?? c.devicePolicyBinding.hardwarePolicyID, sequence: .init(1), stateCommitment: digest(88))
      return try KagemushaNoritoV1.encodeAggregateStateShape(state)
    }
  }

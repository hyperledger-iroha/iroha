import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

/// Scripted endpoints verify orchestration only; they provide no proof or hardware qualification.
final class KagemushaNativeCoreCoordinatorAdapterV1Tests: XCTestCase {
  func testAllElevenMethodsMapExactNativeFields() throws {
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
      canonicalReply: Data([8]), responseAuthenticator: responseAuthenticator, qualification: f.qualification)
    try endpoint.expect(.beginSenderTransition, [id, u32(0), f.archive.paymentRequest] + f.qualificationFields, [id, prep])
    XCTAssertEqual(try core.beginSenderTransition(operationID: id, inputs: f.archive.inputs, qualification: f.qualification), f.archive.preparation)
    endpoint.expect(.provePreparedSenderTransition, [prep, Data([5])], [candidate])
    XCTAssertEqual(try core.provePreparedSenderTransition(preparation: f.archive.preparation, authenticatedPreparationReply: Data([5])), f.archive.candidate)
    endpoint.expect(.buildTerminalEnvelope, [candidate, Data([7])], [f.archive.payment])
    XCTAssertEqual(try core.terminalEnvelope(candidate: f.archive.candidate, authenticatedCommitReply: Data([7])), f.archive.payment)
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
    XCTAssertEqual(endpoint.calls, 12)
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
    for state in try [f.aggregate(lane: digest(9)), f.aggregate(pool: digest(9))] {
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
      default: throw TestError.unexpectedCall
      }
    }
    let transport = Transport(qualification: qualification) { operation, _, _, acceptedKey in
      if operation == 1 { XCTAssertNil(acceptedKey) }
      else { XCTAssertEqual(acceptedKey, qualification.credential.devicePublicKey.sec1Bytes) }
      return try KagemushaAuthenticatedDeviceResponseV1(operation: operation, status: .success,
        canonicalReply: operation == 1 ? qualificationReply : requestReply, authenticator: responseSignature(operation))
    }
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint), intentOwner: testOperationIntentOwner())
    XCTAssertEqual(try provider.createPaymentRequest(operationID: request.requestID, recipient: request.recipient,
      amount: request.amount, validityWindowMS: request.expiresAtMS - request.issuedAtMS), f.archive.paymentRequest)
    XCTAssertEqual(admitted.map { $0.0 }, [1, 22])
    for (operation, bytes) in admitted { XCTAssertEqual(bytes, responseSignature(operation)) }
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
        return try KagemushaAuthenticatedDeviceResponseV1(operation: operation, status: .success,
          canonicalReply: qualificationReply, authenticator: responseSignature(operation))
      }
      XCTAssertEqual(operation, 12)
      XCTAssertEqual(acceptedKey, qualification.credential.devicePublicKey.sec1Bytes)
      let decoded = try KagemushaDeviceOperationCodecV1.decodeSenderCommand(operation: operation, requestID: requestID, canonicalBytes: command)
      XCTAssertEqual(decoded.context, f.archive.preparation.context)
      XCTAssertEqual(decoded.operationID, f.archive.preparation.operationID)
      sawHistoricalRelease = true
      // Stop at the actual hardware boundary: this test does not fabricate a monetary success.
      return try KagemushaAuthenticatedDeviceResponseV1(operation: operation, status: .unavailable,
        canonicalReply: Data(), authenticator: Data())
    }
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint), intentOwner: testOperationIntentOwner())
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
        return try KagemushaAuthenticatedDeviceResponseV1(operation: 1, status: .success,
          canonicalReply: rotated ? currentReply : oldReply, authenticator: responseSignature(1))
      case 19:
        rotationRequests.append(id)
        XCTAssertEqual(acceptedKey, old.credential.devicePublicKey.sec1Bytes)
        if !rotated { rotated = true; throw TestError.unexpectedCall }
        return try KagemushaAuthenticatedDeviceResponseV1(operation: 19, status: .success,
          canonicalReply: rotationReply, authenticator: responseSignature(19))
      case 21:
        snapshots.append(id)
        XCTAssertEqual(acceptedKey, current.credential.devicePublicKey.sec1Bytes)
        return try KagemushaAuthenticatedDeviceResponseV1(operation: 21, status: .success,
          canonicalReply: snapshotReply, authenticator: responseSignature(21))
      default: throw TestError.unexpectedCall
      }
    }
    let original = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
      intentOwner: KagemushaOperationIntentOwnerV1(store: store))
    XCTAssertThrowsError(try original.rotateHardwareEpoch())
    let restarted = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
      intentOwner: KagemushaOperationIntentOwnerV1(store: store))
    XCTAssertEqual(try restarted.rotateHardwareEpoch(), installed)
    XCTAssertEqual(rotationRequests.count, 2)
    XCTAssertEqual(rotationRequests[0], rotationRequests[1])
    rejectRetainedMutation = true
    XCTAssertThrowsError(try restarted.recover(), "Saved mutation evidence must be re-admitted by native Core")
    XCTAssertFalse(try XCTUnwrap(store.records.values.first { $0.operation == 19 }).acknowledged)
    XCTAssertNil(store.records.values.first { $0.operation == 19 }?.authenticatedSnapshotEvidence)
    rejectRetainedMutation = false
    store.failAfterSave = true
    XCTAssertThrowsError(try restarted.recover())
    let interrupted = try XCTUnwrap(store.records.values.first { $0.operation == 19 })
    XCTAssertNotNil(interrupted.authenticatedSnapshotEvidence)
    XCTAssertFalse(interrupted.acknowledged, "Snapshot evidence must be durable before acknowledgement")
    store.failAfterSave = false
    XCTAssertEqual(try restarted.recover().aggregateState, installed)
    XCTAssertEqual(try restarted.recover().aggregateState, installed)
    XCTAssertEqual(Set(snapshots).count, 4)
    XCTAssertEqual(store.records.values.first { $0.operation == 19 }?.authenticatedSnapshotEvidence,
      interrupted.authenticatedSnapshotEvidence, "Retain original accepted historical evidence")
    XCTAssertTrue(store.records.values.allSatisfy { ![UInt8(1), 13, 18, 21].contains($0.operation) })
    XCTAssertTrue(try store.pending(operation: 19, purpose: "internal-19", qualificationScope: nil).isEmpty)
    XCTAssertEqual(store.records.values.filter { $0.operation == 19 }.count, 1)
  }

  func testRecoverAutomaticallyReplaysLostFoldAndRotationUnderOriginalQualification() throws {
    for operation: UInt8 in [17, 19] {
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
        return try KagemushaAuthenticatedDeviceResponseV1(operation: observed, status: .success,
          canonicalReply: bytes, authenticator: responseSignature(observed))
      }
      let original = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
        intentOwner: KagemushaOperationIntentOwnerV1(store: store))
      if operation == 19 { XCTAssertThrowsError(try original.rotateHardwareEpoch()) }
      else { XCTAssertThrowsError(try original.foldPendingCredit(selector: selector)) }
      XCTAssertNil(store.records.values.first?.canonicalReply)
      let restarted = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
        intentOwner: KagemushaOperationIntentOwnerV1(store: store))
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
        return try KagemushaAuthenticatedDeviceResponseV1(operation: 1, status: .success,
          canonicalReply: rotated ? currentReply : oldReply, authenticator: responseSignature(1))
      }
      XCTAssertEqual(operation, 22); XCTAssertEqual(id, request.requestID)
      XCTAssertEqual(acceptedKey, old.credential.devicePublicKey.sec1Bytes)
      requests += 1
      if !rotated { rotated = true; throw TestError.unexpectedCall }
      return try KagemushaAuthenticatedDeviceResponseV1(operation: 22, status: .success,
        canonicalReply: requestReply, authenticator: responseSignature(22))
    }
    func provider() throws -> KagemushaAuthenticatedHardwareProviderV1 {
      KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: try adapter(endpoint),
        intentOwner: KagemushaOperationIntentOwnerV1(store: store))
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
          return try KagemushaAuthenticatedDeviceResponseV1(operation: operation, status: .success,
            canonicalReply: snapshotReply, authenticator: oldAuthenticator!)
        }
        oldAuthenticator = signature
      }
      return try KagemushaAuthenticatedDeviceResponseV1(operation: operation, status: .success,
        canonicalReply: operation == 1 ? credentialReply : snapshotReply, authenticator: signature)
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
      core: try adapter(endpoint(seed: 40)), intentOwner: KagemushaOperationIntentOwnerV1(store: store))
    XCTAssertThrowsError(try original.recover())
    XCTAssertNil(try original.recover().aggregateState)
    XCTAssertTrue(store.records.isEmpty, "Reads never allocate durable host intents")
    replayOld = true
    let recreated = KagemushaAuthenticatedHardwareProviderV1(transport: transport,
      core: try adapter(endpoint(seed: 80)), intentOwner: KagemushaOperationIntentOwnerV1(store: store))
    XCTAssertThrowsError(try recreated.recover(), "Prior-owner signature cannot satisfy the new nonce")
    XCTAssertNil(try recreated.recover().aggregateState)
    XCTAssertEqual(Set(requests).count, 4)
    XCTAssertTrue(store.records.isEmpty)
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

  private final class Endpoint: KagemushaCoreCoordinatorEndpointV1 {
    var calls = 0
    var responseHandler: ((KagemushaCoreCoordinatorMethodV1, [Data]) throws -> [Data])?
    private var method: KagemushaCoreCoordinatorMethodV1 = .reserveOperationID
    private var expected: [Data]?
    private var response = [Data]()
    func expect(_ method: KagemushaCoreCoordinatorMethodV1, _ request: [Data]?, _ response: [Data]) {
      self.method = method; expected = request; self.response = response
    }
    func contract() -> [UInt32] { [2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff] }
    func open(storagePath: Data) -> UInt64 { 1 }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
      calls += 1
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
        qualificationReportDigest: digest(8), validFromMS: 0, expiresAtMS: seed.expiresAtMS + 1)
      let credential = try KagemushaHardwareCredentialV1(credentialID: requestCredential ? seed.credentialID : c.credentialID,
        networkID: requestCredential ? seed.networkID : c.lane.networkID,
        hardwareProfileID: selectedProfileID, suiteID: requestCredential ? seed.suiteID : c.release.suiteID,
        firmwarePolicyDigest: seed.firmwarePolicyDigest, policyEpoch: selectedEpoch,
        laneCommitment: requestCredential ? seed.laneCommitment : c.lane.deviceLaneID,
        hardwareEpochID: generation == 1 ? (requestCredential ? seed.hardwareEpochID : c.hardwareEpoch.epochID) : digest(99),
        hardwareEpochGeneration: generation,
        devicePublicKey: devicePublicKey ?? seed.devicePublicKey,
        deviceKeyReference: requestCredential ? seed.deviceKeyReference : c.devicePolicyBinding.deviceKeyReference,
        issuedAtMS: seed.issuedAtMS, expiresAtMS: seed.expiresAtMS, governanceSignature: seed.governanceSignature)
      return try KagemushaHardwareQualificationV1(releaseID: requestCredential ? request.releaseID : c.release.releaseID,
        hardwarePolicyDigest: policy ?? c.devicePolicyBinding.hardwarePolicyID, coreAuthorizationKeyReference: coreKey ?? c.coreAuthorizationKeyReference,
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

    func aggregate(lane: Data? = nil, pool: Data? = nil) throws -> Data {
      let c = archive.preparation.context
      let state = try KagemushaAggregateStateCommitmentV1(releaseID: c.release.releaseID, networkID: c.lane.networkID,
        asset: c.lane.asset, assetIncarnation: c.release.assetIncarnation, scale: c.lane.scale,
        liabilityPoolID: pool ?? KagemushaNoritoV1.liabilityPoolID(networkID: c.lane.networkID, asset: c.lane.asset, incarnation: c.release.assetIncarnation),
        laneID: lane ?? c.lane.deviceLaneID, hardwareEpochID: c.hardwareEpoch.epochID, keyReference: c.devicePolicyBinding.deviceKeyReference,
        hardwarePolicyID: c.devicePolicyBinding.hardwarePolicyID, sequence: .init(1), stateCommitment: digest(88))
      return try KagemushaNoritoV1.encodeAggregateStateShape(state)
    }
  }

// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import CryptoKit
import Foundation

/// Fail-closed errors from the authenticated ABI-23 hardware-provider boundary.
public enum KagemushaAuthenticatedHardwareProviderErrorV1: Error, Equatable, Sendable {
  case invalidContract(String)
  case operationFailed(
    operation: UInt8,
    status: KagemushaDeviceLifecycleStatusV1
  )
}

/// Result returned only after the platform transport verifies the complete ABI-23 response.
public struct KagemushaAuthenticatedDeviceResponseV1: Equatable, Sendable {
  public let operation: UInt8
  public let status: KagemushaDeviceLifecycleStatusV1
  public let canonicalReply: Data
  public let authenticator: Data

  public init(
    operation: UInt8,
    status: KagemushaDeviceLifecycleStatusV1,
    canonicalReply: Data,
    authenticator: Data
  ) throws {
    guard (1...22).contains(operation) else {
      throw authenticatedProviderInvalid("operation is outside the frozen KAGEMUSHA V1 inventory")
    }
    if status == .success {
      guard !canonicalReply.isEmpty else {
        throw authenticatedProviderInvalid("successful device response omitted its canonical reply")
      }
      _ = try KagemushaDeviceSignatureV1(rawBytes: authenticator)
    } else {
      guard canonicalReply.isEmpty, authenticator.isEmpty else {
        throw authenticatedProviderInvalid("failed device response exposed unauthenticated bytes")
      }
    }
    self.operation = operation
    self.status = status
    self.canonicalReply = Data(canonicalReply)
    self.authenticator = Data(authenticator)
  }
}

/// Platform bridge which verifies a complete ABI-23 response before returning success bytes.
///
/// Operation 1 passes no accepted key. Operations 2 through 22 pass the exact 65-byte P-256 key
/// admitted from operation 1. Implementations must invoke the native ABI-23 response verifier;
/// verifying the signature independently in Swift is not an implementation of this boundary.
public protocol KagemushaNativeAuthenticatedDeviceTransportV1: AnyObject {
  func hardwarePolicyID() throws -> Data
  func qualificationReportDigest() throws -> Data

  func executeAndVerify(
    operation: UInt8,
    requestID: Data,
    canonicalCommand: Data,
    acceptedDevicePublicKey: Data?
  ) throws -> KagemushaAuthenticatedDeviceResponseV1
}

/// The dynamically discovered native lifecycle bridge already satisfies the authenticated
/// transport contract. An unavailable or partial bridge remains online-only.
extension KagemushaDeviceLifecycleBridgeV1: KagemushaNativeAuthenticatedDeviceTransportV1 {
  public func hardwarePolicyID() throws -> Data {
    guard let acceptedCapabilities else {
      throw KagemushaDeviceLifecycleBridgeErrorV1.onlineOnly
    }
    return acceptedCapabilities.hardwarePolicyID
  }

  public func qualificationReportDigest() throws -> Data {
    guard let acceptedCapabilities else {
      throw KagemushaDeviceLifecycleBridgeErrorV1.onlineOnly
    }
    return acceptedCapabilities.qualificationReportDigest
  }

  public func executeAndVerify(
    operation: UInt8,
    requestID: Data,
    canonicalCommand: Data,
    acceptedDevicePublicKey: Data?
  ) throws -> KagemushaAuthenticatedDeviceResponseV1 {
    guard let bridgeOperation = KagemushaDeviceLifecycleOperationV1(rawValue: operation) else {
      throw authenticatedProviderInvalid("operation is outside the bridge inventory")
    }
    let result = try executeAuthenticated(
      operation: bridgeOperation,
      requestID: requestID,
      canonicalCommand: canonicalCommand,
      acceptedDevicePublicKey: acceptedDevicePublicKey
    )
    return try KagemushaAuthenticatedDeviceResponseV1(
      operation: result.operation.rawValue,
      status: result.status,
      canonicalReply: result.payload,
      authenticator: result.authenticator
    )
  }
}

/// A sender transition prepared under one durable native-Core operation identity.
public struct KagemushaNativeSenderPreparationV1: Equatable, Sendable {
  public let operationID: Data
  public let context: KagemushaDeviceSenderWalletContextV1
  public let inputsDigest: Data

  public init(
    operationID: Data,
    context: KagemushaDeviceSenderWalletContextV1,
    inputsDigest: Data
  ) throws {
    self.operationID = try authenticatedProviderDigest(operationID, "operationID")
    self.context = context
    self.inputsDigest = try authenticatedProviderDigest(inputsDigest, "inputsDigest")
  }
}

/// Native-Core proof material admitted after an authenticated operation-5/6 reply.
public struct KagemushaNativeSenderCandidateV1: Equatable, Sendable {
  public let preparation: KagemushaNativeSenderPreparationV1
  public let selector: KagemushaDeviceSenderPreparationSelectorV1
  public let candidateDigest: Data

  public init(
    preparation: KagemushaNativeSenderPreparationV1,
    selector: KagemushaDeviceSenderPreparationSelectorV1,
    candidateDigest: Data
  ) throws {
    self.preparation = preparation
    self.selector = selector
    self.candidateDigest = try authenticatedProviderDigest(candidateDigest, "candidateDigest")
  }
}

/// Native-Core lookup state for byte-identical operation-10 recovery.
public struct KagemushaNativeSenderRecoveryV1: Equatable, Sendable {
  public let operationID: Data
  public let context: KagemushaDeviceSenderWalletContextV1
  public let inputsDigest: Data

  public init(
    operationID: Data,
    context: KagemushaDeviceSenderWalletContextV1,
    inputsDigest: Data
  ) throws {
    self.operationID = try authenticatedProviderDigest(operationID, "operationID")
    self.context = context
    self.inputsDigest = try authenticatedProviderDigest(inputsDigest, "inputsDigest")
  }
}

/// Exact operation-12 release material retained by native Core.
public struct KagemushaNativeOutboxReleaseV1: Equatable, Sendable {
  public let operationID: Data
  public let context: KagemushaDeviceSenderWalletContextV1
  public let inputsDigest: Data
  public let envelopeDigest: Data
  public let inputs: KagemushaDeviceSenderPublicInputsV1
  public let canonicalEnvelope: Data

  public init(
    operationID: Data,
    context: KagemushaDeviceSenderWalletContextV1,
    inputsDigest: Data,
    envelopeDigest: Data,
    inputs: KagemushaDeviceSenderPublicInputsV1,
    canonicalEnvelope: Data
  ) throws {
    guard !canonicalEnvelope.isEmpty else {
      throw authenticatedProviderInvalid("canonicalEnvelope is empty")
    }
    self.operationID = try authenticatedProviderDigest(operationID, "operationID")
    self.context = context
    self.inputsDigest = try authenticatedProviderDigest(inputsDigest, "inputsDigest")
    self.envelopeDigest = try authenticatedProviderDigest(envelopeDigest, "envelopeDigest")
    self.inputs = inputs
    self.canonicalEnvelope = Data(canonicalEnvelope)
  }
}

public enum KagemushaNativeSenderKindV1: Equatable, Sendable {
  case payment
  case redemption
}

/// Audited native Core authority required by `KagemushaAuthenticatedHardwareProviderV1`.
///
/// This protocol intentionally has no stock implementation. It owns durable operation IDs,
/// signed release-catalog membership, recursive proof generation and verification, sender
/// typestate, and byte-identical terminal recovery. A production factory must fail closed unless
/// the signed app contains exactly one qualified implementation.
public protocol KagemushaNativeCoreCoordinatorV1: AnyObject {
  /// Persist and return a fresh non-zero ID before the corresponding device mutation.
  func reserveOperationID(operation: UInt8, publicBinding: Data) throws -> Data

  /// Admit the exact signed release member and bind it to the authenticated hardware tuple.
  func acceptQualification(
    _ qualification: KagemushaHardwareQualificationV1,
    hardwarePolicyDigest: Data
  ) throws

  /// Admit one already P-256-authenticated canonical device reply into Core's typestate.
  func acceptAuthenticatedDeviceReply(
    operation: UInt8,
    requestID: Data,
    canonicalCommand: Data,
    canonicalReply: Data,
    qualification: KagemushaHardwareQualificationV1
  ) throws

  func beginSenderTransition(
    inputs: KagemushaDeviceSenderPublicInputsV1,
    qualification: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeSenderPreparationV1

  /// Generate and persist the actual recursive candidate before operation 7 is constructed.
  func provePreparedSenderTransition(
    preparation: KagemushaNativeSenderPreparationV1,
    authenticatedPreparationReply: Data
  ) throws -> KagemushaNativeSenderCandidateV1

  /// Verify operation 7/8 and construct the final proof-bearing terminal envelope.
  func terminalEnvelope(
    candidate: KagemushaNativeSenderCandidateV1,
    authenticatedCommitReply: Data
  ) throws -> Data

  /// Expose a terminal result only after operations 9, 10, and 21 agree.
  func acceptInstalledTerminal(
    candidate: KagemushaNativeSenderCandidateV1,
    canonicalEnvelope: Data,
    authenticatedInstallReply: Data,
    authenticatedInstalledReply: Data,
    authenticatedWalletSnapshotReply: Data
  ) throws -> KagemushaHardwareTerminalResultV1

  func senderRecovery(
    kind: KagemushaNativeSenderKindV1,
    terminalID: Data,
    qualification: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeSenderRecoveryV1?

  func recoverTerminalEnvelope(
    recovery: KagemushaNativeSenderRecoveryV1,
    authenticatedInstalledReply: Data
  ) throws -> Data

  func outboxRelease(
    creditID: Data,
    inputs: KagemushaDeviceSenderPublicInputsV1,
    canonicalPayment: Data,
    qualification: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeOutboxReleaseV1

}

/// Authenticated, release-pinned client for every frozen KAGEMUSHA V1 operation.
///
/// Successful bytes cross this boundary only after both the platform transport's native verifier
/// and the injected native Core coordinator accept them.
public final class KagemushaAuthenticatedDeviceClientV1: @unchecked Sendable {
  fileprivate let core: any KagemushaNativeCoreCoordinatorV1
  private let transport: any KagemushaNativeAuthenticatedDeviceTransportV1
  private let lock = NSRecursiveLock()
  private var session: Session?

  private struct Session {
    let qualification: KagemushaHardwareQualificationV1
    let responseKey: Data
  }

  public init(
    transport: any KagemushaNativeAuthenticatedDeviceTransportV1,
    core: any KagemushaNativeCoreCoordinatorV1
  ) {
    self.transport = transport
    self.core = core
  }

  public func qualification() throws -> KagemushaHardwareQualificationV1 {
    try locked {
      if let session { return session.qualification }
      return try qualifyLocked().qualification
    }
  }

  fileprivate func invalidateQualification() {
    locked {
      if var key = session?.responseKey {
        key.resetBytes(in: key.startIndex..<key.endIndex)
      }
      session = nil
    }
  }

  fileprivate func control(
    _ command: KagemushaDeviceControlCommandV1,
    requestID: Data
  ) throws -> AuthenticatedCall {
    try locked {
      let canonical = try KagemushaDeviceOperationCodecV1.encodeControlCommand(command)
      _ = try KagemushaDeviceOperationCodecV1.decodeControlCommand(
        operation: command.operation,
        requestID: requestID,
        canonicalBytes: canonical
      )
      return try executeLocked(
        operation: command.operation,
        requestID: requestID,
        command: canonical,
        lane: .control
      )
    }
  }

  fileprivate func receiver(
    _ command: KagemushaDeviceReceiverCommandV1,
    requestID: Data
  ) throws -> AuthenticatedCall {
    try locked {
      let canonical = try KagemushaDeviceOperationCodecV1.encodeReceiverCommand(command)
      _ = try KagemushaDeviceOperationCodecV1.decodeReceiverCommand(
        operation: command.operation,
        requestID: requestID,
        canonicalBytes: canonical
      )
      return try executeLocked(
        operation: command.operation,
        requestID: requestID,
        command: canonical,
        lane: .receiver
      )
    }
  }

  fileprivate func sender(_ command: KagemushaDeviceSenderCommandV1) throws -> AuthenticatedCall {
    try locked {
      let canonical = try KagemushaDeviceOperationCodecV1.encodeSenderCommand(command)
      _ = try KagemushaDeviceOperationCodecV1.decodeSenderCommand(
        operation: command.operation,
        requestID: command.operationID,
        canonicalBytes: canonical
      )
      return try executeLocked(
        operation: command.operation,
        requestID: command.operationID,
        command: canonical,
        lane: .sender
      )
    }
  }

  fileprivate func mintStage(
    requestID: Data,
    canonicalCommand: Data
  ) throws -> AuthenticatedCall {
    try locked {
      _ = try KagemushaNoritoV1.decodeDeviceMintStageCommandShapeExact(canonicalCommand)
      return try executeLocked(
        operation: 16,
        requestID: requestID,
        command: canonicalCommand,
        lane: .mint
      )
    }
  }

  private func qualifyLocked() throws -> Session {
    let operation: UInt8 = 1
    let requestID = try core.reserveOperationID(
      operation: operation,
      publicBinding: Data([operation])
    )
    let command = try KagemushaDeviceOperationCodecV1.encodeControlCommand(
      .readActiveHardwareCredential
    )
    _ = try KagemushaDeviceOperationCodecV1.decodeControlCommand(
      operation: operation,
      requestID: requestID,
      canonicalBytes: command
    )
    let response = try transport.executeAndVerify(
      operation: rawOperation,
      requestID: requestID,
      canonicalCommand: command,
      acceptedDevicePublicKey: nil
    )
    guard response.operation == rawOperation else {
      throw authenticatedProviderInvalid("device response substituted operation 1")
    }
    guard response.status == .success else {
      throw KagemushaAuthenticatedHardwareProviderErrorV1.operationFailed(
        operation: operation,
        status: response.status
      )
    }
    _ = try KagemushaDeviceSignatureV1(rawBytes: response.authenticator)
    let reply = try KagemushaDeviceOperationCodecV1.decodeControlReplyAfterAuthentication(
      operation: operation,
      canonicalBytes: response.canonicalReply
    )
    let decoded =
      try KagemushaDeviceOperationCodecV1
      .decodeQualificationReplyAfterAuthentication(reply)
    let capabilityPolicy = try authenticatedProviderDigest(
      transport.hardwarePolicyID(),
      "hardwarePolicyID"
    )
    let capabilityQualification = try authenticatedProviderDigest(
      transport.qualificationReportDigest(),
      "qualificationReportDigest"
    )
    guard decoded.hardwarePolicyDigest == decoded.profile.hardwareProfileID,
      decoded.hardwarePolicyDigest == capabilityPolicy,
      decoded.profile.qualificationReportDigest == capabilityQualification,
      decoded.profile.hardwareProfileID == decoded.credential.hardwareProfileID
    else {
      throw authenticatedProviderInvalid(
        "qualification does not match the admitted capability frame")
    }
    let responseKey = decoded.credential.devicePublicKey.sec1Bytes
    let qualification = try KagemushaHardwareQualificationV1(
      releaseID: decoded.releaseID,
      hardwarePolicyDigest: decoded.hardwarePolicyDigest,
      profile: decoded.profile,
      credential: decoded.credential
    )
    try core.acceptQualification(
      qualification,
      hardwarePolicyDigest: decoded.hardwarePolicyDigest
    )
    try core.acceptAuthenticatedDeviceReply(
      operation: operation,
      requestID: requestID,
      canonicalCommand: command,
      canonicalReply: reply.canonicalArchive,
      qualification: qualification
    )
    let accepted = Session(qualification: qualification, responseKey: responseKey)
    session = accepted
    return accepted
  }

  private func executeLocked(
    operation rawOperation: UInt8,
    requestID: Data,
    command: Data,
    lane: ReplyLane
  ) throws -> AuthenticatedCall {
    guard (1...22).contains(rawOperation) else {
      throw authenticatedProviderInvalid("operation is outside the frozen KAGEMUSHA V1 inventory")
    }
    let accepted = try session ?? qualifyLocked()
    var responseKey = Data(accepted.responseKey)
    defer { responseKey.resetBytes(in: responseKey.startIndex..<responseKey.endIndex) }
    let response = try transport.executeAndVerify(
      operation: operation,
      requestID: requestID,
      canonicalCommand: command,
      acceptedDevicePublicKey: responseKey
    )
    guard response.operation == operation else {
      throw authenticatedProviderInvalid("device response substituted operation \(rawOperation)")
    }
    guard response.status == .success else {
      return AuthenticatedCall(
        operation: rawOperation,
        status: response.status,
        canonicalCommand: command,
        reply: nil,
        canonicalReply: nil
      )
    }
    _ = try KagemushaDeviceSignatureV1(rawBytes: response.authenticator)
    let reply: KagemushaDeviceAuthenticatedReplyV1?
    switch lane {
    case .control:
      reply = try KagemushaDeviceOperationCodecV1.decodeControlReplyAfterAuthentication(
        operation: rawOperation,
        canonicalBytes: response.canonicalReply
      )
    case .receiver:
      reply = try KagemushaDeviceOperationCodecV1.decodeReceiverReplyAfterAuthentication(
        operation: rawOperation,
        canonicalBytes: response.canonicalReply
      )
    case .sender:
      reply = try KagemushaDeviceOperationCodecV1.decodeSenderReplyAfterAuthentication(
        operation: rawOperation,
        requestID: requestID,
        canonicalBytes: response.canonicalReply
      )
    case .mint:
      _ = try KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(
        response.canonicalReply
      )
      reply = nil
    }
    try core.acceptAuthenticatedDeviceReply(
      operation: rawOperation,
      requestID: requestID,
      canonicalCommand: command,
      canonicalReply: response.canonicalReply,
      qualification: accepted.qualification
    )
    return AuthenticatedCall(
      operation: rawOperation,
      status: response.status,
      canonicalCommand: command,
      reply: reply,
      canonicalReply: response.canonicalReply
    )
  }

  private func locked<T>(_ body: () throws -> T) rethrows -> T {
    lock.lock()
    defer { lock.unlock() }
    return try body()
  }

  private enum ReplyLane {
    case control
    case receiver
    case sender
    case mint
  }
}

private struct AuthenticatedCall {
  let operation: UInt8
  let status: KagemushaDeviceLifecycleStatusV1
  let canonicalCommand: Data
  let reply: KagemushaDeviceAuthenticatedReplyV1?
  let canonicalReply: Data?
}

/// High-level Offline wallet provider backed only by authenticated hardware and native Core.
public final class KagemushaAuthenticatedHardwareProviderV1: KagemushaHardwareProviderV1,
  @unchecked Sendable
{
  private let client: KagemushaAuthenticatedDeviceClientV1
  private let lock = NSRecursiveLock()

  public init(client: KagemushaAuthenticatedDeviceClientV1) {
    self.client = client
  }

  public convenience init(
    transport: any KagemushaNativeAuthenticatedDeviceTransportV1,
    core: any KagemushaNativeCoreCoordinatorV1
  ) {
    self.init(client: KagemushaAuthenticatedDeviceClientV1(transport: transport, core: core))
  }

  public func qualification() throws -> KagemushaHardwareQualificationV1 {
    try client.qualification()
  }

  public func recover() throws -> KagemushaHardwareRecoveryV1 {
    try locked {
      let call = try control(.recoverWalletSnapshot, requestID: freshID(operation: 21))
      var reader = try payloadReader(call, operation: 21)
      let aggregate = try reader.optionVector(maximum: KagemushaWireV1.maximumAggregateStateBytes)
      let journal = try reader.u128Field()
      let pending = try reader.u128Field()
      let retry = try reader.u128Field()
      try reader.finish()
      if let aggregate {
        _ = try KagemushaNoritoV1.decodeAggregateStateShapeExact(aggregate)
      }
      return try KagemushaHardwareRecoveryV1(
        aggregateState: aggregate,
        journalRevision: journal,
        pendingCreditCount: pending,
        retryOutboxCount: retry
      )
    }
  }

  public func bootstrapState() throws -> Data {
    try locked {
      let operationID = try freshID(operation: 20)
      let call = try control(
        .bootstrapAggregateState(operationID: operationID),
        requestID: operationID
      )
      var reader = try payloadReader(call, operation: 20)
      let canonical = try reader.singleVector(
        maximum: KagemushaWireV1.maximumAggregateStateBytes
      )
      _ = try KagemushaNoritoV1.decodeAggregateStateShapeExact(canonical)
      return canonical
    }
  }

  public func createPaymentRequest(
    recipientAccount: KagemushaAccountIDV1,
    amount: KagemushaUInt128V1,
    validityWindowMillis: UInt64
  ) throws -> Data {
    try locked {
      guard !amount.isZero,
        (1...KagemushaWireV1.requestMaximumTTLMS).contains(validityWindowMillis)
      else { throw authenticatedProviderInvalid("invalid payment request amount or lifetime") }
      var binding = Data(recipientAccount.canonicalPayload)
      binding.append(amount.littleEndianBytes)
      binding.append(authenticatedProviderU64LE(validityWindowMillis))
      let requestID = try client.core.reserveOperationID(operation: 22, publicBinding: binding)
      let command = KagemushaDeviceControlCommandV1.createSignedPaymentRequest(
        requestID: requestID,
        recipient: recipientAccount,
        amount: amount,
        validityWindowMS: validityWindowMillis
      )
      var reader = try payloadReader(
        control(command, requestID: requestID),
        operation: 22
      )
      let canonical = try reader.singleVector(
        maximum: KagemushaWireV1.maximumPaymentRequestBytes
      )
      let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(canonical)
      guard request.requestID == requestID,
        request.recipient == recipientAccount,
        request.amount == amount,
        request.expiresAtMS - request.issuedAtMS == validityWindowMillis,
        request.releaseID == (try qualification()).releaseID
      else { throw authenticatedProviderInvalid("signed payment request binding mismatch") }
      return canonical
    }
  }

  public func stagePayment(
    canonicalRequest: Data,
    canonicalPayment: Data
  ) throws -> KagemushaHardwarePaymentStageV1 {
    try locked {
      let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(canonicalRequest)
      let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
        canonicalPayment,
        against: request
      )
      let creditID = payment.output.creditID
      let recovery = try client.receiver(
        .recoverStaged(creditID: creditID),
        requestID: creditID
      )
      let disposition: KagemushaHardwareStageDispositionV1
      let receipt: KagemushaInboxReceiptV1
      if recovery.status == .success {
        disposition = .exactDuplicate
        receipt = try parseStagedReply(
          recovery,
          operation: 3,
          requestBytes: canonicalRequest,
          paymentBytes: canonicalPayment,
          payment: payment
        )
      } else {
        guard recovery.status == .missing else {
          throw KagemushaAuthenticatedHardwareProviderErrorV1.operationFailed(
            operation: recovery.operation,
            status: recovery.status
          )
        }
        disposition = .staged
        let staged = try client.receiver(
          .stage(
            canonicalRequest: canonicalRequest,
            canonicalPayment: canonicalPayment,
            stagingMetadata: Data()
          ),
          requestID: creditID
        )
        receipt = try parseStagedReply(
          requireSuccess(staged),
          operation: 2,
          requestBytes: canonicalRequest,
          paymentBytes: canonicalPayment,
          payment: payment
        )
      }
      var acknowledgementReader = try payloadReader(
        control(
          .signReceiveAcknowledgement(
            canonicalRequest: canonicalRequest,
            canonicalPayment: canonicalPayment,
            inboxReceipt: receipt
          ),
          requestID: creditID
        ),
        operation: 11
      )
      let acknowledgement = try acknowledgementReader.singleVector(
        maximum: KagemushaWireV1.maximumAcknowledgementBytes
      )
      let decoded = try KagemushaNoritoV1.decodeAcknowledgementShapeExact(
        acknowledgement,
        against: request,
        payment: payment
      )
      guard decoded.inboxReceipt.creditID == creditID,
        decoded.inboxReceipt.receiptCommitment == receipt.receiptCommitment
      else { throw authenticatedProviderInvalid("acknowledgement receipt binding mismatch") }
      return try KagemushaHardwarePaymentStageV1(
        disposition: disposition,
        creditID: creditID,
        acknowledgement: acknowledgement
      )
    }
  }

  public func stageMintCredit(
    canonicalAuthorization: Data,
    canonicalMintCredit: Data
  ) throws -> KagemushaHardwareMintStageV1 {
    try locked {
      let authorization = try KagemushaNoritoV1.decodeMintAuthorizationShapeExact(
        canonicalAuthorization
      )
      let commandModel = try KagemushaDeviceMintStageCommandV1(
        canonicalAuthorization: canonicalAuthorization,
        canonicalMintCredit: canonicalMintCredit
      )
      let canonicalCommand = try KagemushaNoritoV1.encodeDeviceMintStageCommandShape(commandModel)
      let operationID = authorization.statement.context.operationID
      let call = try requireSuccess(
        client.mintStage(requestID: operationID, canonicalCommand: canonicalCommand)
      )
      guard let canonicalReply = call.canonicalReply else {
        throw authenticatedProviderInvalid("operation 16 omitted its authenticated reply")
      }
      let result = try KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(
        canonicalReply,
        against: commandModel
      )
      return try KagemushaHardwareMintStageV1(
        disposition: result.disposition == .staged ? .staged : .exactDuplicate,
        creditID: result.creditID
      )
    }
  }

  public func pendingCreditWatermark() throws -> KagemushaUInt128V1 {
    try locked {
      var reader = try payloadReader(
        control(.readPendingCreditWatermark, requestID: freshID(operation: 18)),
        operation: 18
      )
      let watermark = try reader.u128Field()
      try reader.finish()
      return watermark
    }
  }

  public func nextPendingCreditID() throws -> Data? {
    try locked {
      let call = try client.receiver(
        .page(snapshotRevision: nil, after: nil, maximumEntries: 1),
        requestID: freshID(operation: 4)
      )
      if call.status == .missing { return nil }
      var reader = try payloadReader(requireSuccess(call), operation: 4)
      _ = try reader.u128Field()
      let records = try reader.itemVectorFields(maximumEntries: 1)
      _ = try reader.optionDigest()
      try reader.finish()
      guard let first = records.first else { return nil }
      return try stagedRecordCreditID(first)
    }
  }

  public func journalRevision() throws -> KagemushaUInt128V1 {
    try recover().journalRevision
  }

  public func foldReceiveCredit(
    creditID: Data
  ) throws -> KagemushaHardwareReceiveFoldV1 {
    try locked {
      let credit = try authenticatedProviderDigest(creditID, "creditID")
      let operationID = try client.core.reserveOperationID(
        operation: 17,
        publicBinding: credit
      )
      let call = try client.control(
        .foldReceiveCredit(operationID: operationID, creditID: credit),
        requestID: operationID
      )
      var reader = try payloadReader(requireSuccess(call), operation: 17)
      guard try reader.digestField() == credit else {
        throw authenticatedProviderInvalid("receive-fold credit ID mismatch")
      }
      let aggregate = try reader.vectorField(
        maximum: KagemushaWireV1.maximumAggregateStateBytes
      )
      try reader.finish()
      _ = try KagemushaNoritoV1.decodeAggregateStateShapeExact(aggregate)
      return try KagemushaHardwareReceiveFoldV1(
        aggregateState: aggregate,
        creditID: credit
      )
    }
  }

  public func commitPayment(
    canonicalRequest: Data
  ) throws -> KagemushaHardwareTerminalResultV1 {
    try locked {
      _ = try KagemushaNoritoV1.decodePaymentRequestShapeExact(canonicalRequest)
      return try commitSender(inputs: .sendSplit(canonicalRequest: canonicalRequest))
    }
  }

  public func recoverPayment(creditID: Data) throws -> Data? {
    try locked { try recoverTerminal(kind: .payment, terminalID: creditID) }
  }

  public func recordAcknowledgement(
    creditID: Data,
    canonicalRequest: Data,
    canonicalPayment: Data,
    canonicalAcknowledgement: Data
  ) throws {
    try locked {
      let expectedCreditID = try authenticatedProviderDigest(creditID, "creditID")
      let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(canonicalRequest)
      let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
        canonicalPayment,
        against: request
      )
      guard payment.output.creditID == expectedCreditID else {
        throw authenticatedProviderInvalid("payment credit ID mismatch")
      }
      _ = try KagemushaNoritoV1.decodeAcknowledgementShapeExact(
        canonicalAcknowledgement,
        against: request,
        payment: payment
      )
      let inputs = KagemushaDeviceSenderPublicInputsV1.sendSplit(
        canonicalRequest: canonicalRequest
      )
      let release = try client.core.outboxRelease(
        creditID: expectedCreditID,
        inputs: inputs,
        canonicalPayment: canonicalPayment,
        qualification: qualification()
      )
      let command = try KagemushaDeviceSenderCommandV1(
        operation: 12,
        operationID: release.operationID,
        context: release.context,
        body: .release(
          inputsDigest: release.inputsDigest,
          envelopeDigest: release.envelopeDigest,
          inputs: release.inputs,
          canonicalEnvelope: release.canonicalEnvelope,
          canonicalAcknowledgement: canonicalAcknowledgement
        )
      )
      try requireSuccess(client.sender(command))
    }
  }

  public func commitRedemption(
    amount: KagemushaUInt128V1,
    beneficiary: KagemushaAccountIDV1
  ) throws -> KagemushaHardwareTerminalResultV1 {
    try locked {
      guard !amount.isZero else { throw authenticatedProviderInvalid("redemption amount is zero") }
      return try commitSender(inputs: .redeemSplit(amount: amount, beneficiary: beneficiary))
    }
  }

  public func recoverRedemption(redemptionID: Data) throws -> Data? {
    try locked { try recoverTerminal(kind: .redemption, terminalID: redemptionID) }
  }

  public func rotateHardwareEpoch() throws -> Data {
    try locked {
      let operationID = try freshID(operation: 19)
      var reader = try payloadReader(
        control(.rotateHardwareEpoch(operationID: operationID), requestID: operationID),
        operation: 19
      )
      let aggregate = try reader.singleVector(
        maximum: KagemushaWireV1.maximumAggregateStateBytes
      )
      _ = try KagemushaNoritoV1.decodeAggregateStateShapeExact(aggregate)
      client.invalidateQualification()
      _ = try client.qualification()
      return aggregate
    }
  }

  /// Return authenticated qualified time or lease evidence through operation 13.
  public func readTrustedTimeOrLease() throws -> Data {
    try locked {
      let call = try requireSuccess(
        client.control(.readTrustedTimeOrLease, requestID: freshID(operation: 13))
      )
      guard let canonicalReply = call.canonicalReply else {
        throw authenticatedProviderInvalid("operation 13 omitted its authenticated reply")
      }
      return canonicalReply
    }
  }

  /// Prepare a proof-bearing mint authorization through operation 14.
  public func prepareMintAuthorization(
    amount: KagemushaUInt128V1,
    payer: KagemushaAccountIDV1,
    recipient: KagemushaAccountIDV1
  ) throws -> Data {
    try locked {
      guard !amount.isZero else { throw authenticatedProviderInvalid("mint amount is zero") }
      var binding = Data(amount.littleEndianBytes)
      binding.append(payer.canonicalPayload)
      binding.append(recipient.canonicalPayload)
      let operationID = try client.core.reserveOperationID(
        operation: 14,
        publicBinding: binding
      )
      let command = KagemushaDeviceControlCommandV1.prepareMintAuthorization(
        operationID: operationID,
        amount: amount,
        payer: payer,
        recipient: recipient
      )
      var reader = try payloadReader(
        control(command, requestID: operationID),
        operation: 14
      )
      let canonical = try reader.singleVector(
        maximum: KagemushaWireV1.maximumMintAuthorizationBytes
      )
      let authorization = try KagemushaNoritoV1.decodeMintAuthorizationShapeExact(canonical)
      guard authorization.statement.context.operationID == operationID else {
        throw authenticatedProviderInvalid("mint authorization operation ID mismatch")
      }
      return canonical
    }
  }

  /// Recover operation 14 byte-identically through operation 15.
  public func recoverMintAuthorization(operationID: Data) throws -> Data? {
    try locked {
      let call = try client.control(
        .recoverMintAuthorization(operationID: operationID),
        requestID: operationID
      )
      if call.status == .missing { return nil }
      var reader = try payloadReader(requireSuccess(call), operation: 15)
      let canonical = try reader.singleVector(
        maximum: KagemushaWireV1.maximumMintAuthorizationBytes
      )
      let authorization = try KagemushaNoritoV1.decodeMintAuthorizationShapeExact(canonical)
      guard authorization.statement.context.operationID == operationID else {
        throw authenticatedProviderInvalid("recovered mint authorization ID mismatch")
      }
      return canonical
    }
  }

  private func commitSender(
    inputs: KagemushaDeviceSenderPublicInputsV1
  ) throws -> KagemushaHardwareTerminalResultV1 {
    let preparation = try client.core.beginSenderTransition(
      inputs: inputs,
      qualification: qualification()
    )
    let operationID = preparation.operationID
    var prepared = try client.sender(
      KagemushaDeviceSenderCommandV1(
        operation: 5,
        operationID: operationID,
        context: preparation.context,
        body: .prepare(inputs: inputs)
      )
    )
    if prepared.status == .recoveryRequired || prepared.status == .staleOrConcurrent {
      prepared = try client.sender(
        KagemushaDeviceSenderCommandV1(
          operation: 6,
          operationID: operationID,
          context: preparation.context,
          body: .recoverPrepared(inputsDigest: preparation.inputsDigest)
        )
      )
    }
    try requireSuccess(prepared)
    guard let preparedReply = prepared.canonicalReply else {
      throw authenticatedProviderInvalid("prepared transition omitted its authenticated reply")
    }
    let candidate = try client.core.provePreparedSenderTransition(
      preparation: preparation,
      authenticatedPreparationReply: preparedReply
    )
    var committed = try client.sender(
      KagemushaDeviceSenderCommandV1(
        operation: 7,
        operationID: operationID,
        context: preparation.context,
        body: .commit(
          selector: candidate.selector,
          candidateDigest: candidate.candidateDigest
        )
      )
    )
    if committed.status == .recoveryRequired || committed.status == .staleOrConcurrent {
      committed = try client.sender(
        KagemushaDeviceSenderCommandV1(
          operation: 8,
          operationID: operationID,
          context: preparation.context,
          body: .recoverTerminal(inputsDigest: preparation.inputsDigest)
        )
      )
    }
    try requireSuccess(committed)
    guard let committedReply = committed.canonicalReply else {
      throw authenticatedProviderInvalid("committed transition omitted its authenticated reply")
    }
    let envelope = try client.core.terminalEnvelope(
      candidate: candidate,
      authenticatedCommitReply: committedReply
    )
    guard !envelope.isEmpty else {
      throw authenticatedProviderInvalid("native Core returned an empty terminal envelope")
    }
    let install = try requireSuccess(
      client.sender(
        KagemushaDeviceSenderCommandV1(
          operation: 9,
          operationID: operationID,
          context: preparation.context,
          body: .install(
            selector: candidate.selector,
            candidateDigest: candidate.candidateDigest,
            inputs: inputs,
            canonicalEnvelope: envelope
          )
        )
      )
    )
    let installed = try requireSuccess(
      client.sender(
        KagemushaDeviceSenderCommandV1(
          operation: 10,
          operationID: operationID,
          context: preparation.context,
          body: .recoverInstalled(
            selector: .lookup(inputsDigest: preparation.inputsDigest)
          )
        )
      )
    )
    let snapshot = try control(.recoverWalletSnapshot, requestID: freshID(operation: 21))
    guard let installReply = install.canonicalReply,
      let installedReply = installed.canonicalReply,
      let snapshotReply = snapshot.canonicalReply
    else {
      throw authenticatedProviderInvalid("terminal installation omitted an authenticated reply")
    }
    return try client.core.acceptInstalledTerminal(
      candidate: candidate,
      canonicalEnvelope: envelope,
      authenticatedInstallReply: installReply,
      authenticatedInstalledReply: installedReply,
      authenticatedWalletSnapshotReply: snapshotReply
    )
  }

  private func recoverTerminal(
    kind: KagemushaNativeSenderKindV1,
    terminalID: Data
  ) throws -> Data? {
    guard
      let recovery = try client.core.senderRecovery(
        kind: kind,
        terminalID: terminalID,
        qualification: qualification()
      )
    else { return nil }
    let call = try client.sender(
      KagemushaDeviceSenderCommandV1(
        operation: 10,
        operationID: recovery.operationID,
        context: recovery.context,
        body: .recoverInstalled(selector: .lookup(inputsDigest: recovery.inputsDigest))
      )
    )
    if call.status == .missing { return nil }
    try requireSuccess(call)
    guard let reply = call.canonicalReply else {
      throw authenticatedProviderInvalid("terminal recovery omitted its authenticated reply")
    }
    let envelope = try client.core.recoverTerminalEnvelope(
      recovery: recovery,
      authenticatedInstalledReply: reply
    )
    guard !envelope.isEmpty else {
      throw authenticatedProviderInvalid("native Core recovered an empty terminal envelope")
    }
    return envelope
  }

  private func control(
    _ command: KagemushaDeviceControlCommandV1,
    requestID: Data
  ) throws -> AuthenticatedCall {
    try requireSuccess(client.control(command, requestID: requestID))
  }

  private func freshID(operation: UInt8) throws -> Data {
    try client.core.reserveOperationID(operation: operation, publicBinding: Data([operation]))
  }

  private func locked<T>(_ body: () throws -> T) rethrows -> T {
    lock.lock()
    defer { lock.unlock() }
    return try body()
  }
}

@discardableResult
private func requireSuccess(_ call: AuthenticatedCall) throws -> AuthenticatedCall {
  guard call.status == .success else {
    throw KagemushaAuthenticatedHardwareProviderErrorV1.operationFailed(
      operation: call.operation,
      status: call.status
    )
  }
  return call
}

private func payloadReader(
  _ call: AuthenticatedCall,
  operation: UInt8
) throws -> AuthenticatedReplyReader {
  try requireSuccess(call)
  guard let reply = call.reply else {
    throw authenticatedProviderInvalid("operation \(operation) omitted its decoded reply")
  }
  var reader = AuthenticatedReplyReader(reply.payload)
  guard try reader.u16Field() == 1, try reader.u8Field() == operation else {
    throw authenticatedProviderInvalid("authenticated reply binding mismatch")
  }
  return reader
}

private func parseStagedReply(
  _ call: AuthenticatedCall,
  operation: UInt8,
  requestBytes: Data,
  paymentBytes: Data,
  payment: KagemushaPaymentV1
) throws -> KagemushaInboxReceiptV1 {
  var top = try payloadReader(call, operation: operation)
  guard !(try top.u128Field()).isZero else {
    throw authenticatedProviderInvalid("staged inbox revision is zero")
  }
  var record = AuthenticatedReplyReader(try top.field())
  try top.finish()
  guard
    try record.vectorField(maximum: KagemushaWireV1.maximumPaymentRequestBytes)
      == requestBytes,
    try record.vectorField(maximum: KagemushaWireV1.maximumPaymentBytes) == paymentBytes
  else { throw authenticatedProviderInvalid("staged payment exchange binding mismatch") }
  _ = try record.vectorField(maximum: 1_024)
  let receipt = try decodeInboxReceipt(record.field())
  try record.finish()
  guard receipt.creditID == payment.output.creditID else {
    throw authenticatedProviderInvalid("staged payment receipt credit ID mismatch")
  }
  return receipt
}

private func stagedRecordCreditID(_ payload: Data) throws -> Data {
  var record = AuthenticatedReplyReader(payload)
  _ = try record.vectorField(maximum: KagemushaWireV1.maximumPaymentRequestBytes)
  _ = try record.vectorField(maximum: KagemushaWireV1.maximumPaymentBytes)
  _ = try record.vectorField(maximum: 1_024)
  let creditID = try decodeInboxReceipt(record.field()).creditID
  try record.finish()
  return creditID
}

private func decodeInboxReceipt(_ payload: Data) throws -> KagemushaInboxReceiptV1 {
  var reader = AuthenticatedReplyReader(payload)
  let receipt = try KagemushaInboxReceiptV1(
    version: reader.u16Field(),
    creditID: reader.digestField(),
    receiptCommitment: reader.digestField()
  )
  try reader.finish()
  guard receipt.version == 1 else {
    throw authenticatedProviderInvalid("unsupported inbox receipt version")
  }
  return receipt
}

private struct AuthenticatedReplyReader {
  private let bytes: Data
  private var offset = 0

  init(_ bytes: Data) {
    self.bytes = Data(bytes)
  }

  mutating func field(maximum: Int? = nil) throws -> Data {
    let limit = maximum ?? bytes.count
    let length = try compactLength()
    guard length <= limit, length <= bytes.count - offset else {
      throw authenticatedProviderInvalid("authenticated reply field is oversized")
    }
    defer { offset += length }
    return Data(bytes[offset..<(offset + length)])
  }

  mutating func u8Field() throws -> UInt8 {
    let value = try field(maximum: 1)
    guard value.count == 1, let byte = value.first else {
      throw authenticatedProviderInvalid("malformed u8 reply field")
    }
    return byte
  }

  mutating func u16Field() throws -> UInt16 {
    let value = try field(maximum: 2)
    guard value.count == 2 else {
      throw authenticatedProviderInvalid("malformed u16 reply field")
    }
    return UInt16(value[0]) | UInt16(value[1]) << 8
  }

  mutating func u128Field() throws -> KagemushaUInt128V1 {
    let value = try field(maximum: 16)
    guard value.count == 16 else {
      throw authenticatedProviderInvalid("malformed u128 reply field")
    }
    return try KagemushaUInt128V1(littleEndianBytes: value)
  }

  mutating func vectorField(maximum: Int) throws -> Data {
    var nested = AuthenticatedReplyReader(try field(maximum: maximum + 8))
    let countBytes = try nested.raw(count: 8)
    var count: UInt64 = 0
    for index in 0..<8 {
      count |= UInt64(countBytes[index]) << UInt64(index * 8)
    }
    guard count <= UInt64(maximum) else {
      throw authenticatedProviderInvalid("authenticated reply vector is oversized")
    }
    let value = try nested.raw(count: Int(count))
    try nested.finish()
    return value
  }

  mutating func optionVector(maximum: Int) throws -> Data? {
    var nested = AuthenticatedReplyReader(try field(maximum: maximum + 10))
    let tag = try nested.raw(count: 1)[0]
    let value: Data?
    switch tag {
    case 0: value = nil
    case 1: value = try nested.vectorField(maximum: maximum)
    default: throw authenticatedProviderInvalid("invalid authenticated reply option tag")
    }
    try nested.finish()
    return value
  }

  mutating func singleVector(maximum: Int) throws -> Data {
    let value = try vectorField(maximum: maximum)
    try finish()
    return value
  }

  mutating func raw(count: Int) throws -> Data {
    guard count >= 0, count <= bytes.count - offset else {
      throw authenticatedProviderInvalid("truncated authenticated reply")
    }
    defer { offset += count }
    return Data(bytes[offset..<(offset + count)])
  }

  func finish() throws {
    guard offset == bytes.count else {
      throw authenticatedProviderInvalid("authenticated reply has trailing bytes")
    }
  }

  private mutating func compactLength() throws -> Int {
    var value: UInt64 = 0
    var shift: UInt64 = 0
    var count = 0
    while true {
      guard offset < bytes.count, count < 10 else {
        throw authenticatedProviderInvalid("invalid compact reply field length")
      }
      let byte = bytes[offset]
      offset += 1
      count += 1
      let payload = UInt64(byte & 0x7f)
      guard shift < 63 || payload == 0 else {
        throw authenticatedProviderInvalid("compact reply field length overflow")
      }
      value |= payload << shift
      if byte & 0x80 == 0 {
        guard count == 1 || payload != 0, value <= UInt64(Int.max) else {
          throw authenticatedProviderInvalid("non-minimal or oversized compact reply field length")
        }
        return Int(value)
      }
      shift += 7
    }
  }
}

private func authenticatedProviderDigest(_ value: Data, _ field: String) throws -> Data {
  guard value.count == 32, value.contains(where: { $0 != 0 }) else {
    throw authenticatedProviderInvalid("\(field) must be a non-zero digest")
  }
  return Data(value)
}

private func authenticatedProviderInvalid(
  _ reason: String
) -> KagemushaAuthenticatedHardwareProviderErrorV1 {
  .invalidContract(reason)
}

private func authenticatedProviderU16LE(_ value: UInt16) -> Data {
  Data([UInt8(truncatingIfNeeded: value), UInt8(truncatingIfNeeded: value >> 8)])
}

private func authenticatedProviderU64LE(_ value: UInt64) -> Data {
  Data((0..<8).map { UInt8(truncatingIfNeeded: value >> UInt64($0 * 8)) })
}

private func authenticatedProviderU64BE(_ value: UInt64) -> Data {
  Data((0..<8).reversed().map { UInt8(truncatingIfNeeded: value >> UInt64($0 * 8)) })
}

private func authenticatedProviderPeerEnvelopeDigest(_ canonicalPayment: Data) -> Data {
  let domain = Data("iroha:kagemusha:v1:peer-credit-envelope\0".utf8)
  var preimage = authenticatedProviderU64BE(UInt64(domain.count))
  preimage.append(domain)
  preimage.append(authenticatedProviderU64BE(UInt64(canonicalPayment.count)))
  preimage.append(canonicalPayment)
  return Data(SHA256.hash(data: preimage))
}

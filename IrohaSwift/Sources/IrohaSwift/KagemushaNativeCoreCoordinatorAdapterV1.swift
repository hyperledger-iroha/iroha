// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation

/// Typed wallet adapter over the exact native coordinator ABI.
///
/// Canonical projections are selectors for the native durable journal, never recovered
/// capabilities. Opening this adapter does not qualify a hardware provider or supply a
/// software monetary backend. Missing native authority continues to fail closed.
public final class KagemushaNativeCoreCoordinatorAdapterV1: KagemushaNativeCoreCoordinatorV1 {
  private let bridge: KagemushaCoreCoordinatorBridgeV1

  init(bridge: KagemushaCoreCoordinatorBridgeV1) { self.bridge = bridge }

  /// Open the process-owned native coordinator under its durable storage path.
  public static func open(storagePath: String) throws -> KagemushaNativeCoreCoordinatorAdapterV1 {
    try KagemushaNativeCoreCoordinatorAdapterV1(bridge: .open(storagePath: storagePath))
  }

  public func reserveOperationID(operation: UInt8, operationID: Data, publicBinding: Data) throws -> Data {
    try bridge.invoke(.reserveOperationID, fields: [u32(UInt32(operation)), operationID, publicBinding])[0]
  }

  public func acceptQualification(
    _ qualification: KagemushaHardwareQualificationV1, hardwarePolicyDigest: Data
  ) throws {
    try require(hardwarePolicyDigest == qualification.hardwarePolicyDigest, "qualification policy mismatch")
    _ = try bridge.invoke(.acceptQualification, fields: qualificationFields(qualification) + [hardwarePolicyDigest])
  }

  public func acceptAuthenticatedDeviceReply(
    operation: UInt8, requestID: Data, canonicalCommand: Data, canonicalReply: Data, responseAuthenticator: Data,
    qualification: KagemushaHardwareQualificationV1
  ) throws {
    _ = try bridge.invoke(.acceptAuthenticatedReply,
      fields: [u32(UInt32(operation)), requestID, canonicalCommand, canonicalReply, responseAuthenticator] + qualificationFields(qualification))
  }

  public func beginSenderTransition(
    operationID: Data, inputs: KagemushaDeviceSenderPublicInputsV1,
    qualification: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeSenderPreparationV1 {
    let response = try bridge.invoke(.beginSenderTransition,
      fields: [operationID] + inputFields(inputs) + qualificationFields(qualification))
    let preparation = try KagemushaCoreCoordinatorArchiveV1.decodePreparationShapeExact(response[1])
    try require(preparation.operationID == operationID, "preparation operation mismatch")
    try requireCurrentContext(preparation.context, qualification)
    try requireInputs(preparation, inputs)
    return preparation
  }

  public func provePreparedSenderTransition(
    preparation: KagemushaNativeSenderPreparationV1, authenticatedPreparationReply: Data
  ) throws -> KagemushaNativeSenderCandidateV1 {
    let response = try bridge.invoke(.provePreparedSenderTransition,
      fields: [KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(preparation), authenticatedPreparationReply])
    let candidate = try KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(response[0])
    try require(candidate.preparation == preparation, "candidate preparation mismatch")
    return candidate
  }

  public func terminalEnvelope(
    candidate: KagemushaNativeSenderCandidateV1, authenticatedCommitReply: Data
  ) throws -> Data {
    let envelope = try bridge.invoke(.buildTerminalEnvelope,
      fields: [KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(candidate), authenticatedCommitReply])[0]
    _ = try KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(envelope)
    return envelope
  }

  public func acceptInstalledTerminal(
    candidate: KagemushaNativeSenderCandidateV1, canonicalEnvelope: Data,
    authenticatedInstallReply: Data, authenticatedInstalledReply: Data,
    authenticatedWalletSnapshotReply: Data
  ) throws -> KagemushaHardwareTerminalResultV1 {
    let response = try bridge.invoke(.acceptInstalledTerminal,
      fields: [KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(candidate), canonicalEnvelope,
        authenticatedInstallReply, authenticatedInstalledReply, authenticatedWalletSnapshotReply])
    let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(response[1])
    try requireAggregateContext(state, candidate.preparation.context)
    return try KagemushaHardwareTerminalResultV1(canonicalEnvelope: response[0], aggregateState: response[1])
  }

  public func senderRecovery(
    kind: KagemushaNativeSenderKindV1, terminalID: Data,
    qualification: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeSenderRecoveryV1? {
    try recover(selector: 0, selectedID: terminalID, kind: kind, qualification: qualification)
  }

  public func senderRecoveryByOperationID(
    kind: KagemushaNativeSenderKindV1, operationID: Data,
    qualification: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeSenderRecoveryV1? {
    try recover(selector: 1, selectedID: operationID, kind: kind, qualification: qualification)
  }

  public func recoverTerminalEnvelope(
    recovery: KagemushaNativeSenderRecoveryV1, authenticatedInstalledReply: Data
  ) throws -> Data {
    let envelope = try bridge.invoke(.recoverTerminalEnvelope,
      fields: [KagemushaCoreCoordinatorArchiveV1.encodeRecoveryShape(recovery), authenticatedInstalledReply])[0]
    _ = try KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(envelope)
    return envelope
  }

  public func outboxRelease(
    creditID: Data, inputs: KagemushaDeviceSenderPublicInputsV1, canonicalPayment: Data,
    terminalReceipt: KagemushaDeviceSenderTerminalReceiptV1,
    qualification: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeOutboxReleaseV1 {
    let receipt = try receiptField(terminalReceipt, creditID: creditID, inputs: inputs, envelope: canonicalPayment)
    let response = try bridge.invoke(.releaseOutbox,
      fields: [creditID] + inputFields(inputs) + [canonicalPayment, receipt] + qualificationFields(qualification))
    let preparation = try KagemushaCoreCoordinatorArchiveV1.decodePreparationShapeExact(response[1])
    try require(preparation.operationID == response[0], "outbox operation mismatch")
    try requireRetainedContext(preparation.context, qualification)
    try requireInputs(preparation, inputs)
    let digest = try KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(canonicalPayment)
    try require(response[2] == digest, "outbox envelope digest mismatch")
    if case .redemptionSettlement(let receipt) = terminalReceipt {
      try require(receipt.operationID == preparation.operationID, "redemption receipt operation mismatch")
      try require(receipt.networkID == preparation.context.lane.networkID, "redemption receipt network mismatch")
    }
    // Outbox material retains its creation context across ordinary credential rotation.
    // The native backend authenticates that historical context and its release authorization.
    return try KagemushaNativeOutboxReleaseV1(
      operationID: preparation.operationID, context: preparation.context,
      inputsDigest: preparation.inputsDigest, envelopeDigest: digest, inputs: inputs,
      canonicalEnvelope: response[3], hardwareReleaseAuthorization: response[4])
  }

  private func recover(
    selector: UInt8, selectedID: Data, kind: KagemushaNativeSenderKindV1,
    qualification: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeSenderRecoveryV1? {
    let response = try bridge.invoke(.recoverSender,
      fields: [Data([selector]), selectedID, u32(kind == .payment ? 0 : 1)] + qualificationFields(qualification))
    if response.isEmpty { return nil }
    let recovery = try KagemushaCoreCoordinatorArchiveV1.decodeRecoveryShapeExact(response[2])
    try require(recovery.operationID == response[0] && recovery.terminalID == response[1], "recovery archive mismatch")
    try requireRetainedContext(recovery.context, qualification)
    // Recovery uses the authenticated original context; current qualification is not a
    // substitute for that historical journal authority and may name a later epoch.
    return recovery
  }

  private func qualificationFields(_ value: KagemushaHardwareQualificationV1) throws -> [Data] {
    try [u32(UInt32(value.profile.protocolVersion)), value.releaseID,
      KagemushaNoritoV1.encodeHardwareProfileShape(value.profile),
      KagemushaNoritoV1.encodeHardwareCredentialShape(value.credential),
      u32(UInt32(value.profile.capabilityMask))]
  }

  private func inputFields(_ value: KagemushaDeviceSenderPublicInputsV1) throws -> [Data] {
    switch value {
    case .sendSplit(let request):
      _ = try KagemushaNoritoV1.decodePaymentRequestShapeExact(request)
      return [u32(0), request]
    case .redeemSplit(let amount, let beneficiary):
      try require(!amount.isZero, "zero redemption amount")
      return [u32(1), amount.littleEndianBytes, beneficiary.canonicalPayload]
    }
  }

  private func receiptField(
    _ receipt: KagemushaDeviceSenderTerminalReceiptV1, creditID: Data,
    inputs: KagemushaDeviceSenderPublicInputsV1, envelope: Data
  ) throws -> Data {
    switch (inputs, receipt) {
    case (.sendSplit(let requestBytes), .paymentAcknowledgement(let bytes)):
      let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(requestBytes)
      let payment = try KagemushaNoritoV1.decodePaymentShapeExact(envelope, against: request)
      _ = try KagemushaNoritoV1.decodeAcknowledgementShapeExact(bytes, against: request, payment: payment)
      try require(payment.output.creditID == creditID, "payment terminal mismatch")
      return u32(0) + bytes
    case (.redeemSplit(let amount, let beneficiary), .redemptionSettlement(let receipt)):
      let voucher = try KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(envelope)
      let statement = voucher.statement
      try require(statement.amount == amount && statement.beneficiary == beneficiary
        && statement.redemptionID == creditID && receipt.redemptionID == creditID
        && receipt.networkID == statement.lifecycle.networkID
        && receipt.terminalNullifier == statement.terminalNullifier
        && receipt.envelopeDigest == KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(envelope),
        "redemption terminal mismatch")
      return try u32(1) + KagemushaCoreCoordinatorArchiveV1.encodeRedemptionTerminalReceiptShape(receipt)
    default:
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("terminal receipt kind mismatch")
    }
  }

  private func requireInputs(
    _ value: KagemushaNativeSenderPreparationV1, _ inputs: KagemushaDeviceSenderPublicInputsV1
  ) throws {
    try require(value.inputsDigest == KagemushaCoreCoordinatorArchiveV1.senderInputsDigestShape(
      operationID: value.operationID, context: value.context, inputs: inputs), "sender input digest mismatch")
  }

  private func requireCurrentContext(
    _ context: KagemushaDeviceSenderWalletContextV1, _ qualification: KagemushaHardwareQualificationV1
  ) throws {
    let credential = qualification.credential
    try require(context.release.protocolVersion == qualification.profile.protocolVersion
      && context.release.releaseID == qualification.releaseID
      && context.release.suiteID == credential.suiteID
      && context.release.hardwareProfileID == credential.hardwareProfileID
      && context.release.policyEpoch == credential.policyEpoch
      && context.lane.networkID == credential.networkID
      && context.lane.deviceLaneID == credential.laneCommitment
      && context.credentialID == credential.credentialID
      && context.hardwareEpoch.epochID == credential.hardwareEpochID
      && context.hardwareEpoch.generation == KagemushaUInt128V1(credential.hardwareEpochGeneration)
      && context.devicePolicyBinding.deviceKeyReference == credential.deviceKeyReference
      && context.devicePolicyBinding.hardwarePolicyID == qualification.hardwarePolicyDigest
      && context.coreAuthorizationKeyReference == qualification.coreAuthorizationKeyReference,
      "preparation qualification mismatch")
  }

  private func requireRetainedContext(
    _ context: KagemushaDeviceSenderWalletContextV1, _ qualification: KagemushaHardwareQualificationV1
  ) throws {
    let credential = qualification.credential
    let generation = KagemushaUInt128V1(credential.hardwareEpochGeneration)
    try require(context.lane.networkID == credential.networkID
      && context.lane.deviceLaneID == credential.laneCommitment
      && context.hardwareEpoch.generation.isLessThanOrEqual(to: generation)
      && (context.hardwareEpoch.generation != generation || context.hardwareEpoch.epochID == credential.hardwareEpochID),
      "retained context scope mismatch")
  }

  private func requireAggregateContext(
    _ state: KagemushaAggregateStateCommitmentV1, _ context: KagemushaDeviceSenderWalletContextV1
  ) throws {
    try require(state.releaseID == context.release.releaseID && state.networkID == context.lane.networkID
      && state.asset == context.lane.asset && state.assetIncarnation == context.release.assetIncarnation
      && state.scale == context.lane.scale && state.laneID == context.lane.deviceLaneID
      && state.hardwareEpochID == context.hardwareEpoch.epochID
      && state.keyReference == context.devicePolicyBinding.deviceKeyReference
      && state.hardwarePolicyID == context.devicePolicyBinding.hardwarePolicyID
      && state.liabilityPoolID == KagemushaNoritoV1.liabilityPoolID(
        networkID: context.lane.networkID, asset: context.lane.asset, incarnation: context.release.assetIncarnation),
      "installed aggregate context mismatch")
  }

  private func u32(_ value: UInt32) -> Data { KagemushaCoreCoordinatorFrameV1.u32(value) }

  private func require(_ condition: Bool, _ message: String) throws {
    guard condition else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame(message) }
  }
}

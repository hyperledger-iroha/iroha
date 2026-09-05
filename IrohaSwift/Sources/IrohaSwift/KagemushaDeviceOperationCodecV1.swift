// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation

/// A canonical, public-input-only receiver command for ABI-23 operations 2 through 4.
public enum KagemushaDeviceReceiverCommandV1: Equatable, Sendable {
  case stage(
    canonicalRequest: Data,
    canonicalPayment: Data,
    stagingMetadata: Data
  )
  case recoverStaged(creditID: Data)
  case page(
    snapshotRevision: KagemushaUInt128V1?,
    after: Data?,
    maximumEntries: UInt16
  )

  public var operation: UInt8 {
    switch self {
    case .stage: 2
    case .recoverStaged: 3
    case .page: 4
    }
  }
}

/// A canonical, public-input-only control command for ABI-23 operations outside the sender lane.
public enum KagemushaDeviceControlCommandV1: Equatable, Sendable {
  case readActiveHardwareCredential
  case signReceiveAcknowledgement(
    canonicalRequest: Data,
    canonicalPayment: Data,
    inboxReceipt: KagemushaInboxReceiptV1
  )
  case readTrustedTimeOrLease
  case prepareMintAuthorization(
    operationID: Data,
    amount: KagemushaUInt128V1,
    payer: KagemushaAccountIDV1,
    recipient: KagemushaAccountIDV1
  )
  case recoverMintAuthorization(operationID: Data)
  case foldReceiveCredit(operationID: Data, creditID: Data)
  case readPendingCreditWatermark
  case rotateHardwareEpoch(operationID: Data)
  case bootstrapAggregateState(operationID: Data)
  case recoverWalletSnapshot
  case createSignedPaymentRequest(
    requestID: Data,
    recipient: KagemushaAccountIDV1,
    amount: KagemushaUInt128V1,
    validityWindowMS: UInt64
  )

  public var operation: UInt8 {
    switch self {
    case .readActiveHardwareCredential: 1
    case .signReceiveAcknowledgement: 11
    case .readTrustedTimeOrLease: 13
    case .prepareMintAuthorization: 14
    case .recoverMintAuthorization: 15
    case .foldReceiveCredit: 17
    case .readPendingCreditWatermark: 18
    case .rotateHardwareEpoch: 19
    case .bootstrapAggregateState: 20
    case .recoverWalletSnapshot: 21
    case .createSignedPaymentRequest: 22
    }
  }
}

/// Stable network, device-lane, asset and scale identity carried by every sender command.
public struct KagemushaDeviceLaneIDV1: Equatable, Sendable {
  public let networkID: Data
  public let deviceLaneID: Data
  public let asset: KagemushaAssetDefinitionIDV1
  public let scale: UInt32

  public init(
    networkID: Data,
    deviceLaneID: Data,
    asset: KagemushaAssetDefinitionIDV1,
    scale: UInt32
  ) throws {
    self.networkID = try deviceDigest(networkID, "networkID")
    self.deviceLaneID = try deviceDigest(deviceLaneID, "deviceLaneID")
    guard scale <= 28 else { throw deviceInvalid("scale") }
    self.asset = asset
    self.scale = scale
  }
}

/// Proof release and asset-incarnation scope carried by the authenticated native wallet context.
public struct KagemushaDeviceStateContextV1: Equatable, Sendable {
  public let protocolVersion: UInt16
  public let suiteID: Data
  public let vkDigest: Data
  public let releaseID: Data
  public let assetIncarnation: KagemushaAssetIncarnationV1
  public let hardwareProfileID: Data
  public let policyEpoch: UInt64

  public init(
    protocolVersion: UInt16 = 1,
    suiteID: Data,
    vkDigest: Data,
    releaseID: Data,
    assetIncarnation: KagemushaAssetIncarnationV1,
    hardwareProfileID: Data,
    policyEpoch: UInt64
  ) throws {
    guard protocolVersion == 1, policyEpoch != 0 else {
      throw deviceInvalid("stateContext")
    }
    self.protocolVersion = protocolVersion
    self.suiteID = try deviceDigest(suiteID, "suiteID")
    self.vkDigest = try deviceDigest(vkDigest, "vkDigest")
    self.releaseID = try deviceDigest(releaseID, "releaseID")
    self.assetIncarnation = assetIncarnation
    self.hardwareProfileID = try deviceDigest(hardwareProfileID, "hardwareProfileID")
    self.policyEpoch = policyEpoch
  }
}

/// Full-width qualified hardware generation and its exact epoch identity.
public struct KagemushaDeviceHardwareEpochV1: Equatable, Sendable {
  public let generation: KagemushaUInt128V1
  public let epochID: Data

  public init(generation: KagemushaUInt128V1, epochID: Data) throws {
    guard !generation.isZero else { throw deviceInvalid("hardwareEpoch.generation") }
    self.generation = generation
    self.epochID = try deviceDigest(epochID, "hardwareEpoch.epochID")
  }
}

/// Exact native key reference and governed hardware policy bound to an epoch.
public struct KagemushaDevicePolicyBindingV1: Equatable, Sendable {
  public let deviceKeyReference: Data
  public let hardwarePolicyID: Data

  public init(deviceKeyReference: Data, hardwarePolicyID: Data) throws {
    self.deviceKeyReference = try deviceDigest(deviceKeyReference, "deviceKeyReference")
    self.hardwarePolicyID = try deviceDigest(hardwarePolicyID, "hardwarePolicyID")
  }
}

/// Complete immutable public sender-wallet context. Construction does not authenticate it.
public struct KagemushaDeviceSenderWalletContextV1: Equatable, Sendable {
  public let lane: KagemushaDeviceLaneIDV1
  public let release: KagemushaDeviceStateContextV1
  public let credentialID: Data
  public let hardwareEpoch: KagemushaDeviceHardwareEpochV1
  public let devicePolicyBinding: KagemushaDevicePolicyBindingV1

  public init(
    lane: KagemushaDeviceLaneIDV1,
    release: KagemushaDeviceStateContextV1,
    credentialID: Data,
    hardwareEpoch: KagemushaDeviceHardwareEpochV1,
    devicePolicyBinding: KagemushaDevicePolicyBindingV1
  ) throws {
    self.lane = lane
    self.release = release
    self.credentialID = try deviceDigest(credentialID, "credentialID")
    self.hardwareEpoch = hardwareEpoch
    self.devicePolicyBinding = devicePolicyBinding
  }
}

/// Original public inputs fixed before native preparation. Case order is the Rust wire order.
public enum KagemushaDeviceSenderPublicInputsV1: Equatable, Sendable {
  case sendSplit(canonicalRequest: Data)
  case redeemSplit(amount: KagemushaUInt128V1, beneficiary: KagemushaAccountIDV1)
}

/// Exact retained native preparation selector.
public struct KagemushaDeviceSenderPreparationSelectorV1: Equatable, Sendable {
  public let inputsDigest: Data
  public let preparationID: Data

  public init(inputsDigest: Data, preparationID: Data) throws {
    self.inputsDigest = try deviceDigest(inputsDigest, "inputsDigest")
    self.preparationID = try deviceDigest(preparationID, "preparationID")
  }
}

/// Tombstone-aware lookup or stable-revision page selector. Case order is the Rust wire order.
public enum KagemushaDeviceSenderRecoverySelectorV1: Equatable, Sendable {
  case lookup(inputsDigest: Data)
  case page(
    snapshotRevision: KagemushaUInt128V1?,
    after: Data?,
    maximumEntries: UInt16
  )
}

/// Sender command body. Case ordinal and ABI operation code are intentionally distinct.
public enum KagemushaDeviceSenderCommandBodyV1: Equatable, Sendable {
  case prepare(inputs: KagemushaDeviceSenderPublicInputsV1)
  case recoverPrepared(inputsDigest: Data)
  case commit(selector: KagemushaDeviceSenderPreparationSelectorV1, candidateDigest: Data)
  case recoverTerminal(inputsDigest: Data)
  case install(
    selector: KagemushaDeviceSenderPreparationSelectorV1,
    candidateDigest: Data,
    inputs: KagemushaDeviceSenderPublicInputsV1,
    canonicalEnvelope: Data
  )
  case recoverInstalled(selector: KagemushaDeviceSenderRecoverySelectorV1)
  case release(
    inputsDigest: Data,
    envelopeDigest: Data,
    inputs: KagemushaDeviceSenderPublicInputsV1,
    canonicalEnvelope: Data,
    canonicalAcknowledgement: Data
  )

  public var operation: UInt8 {
    switch self {
    case .prepare: 5
    case .recoverPrepared: 6
    case .commit: 7
    case .recoverTerminal: 8
    case .install: 9
    case .recoverInstalled: 10
    case .release: 12
    }
  }

  fileprivate var ordinal: UInt32 {
    switch self {
    case .prepare: 0
    case .recoverPrepared: 1
    case .commit: 2
    case .recoverTerminal: 3
    case .install: 4
    case .recoverInstalled: 5
    case .release: 6
    }
  }
}

/// One exact sender command shared by ABI-23 operations 5 through 10 and 12.
public struct KagemushaDeviceSenderCommandV1: Equatable, Sendable {
  public let version: UInt16
  public let operation: UInt8
  public let operationID: Data
  public let context: KagemushaDeviceSenderWalletContextV1
  public let body: KagemushaDeviceSenderCommandBodyV1

  public init(
    version: UInt16 = 1,
    operation: UInt8,
    operationID: Data,
    context: KagemushaDeviceSenderWalletContextV1,
    body: KagemushaDeviceSenderCommandBodyV1
  ) throws {
    guard version == 1, operation == body.operation else {
      throw deviceInvalid("senderCommand.operation")
    }
    self.version = version
    self.operation = operation
    self.operationID = try deviceDigest(operationID, "operationID")
    self.context = context
    self.body = body
  }
}

/// Canonical Norito body codecs for secure-device ABI-23 operations.
///
/// These methods validate public byte shape only. Reply parsing is kept internal until the
/// native P-256 response-authenticator verifier has admitted the complete response transcript.
public enum KagemushaDeviceOperationCodecV1 {
  public static let controlReadCommandMaximumBytes = 256
  public static let controlAcknowledgementCommandMaximumBytes = 12 * 1024
  public static let controlMintCommandMaximumBytes = 2 * 1024
  public static let controlFoldCommandMaximumBytes = 256
  public static let controlPaymentRequestCommandMaximumBytes = 2 * 1024
  public static let receiverStageCommandMaximumBytes = 16 * 1024
  public static let receiverRecoveryCommandMaximumBytes = 512
  public static let senderCommandMaximumBytes = 16 * 1024
  public static let senderReplyMaximumBytes = 64 * 1024

  public static func encodeReceiverCommand(
    _ value: KagemushaDeviceReceiverCommandV1
  ) throws -> Data {
    let payload: Data
    switch value {
    case .stage(let request, let payment, let metadata):
      guard metadata.count <= inboxStagingMetadataMaximum else {
        throw deviceInvalid("stagingMetadata.size")
      }
      payload = deviceFields([
        deviceU16(1), Data([value.operation]),
        try deviceVector(try bounded(request, paymentRequestMaximum, "request")),
        try deviceVector(try bounded(payment, paymentMaximum, "payment")),
        try deviceVector(metadata),
      ])
    case .recoverStaged(let creditID):
      payload = deviceFields([
        deviceU16(1), Data([value.operation]),
        try deviceDigest(creditID, "creditID"),
      ])
    case .page(let revision, let after, let maximumEntries):
      guard (1...receiverPageCountMaximum).contains(maximumEntries),
        after == nil || revision != nil
      else {
        throw deviceInvalid("maximumEntries")
      }
      payload = deviceFields([
        deviceU16(1), Data([value.operation]),
        try deviceOption(revision?.littleEndianBytes),
        try deviceOption(after.map { try deviceDigest($0, "after") }),
        deviceU16(maximumEntries),
      ])
    }
    let descriptor = try receiverCommandDescriptor(value.operation)
    return try deviceFrame(payload, descriptor)
  }

  public static func decodeReceiverCommand(
    operation: UInt8,
    requestID: Data,
    canonicalBytes: Data
  ) throws -> KagemushaDeviceReceiverCommandV1 {
    let expectedID = try deviceDigest(requestID, "requestID")
    let descriptor = try receiverCommandDescriptor(operation)
    var reader = DeviceOperationReader(try deviceUnframe(canonicalBytes, descriptor))
    guard try reader.u16Field() == 1, try reader.u8Field() == operation else {
      throw deviceInvalid("receiverCommand.binding")
    }
    let value: KagemushaDeviceReceiverCommandV1
    switch operation {
    case 2:
      value = .stage(
        canonicalRequest: try reader.byteVectorField(maximum: paymentRequestMaximum),
        canonicalPayment: try reader.byteVectorField(maximum: paymentMaximum),
        stagingMetadata: try reader.byteVectorField(
          maximum: inboxStagingMetadataMaximum,
          allowEmpty: true)
      )
    case 3:
      value = .recoverStaged(creditID: try reader.digestField())
    case 4:
      value = .page(
        snapshotRevision: try reader.optionField { try $0.u128Raw() },
        after: try reader.optionField { try $0.digestRaw() },
        maximumEntries: try reader.u16Field()
      )
    default:
      throw deviceInvalid("receiverCommand.operation")
    }
    try reader.finish()
    switch value {
    case .stage(let requestBytes, let paymentBytes, _):
      let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(requestBytes)
      let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
        paymentBytes,
        against: request)
      guard payment.output.creditID == expectedID else {
        throw deviceInvalid("payment.creditID does not match requestID")
      }
    case .recoverStaged(let creditID):
      guard creditID == expectedID else {
        throw deviceInvalid("creditID does not match requestID")
      }
    case .page:
      break
    }
    guard try encodeReceiverCommand(value) == canonicalBytes else {
      throw deviceInvalid("receiverCommand.canonical")
    }
    return value
  }

  public static func encodeControlCommand(
    _ value: KagemushaDeviceControlCommandV1
  ) throws -> Data {
    let payload: Data
    switch value {
    case .readActiveHardwareCredential, .readTrustedTimeOrLease,
      .readPendingCreditWatermark, .recoverWalletSnapshot:
      payload = deviceFields([deviceU16(1), Data([value.operation])])
    case .signReceiveAcknowledgement(let request, let payment, let receipt):
      payload = deviceFields([
        deviceU16(1), Data([value.operation]),
        try deviceVector(try bounded(request, paymentRequestMaximum, "request")),
        try deviceVector(try bounded(payment, paymentMaximum, "payment")),
        try encodeInboxReceipt(receipt),
      ])
    case .prepareMintAuthorization(let operationID, let amount, let payer, let recipient):
      guard !amount.isZero else { throw deviceInvalid("amount") }
      payload = deviceFields([
        deviceU16(1), Data([value.operation]), try deviceDigest(operationID, "operationID"),
        amount.littleEndianBytes, payer.canonicalPayload, recipient.canonicalPayload,
      ])
    case .recoverMintAuthorization(let operationID), .rotateHardwareEpoch(let operationID),
      .bootstrapAggregateState(let operationID):
      payload = deviceFields([deviceU16(1), Data([value.operation]), try deviceDigest(operationID, "operationID")])
    case .foldReceiveCredit(let operationID, let creditID):
      payload = deviceFields([
        deviceU16(1), Data([value.operation]), try deviceDigest(operationID, "operationID"),
        try deviceDigest(creditID, "creditID"),
      ])
    case .createSignedPaymentRequest(let requestID, let recipient, let amount, let window):
      guard !amount.isZero,
        (1...KagemushaWireV1.requestMaximumTTLMS).contains(window)
      else {
        throw deviceInvalid("validityWindowMS")
      }
      payload = deviceFields([
        deviceU16(1), Data([value.operation]), try deviceDigest(requestID, "requestID"),
        recipient.canonicalPayload, amount.littleEndianBytes, deviceU64(window),
      ])
    }
    return try deviceFrame(payload, controlCommandDescriptor(value.operation))
  }

  public static func decodeControlCommand(
    operation: UInt8,
    requestID: Data,
    canonicalBytes: Data
  ) throws -> KagemushaDeviceControlCommandV1 {
    let expectedID = try deviceDigest(requestID, "requestID")
    let descriptor = try controlCommandDescriptor(operation)
    var reader = DeviceOperationReader(try deviceUnframe(canonicalBytes, descriptor))
    guard try reader.u16Field() == 1, try reader.u8Field() == operation else {
      throw deviceInvalid("controlCommand.binding")
    }
    let value: KagemushaDeviceControlCommandV1
    switch operation {
    case 1: value = .readActiveHardwareCredential
    case 11:
      value = .signReceiveAcknowledgement(
        canonicalRequest: try reader.byteVectorField(maximum: paymentRequestMaximum),
        canonicalPayment: try reader.byteVectorField(maximum: paymentMaximum),
        inboxReceipt: try decodeInboxReceipt(reader.field())
      )
    case 13: value = .readTrustedTimeOrLease
    case 14:
      value = .prepareMintAuthorization(
        operationID: try reader.digestField(), amount: try reader.u128Field(),
        payer: try KagemushaAccountIDV1(canonicalPayload: reader.field(maximum: accountMaximum)),
        recipient: try KagemushaAccountIDV1(canonicalPayload: reader.field(maximum: accountMaximum))
      )
    case 15: value = .recoverMintAuthorization(operationID: try reader.digestField())
    case 17:
      value = .foldReceiveCredit(
        operationID: try reader.digestField(),
        creditID: try reader.digestField()
      )
    case 18: value = .readPendingCreditWatermark
    case 19: value = .rotateHardwareEpoch(operationID: try reader.digestField())
    case 20: value = .bootstrapAggregateState(operationID: try reader.digestField())
    case 21: value = .recoverWalletSnapshot
    case 22:
      let requestID = try reader.digestField()
      let recipient = try KagemushaAccountIDV1(
        canonicalPayload: reader.field(maximum: accountMaximum)
      )
      let amount = try reader.u128Field()
      let window = try reader.u64Field()
      guard !amount.isZero,
        (1...KagemushaWireV1.requestMaximumTTLMS).contains(window)
      else {
        throw deviceInvalid("validityWindowMS")
      }
      value = .createSignedPaymentRequest(
        requestID: requestID, recipient: recipient,
        amount: amount, validityWindowMS: window
      )
    default: throw deviceInvalid("controlCommand.operation")
    }
    try reader.finish()
    switch value {
    case .recoverMintAuthorization(let id), .rotateHardwareEpoch(let id),
      .bootstrapAggregateState(let id):
      guard id == expectedID else { throw deviceInvalid("controlCommand.requestID") }
    case .signReceiveAcknowledgement(_, _, let receipt):
      guard receipt.creditID == expectedID else { throw deviceInvalid("controlCommand.requestID") }
    case .prepareMintAuthorization(let id, _, _, _), .foldReceiveCredit(let id, _):
      guard id == expectedID else { throw deviceInvalid("controlCommand.requestID") }
    case .createSignedPaymentRequest(let id, _, _, _):
      guard id == expectedID else { throw deviceInvalid("controlCommand.requestID") }
    default: break
    }
    guard try encodeControlCommand(value) == canonicalBytes else {
      throw deviceInvalid("controlCommand.canonical")
    }
    return value
  }

  public static func encodeSenderCommand(
    _ value: KagemushaDeviceSenderCommandV1
  ) throws -> Data {
    guard value.version == 1, value.operation == value.body.operation else {
      throw deviceInvalid("senderCommand.binding")
    }
    let payload = deviceFields([
      deviceU16(value.version), Data([value.operation]),
      try deviceDigest(value.operationID, "operationID"),
      try encodeSenderContext(value.context),
      try encodeSenderBody(value.body),
    ])
    return try deviceFrame(
      payload,
      DeviceOperationArchiveDescriptor(
        schema: senderCommandSchema, alignment: 16, maximum: senderCommandMaximumBytes
      )
    )
  }

  public static func decodeSenderCommand(
    operation: UInt8,
    requestID: Data,
    canonicalBytes: Data
  ) throws -> KagemushaDeviceSenderCommandV1 {
    let descriptor = DeviceOperationArchiveDescriptor(
      schema: senderCommandSchema, alignment: 16, maximum: senderCommandMaximumBytes
    )
    var reader = DeviceOperationReader(try deviceUnframe(canonicalBytes, descriptor))
    let version = try reader.u16Field()
    let carriedOperation = try reader.u8Field()
    let operationID = try reader.digestField()
    let context = try decodeSenderContext(reader.field())
    let body = try decodeSenderBody(reader.field())
    try reader.finish()
    let expectedID = try deviceDigest(requestID, "requestID")
    guard version == 1, carriedOperation == operation, body.operation == operation,
      operationID == expectedID
    else { throw deviceInvalid("senderCommand.binding") }
    let value = try KagemushaDeviceSenderCommandV1(
      version: version, operation: operation, operationID: operationID,
      context: context, body: body
    )
    guard try encodeSenderCommand(value) == canonicalBytes else {
      throw deviceInvalid("senderCommand.canonical")
    }
    return value
  }
}

extension KagemushaDeviceOperationCodecV1 {
  fileprivate static func encodeSenderContext(
    _ value: KagemushaDeviceSenderWalletContextV1
  ) throws -> Data {
    deviceFields([
      deviceFields([
        value.lane.networkID, value.lane.deviceLaneID,
        value.lane.asset.canonicalPayload, deviceU32(value.lane.scale),
      ]),
      deviceFields([
        deviceU16(value.release.protocolVersion), value.release.suiteID,
        value.release.vkDigest, value.release.releaseID,
        deviceFields([value.release.assetIncarnation.bytes]),
        value.release.hardwareProfileID, deviceU64(value.release.policyEpoch),
      ]),
      value.credentialID,
      deviceFields([value.hardwareEpoch.generation.littleEndianBytes, value.hardwareEpoch.epochID]),
      deviceFields([
        value.devicePolicyBinding.deviceKeyReference,
        value.devicePolicyBinding.hardwarePolicyID,
      ]),
    ])
  }

  fileprivate static func decodeSenderContext(
    _ payload: Data
  ) throws -> KagemushaDeviceSenderWalletContextV1 {
    var reader = DeviceOperationReader(payload)
    var laneReader = DeviceOperationReader(try reader.field())
    let lane = try KagemushaDeviceLaneIDV1(
      networkID: laneReader.digestField(),
      deviceLaneID: laneReader.digestField(),
      asset: KagemushaAssetDefinitionIDV1(
        canonicalPayload: laneReader.field(maximum: accountMaximum)
      ),
      scale: laneReader.u32Field()
    )
    try laneReader.finish()

    var releaseReader = DeviceOperationReader(try reader.field())
    let protocolVersion = try releaseReader.u16Field()
    let suiteID = try releaseReader.digestField()
    let vkDigest = try releaseReader.digestField()
    let releaseID = try releaseReader.digestField()
    var incarnationReader = DeviceOperationReader(try releaseReader.field())
    let incarnation = try KagemushaAssetIncarnationV1(bytes: incarnationReader.exactField(32))
    try incarnationReader.finish()
    let release = try KagemushaDeviceStateContextV1(
      protocolVersion: protocolVersion,
      suiteID: suiteID,
      vkDigest: vkDigest,
      releaseID: releaseID,
      assetIncarnation: incarnation,
      hardwareProfileID: releaseReader.digestField(),
      policyEpoch: releaseReader.u64Field()
    )
    try releaseReader.finish()

    let credentialID = try reader.digestField()
    var epochReader = DeviceOperationReader(try reader.field())
    let epoch = try KagemushaDeviceHardwareEpochV1(
      generation: epochReader.u128Field(), epochID: epochReader.digestField()
    )
    try epochReader.finish()
    var policyReader = DeviceOperationReader(try reader.field())
    let policy = try KagemushaDevicePolicyBindingV1(
      deviceKeyReference: policyReader.digestField(),
      hardwarePolicyID: policyReader.digestField()
    )
    try policyReader.finish()
    try reader.finish()
    return try KagemushaDeviceSenderWalletContextV1(
      lane: lane, release: release, credentialID: credentialID,
      hardwareEpoch: epoch, devicePolicyBinding: policy
    )
  }

  fileprivate static func encodeSenderInputs(
    _ value: KagemushaDeviceSenderPublicInputsV1
  ) throws -> Data {
    switch value {
    case .sendSplit(let request):
      return deviceEnum(0, [
        try deviceVector(try bounded(request, paymentRequestMaximum, "request")),
      ])
    case .redeemSplit(let amount, let beneficiary):
      guard !amount.isZero else { throw deviceInvalid("redeem.amount") }
      return deviceEnum(1, [amount.littleEndianBytes, beneficiary.canonicalPayload])
    }
  }

  fileprivate static func decodeSenderInputs(
    _ payload: Data
  ) throws -> KagemushaDeviceSenderPublicInputsV1 {
    var reader = DeviceOperationReader(payload)
    let value: KagemushaDeviceSenderPublicInputsV1
    switch try reader.enumTag(variants: 2) {
    case 0:
      value = .sendSplit(
        canonicalRequest: try reader.byteVectorField(maximum: paymentRequestMaximum)
      )
    case 1:
      let amount = try reader.u128Field()
      guard !amount.isZero else { throw deviceInvalid("redeem.amount") }
      value = .redeemSplit(
        amount: amount,
        beneficiary: try KagemushaAccountIDV1(
          canonicalPayload: reader.field(maximum: accountMaximum)
        )
      )
    default: throw deviceInvalid("senderInputs.tag")
    }
    try reader.finish()
    return value
  }

  fileprivate static func encodePreparationSelector(
    _ value: KagemushaDeviceSenderPreparationSelectorV1
  ) throws -> Data {
    deviceFields([
      try deviceDigest(value.inputsDigest, "inputsDigest"),
      try deviceDigest(value.preparationID, "preparationID"),
    ])
  }

  fileprivate static func decodePreparationSelector(
    _ payload: Data
  ) throws -> KagemushaDeviceSenderPreparationSelectorV1 {
    var reader = DeviceOperationReader(payload)
    let value = try KagemushaDeviceSenderPreparationSelectorV1(
      inputsDigest: reader.digestField(), preparationID: reader.digestField()
    )
    try reader.finish()
    return value
  }

  fileprivate static func encodeRecoverySelector(
    _ value: KagemushaDeviceSenderRecoverySelectorV1
  ) throws -> Data {
    switch value {
    case .lookup(let inputsDigest):
      return deviceEnum(0, [try deviceDigest(inputsDigest, "inputsDigest")])
    case .page(let revision, let after, let maximumEntries):
      guard (1...senderPageCountMaximum).contains(maximumEntries),
        after == nil || revision != nil
      else { throw deviceInvalid("senderRecovery.page") }
      return deviceEnum(1, [
        try deviceOption(revision?.littleEndianBytes),
        try deviceOption(after.map { try deviceDigest($0, "after") }),
        deviceU16(maximumEntries),
      ])
    }
  }

  fileprivate static func decodeRecoverySelector(
    _ payload: Data
  ) throws -> KagemushaDeviceSenderRecoverySelectorV1 {
    var reader = DeviceOperationReader(payload)
    let value: KagemushaDeviceSenderRecoverySelectorV1
    switch try reader.enumTag(variants: 2) {
    case 0: value = .lookup(inputsDigest: try reader.digestField())
    case 1:
      let revision = try reader.optionField { try $0.u128Raw() }
      let after = try reader.optionField { try $0.digestRaw() }
      let maximumEntries = try reader.u16Field()
      guard (1...senderPageCountMaximum).contains(maximumEntries),
        after == nil || revision != nil
      else { throw deviceInvalid("senderRecovery.page") }
      value = .page(
        snapshotRevision: revision, after: after, maximumEntries: maximumEntries
      )
    default: throw deviceInvalid("senderRecovery.tag")
    }
    try reader.finish()
    return value
  }

  fileprivate static func encodeSenderBody(
    _ value: KagemushaDeviceSenderCommandBodyV1
  ) throws -> Data {
    let fields: [Data]
    switch value {
    case .prepare(let inputs):
      fields = [try encodeSenderInputs(inputs)]
    case .recoverPrepared(let digest), .recoverTerminal(let digest):
      fields = [try deviceDigest(digest, "inputsDigest")]
    case .commit(let selector, let candidateDigest):
      fields = [
        try encodePreparationSelector(selector),
        try deviceDigest(candidateDigest, "candidateDigest"),
      ]
    case .install(let selector, let candidateDigest, let inputs, let envelope):
      fields = [
        try encodePreparationSelector(selector),
        try deviceDigest(candidateDigest, "candidateDigest"),
        try encodeSenderInputs(inputs),
        try deviceVector(try bounded(envelope, terminalEnvelopeMaximum, "envelope")),
      ]
    case .recoverInstalled(let selector):
      fields = [try encodeRecoverySelector(selector)]
    case .release(let inputsDigest, let envelopeDigest, let inputs, let envelope, let acknowledgement):
      fields = [
        try deviceDigest(inputsDigest, "inputsDigest"),
        try deviceDigest(envelopeDigest, "envelopeDigest"),
        try encodeSenderInputs(inputs),
        try deviceVector(try bounded(envelope, terminalEnvelopeMaximum, "envelope")),
        try deviceVector(try bounded(acknowledgement, acknowledgementMaximum, "acknowledgement")),
      ]
    }
    return deviceEnum(value.ordinal, fields)
  }

  fileprivate static func decodeSenderBody(
    _ payload: Data
  ) throws -> KagemushaDeviceSenderCommandBodyV1 {
    var reader = DeviceOperationReader(payload)
    let value: KagemushaDeviceSenderCommandBodyV1
    switch try reader.enumTag(variants: 7) {
    case 0: value = .prepare(inputs: try decodeSenderInputs(reader.field()))
    case 1: value = .recoverPrepared(inputsDigest: try reader.digestField())
    case 2:
      value = .commit(
        selector: try decodePreparationSelector(reader.field()),
        candidateDigest: try reader.digestField()
      )
    case 3: value = .recoverTerminal(inputsDigest: try reader.digestField())
    case 4:
      value = .install(
        selector: try decodePreparationSelector(reader.field()),
        candidateDigest: try reader.digestField(),
        inputs: try decodeSenderInputs(reader.field()),
        canonicalEnvelope: try reader.byteVectorField(maximum: terminalEnvelopeMaximum)
      )
    case 5:
      value = .recoverInstalled(selector: try decodeRecoverySelector(reader.field()))
    case 6:
      value = .release(
        inputsDigest: try reader.digestField(), envelopeDigest: try reader.digestField(),
        inputs: try decodeSenderInputs(reader.field()),
        canonicalEnvelope: try reader.byteVectorField(maximum: terminalEnvelopeMaximum),
        canonicalAcknowledgement: try reader.byteVectorField(maximum: acknowledgementMaximum)
      )
    default: throw deviceInvalid("senderBody.tag")
    }
    try reader.finish()
    return value
  }

  fileprivate static func encodeInboxReceipt(_ value: KagemushaInboxReceiptV1) throws -> Data {
    guard value.version == 1 else { throw deviceInvalid("inboxReceipt.version") }
    return deviceFields([
      deviceU16(value.version), try deviceDigest(value.creditID, "creditID"),
      try deviceDigest(value.receiptCommitment, "receiptCommitment"),
    ])
  }

  fileprivate static func decodeInboxReceipt(_ payload: Data) throws -> KagemushaInboxReceiptV1 {
    var reader = DeviceOperationReader(payload)
    let value = try KagemushaInboxReceiptV1(
      version: reader.u16Field(), creditID: reader.digestField(),
      receiptCommitment: reader.digestField()
    )
    try reader.finish()
    return value
  }
}

/// Closed codec failures. They grant no secure-device or monetary authority.
public enum KagemushaDeviceOperationCodecErrorV1: Error, Equatable {
  case invalid(String)
}

private struct DeviceOperationArchiveDescriptor {
  let schema: String
  let alignment: Int
  let maximum: Int
}

private let deviceSchemaPrefix = "iroha.kagemusha.device.v1."
private let kagemushaModelSchemaPrefix = "iroha_data_model::kagemusha::kagemusha_v1::"
private let senderCommandSchema = deviceSchemaPrefix + "sender-command"
private let senderReplySchema = deviceSchemaPrefix + "sender-reply"
private let paymentRequestMaximum = 928
private let paymentMaximum = 7_552
private let inboxStagingMetadataMaximum = 1_024
private let redemptionVoucherMaximum = 7_936
private let terminalEnvelopeMaximum = redemptionVoucherMaximum
private let acknowledgementMaximum = 256
private let aggregateStateMaximum = 768
private let accountMaximum = 512
private let receiverPageCountMaximum: UInt16 = 4
private let senderPageCountMaximum: UInt16 = 4

private func receiverCommandDescriptor(
  _ operation: UInt8
) throws -> DeviceOperationArchiveDescriptor {
  switch operation {
  case 2:
    .init(
      schema: deviceSchemaPrefix + "stage-inbound-payment-command", alignment: 8,
      maximum: KagemushaDeviceOperationCodecV1.receiverStageCommandMaximumBytes
    )
  case 3:
    .init(
      schema: deviceSchemaPrefix + "recover-staged-inbound-payment-command", alignment: 2,
      maximum: KagemushaDeviceOperationCodecV1.receiverRecoveryCommandMaximumBytes
    )
  case 4:
    .init(
      schema: deviceSchemaPrefix + "recover-inbound-inbox-page-command", alignment: 16,
      maximum: KagemushaDeviceOperationCodecV1.receiverRecoveryCommandMaximumBytes
    )
  default: throw deviceInvalid("receiverCommand.operation")
  }
}

private func controlCommandDescriptor(
  _ operation: UInt8
) throws -> DeviceOperationArchiveDescriptor {
  switch operation {
  case 1:
    .init(schema: deviceSchemaPrefix + "read-active-hardware-credential-command", alignment: 2, maximum: 256)
  case 11:
    .init(schema: deviceSchemaPrefix + "sign-receive-acknowledgement-command", alignment: 8, maximum: 12 * 1024)
  case 13:
    .init(schema: deviceSchemaPrefix + "read-trusted-time-or-lease-command", alignment: 2, maximum: 256)
  case 14:
    .init(schema: deviceSchemaPrefix + "prepare-mint-authorization-command", alignment: 16, maximum: 2 * 1024)
  case 15:
    .init(schema: deviceSchemaPrefix + "recover-mint-authorization-command", alignment: 2, maximum: 256)
  case 17:
    .init(schema: deviceSchemaPrefix + "fold-receive-credit-command", alignment: 16, maximum: 256)
  case 18:
    .init(schema: deviceSchemaPrefix + "read-pending-credit-watermark-command", alignment: 2, maximum: 256)
  case 19:
    .init(schema: deviceSchemaPrefix + "rotate-hardware-epoch-command", alignment: 2, maximum: 256)
  case 20:
    .init(schema: deviceSchemaPrefix + "bootstrap-aggregate-state-command", alignment: 2, maximum: 256)
  case 21:
    .init(schema: deviceSchemaPrefix + "recover-wallet-snapshot-command", alignment: 2, maximum: 256)
  case 22:
    .init(schema: deviceSchemaPrefix + "create-signed-payment-request-command", alignment: 16, maximum: 2 * 1024)
  default: throw deviceInvalid("controlCommand.operation")
  }
}

private func deviceFrame(
  _ payload: Data,
  _ descriptor: DeviceOperationArchiveDescriptor
) throws -> Data {
  let archive = noritoEncode(
    typeName: descriptor.schema, payload: payload,
    flags: NoritoHeader.compactLen, payloadAlignment: descriptor.alignment
  )
  guard !archive.isEmpty, archive.count <= descriptor.maximum else {
    throw deviceInvalid("archive.size")
  }
  return archive
}

private func deviceUnframe(
  _ bytes: Data,
  _ descriptor: DeviceOperationArchiveDescriptor
) throws -> Data {
  let canonical = Data(bytes)
  guard !canonical.isEmpty, canonical.count <= descriptor.maximum,
    let frame = noritoDecodeFrame(canonical), frame.header.compression == .none,
    frame.header.flags == NoritoHeader.compactLen,
    frame.header.schema == noritoSchemaHash(forTypeName: descriptor.schema),
    frame.paddingLength == noritoHeaderPaddingLength(payloadAlignment: descriptor.alignment),
    try deviceFrame(frame.payload, descriptor) == canonical
  else { throw deviceInvalid("archive.canonical") }
  return frame.payload
}

private func deviceFields(_ values: [Data]) -> Data {
  var writer = DeviceOperationWriter()
  for value in values { writer.field(value) }
  return writer.data
}

private func deviceEnum(_ tag: UInt32, _ values: [Data]) -> Data {
  var writer = DeviceOperationWriter()
  writer.raw(deviceU32(tag))
  for value in values { writer.field(value) }
  return writer.data
}

private func deviceOption(_ value: Data?) throws -> Data {
  var writer = DeviceOperationWriter()
  guard let value else {
    writer.raw(Data([0]))
    return writer.data
  }
  writer.raw(Data([1]))
  writer.field(value)
  return writer.data
}

private func deviceVector(_ value: Data) throws -> Data {
  var writer = DeviceOperationWriter()
  writer.raw(deviceU64(UInt64(value.count)))
  writer.raw(value)
  return writer.data
}

private func deviceU16(_ value: UInt16) -> Data {
  var value = value.littleEndian
  return withUnsafeBytes(of: &value) { Data($0) }
}

private func deviceU32(_ value: UInt32) -> Data {
  var value = value.littleEndian
  return withUnsafeBytes(of: &value) { Data($0) }
}

private func deviceU64(_ value: UInt64) -> Data {
  var value = value.littleEndian
  return withUnsafeBytes(of: &value) { Data($0) }
}

private struct DeviceOperationWriter {
  var data = Data()

  mutating func raw(_ value: Data) { data.append(value) }

  mutating func field(_ value: Data) {
    length(value.count)
    raw(value)
  }

  private mutating func length(_ count: Int) {
    precondition(count >= 0)
    var value = UInt64(count)
    while value >= 0x80 {
      data.append(UInt8(value & 0x7f) | 0x80)
      value >>= 7
    }
    data.append(UInt8(value))
  }
}

private struct DeviceOperationReader {
  private let data: Data
  private var offset = 0

  init(_ data: Data) { self.data = Data(data) }

  var hasRemaining: Bool { offset < data.count }

  mutating func field(maximum: Int? = nil) throws -> Data {
    let count = try length()
    guard count <= (maximum ?? data.count), count <= data.count - offset else {
      throw deviceInvalid("field.size")
    }
    return try raw(count)
  }

  mutating func exactField(_ count: Int) throws -> Data {
    let value = try field(maximum: count)
    guard value.count == count else { throw deviceInvalid("field.width") }
    return value
  }

  mutating func digestField() throws -> Data {
    try deviceDigest(exactField(32), "digest")
  }

  mutating func u8Field() throws -> UInt8 { try exactField(1)[0] }

  mutating func u16Field() throws -> UInt16 {
    try readInteger(exactField(2), UInt16.self)
  }

  mutating func u32Field() throws -> UInt32 {
    try readInteger(exactField(4), UInt32.self)
  }

  mutating func u64Field() throws -> UInt64 {
    try readInteger(exactField(8), UInt64.self)
  }

  mutating func u128Field() throws -> KagemushaUInt128V1 {
    try KagemushaUInt128V1(littleEndianBytes: exactField(16))
  }

  mutating func u128Raw() throws -> KagemushaUInt128V1 {
    try KagemushaUInt128V1(littleEndianBytes: raw(16))
  }

  mutating func digestRaw() throws -> Data { try deviceDigest(raw(32), "digest") }

  mutating func enumTag(variants: UInt32) throws -> UInt32 {
    let tag: UInt32 = try readInteger(raw(4), UInt32.self)
    guard tag < variants else { throw deviceInvalid("enum.tag") }
    return tag
  }

  mutating func byteVectorField(
    maximum: Int,
    allowEmpty: Bool = false
  ) throws -> Data {
    var nested = DeviceOperationReader(try field(maximum: maximum + 8))
    let value = try nested.byteVectorRaw(maximum: maximum, allowEmpty: allowEmpty)
    try nested.finish()
    return value
  }

  mutating func byteVectorRaw(
    maximum: Int,
    allowEmpty: Bool = false
  ) throws -> Data {
    let count = try u64Raw()
    guard count <= UInt64(maximum), count <= UInt64(Int.max) else {
      throw deviceInvalid("vector.size")
    }
    let value = try raw(Int(count))
    guard allowEmpty || !value.isEmpty else { throw deviceInvalid("vector.empty") }
    return value
  }

  mutating func optionField<T>(
    _ decode: (inout DeviceOperationReader) throws -> T
  ) throws -> T? {
    var nested = DeviceOperationReader(try field())
    let tag = try nested.raw(1)[0]
    let result: T?
    switch tag {
    case 0: result = nil
    case 1:
      var item = DeviceOperationReader(try nested.field())
      result = try decode(&item)
      try item.finish()
    default: throw deviceInvalid("option.tag")
    }
    try nested.finish()
    return result
  }

  mutating func itemVectorField(
    maximumEntries: UInt16,
    validate: (Data) throws -> Void
  ) throws -> Int {
    var nested = DeviceOperationReader(try field())
    let count = try nested.u64Raw()
    guard count <= UInt64(maximumEntries) else { throw deviceInvalid("vector.count") }
    for _ in 0..<Int(count) { try validate(nested.field()) }
    try nested.finish()
    return Int(count)
  }

  mutating func raw(_ count: Int) throws -> Data {
    guard count >= 0, count <= data.count - offset else { throw deviceInvalid("payload.truncated") }
    defer { offset += count }
    return Data(data[offset..<(offset + count)])
  }

  mutating func allRaw() throws -> Data { try raw(data.count - offset) }

  mutating func finish() throws {
    guard offset == data.count else { throw deviceInvalid("payload.trailing") }
  }

  private mutating func u64Raw() throws -> UInt64 {
    try readInteger(raw(8), UInt64.self)
  }

  private mutating func length() throws -> Int {
    var result: UInt64 = 0
    var shift: UInt64 = 0
    for index in 0..<10 {
      let byte = try raw(1)[0]
      let chunk = UInt64(byte & 0x7f)
      guard shift < 64, !(shift == 63 && chunk > 1) else {
        throw deviceInvalid("length.overflow")
      }
      result |= chunk << shift
      if byte & 0x80 == 0 {
        guard index < 5,
          index == 0 || result >= (UInt64(1) << UInt64(7 * index)),
          result <= UInt64(Int.max)
        else { throw deviceInvalid("length.noncanonical") }
        return Int(result)
      }
      shift += 7
    }
    throw deviceInvalid("length.overflow")
  }

  private func readInteger<T: FixedWidthInteger>(
    _ bytes: Data,
    _: T.Type
  ) throws -> T {
    guard bytes.count == MemoryLayout<T>.size else { throw deviceInvalid("integer.width") }
    return bytes.withUnsafeBytes { T(littleEndian: $0.loadUnaligned(as: T.self)) }
  }
}

private func bounded(_ value: Data, _ maximum: Int, _ label: String) throws -> Data {
  guard !value.isEmpty, value.count <= maximum else { throw deviceInvalid(label + ".size") }
  return Data(value)
}

private func deviceDigest(_ value: Data, _ label: String) throws -> Data {
  guard value.count == 32, value.contains(where: { $0 != 0 }) else {
    throw deviceInvalid(label)
  }
  return Data(value)
}

private func deviceInvalid(_ reason: String) -> KagemushaDeviceOperationCodecErrorV1 {
  .invalid(reason)
}

/// Retained only after a native response authenticator has admitted the complete outer response.
internal struct KagemushaDeviceAuthenticatedReplyV1: Equatable, Sendable {
  let operation: UInt8
  let canonicalArchive: Data
  let payload: Data
}

internal struct KagemushaDeviceQualificationReplyV1: Equatable, Sendable {
  let releaseID: Data
  let hardwarePolicyDigest: Data
  let profile: KagemushaHardwareProfileV1
  let credential: KagemushaHardwareCredentialV1
}

extension KagemushaDeviceOperationCodecV1 {
  internal static func decodeControlReplyAfterAuthentication(
    operation: UInt8,
    canonicalBytes: Data
  ) throws -> KagemushaDeviceAuthenticatedReplyV1 {
    let descriptor = try controlReplyDescriptor(operation)
    let payload = try deviceUnframe(canonicalBytes, descriptor)
    try validateControlReply(operation: operation, payload: payload)
    return .init(
      operation: operation, canonicalArchive: Data(canonicalBytes), payload: payload
    )
  }

  internal static func decodeReceiverReplyAfterAuthentication(
    operation: UInt8,
    canonicalBytes: Data
  ) throws -> KagemushaDeviceAuthenticatedReplyV1 {
    let descriptor = try receiverReplyDescriptor(operation)
    let payload = try deviceUnframe(canonicalBytes, descriptor)
    try validateReceiverReply(operation: operation, payload: payload)
    return .init(
      operation: operation, canonicalArchive: Data(canonicalBytes), payload: payload
    )
  }

  /// Decode operation 1's nested qualification models after the outer reply was authenticated.
  internal static func decodeQualificationReplyAfterAuthentication(
    _ reply: KagemushaDeviceAuthenticatedReplyV1
  ) throws -> KagemushaDeviceQualificationReplyV1 {
    guard reply.operation == 1 else { throw deviceInvalid("qualificationReply.operation") }
    var reader = DeviceOperationReader(reply.payload)
    guard try reader.u16Field() == 1, try reader.u8Field() == 1 else {
      throw deviceInvalid("qualificationReply.binding")
    }
    let releaseID = try reader.digestField()
    let hardwarePolicyDigest = try reader.digestField()
    let profilePayload = try reader.field(maximum: 512)
    let credentialPayload = try reader.field(maximum: 768)
    try reader.finish()
    let profileArchive = noritoEncode(
      typeName: kagemushaModelSchemaPrefix + "KagemushaHardwareProfileV1",
      payload: profilePayload,
      flags: NoritoHeader.compactLen,
      payloadAlignment: 8
    )
    let credentialArchive = noritoEncode(
      typeName: kagemushaModelSchemaPrefix + "KagemushaHardwareCredentialV1",
      payload: credentialPayload,
      flags: NoritoHeader.compactLen,
      payloadAlignment: 8
    )
    return try .init(
      releaseID: releaseID,
      hardwarePolicyDigest: hardwarePolicyDigest,
      profile: KagemushaNoritoV1.decodeHardwareProfileShapeExact(profileArchive),
      credential: KagemushaNoritoV1.decodeHardwareCredentialShapeExact(credentialArchive)
    )
  }

  internal static func decodeSenderReplyAfterAuthentication(
    operation: UInt8,
    requestID: Data,
    canonicalBytes: Data
  ) throws -> KagemushaDeviceAuthenticatedReplyV1 {
    let descriptor = DeviceOperationArchiveDescriptor(
      schema: senderReplySchema, alignment: 16, maximum: senderReplyMaximumBytes
    )
    let payload = try deviceUnframe(canonicalBytes, descriptor)
    var reader = DeviceOperationReader(payload)
    guard try reader.u16Field() == 1, try reader.u8Field() == operation,
      try reader.digestField() == deviceDigest(requestID, "requestID")
    else { throw deviceInvalid("senderReply.binding") }
    _ = try decodeSenderContext(reader.field())
    _ = try reader.u128Field()
    try validateSenderReplyBody(reader.field())
    try reader.finish()
    return .init(
      operation: operation, canonicalArchive: Data(canonicalBytes), payload: payload
    )
  }

  private static func validateControlReply(operation: UInt8, payload: Data) throws {
    var reader = DeviceOperationReader(payload)
    guard try reader.u16Field() == 1, try reader.u8Field() == operation else {
      throw deviceInvalid("controlReply.binding")
    }
    switch operation {
    case 1:
      _ = try reader.digestField()
      _ = try reader.digestField()
      guard !(try reader.field(maximum: 512)).isEmpty,
        !(try reader.field(maximum: 768)).isEmpty
      else { throw deviceInvalid("qualificationReply.empty") }
    case 11:
      _ = try reader.byteVectorField(maximum: acknowledgementMaximum)
    case 13:
      try validateCommitEvidence(reader.field())
    case 14, 15:
      _ = try reader.byteVectorField(maximum: redemptionVoucherMaximum)
    case 17:
      _ = try reader.digestField()
      _ = try reader.byteVectorField(maximum: aggregateStateMaximum)
    case 18:
      _ = try reader.u128Field()
    case 19, 20:
      _ = try reader.byteVectorField(maximum: aggregateStateMaximum)
    case 21:
      _ = try reader.optionField { try $0.byteVectorRaw(maximum: aggregateStateMaximum) }
      _ = try reader.u128Field()
      _ = try reader.u128Field()
      _ = try reader.u128Field()
    case 22:
      _ = try reader.byteVectorField(maximum: paymentRequestMaximum)
    default: throw deviceInvalid("controlReply.operation")
    }
    try reader.finish()
  }

  private static func validateCommitEvidence(_ payload: Data) throws {
    var reader = DeviceOperationReader(payload)
    _ = try reader.enumTag(variants: 2)
    var statement = DeviceOperationReader(try reader.field())
    _ = try statement.digestField()
    try statement.finish()
    try reader.finish()
  }

  private static func validateReceiverReply(operation: UInt8, payload: Data) throws {
    var reader = DeviceOperationReader(payload)
    guard try reader.u16Field() == 1, try reader.u8Field() == operation else {
      throw deviceInvalid("receiverReply.binding")
    }
    switch operation {
    case 2, 3:
      _ = try reader.u128Field()
      try validateStagedReceipt(reader.field())
    case 4:
      _ = try reader.u128Field()
      _ = try reader.itemVectorField(maximumEntries: receiverPageCountMaximum) {
        try validateStagedReceipt($0)
      }
      _ = try reader.optionField { try $0.digestRaw() }
    default: throw deviceInvalid("receiverReply.operation")
    }
    try reader.finish()
  }

  private static func validateStagedReceipt(_ payload: Data) throws {
    var reader = DeviceOperationReader(payload)
    _ = try reader.byteVectorField(maximum: paymentRequestMaximum)
    _ = try reader.byteVectorField(maximum: paymentMaximum)
    _ = try reader.byteVectorField(maximum: inboxStagingMetadataMaximum, allowEmpty: true)
    _ = try decodeInboxReceipt(reader.field())
    try reader.finish()
  }

  private static func validateSenderReplyBody(_ payload: Data) throws {
    var reader = DeviceOperationReader(payload)
    switch try reader.enumTag(variants: 2) {
    case 0:
      _ = try reader.optionField { nested in
        try validateSenderRecoveryItem(nested.allRaw())
      }
    case 1:
      _ = try reader.itemVectorField(maximumEntries: senderPageCountMaximum) {
        try validateSenderRecoveryItem($0)
      }
      _ = try reader.optionField { try $0.digestRaw() }
    default: throw deviceInvalid("senderReplyBody.tag")
    }
    try reader.finish()
  }

  private static func validateSenderRecoveryItem(_ payload: Data) throws {
    var reader = DeviceOperationReader(payload)
    try validateSenderRecord(reader.field())
    _ = try reader.byteVectorField(maximum: terminalEnvelopeMaximum, allowEmpty: true)
    try reader.finish()
  }

  private static func validateSenderRecord(_ payload: Data) throws {
    var reader = DeviceOperationReader(payload)
    _ = try reader.digestField()
    _ = try decodeSenderContext(reader.field())
    _ = try reader.digestField()
    var kind = DeviceOperationReader(try reader.field())
    let operationKind = try kind.enumTag(variants: 7)
    guard operationKind == 2 || operationKind == 4 else {
      throw deviceInvalid("senderRecord.operationKind")
    }
    try kind.finish()
    _ = try reader.digestField()
    _ = try reader.digestField()
    _ = try reader.digestField()
    var phase = DeviceOperationReader(try reader.field())
    _ = try phase.enumTag(variants: 6)
    try phase.finish()
    _ = try reader.u128Field()
    _ = try reader.optionField { try decodeSenderInputs($0.allRaw()) }
    for _ in 0..<4 { _ = try reader.optionField { try $0.digestRaw() } }
    try reader.finish()
  }
}

private func receiverReplyDescriptor(
  _ operation: UInt8
) throws -> DeviceOperationArchiveDescriptor {
  switch operation {
  case 2, 3:
    .init(schema: deviceSchemaPrefix + "staged-inbound-payment-reply", alignment: 16, maximum: 64 * 1024)
  case 4:
    .init(schema: deviceSchemaPrefix + "inbound-inbox-page-reply", alignment: 16, maximum: 64 * 1024)
  default: throw deviceInvalid("receiverReply.operation")
  }
}

private func controlReplyDescriptor(
  _ operation: UInt8
) throws -> DeviceOperationArchiveDescriptor {
  switch operation {
  case 1:
    .init(schema: deviceSchemaPrefix + "active-hardware-credential-reply", alignment: 8, maximum: 2 * 1024)
  case 11:
    .init(schema: deviceSchemaPrefix + "receive-acknowledgement-reply", alignment: 8, maximum: 2 * 1024)
  case 13:
    .init(schema: deviceSchemaPrefix + "trusted-time-or-lease-reply", alignment: 8, maximum: 512)
  case 14, 15:
    .init(schema: deviceSchemaPrefix + "mint-authorization-reply", alignment: 8, maximum: 12 * 1024)
  case 17:
    .init(schema: deviceSchemaPrefix + "fold-receive-credit-reply", alignment: 16, maximum: 2 * 1024)
  case 18:
    .init(schema: deviceSchemaPrefix + "pending-credit-watermark-reply", alignment: 16, maximum: 256)
  case 19:
    .init(schema: deviceSchemaPrefix + "rotate-hardware-epoch-reply", alignment: 8, maximum: 2 * 1024)
  case 20:
    .init(schema: deviceSchemaPrefix + "bootstrap-aggregate-state-reply", alignment: 8, maximum: 2 * 1024)
  case 21:
    .init(schema: deviceSchemaPrefix + "wallet-recovery-snapshot-reply", alignment: 16, maximum: 2 * 1024)
  case 22:
    .init(schema: deviceSchemaPrefix + "signed-payment-request-reply", alignment: 8, maximum: 2 * 1024)
  default: throw deviceInvalid("controlReply.operation")
  }
}

// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation

/// A canonical, public-input-only receiver command for ABI-23 operations 4 through 8.
public enum KagemushaDeviceReceiverCommandV1: Equatable, Sendable {
  case reserve(canonicalRequest: Data, canonicalIntent: Data)
  case recoverTicket(ticketID: Data)
  case stage(
    canonicalRequest: Data,
    canonicalIntent: Data,
    canonicalTicket: Data,
    canonicalPayment: Data
  )
  case recoverStaged(creditID: Data, envelopeDigest: Data)
  case page(
    revision: KagemushaUInt128V1,
    after: Data?,
    maximumEntries: UInt16
  )

  public var operation: UInt8 {
    switch self {
    case .reserve: 4
    case .recoverTicket: 5
    case .stage: 6
    case .recoverStaged: 7
    case .page: 8
    }
  }
}

/// A canonical, public-input-only control command for ABI-23 operations outside the sender lane.
public enum KagemushaDeviceControlCommandV1: Equatable, Sendable {
  case readActiveHardwareCredential
  case prepareAcceptanceIntent(
    intentID: Data,
    canonicalRequest: Data,
    exactAmount: KagemushaUInt128V1
  )
  case recoverAcceptanceIntent(intentID: Data)
  case signReceiveAcknowledgement(
    canonicalRequest: Data,
    canonicalIntent: Data,
    canonicalTicket: Data,
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
  case foldReceiveBatch(operationID: Data, inboxSequenceInclusive: KagemushaUInt128V1)
  case readPendingCreditWatermark
  case rotateHardwareEpoch(operationID: Data)
  case bootstrapAggregateState(operationID: Data)
  case recoverWalletSnapshot
  case createSignedPaymentRequest(
    requestID: Data,
    recipient: KagemushaAccountIDV1,
    requestMode: KagemushaPaymentRequestModeV1,
    validityWindowMS: UInt64
  )

  public var operation: UInt8 {
    switch self {
    case .readActiveHardwareCredential: 1
    case .prepareAcceptanceIntent: 2
    case .recoverAcceptanceIntent: 3
    case .signReceiveAcknowledgement: 16
    case .readTrustedTimeOrLease: 18
    case .prepareMintAuthorization: 19
    case .recoverMintAuthorization: 20
    case .foldReceiveBatch: 22
    case .readPendingCreditWatermark: 23
    case .rotateHardwareEpoch: 24
    case .bootstrapAggregateState: 25
    case .recoverWalletSnapshot: 26
    case .createSignedPaymentRequest: 27
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
  case sendSplit(canonicalRequest: Data, canonicalIntent: Data, canonicalTicket: Data)
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
  case abandonUncommitted(selector: KagemushaDeviceSenderPreparationSelectorV1)
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
    case .prepare: 9
    case .recoverPrepared: 10
    case .abandonUncommitted: 11
    case .commit: 12
    case .recoverTerminal: 13
    case .install: 14
    case .recoverInstalled: 15
    case .release: 17
    }
  }

  fileprivate var ordinal: UInt32 {
    switch self {
    case .prepare: 0
    case .recoverPrepared: 1
    case .abandonUncommitted: 2
    case .commit: 3
    case .recoverTerminal: 4
    case .install: 5
    case .recoverInstalled: 6
    case .release: 7
    }
  }
}

/// One exact sender command shared by ABI-23 operations 9 through 15 and 17.
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
  public static let controlIntentCommandMaximumBytes = 2 * 1024
  public static let controlAcknowledgementCommandMaximumBytes = 12 * 1024
  public static let controlMintCommandMaximumBytes = 2 * 1024
  public static let controlFoldCommandMaximumBytes = 256
  public static let controlPaymentRequestCommandMaximumBytes = 2 * 1024
  public static let receiverReserveCommandMaximumBytes = 2 * 1024
  public static let receiverStageCommandMaximumBytes = 12 * 1024
  public static let receiverRecoveryCommandMaximumBytes = 512
  public static let senderCommandMaximumBytes = 16 * 1024
  public static let senderReplyMaximumBytes = 64 * 1024

  public static func encodeReceiverCommand(
    _ value: KagemushaDeviceReceiverCommandV1
  ) throws -> Data {
    let payload: Data
    switch value {
    case .reserve(let request, let intent):
      payload = deviceFields([
        deviceU16(1), Data([value.operation]),
        try deviceVector(try bounded(request, paymentRequestMaximum, "request")),
        try deviceVector(try bounded(intent, acceptanceIntentMaximum, "intent")),
      ])
    case .recoverTicket(let ticketID):
      payload = deviceFields([deviceU16(1), Data([value.operation]), try deviceDigest(ticketID, "ticketID")])
    case .stage(let request, let intent, let ticket, let payment):
      payload = deviceFields([
        deviceU16(1), Data([value.operation]),
        try deviceVector(try bounded(request, paymentRequestMaximum, "request")),
        try deviceVector(try bounded(intent, acceptanceIntentMaximum, "intent")),
        try deviceVector(try bounded(ticket, acceptanceTicketMaximum, "ticket")),
        try deviceVector(try bounded(payment, paymentMaximum, "payment")),
      ])
    case .recoverStaged(let creditID, let envelopeDigest):
      payload = deviceFields([
        deviceU16(1), Data([value.operation]),
        try deviceDigest(creditID, "creditID"),
        try deviceDigest(envelopeDigest, "envelopeDigest"),
      ])
    case .page(let revision, let after, let maximumEntries):
      guard (1...receiverPageCountMaximum).contains(maximumEntries) else {
        throw deviceInvalid("maximumEntries")
      }
      payload = deviceFields([
        deviceU16(1), Data([value.operation]), revision.littleEndianBytes,
        try deviceOption(after.map { try deviceDigest($0, "after") }),
        deviceU16(maximumEntries),
      ])
    }
    let descriptor = try receiverCommandDescriptor(value.operation)
    return try deviceFrame(payload, descriptor)
  }

  public static func decodeReceiverCommand(
    operation: UInt8,
    canonicalBytes: Data
  ) throws -> KagemushaDeviceReceiverCommandV1 {
    let descriptor = try receiverCommandDescriptor(operation)
    var reader = DeviceOperationReader(try deviceUnframe(canonicalBytes, descriptor))
    guard try reader.u16Field() == 1, try reader.u8Field() == operation else {
      throw deviceInvalid("receiverCommand.binding")
    }
    let value: KagemushaDeviceReceiverCommandV1
    switch operation {
    case 4:
      value = .reserve(
        canonicalRequest: try reader.byteVectorField(maximum: paymentRequestMaximum),
        canonicalIntent: try reader.byteVectorField(maximum: acceptanceIntentMaximum)
      )
    case 5:
      value = .recoverTicket(ticketID: try reader.digestField())
    case 6:
      value = .stage(
        canonicalRequest: try reader.byteVectorField(maximum: paymentRequestMaximum),
        canonicalIntent: try reader.byteVectorField(maximum: acceptanceIntentMaximum),
        canonicalTicket: try reader.byteVectorField(maximum: acceptanceTicketMaximum),
        canonicalPayment: try reader.byteVectorField(maximum: paymentMaximum)
      )
    case 7:
      value = .recoverStaged(
        creditID: try reader.digestField(),
        envelopeDigest: try reader.digestField()
      )
    case 8:
      value = .page(
        revision: try reader.u128Field(),
        after: try reader.optionField { try $0.digestRaw() },
        maximumEntries: try reader.u16Field()
      )
    default:
      throw deviceInvalid("receiverCommand.operation")
    }
    try reader.finish()
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
    case .prepareAcceptanceIntent(let intentID, let request, let amount):
      guard !amount.isZero else { throw deviceInvalid("exactAmount") }
      payload = deviceFields([
        deviceU16(1), Data([value.operation]), try deviceDigest(intentID, "intentID"),
        try deviceVector(try bounded(request, paymentRequestMaximum, "request")),
        amount.littleEndianBytes,
      ])
    case .recoverAcceptanceIntent(let intentID):
      payload = deviceFields([deviceU16(1), Data([value.operation]), try deviceDigest(intentID, "intentID")])
    case .signReceiveAcknowledgement(let request, let intent, let ticket, let payment, let receipt):
      payload = deviceFields([
        deviceU16(1), Data([value.operation]),
        try deviceVector(try bounded(request, paymentRequestMaximum, "request")),
        try deviceVector(try bounded(intent, acceptanceIntentMaximum, "intent")),
        try deviceVector(try bounded(ticket, acceptanceTicketMaximum, "ticket")),
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
    case .foldReceiveBatch(let operationID, let inclusive):
      payload = deviceFields([
        deviceU16(1), Data([value.operation]), try deviceDigest(operationID, "operationID"),
        inclusive.littleEndianBytes,
      ])
    case .createSignedPaymentRequest(let requestID, let recipient, let requestMode, let window):
      guard (1...KagemushaWireV1.requestMaximumTTLMS).contains(window) else {
        throw deviceInvalid("validityWindowMS")
      }
      payload = deviceFields([
        deviceU16(1), Data([value.operation]), try deviceDigest(requestID, "requestID"),
        recipient.canonicalPayload, try encodeRequestMode(requestMode), deviceU64(window),
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
    case 2:
      value = .prepareAcceptanceIntent(
        intentID: try reader.digestField(),
        canonicalRequest: try reader.byteVectorField(maximum: paymentRequestMaximum),
        exactAmount: try reader.u128Field()
      )
    case 3: value = .recoverAcceptanceIntent(intentID: try reader.digestField())
    case 16:
      value = .signReceiveAcknowledgement(
        canonicalRequest: try reader.byteVectorField(maximum: paymentRequestMaximum),
        canonicalIntent: try reader.byteVectorField(maximum: acceptanceIntentMaximum),
        canonicalTicket: try reader.byteVectorField(maximum: acceptanceTicketMaximum),
        canonicalPayment: try reader.byteVectorField(maximum: paymentMaximum),
        inboxReceipt: try decodeInboxReceipt(reader.field())
      )
    case 18: value = .readTrustedTimeOrLease
    case 19:
      value = .prepareMintAuthorization(
        operationID: try reader.digestField(), amount: try reader.u128Field(),
        payer: try KagemushaAccountIDV1(canonicalPayload: reader.field(maximum: accountMaximum)),
        recipient: try KagemushaAccountIDV1(canonicalPayload: reader.field(maximum: accountMaximum))
      )
    case 20: value = .recoverMintAuthorization(operationID: try reader.digestField())
    case 22:
      value = .foldReceiveBatch(
        operationID: try reader.digestField(),
        inboxSequenceInclusive: try reader.u128Field()
      )
    case 23: value = .readPendingCreditWatermark
    case 24: value = .rotateHardwareEpoch(operationID: try reader.digestField())
    case 25: value = .bootstrapAggregateState(operationID: try reader.digestField())
    case 26: value = .recoverWalletSnapshot
    case 27:
      let requestID = try reader.digestField()
      let recipient = try KagemushaAccountIDV1(
        canonicalPayload: reader.field(maximum: accountMaximum)
      )
      let requestMode = try decodeRequestMode(reader.field())
      let window = try reader.u64Field()
      guard (1...KagemushaWireV1.requestMaximumTTLMS).contains(window) else {
        throw deviceInvalid("validityWindowMS")
      }
      value = .createSignedPaymentRequest(
        requestID: requestID, recipient: recipient,
        requestMode: requestMode, validityWindowMS: window
      )
    default: throw deviceInvalid("controlCommand.operation")
    }
    try reader.finish()
    switch value {
    case .prepareAcceptanceIntent(let id, _, _), .recoverAcceptanceIntent(let id),
      .recoverMintAuthorization(let id), .rotateHardwareEpoch(let id),
      .bootstrapAggregateState(let id):
      guard id == expectedID else { throw deviceInvalid("controlCommand.requestID") }
    case .signReceiveAcknowledgement(_, _, _, _, let receipt):
      guard receipt.creditID == expectedID else { throw deviceInvalid("controlCommand.requestID") }
    case .prepareMintAuthorization(let id, _, _, _), .foldReceiveBatch(let id, _):
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
  fileprivate static func encodeRequestMode(
    _ value: KagemushaPaymentRequestModeV1
  ) throws -> Data {
    let body: Data
    switch value {
    case .singleExact(let exact):
      guard !exact.amount.isZero else { throw deviceInvalid("requestMode.amount") }
      body = deviceFields([exact.amount.littleEndianBytes])
    case .partialUntilTotal(let partial):
      guard !partial.totalAmount.isZero else { throw deviceInvalid("requestMode.totalAmount") }
      body = deviceFields([partial.totalAmount.littleEndianBytes])
    case .boundedMultiPayment(let bounded):
      guard bounded.maxPayments != 0 else { throw deviceInvalid("requestMode.maxPayments") }
      body = deviceFields([
        deviceFields([
          bounded.amountPolicy.minimumAmount.littleEndianBytes,
          bounded.amountPolicy.maximumAmount.littleEndianBytes,
        ]),
        deviceU32(bounded.maxPayments),
      ])
    case .openReceive(let open):
      body = deviceFields([
        deviceFields([
          open.amountPolicy.minimumAmount.littleEndianBytes,
          open.amountPolicy.maximumAmount.littleEndianBytes,
        ])
      ])
    }
    return deviceEnum(value.wireTag, [body])
  }

  fileprivate static func decodeRequestMode(
    _ payload: Data
  ) throws -> KagemushaPaymentRequestModeV1 {
    var reader = DeviceOperationReader(payload)
    let tag = try reader.enumTag(variants: 4)
    var body = DeviceOperationReader(try reader.field())
    let value: KagemushaPaymentRequestModeV1
    switch tag {
    case 0:
      value = .singleExact(try KagemushaSingleExactV1(amount: body.u128Field()))
    case 1:
      value = .partialUntilTotal(
        try KagemushaPartialUntilTotalV1(totalAmount: body.u128Field())
      )
    case 2:
      var policy = DeviceOperationReader(try body.field())
      let amountPolicy = try KagemushaAmountPolicyV1(
        minimumAmount: policy.u128Field(), maximumAmount: policy.u128Field()
      )
      try policy.finish()
      value = .boundedMultiPayment(
        try KagemushaBoundedMultiPaymentV1(
          amountPolicy: amountPolicy, maxPayments: body.u32Field()
        )
      )
    case 3:
      var policy = DeviceOperationReader(try body.field())
      let amountPolicy = try KagemushaAmountPolicyV1(
        minimumAmount: policy.u128Field(), maximumAmount: policy.u128Field()
      )
      try policy.finish()
      value = .openReceive(KagemushaOpenReceiveV1(amountPolicy: amountPolicy))
    default: throw deviceInvalid("requestMode.tag")
    }
    try body.finish()
    try reader.finish()
    return value
  }

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
    case .sendSplit(let request, let intent, let ticket):
      return deviceEnum(0, [
        try deviceVector(try bounded(request, paymentRequestMaximum, "request")),
        try deviceVector(try bounded(intent, acceptanceIntentMaximum, "intent")),
        try deviceVector(try bounded(ticket, acceptanceTicketMaximum, "ticket")),
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
        canonicalRequest: try reader.byteVectorField(maximum: paymentRequestMaximum),
        canonicalIntent: try reader.byteVectorField(maximum: acceptanceIntentMaximum),
        canonicalTicket: try reader.byteVectorField(maximum: acceptanceTicketMaximum)
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
    case .abandonUncommitted(let selector):
      fields = [try encodePreparationSelector(selector)]
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
    switch try reader.enumTag(variants: 8) {
    case 0: value = .prepare(inputs: try decodeSenderInputs(reader.field()))
    case 1: value = .recoverPrepared(inputsDigest: try reader.digestField())
    case 2:
      value = .abandonUncommitted(selector: try decodePreparationSelector(reader.field()))
    case 3:
      value = .commit(
        selector: try decodePreparationSelector(reader.field()),
        candidateDigest: try reader.digestField()
      )
    case 4: value = .recoverTerminal(inputsDigest: try reader.digestField())
    case 5:
      value = .install(
        selector: try decodePreparationSelector(reader.field()),
        candidateDigest: try reader.digestField(),
        inputs: try decodeSenderInputs(reader.field()),
        canonicalEnvelope: try reader.byteVectorField(maximum: terminalEnvelopeMaximum)
      )
    case 6:
      value = .recoverInstalled(selector: try decodeRecoverySelector(reader.field()))
    case 7:
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
private let acceptanceIntentMaximum = 192
private let acceptanceTicketMaximum = 256
private let paymentMaximum = 7_552
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
  case 4:
    .init(
      schema: deviceSchemaPrefix + "reserve-acceptance-ticket-command", alignment: 8,
      maximum: KagemushaDeviceOperationCodecV1.receiverReserveCommandMaximumBytes
    )
  case 5:
    .init(
      schema: deviceSchemaPrefix + "recover-acceptance-ticket-command", alignment: 2,
      maximum: KagemushaDeviceOperationCodecV1.receiverRecoveryCommandMaximumBytes
    )
  case 6:
    .init(
      schema: deviceSchemaPrefix + "stage-inbound-payment-command", alignment: 8,
      maximum: KagemushaDeviceOperationCodecV1.receiverStageCommandMaximumBytes
    )
  case 7:
    .init(
      schema: deviceSchemaPrefix + "recover-staged-inbound-payment-command", alignment: 2,
      maximum: KagemushaDeviceOperationCodecV1.receiverRecoveryCommandMaximumBytes
    )
  case 8:
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
  case 2:
    .init(schema: deviceSchemaPrefix + "prepare-acceptance-intent-command", alignment: 16, maximum: 2 * 1024)
  case 3:
    .init(schema: deviceSchemaPrefix + "recover-acceptance-intent-command", alignment: 2, maximum: 256)
  case 16:
    .init(schema: deviceSchemaPrefix + "sign-receive-acknowledgement-command", alignment: 8, maximum: 12 * 1024)
  case 18:
    .init(schema: deviceSchemaPrefix + "read-trusted-time-or-lease-command", alignment: 2, maximum: 256)
  case 19:
    .init(schema: deviceSchemaPrefix + "prepare-mint-authorization-command", alignment: 16, maximum: 2 * 1024)
  case 20:
    .init(schema: deviceSchemaPrefix + "recover-mint-authorization-command", alignment: 2, maximum: 256)
  case 22:
    .init(schema: deviceSchemaPrefix + "fold-receive-batch-command", alignment: 16, maximum: 256)
  case 23:
    .init(schema: deviceSchemaPrefix + "read-pending-credit-watermark-command", alignment: 2, maximum: 256)
  case 24:
    .init(schema: deviceSchemaPrefix + "rotate-hardware-epoch-command", alignment: 2, maximum: 256)
  case 25:
    .init(schema: deviceSchemaPrefix + "bootstrap-aggregate-state-command", alignment: 2, maximum: 256)
  case 26:
    .init(schema: deviceSchemaPrefix + "recover-wallet-snapshot-command", alignment: 2, maximum: 256)
  case 27:
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
    case 2, 3:
      _ = try reader.byteVectorField(maximum: acceptanceIntentMaximum)
    case 16:
      _ = try reader.byteVectorField(maximum: acknowledgementMaximum)
    case 18:
      try validateCommitEvidence(reader.field())
    case 19, 20:
      _ = try reader.byteVectorField(maximum: redemptionVoucherMaximum)
    case 22:
      _ = try reader.u128Field()
      let count = try reader.u8Field()
      guard (1...16).contains(count) else { throw deviceInvalid("activeCreditCount") }
      _ = try reader.byteVectorField(maximum: aggregateStateMaximum)
    case 23:
      _ = try reader.u128Field()
    case 24, 25:
      _ = try reader.byteVectorField(maximum: aggregateStateMaximum)
    case 26:
      _ = try reader.optionField { try $0.byteVectorRaw(maximum: aggregateStateMaximum) }
      _ = try reader.u128Field()
      _ = try reader.u128Field()
      _ = try reader.u128Field()
    case 27:
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
    case 4, 5:
      _ = try reader.byteVectorField(maximum: paymentRequestMaximum)
      _ = try reader.byteVectorField(maximum: acceptanceIntentMaximum)
      _ = try reader.byteVectorField(maximum: acceptanceTicketMaximum)
      try validateTwoFieldCertificate(reader.field())
    case 6, 7:
      try validateStagedReceipt(reader.field())
    case 8:
      _ = try reader.u128Field()
      _ = try reader.itemVectorField(maximumEntries: receiverPageCountMaximum) {
        try validateStagedReceipt($0)
      }
      _ = try reader.optionField { try $0.digestRaw() }
    default: throw deviceInvalid("receiverReply.operation")
    }
    try reader.finish()
  }

  private static func validateTwoFieldCertificate(_ payload: Data) throws {
    var reader = DeviceOperationReader(payload)
    guard !(try reader.field()).isEmpty else { throw deviceInvalid("certificate.statement") }
    _ = try reader.byteVectorField(maximum: 8 * 1024)
    try reader.finish()
  }

  private static func validateStagedReceipt(_ payload: Data) throws {
    var reader = DeviceOperationReader(payload)
    var exchange = DeviceOperationReader(try reader.field())
    guard try exchange.u16Field() == 1, try exchange.u8Field() == 6 else {
      throw deviceInvalid("stagedReceipt.exchange")
    }
    _ = try exchange.byteVectorField(maximum: paymentRequestMaximum)
    _ = try exchange.byteVectorField(maximum: acceptanceIntentMaximum)
    _ = try exchange.byteVectorField(maximum: acceptanceTicketMaximum)
    _ = try exchange.byteVectorField(maximum: paymentMaximum)
    try exchange.finish()
    try validateTwoFieldCertificate(reader.field())
    var acknowledgement = DeviceOperationReader(try reader.field())
    guard !(try acknowledgement.field()).isEmpty else {
      throw deviceInvalid("durableAcknowledgement.value")
    }
    _ = try acknowledgement.byteVectorField(maximum: acknowledgementMaximum)
    try acknowledgement.finish()
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
  case 4, 5:
    .init(schema: deviceSchemaPrefix + "acceptance-ticket-reply", alignment: 16, maximum: 16 * 1024)
  case 6, 7:
    .init(schema: deviceSchemaPrefix + "staged-inbound-payment-reply", alignment: 16, maximum: 24 * 1024)
  case 8:
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
  case 2, 3:
    .init(schema: deviceSchemaPrefix + "acceptance-intent-reply", alignment: 8, maximum: 2 * 1024)
  case 16:
    .init(schema: deviceSchemaPrefix + "receive-acknowledgement-reply", alignment: 8, maximum: 2 * 1024)
  case 18:
    .init(schema: deviceSchemaPrefix + "trusted-time-or-lease-reply", alignment: 8, maximum: 512)
  case 19, 20:
    .init(schema: deviceSchemaPrefix + "mint-authorization-reply", alignment: 8, maximum: 12 * 1024)
  case 22:
    .init(schema: deviceSchemaPrefix + "fold-receive-batch-reply", alignment: 16, maximum: 2 * 1024)
  case 23:
    .init(schema: deviceSchemaPrefix + "pending-credit-watermark-reply", alignment: 16, maximum: 256)
  case 24:
    .init(schema: deviceSchemaPrefix + "rotate-hardware-epoch-reply", alignment: 8, maximum: 2 * 1024)
  case 25:
    .init(schema: deviceSchemaPrefix + "bootstrap-aggregate-state-reply", alignment: 8, maximum: 2 * 1024)
  case 26:
    .init(schema: deviceSchemaPrefix + "wallet-recovery-snapshot-reply", alignment: 16, maximum: 2 * 1024)
  case 27:
    .init(schema: deviceSchemaPrefix + "signed-payment-request-reply", alignment: 8, maximum: 2 * 1024)
  default: throw deviceInvalid("controlReply.operation")
  }
}

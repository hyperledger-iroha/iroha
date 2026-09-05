// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation

/// Transport-neutral NFC constants for the sole three-message KAGEMUSHA V1 protocol.
public enum IrohaPeerNfcV1 {
  public static let applicationIdentifier = Data([
    0xf0, 0x50, 0x4b, 0x45, 0x50, 0x4b, 0x52, 0x4e, 0x46, 0x43, 0x01,
  ])
  public static let applicationIdentifierHex = "F0504B45504B524E464301"
  public static let commandClass: UInt8 = 0x80
  public static let wireVersion: UInt8 = 1
  public static let sessionIDBytes = 16
  public static let hashBytes = 32
  public static let maximumChunkBytes = 4_096
  public static let maximumMessageBytes =
    IrohaPeerWireMessageV1.headerBytes + IrohaPeerWireLimitsV1.maximumKagemushaProfileBytes
}

/// Closed APDU instruction inventory for Request -> Payment -> Acknowledgement.
public enum IrohaPeerNfcInstructionV1: UInt8, CaseIterable, Sendable {
  case getInfo = 0x10
  case readRequest = 0x11
  case beginPayment = 0x20
  case writePayment = 0x21
  case commitPayment = 0x22
  case readAcknowledgement = 0x23
  case confirmAcknowledgement = 0x24
  case getStatus = 0x25
  case resetSession = 0x7f
}

/// Monotonic receiver phases for the direct three-message exchange.
public enum IrohaPeerNfcPhaseV1: UInt8, CaseIterable, Sendable {
  case requestReady = 1
  case paymentReceiving = 2
  case acknowledgementReady = 3
  case complete = 4
}

/// NFC status flags. Durable is set only after irreversible hardware staging.
public struct IrohaPeerNfcFlagsV1: OptionSet, Equatable, Sendable {
  public let rawValue: UInt8
  public init(rawValue: UInt8) { self.rawValue = rawValue }
  public static let idempotentWrites = Self(rawValue: 1)
  public static let durableState = Self(rawValue: 2)
  public static let request: Self = [.idempotentWrites]
  public static let durable: Self = [.idempotentWrites, .durableState]
}

/// Allocation and chunk bounds shared by reader and receiver.
public struct IrohaPeerNfcLimitsV1: Equatable, Sendable {
  public let maximumMessageBytes: Int
  public let maximumReadChunkBytes: Int
  public let maximumWriteChunkBytes: Int

  public init(
    maximumMessageBytes: Int = IrohaPeerNfcV1.maximumMessageBytes,
    maximumReadChunkBytes: Int = IrohaPeerNfcV1.maximumChunkBytes,
    maximumWriteChunkBytes: Int = IrohaPeerNfcV1.maximumChunkBytes
  ) {
    precondition((IrohaPeerWireMessageV1.headerBytes + 1...IrohaPeerNfcV1.maximumMessageBytes)
      .contains(maximumMessageBytes))
    precondition((1...IrohaPeerNfcV1.maximumChunkBytes).contains(maximumReadChunkBytes))
    precondition((1...IrohaPeerNfcV1.maximumChunkBytes).contains(maximumWriteChunkBytes))
    self.maximumMessageBytes = maximumMessageBytes
    self.maximumReadChunkBytes = maximumReadChunkBytes
    self.maximumWriteChunkBytes = maximumWriteChunkBytes
  }

  public static let `default` = Self()
}

/// The one peer profile accepted throughout a session.
public struct IrohaPeerNfcProfilePolicyV1: Equatable, Sendable {
  public let profile: IrohaPeerWireProfileV1
  public init(profile: IrohaPeerWireProfileV1) {
    precondition(profile != .reject)
    self.profile = profile
  }
  public func accepts(_ candidate: IrohaPeerWireProfileV1) -> Bool { candidate == profile }
}

/// Stable session identity bound to the exact request.
public struct IrohaPeerNfcRequestIdentityV1: Equatable, Sendable {
  public let profile: IrohaPeerWireProfileV1
  public let sessionID: Data
  public let requestCanonicalHash: Data
  public let requestWireHash: Data

  public init(
    profile: IrohaPeerWireProfileV1, sessionID: Data,
    requestCanonicalHash: Data, requestWireHash: Data
  ) throws {
    guard profile != .reject,
      sessionID.count == IrohaPeerNfcV1.sessionIDBytes,
      requestCanonicalHash.count == IrohaPeerNfcV1.hashBytes,
      requestWireHash.count == IrohaPeerNfcV1.hashBytes,
      sessionID.contains(where: { $0 != 0 }),
      requestCanonicalHash.contains(where: { $0 != 0 }),
      requestWireHash.contains(where: { $0 != 0 })
    else { throw IrohaPeerNfcErrorV1.invalidIdentity }
    self.profile = profile
    self.sessionID = Data(sessionID)
    self.requestCanonicalHash = Data(requestCanonicalHash)
    self.requestWireHash = Data(requestWireHash)
  }
}

/// Session descriptor returned by `GET_INFO`.
public struct IrohaPeerNfcInfoV1: Equatable, Sendable {
  public let phase: IrohaPeerNfcPhaseV1
  public let flags: IrohaPeerNfcFlagsV1
  public let identity: IrohaPeerNfcRequestIdentityV1
  public let requestLength: Int
  public let maximumReadChunkBytes: Int
  public let maximumWriteChunkBytes: Int

  public init(
    phase: IrohaPeerNfcPhaseV1, flags: IrohaPeerNfcFlagsV1,
    identity: IrohaPeerNfcRequestIdentityV1, requestLength: Int,
    maximumReadChunkBytes: Int, maximumWriteChunkBytes: Int
  ) throws {
    guard (1...IrohaPeerNfcV1.maximumMessageBytes).contains(requestLength),
      (1...IrohaPeerNfcV1.maximumChunkBytes).contains(maximumReadChunkBytes),
      (1...IrohaPeerNfcV1.maximumChunkBytes).contains(maximumWriteChunkBytes)
    else { throw IrohaPeerNfcErrorV1.invalidLength }
    self.phase = phase
    self.flags = flags
    self.identity = identity
    self.requestLength = requestLength
    self.maximumReadChunkBytes = maximumReadChunkBytes
    self.maximumWriteChunkBytes = maximumWriteChunkBytes
  }

  public var encoded: Data {
    var output = Data("INF1".utf8)
    output.append(IrohaPeerNfcV1.wireVersion)
    output.append(phase.rawValue)
    output.append(flags.rawValue)
    output.append(UInt8(identity.profile.rawValue))
    output.append(identity.sessionID)
    output.append(identity.requestCanonicalHash)
    output.append(identity.requestWireHash)
    output.nfcAppendU32(requestLength)
    output.nfcAppendU16(maximumReadChunkBytes)
    output.nfcAppendU16(maximumWriteChunkBytes)
    return output
  }

  public static func decode(_ data: Data) throws -> Self {
    var reader = NfcReaderV1(data)
    guard try reader.read(4) == Data("INF1".utf8),
      try reader.u8() == IrohaPeerNfcV1.wireVersion,
      let phase = IrohaPeerNfcPhaseV1(rawValue: try reader.u8())
    else { throw IrohaPeerNfcErrorV1.invalidEncoding }
    let flags = IrohaPeerNfcFlagsV1(rawValue: try reader.u8())
    guard let profile = IrohaPeerWireProfileV1(rawValue: UInt16(try reader.u8())),
      profile != .reject
    else { throw IrohaPeerNfcErrorV1.invalidProfile }
    let identity = try IrohaPeerNfcRequestIdentityV1(
      profile: profile, sessionID: reader.read(16),
      requestCanonicalHash: reader.read(32), requestWireHash: reader.read(32))
    let value = try Self(
      phase: phase, flags: flags, identity: identity,
      requestLength: reader.u32(), maximumReadChunkBytes: reader.u16(),
      maximumWriteChunkBytes: reader.u16())
    try reader.finish()
    return value
  }
}

/// Compact restart/status projection.
public struct IrohaPeerNfcStatusV1: Equatable, Sendable {
  public let phase: IrohaPeerNfcPhaseV1
  public let flags: IrohaPeerNfcFlagsV1
  public let identity: IrohaPeerNfcRequestIdentityV1
  public let receivedPaymentBytes: Int
  public let paymentWireHash: Data?
  public let acknowledgementLength: Int
  public let acknowledgementWireHash: Data?

  public var encoded: Data {
    var output = Data("NST1".utf8)
    output.append(IrohaPeerNfcV1.wireVersion)
    output.append(phase.rawValue)
    output.append(flags.rawValue)
    output.append(UInt8(identity.profile.rawValue))
    output.append(identity.sessionID)
    output.append(identity.requestCanonicalHash)
    output.append(identity.requestWireHash)
    output.nfcAppendU32(receivedPaymentBytes)
    output.nfcAppendOptionalHash(paymentWireHash)
    output.nfcAppendU32(acknowledgementLength)
    output.nfcAppendOptionalHash(acknowledgementWireHash)
    return output
  }

  public static func decode(_ data: Data) throws -> Self {
    var reader = NfcReaderV1(data)
    guard try reader.read(4) == Data("NST1".utf8),
      try reader.u8() == IrohaPeerNfcV1.wireVersion,
      let phase = IrohaPeerNfcPhaseV1(rawValue: try reader.u8())
    else { throw IrohaPeerNfcErrorV1.invalidEncoding }
    let flags = IrohaPeerNfcFlagsV1(rawValue: try reader.u8())
    guard let profile = IrohaPeerWireProfileV1(rawValue: UInt16(try reader.u8())),
      profile != .reject
    else { throw IrohaPeerNfcErrorV1.invalidProfile }
    let identity = try IrohaPeerNfcRequestIdentityV1(
      profile: profile, sessionID: reader.read(16),
      requestCanonicalHash: reader.read(32), requestWireHash: reader.read(32))
    let received = try reader.u32()
    let paymentHash = try reader.optionalHash()
    let acknowledgementLength = try reader.u32()
    let acknowledgementHash = try reader.optionalHash()
    try reader.finish()
    return Self(
      phase: phase, flags: flags, identity: identity,
      receivedPaymentBytes: received, paymentWireHash: paymentHash,
      acknowledgementLength: acknowledgementLength,
      acknowledgementWireHash: acknowledgementHash)
  }
}

/// Header-derived immutable descriptor for one payment transfer.
public struct IrohaPeerNfcPaymentDescriptorV1: Equatable, Sendable {
  public let profile: IrohaPeerWireProfileV1
  public let schemaVersion: UInt16
  public let messageLength: Int
  public let canonicalHash: Data
  public let wireHash: Data

  public init(payment: IrohaPeerWireMessageV1) throws {
    guard payment.kind == .payment else { throw IrohaPeerNfcErrorV1.invalidKind }
    profile = payment.profile
    schemaVersion = payment.schemaVersion
    messageLength = payment.encoded.count
    canonicalHash = payment.canonicalHash
    wireHash = payment.wireHash
  }

  private init(
    profile: IrohaPeerWireProfileV1, schemaVersion: UInt16,
    messageLength: Int, canonicalHash: Data, wireHash: Data
  ) throws {
    guard profile != .reject, schemaVersion == profile.requiredSchemaVersion,
      (1...IrohaPeerNfcV1.maximumMessageBytes).contains(messageLength),
      canonicalHash.count == 32, wireHash.count == 32,
      canonicalHash.contains(where: { $0 != 0 }), wireHash.contains(where: { $0 != 0 })
    else { throw IrohaPeerNfcErrorV1.invalidDescriptor }
    self.profile = profile
    self.schemaVersion = schemaVersion
    self.messageLength = messageLength
    self.canonicalHash = Data(canonicalHash)
    self.wireHash = Data(wireHash)
  }

  public var encoded: Data {
    var output = Data([UInt8(profile.rawValue)])
    output.nfcAppendU16(Int(schemaVersion))
    output.nfcAppendU32(messageLength)
    output.append(canonicalHash)
    output.append(wireHash)
    return output
  }

  public static func decode(_ data: Data) throws -> Self {
    var reader = NfcReaderV1(data)
    guard let profile = IrohaPeerWireProfileV1(rawValue: UInt16(try reader.u8())) else {
      throw IrohaPeerNfcErrorV1.invalidProfile
    }
    let value = try Self(
      profile: profile, schemaVersion: UInt16(reader.u16()),
      messageLength: reader.u32(), canonicalHash: reader.read(32), wireHash: reader.read(32))
    try reader.finish()
    return value
  }
}

/// Immutable, strictly bounded APDU command.
public enum IrohaPeerNfcCommandV1: Equatable, Sendable {
  case selectApplication
  case getInfo
  case readRequest(offset: Int, length: Int)
  case beginPayment(IrohaPeerNfcPaymentDescriptorV1)
  case writePayment(offset: Int, bytes: Data)
  case commitPayment
  case readAcknowledgement(offset: Int, length: Int)
  case confirmAcknowledgement
  case getStatus
  case resetSession
}

/// ISO-7816-compatible envelope for the closed command inventory.
public enum IrohaPeerNfcAPDUCodecV1 {
  public static func encode(_ command: IrohaPeerNfcCommandV1) throws -> Data {
    if command == .selectApplication {
      var select = Data([0x00, 0xa4, 0x04, 0x00])
      select.append(UInt8(IrohaPeerNfcV1.applicationIdentifier.count))
      select.append(IrohaPeerNfcV1.applicationIdentifier)
      select.append(0)
      return select
    }
    let instruction: IrohaPeerNfcInstructionV1
    var body = Data()
    switch command {
    case .selectApplication: preconditionFailure("handled above")
    case .getInfo: instruction = .getInfo
    case .readRequest(let offset, let length):
      instruction = .readRequest; body.nfcAppendU32(offset); body.nfcAppendU16(length)
    case .beginPayment(let descriptor): instruction = .beginPayment; body = descriptor.encoded
    case .writePayment(let offset, let bytes):
      guard !bytes.isEmpty, bytes.count <= IrohaPeerNfcV1.maximumChunkBytes else {
        throw IrohaPeerNfcErrorV1.invalidLength
      }
      instruction = .writePayment; body.nfcAppendU32(offset); body.append(bytes)
    case .commitPayment: instruction = .commitPayment
    case .readAcknowledgement(let offset, let length):
      instruction = .readAcknowledgement; body.nfcAppendU32(offset); body.nfcAppendU16(length)
    case .confirmAcknowledgement: instruction = .confirmAcknowledgement
    case .getStatus: instruction = .getStatus
    case .resetSession: instruction = .resetSession
    }
    guard body.count <= Int(UInt16.max) else { throw IrohaPeerNfcErrorV1.invalidLength }
    var output = Data([IrohaPeerNfcV1.commandClass, instruction.rawValue, 0, 0])
    output.nfcAppendU16(body.count)
    output.append(body)
    output.append(0)
    return output
  }

  public static func decode(_ data: Data) throws -> IrohaPeerNfcCommandV1 {
    let select = try encode(.selectApplication)
    if data == select { return .selectApplication }
    var reader = NfcReaderV1(data)
    guard try reader.u8() == IrohaPeerNfcV1.commandClass,
      let instruction = IrohaPeerNfcInstructionV1(rawValue: try reader.u8()),
      try reader.u8() == 0, try reader.u8() == 0
    else { throw IrohaPeerNfcErrorV1.invalidEncoding }
    var body = NfcReaderV1(try reader.read(reader.u16()))
    guard try reader.u8() == 0 else { throw IrohaPeerNfcErrorV1.invalidEncoding }
    try reader.finish()
    let command: IrohaPeerNfcCommandV1
    switch instruction {
    case .getInfo: command = .getInfo
    case .readRequest: command = .readRequest(offset: try body.u32(), length: try body.u16())
    case .beginPayment: command = .beginPayment(try .decode(body.remaining()))
    case .writePayment:
      command = .writePayment(offset: try body.u32(), bytes: body.remaining())
    case .commitPayment: command = .commitPayment
    case .readAcknowledgement:
      command = .readAcknowledgement(offset: try body.u32(), length: try body.u16())
    case .confirmAcknowledgement: command = .confirmAcknowledgement
    case .getStatus: command = .getStatus
    case .resetSession: command = .resetSession
    }
    try body.finish()
    return command
  }
}

/// Public inputs handed to the irreversible hardware staging callback.
public final class IrohaPeerNfcPaymentAdmissionContextV1: @unchecked Sendable {
  public let canonicalRequest: Data
  public let canonicalPayment: Data
  public init(canonicalRequest: Data, canonicalPayment: Data) {
    self.canonicalRequest = Data(canonicalRequest)
    self.canonicalPayment = Data(canonicalPayment)
  }
}

/// Durable result containing the byte-identical acknowledgement.
public struct IrohaPeerNfcDurablePaymentAdmissionV1: Sendable {
  public let context: IrohaPeerNfcPaymentAdmissionContextV1
  public let canonicalAcknowledgement: Data
  public init(
    context: IrohaPeerNfcPaymentAdmissionContextV1,
    canonicalAcknowledgement: Data
  ) throws {
    guard !canonicalAcknowledgement.isEmpty else { throw IrohaPeerNfcErrorV1.invalidLength }
    self.context = context
    self.canonicalAcknowledgement = Data(canonicalAcknowledgement)
  }
}

/// Receiver action. `stagePayment` must complete durably before any ACK is exposed.
public enum IrohaPeerNfcReceiverActionV1: Sendable {
  case response(Data)
  case stagePayment(IrohaPeerNfcPaymentAdmissionContextV1)
}

/// Receiver session for exactly three messages.
public struct IrohaPeerNfcReceiverSessionV1: Sendable {
  public let identity: IrohaPeerNfcRequestIdentityV1
  public let profilePolicy: IrohaPeerNfcProfilePolicyV1
  public let limits: IrohaPeerNfcLimitsV1
  private let requestBytes: Data
  private let request: IrohaPeerWireMessageV1
  private var phaseValue: IrohaPeerNfcPhaseV1 = .requestReady
  private var descriptor: IrohaPeerNfcPaymentDescriptorV1?
  private var paymentBuffer = Data()
  private var paymentBytes: Data?
  private var acknowledgementBytes: Data?

  public init(
    canonicalRequest: Data, sessionID: Data,
    profilePolicy: IrohaPeerNfcProfilePolicyV1,
    limits: IrohaPeerNfcLimitsV1 = .default
  ) throws {
    let request = try decodeNfcMessage(
      canonicalRequest, profile: profilePolicy.profile, kind: .request, limits: limits)
    self.requestBytes = Data(canonicalRequest)
    self.request = request
    self.profilePolicy = profilePolicy
    self.limits = limits
    identity = try IrohaPeerNfcRequestIdentityV1(
      profile: request.profile, sessionID: sessionID,
      requestCanonicalHash: request.canonicalHash, requestWireHash: request.wireHash)
  }

  public var phase: IrohaPeerNfcPhaseV1 { phaseValue }

  public func info() throws -> IrohaPeerNfcInfoV1 {
    try .init(
      phase: phaseValue,
      flags: phaseValue == .acknowledgementReady || phaseValue == .complete ? .durable : .request,
      identity: identity, requestLength: requestBytes.count,
      maximumReadChunkBytes: limits.maximumReadChunkBytes,
      maximumWriteChunkBytes: limits.maximumWriteChunkBytes)
  }

  public func status() throws -> IrohaPeerNfcStatusV1 {
    let payment = try paymentBytes.map { try IrohaPeerWireMessageV1.decode($0) }
    let acknowledgement = try acknowledgementBytes.map { try IrohaPeerWireMessageV1.decode($0) }
    return .init(
      phase: phaseValue,
      flags: phaseValue == .acknowledgementReady || phaseValue == .complete ? .durable : .request,
      identity: identity, receivedPaymentBytes: paymentBuffer.count,
      paymentWireHash: payment?.wireHash,
      acknowledgementLength: acknowledgementBytes?.count ?? 0,
      acknowledgementWireHash: acknowledgement?.wireHash)
  }

  public mutating func handle(_ command: IrohaPeerNfcCommandV1) throws
    -> IrohaPeerNfcReceiverActionV1
  {
    switch command {
    case .selectApplication: return .response(Data())
    case .getInfo: return .response(try info().encoded)
    case .getStatus: return .response(try status().encoded)
    case .readRequest(let offset, let length):
      return .response(try nfcRange(requestBytes, offset: offset, length: length))
    case .beginPayment(let next):
      guard phaseValue == .requestReady, profilePolicy.accepts(next.profile),
        next.schemaVersion == profilePolicy.profile.requiredSchemaVersion,
        next.messageLength <= limits.maximumMessageBytes
      else { throw IrohaPeerNfcErrorV1.stateMismatch }
      descriptor = next
      paymentBuffer = Data()
      paymentBytes = nil
      phaseValue = .paymentReceiving
      return .response(Data())
    case .writePayment(let offset, let bytes):
      guard phaseValue == .paymentReceiving, offset == paymentBuffer.count,
        !bytes.isEmpty, bytes.count <= limits.maximumWriteChunkBytes,
        let descriptor, paymentBuffer.count + bytes.count <= descriptor.messageLength
      else { throw IrohaPeerNfcErrorV1.invalidOffset }
      paymentBuffer.append(bytes)
      return .response(Data())
    case .commitPayment:
      guard phaseValue == .paymentReceiving, let descriptor,
        paymentBuffer.count == descriptor.messageLength
      else { throw IrohaPeerNfcErrorV1.stateMismatch }
      let payment = try decodeNfcMessage(
        paymentBuffer, profile: profilePolicy.profile, kind: .payment, limits: limits)
      guard payment.canonicalHash == descriptor.canonicalHash,
        payment.wireHash == descriptor.wireHash
      else { throw IrohaPeerNfcErrorV1.continuityMismatch }
      try validateNfcExchange(request: request, payment: payment, acknowledgement: nil)
      paymentBytes = paymentBuffer
      return .stagePayment(.init(
        canonicalRequest: requestBytes, canonicalPayment: paymentBuffer))
    case .readAcknowledgement(let offset, let length):
      guard phaseValue == .acknowledgementReady || phaseValue == .complete,
        let acknowledgementBytes
      else { throw IrohaPeerNfcErrorV1.stateMismatch }
      return .response(try nfcRange(acknowledgementBytes, offset: offset, length: length))
    case .confirmAcknowledgement:
      guard phaseValue == .acknowledgementReady || phaseValue == .complete else {
        throw IrohaPeerNfcErrorV1.stateMismatch
      }
      phaseValue = .complete
      return .response(Data())
    case .resetSession:
      guard phaseValue == .requestReady || phaseValue == .paymentReceiving else {
        throw IrohaPeerNfcErrorV1.stateMismatch
      }
      descriptor = nil
      paymentBuffer = Data()
      paymentBytes = nil
      phaseValue = .requestReady
      return .response(Data())
    }
  }

  /// Publish an ACK only after the supplied context was irreversibly staged.
  public mutating func completePayment(
    context: IrohaPeerNfcPaymentAdmissionContextV1,
    durable: IrohaPeerNfcDurablePaymentAdmissionV1
  ) throws {
    guard durable.context === context, phaseValue == .paymentReceiving,
      context.canonicalRequest == requestBytes,
      context.canonicalPayment == paymentBytes
    else { throw IrohaPeerNfcErrorV1.continuityMismatch }
    let acknowledgement = try decodeNfcMessage(
      durable.canonicalAcknowledgement, profile: profilePolicy.profile,
      kind: .acknowledgement, limits: limits)
    guard let paymentBytes else { throw IrohaPeerNfcErrorV1.stateMismatch }
    let payment = try decodeNfcMessage(
      paymentBytes, profile: profilePolicy.profile, kind: .payment, limits: limits)
    try validateNfcExchange(
      request: request, payment: payment, acknowledgement: acknowledgement)
    acknowledgementBytes = durable.canonicalAcknowledgement
    phaseValue = .acknowledgementReady
  }

  public mutating func rejectPayment(context: IrohaPeerNfcPaymentAdmissionContextV1) {
    guard context.canonicalRequest == requestBytes else { return }
    descriptor = nil
    paymentBuffer = Data()
    paymentBytes = nil
    phaseValue = .requestReady
  }
}

/// ISO-7816 status words used by the transport-neutral reducer.
public enum IrohaPeerNfcStatusWordV1: UInt16, CaseIterable, Sendable {
  case success = 0x9000
  case wrongData = 0x6a80
  case notFound = 0x6a82
  case wrongLength = 0x6700
  case conditionsNotSatisfied = 0x6985
  case securityStatusNotSatisfied = 0x6982
  case storageFailure = 0x6581
  case instructionNotSupported = 0x6d00
  case classNotSupported = 0x6e00
}

public struct IrohaPeerNfcAPDUResponseV1: Equatable, Sendable {
  public let data: Data
  public let statusWord: IrohaPeerNfcStatusWordV1
  public init(data: Data = Data(), statusWord: IrohaPeerNfcStatusWordV1 = .success) {
    self.data = Data(data)
    self.statusWord = statusWord
  }
  public var encoded: Data {
    data + Data([UInt8(statusWord.rawValue >> 8), UInt8(statusWord.rawValue & 0xff)])
  }
}

public protocol IrohaPeerNfcAmbiguousResponseErrorV1: Error {}

public struct IrohaPeerNfcReaderExchangeResultV1: Equatable, Sendable {
  public let request: IrohaPeerWireMessageV1
  public let payment: IrohaPeerWireMessageV1
  public let acknowledgement: IrohaPeerWireMessageV1
}

/// Synchronous transport-neutral reader flow for exactly three messages.
public enum IrohaPeerNfcReaderExchangeV1 {
  public typealias Transceive = (IrohaPeerNfcCommandV1) throws -> IrohaPeerNfcAPDUResponseV1
  public typealias PreparePayment = (IrohaPeerWireMessageV1) throws -> IrohaPeerWireMessageV1

  public static func run(
    profilePolicy: IrohaPeerNfcProfilePolicyV1,
    limits: IrohaPeerNfcLimitsV1 = .default,
    transceive: Transceive,
    preparePayment: PreparePayment
  ) throws -> IrohaPeerNfcReaderExchangeResultV1 {
    func send(_ command: IrohaPeerNfcCommandV1) throws -> Data {
      let response = try transceive(command)
      guard response.statusWord == .success else {
        throw IrohaPeerNfcErrorV1.peerStatus(response.statusWord)
      }
      return response.data
    }
    let info = try IrohaPeerNfcInfoV1.decode(send(.getInfo))
    guard profilePolicy.accepts(info.identity.profile) else {
      throw IrohaPeerNfcErrorV1.invalidProfile
    }
    let requestBytes = try nfcReadChunks(
      length: info.requestLength, chunkSize: info.maximumReadChunkBytes
    ) { try send(.readRequest(offset: $0, length: $1)) }
    let request = try decodeNfcMessage(
      requestBytes, profile: profilePolicy.profile, kind: .request, limits: limits)
    let payment = try preparePayment(request)
    guard payment.profile == profilePolicy.profile, payment.kind == .payment else {
      throw IrohaPeerNfcErrorV1.invalidKind
    }
    let paymentBytes = payment.encoded
    _ = try send(.beginPayment(try .init(payment: payment)))
    var offset = 0
    while offset < paymentBytes.count {
      let end = min(paymentBytes.count, offset + info.maximumWriteChunkBytes)
      _ = try send(.writePayment(offset: offset, bytes: paymentBytes.subdata(in: offset..<end)))
      offset = end
    }
    _ = try send(.commitPayment)
    let status = try IrohaPeerNfcStatusV1.decode(send(.getStatus))
    guard status.phase == .acknowledgementReady || status.phase == .complete else {
      throw IrohaPeerNfcErrorV1.stateMismatch
    }
    let acknowledgementBytes = try nfcReadChunks(
      length: status.acknowledgementLength, chunkSize: info.maximumReadChunkBytes
    ) { try send(.readAcknowledgement(offset: $0, length: $1)) }
    let acknowledgement = try decodeNfcMessage(
      acknowledgementBytes, profile: profilePolicy.profile,
      kind: .acknowledgement, limits: limits)
    try validateNfcExchange(
      request: request, payment: payment, acknowledgement: acknowledgement)
    _ = try send(.confirmAcknowledgement)
    return .init(request: request, payment: payment, acknowledgement: acknowledgement)
  }
}

public enum IrohaPeerNfcErrorV1: Error, Equatable, Sendable {
  case invalidEncoding
  case invalidLength
  case invalidOffset
  case invalidIdentity
  case invalidDescriptor
  case invalidProfile
  case invalidKind
  case stateMismatch
  case continuityMismatch
  case peerStatus(IrohaPeerNfcStatusWordV1)
}

private func decodeNfcMessage(
  _ data: Data, profile: IrohaPeerWireProfileV1,
  kind: IrohaPeerWireKindV1, limits: IrohaPeerNfcLimitsV1
) throws -> IrohaPeerWireMessageV1 {
  guard (1...limits.maximumMessageBytes).contains(data.count) else {
    throw IrohaPeerNfcErrorV1.invalidLength
  }
  return try IrohaPeerWireMessageV1.decode(
    data, expectedProfile: profile, expectedKind: kind)
}

private func validateNfcExchange(
  request: IrohaPeerWireMessageV1,
  payment: IrohaPeerWireMessageV1,
  acknowledgement: IrohaPeerWireMessageV1?
) throws {
  let requestModel = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
    request.canonicalPayload)
  let paymentModel = try KagemushaNoritoV1.decodePaymentShapeExact(
    payment.canonicalPayload, against: requestModel)
  if let acknowledgement {
    _ = try KagemushaNoritoV1.decodeAcknowledgementShapeExact(
      acknowledgement.canonicalPayload, against: requestModel, payment: paymentModel)
  }
}

private func nfcReadChunks(
  length: Int, chunkSize: Int,
  read: (Int, Int) throws -> Data
) throws -> Data {
  guard length > 0 else { throw IrohaPeerNfcErrorV1.invalidLength }
  var output = Data()
  var offset = 0
  while offset < length {
    let count = min(chunkSize, length - offset)
    let chunk = try read(offset, count)
    guard chunk.count == count else { throw IrohaPeerNfcErrorV1.invalidLength }
    output.append(chunk)
    offset += count
  }
  return output
}

private func nfcRange(_ data: Data, offset: Int, length: Int) throws -> Data {
  guard offset >= 0, length > 0, offset <= data.count - length else {
    throw IrohaPeerNfcErrorV1.invalidOffset
  }
  return data.subdata(in: offset..<(offset + length))
}

private struct NfcReaderV1 {
  private let data: Data
  private var offset = 0
  init(_ data: Data) { self.data = Data(data) }
  mutating func u8() throws -> UInt8 { try read(1)[0] }
  mutating func u16() throws -> Int {
    let bytes = try read(2); return Int(bytes[0]) << 8 | Int(bytes[1])
  }
  mutating func u32() throws -> Int {
    let bytes = try read(4)
    let value = UInt64(bytes[0]) << 24 | UInt64(bytes[1]) << 16
      | UInt64(bytes[2]) << 8 | UInt64(bytes[3])
    guard value <= UInt64(Int.max) else { throw IrohaPeerNfcErrorV1.invalidLength }
    return Int(value)
  }
  mutating func optionalHash() throws -> Data? {
    switch try u8() {
    case 0: return nil
    case 1:
      let value = try read(32)
      guard value.contains(where: { $0 != 0 }) else {
        throw IrohaPeerNfcErrorV1.invalidIdentity
      }
      return value
    default: throw IrohaPeerNfcErrorV1.invalidEncoding
    }
  }
  mutating func read(_ count: Int) throws -> Data {
    guard count >= 0, count <= data.count - offset else {
      throw IrohaPeerNfcErrorV1.invalidLength
    }
    defer { offset += count }
    return data.subdata(in: offset..<(offset + count))
  }
  mutating func remaining() -> Data {
    let result = data.subdata(in: offset..<data.count); offset = data.count; return result
  }
  func finish() throws {
    guard offset == data.count else { throw IrohaPeerNfcErrorV1.invalidEncoding }
  }
}

private extension Data {
  mutating func nfcAppendU16(_ value: Int) {
    precondition((0...Int(UInt16.max)).contains(value))
    append(UInt8((value >> 8) & 0xff)); append(UInt8(value & 0xff))
  }
  mutating func nfcAppendU32(_ value: Int) {
    precondition(value >= 0 && UInt64(value) <= UInt64(UInt32.max))
    append(UInt8((value >> 24) & 0xff)); append(UInt8((value >> 16) & 0xff))
    append(UInt8((value >> 8) & 0xff)); append(UInt8(value & 0xff))
  }
  mutating func nfcAppendOptionalHash(_ value: Data?) {
    append(value == nil ? 0 : 1); if let value { append(value) }
  }
}

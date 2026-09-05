// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation
import IrohaSwift

#if os(iOS) && canImport(CoreNFC)
@preconcurrency import CoreNFC

/// Core NFC failures for the direct three-message KAGEMUSHA exchange.
public enum IrohaPeerNfcCoreNFCErrorV1: Error, Equatable, LocalizedError, Sendable {
  case invalidCommandAPDU
  case unavailable
  case invalidTag
  case cancelled
  case operationInProgress
  case peerStatus(IrohaPeerNfcStatusWordV1)

  public var errorDescription: String? {
    switch self {
    case .invalidCommandAPDU: "Core NFC could not represent the KAGEMUSHA command."
    case .unavailable: "NFC is unavailable on this device."
    case .invalidTag: "The detected tag is not a KAGEMUSHA V1 peer."
    case .cancelled: "The NFC exchange was cancelled."
    case .operationInProgress: "Another NFC exchange is already active."
    case .peerStatus(let status): "The NFC peer rejected the command (\(status.rawValue))."
    }
  }
}

/// Thin Core NFC conversion layer around the transport-neutral reducer.
public enum IrohaPeerNfcCoreNFCAdapterV1 {
  @available(iOS 15.0, *)
  public static func readerAPDU(for command: IrohaPeerNfcCommandV1) throws -> NFCISO7816APDU {
    guard let apdu = NFCISO7816APDU(data: try IrohaPeerNfcAPDUCodecV1.encode(command)) else {
      throw IrohaPeerNfcCoreNFCErrorV1.invalidCommandAPDU
    }
    return apdu
  }

  @available(iOS 15.0, *)
  public static func transceive(
    _ command: IrohaPeerNfcCommandV1,
    using tag: NFCISO7816Tag
  ) async throws -> IrohaPeerNfcAPDUResponseV1 {
    let apdu = try readerAPDU(for: command)
    return try await withCheckedThrowingContinuation { continuation in
      tag.sendCommand(apdu: apdu) { data, sw1, sw2, error in
        if let error { continuation.resume(throwing: error); return }
        let raw = UInt16(sw1) << 8 | UInt16(sw2)
        guard let status = IrohaPeerNfcStatusWordV1(rawValue: raw) else {
          continuation.resume(throwing: IrohaPeerNfcCoreNFCErrorV1.invalidCommandAPDU)
          return
        }
        continuation.resume(returning: .init(data: data, statusWord: status))
      }
    }
  }

  @available(iOS 17.4, *)
  public static func cardCommand(from apdu: CardSession.APDU) throws -> IrohaPeerNfcCommandV1 {
    try IrohaPeerNfcAPDUCodecV1.decode(apdu.payload)
  }

  @available(iOS 17.4, *)
  public static func respond(
    to apdu: CardSession.APDU,
    with response: IrohaPeerNfcAPDUResponseV1
  ) async throws {
    try await apdu.respond(response: response.encoded)
  }
}

/// App-owned presentation strings. The KAGEMUSHA application identifier is fixed.
public struct IrohaPeerNfcCoreNFCConfigurationV1: Equatable, Sendable {
  public let cardSessionRuntimeEnabled: Bool
  public let cardAlertMessage: String
  public let readerAlertMessage: String
  public let completionAlertMessage: String

  public init(
    cardSessionRuntimeEnabled: Bool,
    cardAlertMessage: String,
    readerAlertMessage: String,
    completionAlertMessage: String
  ) {
    self.cardSessionRuntimeEnabled = cardSessionRuntimeEnabled
    self.cardAlertMessage = cardAlertMessage
    self.readerAlertMessage = readerAlertMessage
    self.completionAlertMessage = completionAlertMessage
  }
}

public enum IrohaPeerNfcCardAvailabilityV1: Equatable, Sendable {
  case available
  case unavailable
  case runtimeDisabled
  case unsupported
  case ineligible
}

/// Events from the receiver/card side of the direct protocol.
public enum IrohaPeerNfcCardEventV1: Equatable, Sendable {
  case sessionActive
  case requestExposed
  case paymentStaging
  case acknowledgementReady
  case complete
  case failed
}

/// Serial card-side reducer. ACK bytes are unavailable until `stagePayment` returns durably.
@available(iOS 17.4, *)
public final class IrohaPeerNfcCardSessionControllerV1: @unchecked Sendable {
  public typealias StagePayment = @Sendable (
    IrohaPeerNfcPaymentAdmissionContextV1
  ) async throws -> IrohaPeerNfcDurablePaymentAdmissionV1
  public typealias EventHandler = @Sendable (IrohaPeerNfcCardEventV1) -> Void

  private let lock = NSLock()
  private var receiver: IrohaPeerNfcReceiverSessionV1?
  private var stagePayment: StagePayment?
  private var onEvent: EventHandler?

  public init(configuration _: IrohaPeerNfcCoreNFCConfigurationV1) {}

  @discardableResult
  public func start(
    sessionID: Data,
    receiveRequest: Data,
    profilePolicy: IrohaPeerNfcProfilePolicyV1,
    limits: IrohaPeerNfcLimitsV1 = .default,
    onEvent: @escaping EventHandler,
    stagePayment: @escaping StagePayment
  ) throws -> IrohaPeerNfcRequestIdentityV1 {
    let next = try IrohaPeerNfcReceiverSessionV1(
      canonicalRequest: receiveRequest, sessionID: sessionID,
      profilePolicy: profilePolicy, limits: limits)
    lock.lock()
    defer { lock.unlock() }
    guard receiver == nil else { throw IrohaPeerNfcCoreNFCErrorV1.operationInProgress }
    receiver = next
    self.stagePayment = stagePayment
    self.onEvent = onEvent
    onEvent(.sessionActive)
    return next.identity
  }

  public func stop() {
    lock.lock(); receiver = nil; stagePayment = nil; onEvent = nil; lock.unlock()
  }

  /// Process one CardSession APDU. The caller owns the CardSession event loop.
  public func process(_ apdu: CardSession.APDU) async {
    do {
      let command = try IrohaPeerNfcCoreNFCAdapterV1.cardCommand(from: apdu)
      let response = try await process(command)
      try await IrohaPeerNfcCoreNFCAdapterV1.respond(to: apdu, with: response)
    } catch {
      try? await IrohaPeerNfcCoreNFCAdapterV1.respond(
        to: apdu, with: .init(statusWord: .wrongData))
    }
  }

  public func process(_ command: IrohaPeerNfcCommandV1) async throws
    -> IrohaPeerNfcAPDUResponseV1
  {
    lock.lock()
    guard var current = receiver, let stage = stagePayment else {
      lock.unlock(); throw IrohaPeerNfcCoreNFCErrorV1.unavailable
    }
    do {
      let action = try current.handle(command)
      receiver = current
      lock.unlock()
      switch action {
      case .response(let data):
        if command == .confirmAcknowledgement { onEvent?(.complete) }
        if case .readRequest = command { onEvent?(.requestExposed) }
        return .init(data: data)
      case .stagePayment(let context):
        onEvent?(.paymentStaging)
        let durable = try await stage(context)
        lock.lock()
        guard var latest = receiver else {
          lock.unlock(); throw IrohaPeerNfcCoreNFCErrorV1.cancelled
        }
        try latest.completePayment(context: context, durable: durable)
        receiver = latest
        lock.unlock()
        onEvent?(.acknowledgementReady)
        return .init()
      }
    } catch {
      lock.unlock()
      onEvent?(.failed)
      throw error
    }
  }
}

/// Core NFC reader service for the direct three-message exchange.
@available(iOS 15.0, *)
public final class IrohaPeerNfcReaderServiceV1: NSObject, @unchecked Sendable,
  NFCTagReaderSessionDelegate
{
  public typealias PreparePayment = @Sendable (
    IrohaPeerWireMessageV1
  ) async throws -> IrohaPeerWireMessageV1

  public static var isAvailable: Bool {
#if targetEnvironment(simulator)
    false
#else
    NFCReaderSession.readingAvailable
#endif
  }

  private let configuration: IrohaPeerNfcCoreNFCConfigurationV1
  private let lock = NSLock()
  private var session: NFCTagReaderSession?
  private var continuation: CheckedContinuation<IrohaPeerNfcReaderExchangeResultV1, Error>?
  private var profilePolicy: IrohaPeerNfcProfilePolicyV1?
  private var limits: IrohaPeerNfcLimitsV1 = .default
  private var preparePayment: PreparePayment?

  public init(configuration: IrohaPeerNfcCoreNFCConfigurationV1) {
    self.configuration = configuration
    super.init()
  }

  public func run(
    profilePolicy: IrohaPeerNfcProfilePolicyV1,
    limits: IrohaPeerNfcLimitsV1 = .default,
    preparePayment: @escaping PreparePayment
  ) async throws -> IrohaPeerNfcReaderExchangeResultV1 {
    guard Self.isAvailable else { throw IrohaPeerNfcCoreNFCErrorV1.unavailable }
    return try await withTaskCancellationHandler {
      try await withCheckedThrowingContinuation { continuation in
        lock.lock()
        guard self.continuation == nil else {
          lock.unlock()
          continuation.resume(throwing: IrohaPeerNfcCoreNFCErrorV1.operationInProgress)
          return
        }
        self.continuation = continuation
        self.profilePolicy = profilePolicy
        self.limits = limits
        self.preparePayment = preparePayment
        let session = NFCTagReaderSession(
          pollingOption: [.iso14443], delegate: self, queue: nil)
        self.session = session
        session?.alertMessage = configuration.readerAlertMessage
        lock.unlock()
        session?.begin()
      }
    } onCancel: { [weak self] in
      self?.finish(.failure(IrohaPeerNfcCoreNFCErrorV1.cancelled))
    }
  }

  public func tagReaderSessionDidBecomeActive(_ session: NFCTagReaderSession) {}

  public func tagReaderSession(
    _ session: NFCTagReaderSession,
    didInvalidateWithError error: Error
  ) {
    finish(.failure(error))
  }

  public func tagReaderSession(_ session: NFCTagReaderSession, didDetect tags: [NFCTag]) {
    guard tags.count == 1, case .iso7816(let tag) = tags[0] else {
      session.invalidate(errorMessage: "KAGEMUSHA requires one ISO 7816 peer.")
      return
    }
    Task { [weak self] in
      guard let self else { return }
      do {
        try await connect(tag, session: session)
        let result = try await exchange(tag)
        session.alertMessage = configuration.completionAlertMessage
        session.invalidate()
        finish(.success(result))
      } catch {
        session.invalidate(errorMessage: "KAGEMUSHA exchange failed.")
        finish(.failure(error))
      }
    }
  }

  private func connect(_ tag: NFCISO7816Tag, session: NFCTagReaderSession) async throws {
    try await withCheckedThrowingContinuation { continuation in
      session.connect(to: .iso7816(tag)) { error in
        if let error { continuation.resume(throwing: error) }
        else { continuation.resume(returning: ()) }
      }
    }
  }

  private func exchange(_ tag: NFCISO7816Tag) async throws
    -> IrohaPeerNfcReaderExchangeResultV1
  {
    guard let policy = profilePolicy, let prepare = preparePayment else {
      throw IrohaPeerNfcCoreNFCErrorV1.cancelled
    }
    func send(_ command: IrohaPeerNfcCommandV1) async throws -> Data {
      let response = try await IrohaPeerNfcCoreNFCAdapterV1.transceive(command, using: tag)
      guard response.statusWord == .success else {
        throw IrohaPeerNfcCoreNFCErrorV1.peerStatus(response.statusWord)
      }
      return response.data
    }
    _ = try await send(.selectApplication)
    let info = try IrohaPeerNfcInfoV1.decode(await send(.getInfo))
    guard policy.accepts(info.identity.profile) else {
      throw IrohaPeerNfcCoreNFCErrorV1.invalidTag
    }
    let requestBytes = try await readChunks(
      length: info.requestLength, chunk: info.maximumReadChunkBytes
    ) { try await send(.readRequest(offset: $0, length: $1)) }
    let request = try IrohaPeerWireMessageV1.decode(
      requestBytes, expectedProfile: policy.profile, expectedKind: .request)
    let payment = try await prepare(request)
    guard payment.profile == policy.profile, payment.kind == .payment else {
      throw IrohaPeerNfcCoreNFCErrorV1.invalidTag
    }
    _ = try await send(.beginPayment(try .init(payment: payment)))
    var offset = 0
    while offset < payment.encoded.count {
      let end = min(payment.encoded.count, offset + info.maximumWriteChunkBytes)
      _ = try await send(.writePayment(
        offset: offset, bytes: payment.encoded.subdata(in: offset..<end)))
      offset = end
    }
    _ = try await send(.commitPayment)
    let status = try IrohaPeerNfcStatusV1.decode(await send(.getStatus))
    guard status.phase == .acknowledgementReady || status.phase == .complete else {
      throw IrohaPeerNfcCoreNFCErrorV1.invalidTag
    }
    let acknowledgementBytes = try await readChunks(
      length: status.acknowledgementLength, chunk: info.maximumReadChunkBytes
    ) { try await send(.readAcknowledgement(offset: $0, length: $1)) }
    let acknowledgement = try IrohaPeerWireMessageV1.decode(
      acknowledgementBytes, expectedProfile: policy.profile,
      expectedKind: .acknowledgement)
    let requestModel = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
      request.canonicalPayload)
    let paymentModel = try KagemushaNoritoV1.decodePaymentShapeExact(
      payment.canonicalPayload, against: requestModel)
    _ = try KagemushaNoritoV1.decodeAcknowledgementShapeExact(
      acknowledgement.canonicalPayload, against: requestModel, payment: paymentModel)
    _ = try await send(.confirmAcknowledgement)
    return .init(request: request, payment: payment, acknowledgement: acknowledgement)
  }

  private func finish(_ result: Result<IrohaPeerNfcReaderExchangeResultV1, Error>) {
    lock.lock()
    guard let continuation else { lock.unlock(); return }
    self.continuation = nil
    session = nil
    profilePolicy = nil
    preparePayment = nil
    lock.unlock()
    continuation.resume(with: result)
  }
}

private func readChunks(
  length: Int, chunk: Int,
  read: (Int, Int) async throws -> Data
) async throws -> Data {
  var output = Data()
  var offset = 0
  while offset < length {
    let count = min(chunk, length - offset)
    let bytes = try await read(offset, count)
    guard bytes.count == count else { throw IrohaPeerNfcCoreNFCErrorV1.invalidTag }
    output.append(bytes)
    offset += count
  }
  return output
}
#endif

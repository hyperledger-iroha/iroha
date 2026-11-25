import Foundation
import NoritoBridge
#if os(iOS)
import UIKit
#endif

/// Exact future-profile account admission DATA; only Native grants authority.
public struct KagemushaSpartanAccountOriginalsV1: Sendable {
  let originals: KagemushaWalletOpenOriginalsV1
  public init(credential: Data, enrollmentCertificate: Data, account: Data, assetScope: Data) throws {
    guard credential.count == 596, enrollmentCertificate.count == 172
    else { throw KagemushaWalletErrorV1.invalidInput }
    originals = try .init(credential: credential, enrollmentCertificates: enrollmentCertificate,
      account: account, assetScope: assetScope)
  }
}

/// Experimental proof backend sharing the admitted wallet's exclusive hardware custody owner.
public final class KagemushaSpartanWalletV1: @unchecked Sendable {
  private let owner: KagemushaWalletNativeLeaseV1
  private let driver: KagemushaSpartanDriverV1
  public let parameterId: Data
  public let slotId: Data
  /// True when Native admits only exact retained output reads after delivery-data loss.
  public let retainedOutputsOnly: Bool
  fileprivate init(owner: KagemushaWalletNativeLeaseV1, driver: KagemushaSpartanDriverV1, parameterId: Data, slotId: Data, retainedOutputsOnly: Bool) {
    self.owner = owner; self.driver = driver; self.parameterId = parameterId; self.slotId = slotId
    self.retainedOutputsOnly = retainedOutputsOnly
  }

  /// Check a released Unload against one exact account-signed transaction and its certified result.
  /// Passing an empty proof reads the previously selected local decision.
  public func unloadSettlement(operationId: Data, signedTransaction: Data, finality: Data = Data()) throws -> KagemushaSpartanUnloadSettlementV1 {
    guard !retainedOutputsOnly, operationId.count == 32, (1...65_536).contains(signedTransaction.count),
          finality.count <= 1_327_104 else { throw KagemushaWalletErrorV1.invalidInput }
    let handle = try owner.handle()
    let function = try driver.settlementFunction()
    let reply = try driver.call { out in
      slotId.withUnsafeBytes { slot in operationId.withUnsafeBytes { op in
        signedTransaction.withUnsafeBytes { signed in finality.withUnsafeBytes { proof in
          function(handle, slot.bindMemory(to: UInt8.self).baseAddress, op.bindMemory(to: UInt8.self).baseAddress,
                   signed.bindMemory(to: UInt8.self).baseAddress, signed.count,
                   proof.bindMemory(to: UInt8.self).baseAddress, proof.count, out)
        } }
      } }
    }
    return try .init(reply)
  }

  /// Check finality for this exact account-signed activation, or read its durable decision with an empty proof.
  public func activationSettlement(signedTransaction: Data, finality: Data = Data()) throws -> KagemushaSpartanActivationSettlementV1 {
    guard !retainedOutputsOnly, (1...65_536).contains(signedTransaction.count), finality.count <= 1_327_104 else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    let handle = try owner.handle()
    let function = try driver.activationSettlementFunction()
    return try .init(driver.call { out in
      slotId.withUnsafeBytes { slot in signedTransaction.withUnsafeBytes { signed in finality.withUnsafeBytes { proof in
        function(handle, slot.bindMemory(to: UInt8.self).baseAddress, signed.bindMemory(to: UInt8.self).baseAddress, signed.count,
                 proof.bindMemory(to: UInt8.self).baseAddress, proof.count, out)
      } } }
    })
  }

  /// Read the last authenticated committee selected by this wallet.
  public func epochProgress() throws -> KagemushaWalletEpochProgressV1 {
    try exchangeEpoch(0, Data())
  }

  /// Verify the incumbent committee's final certificate and retain its successor under the wallet marker.
  public func ingestEpochBoundary(expectedEpoch: UInt64, original: Data) throws -> KagemushaWalletEpochProgressV1 {
    guard (1...262_144).contains(original.count) else { throw KagemushaWalletErrorV1.invalidInput }
    return try exchangeEpoch(expectedEpoch, original)
  }

  private func exchangeEpoch(_ expected: UInt64, _ original: Data) throws -> KagemushaWalletEpochProgressV1 {
    guard !retainedOutputsOnly else { throw KagemushaWalletErrorV1.invalidInput }
    let handle = try owner.handle()
    let function = try driver.epochFunction()
    let reply = try driver.call { out in
      slotId.withUnsafeBytes { slot in original.withUnsafeBytes { boundary in
        function(handle, slot.bindMemory(to: UInt8.self).baseAddress, expected,
                 boundary.bindMemory(to: UInt8.self).baseAddress, boundary.count, out)
      } }
    }
    try reply.requireShape(status: 103, count: 24)
    return try .init(epoch: reply.settlementWord(at: 0), firstHeight: reply.settlementWord(at: 8),
                     boundaryHeight: reply.settlementWord(at: 16))
  }

  /// Issue or retrieve the receiver's exact signed Request for one offline exchange.
  public func issueOpenRequest(requestId: Data, amount: KagemushaWalletUInt128V1,
                               feeSchedule: Data = Data(), feeCertificate: Data = Data()) throws -> IrohaPeerWireMessageV1 {
    guard !retainedOutputsOnly, requestId.count == 32, requestId.contains(where: { $0 != 0 }),
          amount.low != 0 || amount.high != 0,
          (feeSchedule.isEmpty && feeCertificate.isEmpty) || (feeSchedule.count == 255 && feeCertificate.count == 172)
    else { throw KagemushaWalletErrorV1.invalidInput }
    let units = Data([amount.low, amount.high].flatMap { word in
      (0..<8).map { UInt8(truncatingIfNeeded: word >> ($0 * 8)) }
    })
    let handle = try owner.handle()
    let reply = try driver.call { out in
      slotId.withUnsafeBytes { slot in requestId.withUnsafeBytes { id in units.withUnsafeBytes { amount in
        feeSchedule.withUnsafeBytes { schedule in feeCertificate.withUnsafeBytes { certificate in
          driver.issueOpenRequest(handle, slot.bindMemory(to: UInt8.self).baseAddress,
            id.bindMemory(to: UInt8.self).baseAddress, amount.bindMemory(to: UInt8.self).baseAddress,
            schedule.bindMemory(to: UInt8.self).baseAddress, schedule.count,
            certificate.bindMemory(to: UInt8.self).baseAddress, certificate.count, out)
        } }
      } } }
    }
    return try reply.issuedOpenRequest(amount: units, feeSchedule: feeSchedule)
  }

  /// Verify and retain a complete higher-version local regulatory blacklist for this wallet.
  @discardableResult public func installBlacklist(original: Data, certificates: Data) throws -> Data {
    guard (1...2_228_736).contains(original.count), (1...10_034).contains(certificates.count)
    else { throw KagemushaWalletErrorV1.invalidInput }
    let handle = try owner.handle()
    let reply = try driver.call { out in
      slotId.withUnsafeBytes { slot in original.withUnsafeBytes { list in certificates.withUnsafeBytes { certs in
        driver.installBlacklist(handle, slot.bindMemory(to: UInt8.self).baseAddress,
          list.bindMemory(to: UInt8.self).baseAddress, list.count, certs.bindMemory(to: UInt8.self).baseAddress, certs.count, out)
      } } }
    }
    return try reply.blacklistDigest()
  }

  /// Capture a native request-time reading and nonce for the signed time service.
  public func beginTimeExchange() throws -> KagemushaWalletTimeExchangeV1 {
    let reply = try time(0)
    try reply.requireShape(status: 13, count: 32, token: true)
    guard reply.bytes.contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return .init(origin: self, token: reply.sequenceLow, nonce: reply.bytes)
  }

  /// Retain the signed response and the exact local observations in protected custody.
  public func finishTimeExchange(_ exchange: KagemushaWalletTimeExchangeV1, anchor: Data, certificate: Data) throws {
    guard (1...512).contains(anchor.count), (1...512).contains(certificate.count) else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    let token = try exchange.tokenFor(origin: self)
    _ = try owner.handle()
    try exchange.consume(origin: self)
    try time(1, token: token, first: anchor, second: certificate).requireShape(status: 14, count: 0)
  }

  /// Release an outstanding request without creating a local time observation.
  public func cancelTimeExchange(_ exchange: KagemushaWalletTimeExchangeV1) throws {
    let token = try exchange.tokenFor(origin: self)
    _ = try owner.handle()
    try exchange.consume(origin: self)
    try time(2, token: token).requireShape(status: 84, count: 0)
  }

  /// Capture the native time interval for one Send that uses signed-time controls.
  public func captureSendTime(operationId: Data) throws -> KagemushaSpartanTimeIntervalV1 {
    guard operationId.count == 32 else { throw KagemushaWalletErrorV1.invalidInput }
    let reply = try time(3, first: operationId)
    try reply.requireShape(status: 86, count: 16)
    return try .init(bytes: reply.bytes)
  }

  /// Abandon an interval captured before a Send review was prepared.
  public func abandonSendCapture(operationId: Data) throws {
    guard operationId.count == 32 else { throw KagemushaWalletErrorV1.invalidInput }
    try time(4, first: operationId).requireShape(status: 84, count: 0)
  }

  private func time(_ selector: UInt32, token: UInt64 = 0, first: Data = Data(), second: Data = Data()) throws -> KagemushaSpartanReplyV1 {
    let handle = try owner.handle()
    return try driver.call { out in
      slotId.withUnsafeBytes { slot in first.withUnsafeBytes { a in second.withUnsafeBytes { b in
        driver.time(handle, slot.bindMemory(to: UInt8.self).baseAddress, selector, token,
          a.bindMemory(to: UInt8.self).baseAddress, a.count, b.bindMemory(to: UInt8.self).baseAddress, b.count, out)
      } } }
    }
  }

  /// Prepare the initial native wallet state from its account-authorized enrollment.
  public func prepareBootstrap() throws -> KagemushaSpartanReviewV1 { try prepareOwned(.bootstrap, []) }

  /// Load only after Native verifies the supplied receipt and its canonical chain finality.
  public func prepareLoad(receiptOriginal: Data, finalityOriginal: Data) throws -> KagemushaSpartanReviewV1 {
    try prepareOwned(.load, [receiptOriginal, finalityOriginal])
  }

  public func prepareSend(request: IrohaPeerWireMessageV1) throws -> KagemushaSpartanReviewV1 {
    guard request.profile == .kagemushaSpartanPaymentV3, request.kind == .request else { throw KagemushaWalletErrorV1.invalidInput }
    return try prepareOwned(.send, [request.encoded])
  }

  public func prepareReceive(requestId: Data, payment: IrohaPeerWireMessageV1) throws -> KagemushaSpartanReviewV1 {
    guard requestId.count == 32, requestId.contains(where: { $0 != 0 }), payment.profile == .kagemushaSpartanPaymentV3,
          payment.kind == .payment else { throw KagemushaWalletErrorV1.invalidInput }
    return try prepareOwned(.receive, [requestId, payment.encoded])
  }

  /// Prepare one deferred ArchiveSent from the exact receipt retained for this Send.
  public func prepareArchiveSent(sendOperationId: Data, credited: IrohaPeerWireMessageV1) throws -> KagemushaSpartanReviewV1 {
    guard !retainedOutputsOnly else { throw KagemushaWalletErrorV1.invalidInput }
    return try prepareCommand(.archiveSent, Self.archiveSentCommand(sendOperationId: sendOperationId, credited: credited))
  }

  static func archiveSentCommand(sendOperationId: Data, credited: IrohaPeerWireMessageV1) throws -> Data {
    guard sendOperationId.count == 32, sendOperationId.contains(where: { $0 != 0 }),
          credited.profile == .kagemushaSpartanPaymentV3, credited.kind == .credited, credited.encoding == .none,
          credited.canonicalPayload.count <= 10_084 - IrohaPeerWireMessageV1.headerBytes else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    return try ownedCommand(.archiveSent, [sendOperationId, credited.encoded])
  }

  private func prepareOwned(_ operation: KagemushaSpartanOperationV1, _ originals: [Data]) throws -> KagemushaSpartanReviewV1 {
    try prepareCommand(operation, Self.ownedCommand(operation, originals))
  }

  private func prepareCommand(_ operation: KagemushaSpartanOperationV1, _ command: Data) throws -> KagemushaSpartanReviewV1 {
    return try prepare(witness: command, predecessorHead: Data()) { original in
      let projection = try KagemushaSpartanReviewProjectionV1(original)
      guard projection.operation == operation else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return projection
    }
  }

  static func ownedCommand(_ operation: KagemushaSpartanOperationV1, _ originals: [Data]) throws -> Data {
    try ownedCommand(tag: operation.rawValue, originals)
  }

  private static func ownedCommand(tag: UInt8, _ originals: [Data]) throws -> Data {
    var bytes = Data("KSO1".utf8); bytes.append(tag)
    for original in originals {
      guard !original.isEmpty, original.count <= maximumRecordBytes - 4 - bytes.count else {
        throw KagemushaWalletErrorV1.invalidInput
      }
      var length = UInt32(original.count).littleEndian
      withUnsafeBytes(of: &length) { bytes.append(contentsOf: $0) }
      bytes.append(original)
    }
    return bytes
  }

  /// Native authenticates the complete witness and freezes a hardware-signable review.
  public func prepare(witness: Data, predecessorHead: Data) throws -> KagemushaSpartanReviewV1 {
    try prepare(witness: witness, predecessorHead: predecessorHead, decode: KagemushaSpartanReviewProjectionV1.init)
  }

  /// Prepare Native's signed anchor update for the wallet's ordinary durable Advance.
  public func prepareRefreshTimeAnchor(witness: Data, predecessorHead: Data) throws -> KagemushaSpartanTimeAnchorReviewV2 {
    try prepare(witness: witness, predecessorHead: predecessorHead, decode: KagemushaSpartanTimeAnchorProjectionV2.init)
  }

  /// Prepare Native's signed anchor update from the exact canonical anchor and TimeAnchor-role
  /// certificate originals that `finishTimeExchange` accepts. Retry with the same originals.
  /// Only the signed floor can advance: a time-controlled Send still requires Native's own
  /// direct exchange for the held anchor.
  public func prepareRefreshTimeAnchor(anchor: Data, certificate: Data) throws -> KagemushaSpartanTimeAnchorReviewV2 {
    try prepareRefreshTimeAnchor(witness: Self.refreshTimeAnchorCommand(anchor: anchor, certificate: certificate),
                                 predecessorHead: Data())
  }

  static func refreshTimeAnchorCommand(anchor: Data, certificate: Data) throws -> Data {
    guard (1...512).contains(anchor.count), (1...512).contains(certificate.count) else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    return try ownedCommand(tag: 7, [anchor, certificate])
  }

  /// Retain the exact reviewed Unload for its protected confirmation.
  public func prepareUnload(witness: Data, predecessorHead: Data) throws -> KagemushaSpartanUnloadReviewV3 {
    try prepare(witness: witness, predecessorHead: predecessorHead, decode: KagemushaSpartanUnloadProjectionV3.init)
  }

  /// Prepare an Unload to the enrolled account. Retry with the original request id and charge terms.
  public func prepareUnload(requestId: Data, amount: KagemushaWalletUInt128V1, quote: Data = Data(),
                            certificates: Data = Data(), beneficiary: Data = Data()) throws -> KagemushaSpartanUnloadReviewV3 {
    let command = try Self.unloadCommand(requestId: requestId, amount: amount, quote: quote,
                                         certificates: certificates, beneficiary: beneficiary)
    return try prepareUnload(witness: command, predecessorHead: Data())
  }

  static func unloadCommand(requestId: Data, amount: KagemushaWalletUInt128V1,
                            quote: Data, certificates: Data, beneficiary: Data) throws -> Data {
    guard requestId.count == 32, requestId.contains(where: { $0 != 0 }), amount.low != 0 || amount.high != 0 else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    let units = Data([amount.low, amount.high].flatMap { word in
      (0..<8).map { UInt8(truncatingIfNeeded: word >> ($0 * 8)) }
    })
    var originals = [requestId, units]
    if !quote.isEmpty || !certificates.isEmpty || !beneficiary.isEmpty {
      guard (1...1024).contains(quote.count), (1...KagemushaWalletWireV1.messageMaximumBytes).contains(certificates.count),
            (1...4096).contains(beneficiary.count) else { throw KagemushaWalletErrorV1.invalidInput }
      originals += [quote, certificates, beneficiary]
    }
    return try ownedCommand(tag: 6, originals)
  }

  /// Read the authenticated Unload terms and release the unconfirmed preparation.
  public func reviewUnload(witness: Data, predecessorHead: Data) throws -> KagemushaSpartanUnloadProjectionV3 {
    let review = try prepare(witness: witness, predecessorHead: predecessorHead,
      decode: KagemushaSpartanUnloadProjectionV3.init)
    try review.discard()
    return review.projection
  }

  private func prepare<Projection: Sendable>(witness: Data, predecessorHead: Data,
    decode: (Data) throws -> Projection) throws -> KagemushaSpartanPreparedReviewV1<Projection> {
    guard !witness.isEmpty, witness.count <= Self.maximumRecordBytes,
      predecessorHead.count <= 1_048_576 else { throw KagemushaWalletErrorV1.invalidInput }
    let handle = try owner.handle()
    let reply = try driver.call { out in
      slotId.withUnsafeBytes { slot in witness.withUnsafeBytes { witness in predecessorHead.withUnsafeBytes { head in
        driver.prepare(handle, slot.bindMemory(to: UInt8.self).baseAddress,
          witness.bindMemory(to: UInt8.self).baseAddress, witness.count,
          head.bindMemory(to: UInt8.self).baseAddress, head.count, out)
      } } }
    }
    try reply.requireShape(status: 82, token: true)
    do {
      return try .init(owner: self, token: reply.sequenceLow, projection: decode(reply.bytes))
    } catch {
      try? discard(reply.sequenceLow)
      throw error
    }
  }

  /// Read the offline wallet's selected native stage and its exact available balance, when Released.
  public func snapshot() throws -> KagemushaSpartanSnapshotV1 {
    try .init(currentStateOutput(3))
  }

  /// Read the current Released head's expected funding ordinal. This read does not reserve or fund a Load.
  public func nextLoadOrdinal() throws -> KagemushaWalletUInt128V1 {
    try currentStateOutput(4).nextLoadOrdinal()
  }

  /// Issue or recover the same hardware-authorized claim registering this Bootstrap on the ledger.
  public func bootstrapActivation() throws -> KagemushaSpartanActivationV1 {
    guard !retainedOutputsOnly else { throw KagemushaWalletErrorV1.invalidInput }
    return try .init(currentStateOutput(5))
  }

  private func currentStateOutput(_ part: UInt32) throws -> KagemushaSpartanReplyV1 {
    let handle = try owner.handle()
    let zero = Data(repeating: 0, count: 32)
    return try driver.call { out in slotId.withUnsafeBytes { slot in zero.withUnsafeBytes { operation in
      driver.output(handle, slot.bindMemory(to: UInt8.self).baseAddress,
                    operation.bindMemory(to: UInt8.self).baseAddress, part, out)
    } } }
  }

  /// Reconcile durable selection and read retained originals. Retry only these exact Payment bytes.
  public func output(operationId: Data, part: KagemushaSpartanOutputV1) throws -> KagemushaSpartanRetainedOutputV1 {
    guard operationId.count == 32, part != .unloadClaim || operationId.contains(where: { $0 != 0 }) else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    let handle = try owner.handle()
    let reply = try driver.call { out in slotId.withUnsafeBytes { slot in operationId.withUnsafeBytes { operation in
      driver.output(handle, slot.bindMemory(to: UInt8.self).baseAddress,
        operation.bindMemory(to: UInt8.self).baseAddress, part.rawValue, out)
    } } }
    return try .init(reply, part: part)
  }

  /// Reconstruct a receiver's signed acknowledgement from its protected Request and released Receive.
  public func creditStatus(requestId: Data, receiveOperationId: Data) throws -> IrohaPeerWireMessageV1 {
    try receivedCreditStatus(requestId: requestId, receiveOperationId: receiveOperationId).message
  }

  /// Read the receiver's actual credited or burned opening and the exact wire proving that outcome, for the archived
  /// display of one selected Receive. A receipt answering a particular incoming Payment uses
  /// `receivedCreditStatus(requestId:receiveOperationId:payment:)`.
  public func receivedCreditStatus(requestId: Data, receiveOperationId: Data) throws -> KagemushaSpartanReceiverCreditStatusV1 {
    guard requestId.count == 32, requestId.contains(where: { $0 != 0 }) else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    return try .init(creditExchange(selector: 1, operation: receiveOperationId, request: requestId, status: Data()))
  }

  /// The receipt for one particular incoming Payment: Native's credited or burned opening of its released Receive
  /// `receiveOperationId` for the hosted Request `requestId`. Native compares these complete Payment IPM1 bytes with
  /// the ones that Receive took and answers only when they are identical. Another eligible Payment for the same
  /// Request, or another operation, gets no receipt.
  public func receivedCreditStatus(requestId: Data, receiveOperationId: Data,
                                   payment: IrohaPeerWireMessageV1) throws -> KagemushaSpartanReceiverCreditStatusV1 {
    guard requestId.count == 32, requestId.contains(where: { $0 != 0 }) else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    return try .init(creditExchange(selector: 3, operation: receiveOperationId, request: requestId,
                                    status: Self.receiptPayment(payment)))
  }

  /// The exact complete Payment IPM1 bytes a receipt names, within the complete Payment carrier cap
  /// (`IrohaPeerNfcV1.maximumPaymentMessageBytes`). Any other profile, kind or size is refused before Native.
  static func receiptPayment(_ payment: IrohaPeerWireMessageV1) throws -> Data {
    guard payment.profile == .kagemushaSpartanPaymentV3, payment.kind == .payment,
          payment.encoded.count <= IrohaPeerNfcV1.maximumPaymentMessageBytes else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    return payment.encoded
  }

  /// Verify the receiver's folded head and credit opening against this wallet's exact released Payment.
  public func verifyCreditStatus(sendOperationId: Data, request: IrohaPeerWireMessageV1? = nil,
                                 status: IrohaPeerWireMessageV1) throws -> KagemushaSpartanCreditOutcomeV1 {
    guard status.profile == .kagemushaSpartanPaymentV3, status.kind == .credited else { throw KagemushaWalletErrorV1.invalidInput }
    if let request, request.profile != status.profile || request.kind != .request { throw KagemushaWalletErrorV1.invalidInput }
    return try creditExchange(selector: 2, operation: sendOperationId, request: request?.encoded ?? Data(), status: status.encoded)
      .verifiedCreditOutcome(for: status)
  }

  private func creditExchange(selector: UInt32, operation: Data, request: Data, status: Data) throws -> KagemushaSpartanReplyV1 {
    guard operation.count == 32, operation.contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidInput }
    let handle = try owner.handle()
    let function = try driver.creditStatusFunction()
    return try driver.call { out in
      slotId.withUnsafeBytes { slot in operation.withUnsafeBytes { op in
        request.withUnsafeBytes { req in status.withUnsafeBytes { evidence in
          function(handle, slot.bindMemory(to: UInt8.self).baseAddress, selector, op.bindMemory(to: UInt8.self).baseAddress,
                   req.bindMemory(to: UInt8.self).baseAddress, req.count,
                   evidence.bindMemory(to: UInt8.self).baseAddress, evidence.count, out)
        } }
      } }
    }
  }

  /// Supply the exact durable Payment to a generic QR, NFC or Nearby carrier.
  public func releasedPeerPayment(operationId: Data) throws -> IrohaPeerWireMessageV1? {
    guard case .released(let bytes) = try output(operationId: operationId, part: .payment),
          !bytes.isEmpty else { return nil }
    return try IrohaPeerWireMessageV1.decode(bytes, expectedProfile: .kagemushaSpartanPaymentV3,
                                              expectedKind: .payment)
  }

  /// Verify the complete carrier output against this wallet's authenticated manifest.
  public func verifyPayment(_ message: IrohaPeerWireMessageV1) throws -> KagemushaSpartanVerifiedPaymentV1 {
    guard message.profile == .kagemushaSpartanPaymentV3, message.kind == .payment else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    return try verifyPayment(message.encoded)
  }

  /// Verify an incoming Payment and bind its completed Request to the receiver's exact hosted terms.
  public func verifyPayment(_ message: IrohaPeerWireMessageV1, matching hosted: IrohaPeerWireMessageV1) throws -> KagemushaSpartanVerifiedPaymentV1 {
    guard hosted.profile == .kagemushaSpartanPaymentV3, hosted.kind == .request else { throw KagemushaWalletErrorV1.invalidInput }
    let open = try IrohaPeerSpartanOpenRequestV3(canonicalPayload: hosted.canonicalPayload)
    let verified = try verifyPayment(message)
    guard verified.matches(open) else { throw KagemushaWalletErrorV1.invalidInput }
    return verified
  }

  /// Full native proof verification, binding the returned Request to the exact Payment bytes.
  public func verifyPayment(_ payment: Data) throws -> KagemushaSpartanVerifiedPaymentV1 {
    guard !payment.isEmpty, payment.count <= 65_536 else { throw KagemushaWalletErrorV1.invalidInput }
    let handle = try owner.handle()
    let reply = try driver.call { out in payment.withUnsafeBytes {
      driver.verifyPayment(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count, out)
    } }
    try reply.requireShape(status: 83, count: 514)
    return .init(digest: reply.bytes.subdata(in: 0..<32), request: reply.bytes.subdata(in: 32..<514))
  }

  #if os(iOS)
  private func nativeDebitConfirmation(_ token: UInt64, _ message: Data, _ der: Data,
    control: Bool) throws -> KagemushaSpartanReplyV1 {
    let handle = try owner.handle()
    return try driver.call(control: control) { out in message.withUnsafeBytes { m in der.withUnsafeBytes { d in
      driver.confirm(handle, token, m.bindMemory(to: UInt8.self).baseAddress, m.count,
        d.bindMemory(to: UInt8.self).baseAddress, d.count, out)
    } } }
  }

  @MainActor fileprivate func confirmDebit(_ token: UInt64, terms: (Int) -> String, presenting: UIViewController) async throws {
    let asset = try await Task.detached(priority: .userInitiated) { [self] in
      let reply = try nativeDebitConfirmation(token, Data(), Data([2]), control: false)
      try reply.requireShape(status: 94, count: 33)
      let scale = Int(reply.bytes[32])
      guard scale <= 28 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return (reply.bytes.prefix(32).hexEncodedString(), scale)
    }.value
    try await KagemushaSpartanDebitTermsV1(terms: "Asset ID \(asset.0)\n\n" + terms(asset.1)).present(on: presenting)
    let cancellation = KagemushaSpartanDebitCancellationV1()
    try await withTaskCancellationHandler {
      try await Task.detached(priority: .userInitiated) { [self] in
        try cancellation.requireCurrent()
        try nativeDebitConfirmation(token, Data(), Data([1]), control: false).requireShape(status: 93, count: 0)
        let challenge = try nativeDebitConfirmation(token, Data(), Data(), control: false)
        try challenge.requireShape(status: 88, count: 32)
        let cancel: @Sendable () -> Void = { [self] in
          _ = try? nativeDebitConfirmation(token, challenge.bytes, Data(), control: true).requireShape(status: 84, count: 0)
        }
        do {
          try cancellation.arm(cancel)
          defer { cancellation.disarm() }
          // Native constructs the reviewed terms and owns the fresh LAContext.
          // Interactive and cancel calls never occupy the shared proof worker.
          try nativeDebitConfirmation(token, challenge.bytes, Data([0]), control: true).requireShape(status: 89, count: 0)
          try cancellation.requireCurrent()
        } catch {
          cancel()
          throw error
        }
      }.value
    } onCancel: { cancellation.cancel() }
  }
  #endif

  fileprivate func execute(_ token: UInt64) throws -> KagemushaSpartanExecutionV1 {
    let handle = try owner.handle()
    return try .init(driver.call { driver.execute(handle, token, $0) })
  }
  fileprivate func discard(_ token: UInt64) throws {
    let handle = try owner.handle()
    try driver.call(control: true) { driver.discard(handle, token, $0) }.requireShape(status: 84, count: 0)
  }
  /// Closing retains committed custody and exact outputs.
  public func close() throws { try owner.close() }
  deinit { try? owner.close() }
  static let maximumRecordBytes = 4 * 1024 * 1024
}

/// Retained installed backend before the existing account signs Native's fresh challenge.
public final class KagemushaSpartanRuntimeV1: @unchecked Sendable {
  private let lock = NSLock()
  private var lease: KagemushaWalletNativeLeaseV1?
  private let driver: KagemushaSpartanDriverV1
  public let parameterId: Data
  fileprivate init(lease: KagemushaWalletNativeLeaseV1, driver: KagemushaSpartanDriverV1, parameterId: Data) {
    self.lease = lease; self.driver = driver; self.parameterId = parameterId
  }
  /// Install public verifier programs against Native's independently installed root and scheme.
  static func install(lease: KagemushaWalletNativeLeaseV1, programBundle: Data, signedManifest: Data, signature: Data) throws -> KagemushaSpartanRuntimeV1 {
    guard !programBundle.isEmpty, programBundle.count <= 256 * 1024 * 1024,
      !signedManifest.isEmpty, signedManifest.count <= 16_384,
      signature.count == 64 else { throw KagemushaWalletErrorV1.invalidInput }
    let driver = try KagemushaSpartanDriverV1()
    let handle = try lease.handle()
    let reply = try driver.call { out in
      programBundle.withUnsafeBytes { programs in signedManifest.withUnsafeBytes { manifest in signature.withUnsafeBytes { signature in
        driver.install(handle, programs.bindMemory(to: UInt8.self).baseAddress, programs.count,
          manifest.bindMemory(to: UInt8.self).baseAddress, manifest.count,
          signature.bindMemory(to: UInt8.self).baseAddress, signature.count, out)
      } } }
    }
    try reply.requireShape(status: 81, count: 32, token: true)
    guard reply.sequenceLow == handle else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return .init(lease: lease, driver: driver, parameterId: reply.bytes)
  }
  /// Open the programs Native already authenticated from this owner's installed signed carrier.
  /// All three originals absent is the one buffer-free form of the install entry: no program,
  /// manifest or signature bytes are offered, and an owner without such programs refuses.
  static func openInstalled(lease: KagemushaWalletNativeLeaseV1) throws -> KagemushaSpartanRuntimeV1 {
    let driver = try KagemushaSpartanDriverV1()
    let handle = try lease.handle()
    let reply = try driver.call { driver.install(handle, nil, 0, nil, 0, nil, 0, $0) }
    return try .init(lease: lease, driver: driver, parameterId: installedParameterId(reply, handle: handle))
  }
  /// The installed-owner reply is status 81 for this exact handle with a nonzero 32-byte identity.
  static func installedParameterId(_ reply: KagemushaSpartanReplyV1, handle: UInt64) throws -> Data {
    try reply.requireShape(status: 81, count: 32, token: true)
    guard reply.sequenceLow == handle, reply.bytes.contains(where: { $0 != 0 })
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return reply.bytes
  }
  /// Return the exact challenge for the existing Ed25519 account. No payment key is created.
  public func beginAccount(originals: KagemushaSpartanAccountOriginalsV1) throws -> Data {
    lock.lock(); defer { lock.unlock() }
    guard let lease else { throw KagemushaWalletErrorV1.closed }
    let handle = try lease.handle()
    let reply = try driver.call { out in originals.originals.withRequest { request in
      driver.beginAccount(handle, request, out)
    } }
    try reply.requireShape(status: 15, count: 32, token: true)
    guard reply.sequenceLow == handle, reply.bytes.contains(where: { $0 != 0 })
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return reply.bytes
  }
  /// Return the exact challenge for the existing Ed25519 account from one complete E6 enrollment
  /// result and the existing account original. Native's signed selected owner supplies the asset
  /// scope and Enrollment certificate itself and verifies the result against its retained E5,
  /// hardware key and evidence before retaining anything. No payment key is created. An exact
  /// retry returns the retained challenge. Continue with `authorizeAccount(signature:)`.
  public func beginAccount(resultOriginal: Data, accountOriginal: Data) throws -> Data {
    guard !resultOriginal.isEmpty, resultOriginal.count <= 262_144, !accountOriginal.isEmpty
    else { throw KagemushaWalletErrorV1.invalidInput }
    lock.lock(); defer { lock.unlock() }
    guard let lease else { throw KagemushaWalletErrorV1.closed }
    let handle = try lease.handle()
    let reply = try driver.call { out in
      resultOriginal.withUnsafeBytes { result in accountOriginal.withUnsafeBytes { account -> Int32 in
        // Same entry and wire as the split form. The certificate and asset stay separate ABI
        // inputs, and both absent (null, zero length) select the complete-result form.
        var request = connect_norito_kagemusha_wallet_open_request_v1()
        request.credential = result.bindMemory(to: UInt8.self).baseAddress; request.credential_length = result.count
        request.certificates = nil; request.certificates_length = 0
        request.account = account.bindMemory(to: UInt8.self).baseAddress; request.account_length = account.count
        request.asset = nil; request.asset_length = 0
        return driver.beginAccount(handle, &request, out)
      } }
    }
    try reply.requireShape(status: 15, count: 32, token: true)
    guard reply.sequenceLow == handle, reply.bytes.contains(where: { $0 != 0 })
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return reply.bytes
  }
  /// Finish Native account admission and transfer this same exclusive custody lease once.
  public func authorizeAccount(signature: Data) throws -> KagemushaSpartanWalletV1 {
    guard signature.count == 64 else { throw KagemushaWalletErrorV1.invalidInput }
    lock.lock(); defer { lock.unlock() }
    guard let lease else { throw KagemushaWalletErrorV1.closed }
    let handle = try lease.handle()
    let reply = try driver.call { out in signature.withUnsafeBytes {
      driver.authorizeAccount(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count, out)
    } }
    let admission = try reply.authorizedSlot(handle: handle, parameterId: parameterId)
    let wallet = KagemushaSpartanWalletV1(owner: lease, driver: driver, parameterId: parameterId,
      slotId: admission.slot, retainedOutputsOnly: admission.retainedOutputsOnly)
    self.lease = nil
    return wallet
  }
  public func close() throws {
    lock.lock(); defer { lock.unlock() }
    try lease?.close(); lease = nil
  }
  deinit { try? close() }
}

/// Native signed-time bounds for this operation, in Unix milliseconds.
public struct KagemushaSpartanTimeIntervalV1: Sendable {
  public let lowerMs: UInt64
  public let upperMs: UInt64
  init(bytes: Data) throws {
    guard bytes.count == 16 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    lowerMs = bytes.prefix(8).reversed().reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
    upperMs = bytes.suffix(8).reversed().reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
    guard lowerMs <= upperMs else { throw KagemushaWalletErrorV1.invalidNativeOutput }
  }
}

public enum KagemushaSpartanOutputV1: UInt32, Sendable { case head = 0, payment = 1, completion = 2, unloadClaim = 3 }
public enum KagemushaSpartanOperationV1: UInt8, Sendable { case bootstrap = 1, load = 2, send = 3, receive = 4, archiveSent = 5 }

public enum KagemushaSpartanRetainedOutputV1: Sendable {
  case unknown, pending, archived, deliveryLoss, released(Data)
  init(_ reply: KagemushaSpartanReplyV1, part: KagemushaSpartanOutputV1) throws {
    try reply.requireShape(status: reply.status, count: reply.status == 1 ? nil : 0)
    switch reply.status {
    case 0: self = .unknown
    case 2: self = .pending
    case 4: self = .archived
    case 5: self = .deliveryLoss
    case 1:
      // Completed Bootstrap, Load and Receive intentionally have no outgoing Payment.
      guard part == .payment || !reply.bytes.isEmpty else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      if part == .unloadClaim && reply.bytes.count > 16_384 { throw KagemushaWalletErrorV1.invalidNativeOutput }
      self = .released(reply.bytes)
    default: throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
}

/// Native's exact review for confirmation UI; no caller-supplied validity flag.
public struct KagemushaSpartanReviewProjectionV1: Sendable {
  /// The complete Native review original for device confirmation.
  public let bytes: Data
  public let operation: KagemushaSpartanOperationV1
  public let schemeId, walletId, operationId, capsuleDigest: Data
  public let amount, fee: KagemushaWalletUInt128V1
  public let receiverWalletId, receiverAccountId, beforeHead, afterHead: Data
  init(_ original: Data) throws {
    guard original.count == 291 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let bytes = [UInt8](original)
    guard bytes[0] == 1, bytes[1] == 0,
      let operation = KagemushaSpartanOperationV1(rawValue: bytes[2])
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    if operation == .archiveSent && bytes[131..<227].contains(where: { $0 != 0 }) {
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
    func word(_ offset: Int) -> UInt64 {
      (0..<8).reduce(UInt64(0)) { $0 | (UInt64(bytes[offset + $1]) << ($1 * 8)) }
    }
    func data(_ offset: Int) -> Data { Data(bytes[offset..<offset + 32]) }
    self.bytes = Data(bytes)
    self.operation = operation
    schemeId = data(3); walletId = data(35); operationId = data(67); capsuleDigest = data(99)
    amount = .init(low: word(131), high: word(139)); fee = .init(low: word(147), high: word(155))
    receiverWalletId = data(163); receiverAccountId = data(195); beforeHead = data(227); afterHead = data(259)
  }
}

/// Authenticated Native fields for RefreshTimeAnchor under review wire version 2.
public struct KagemushaSpartanTimeAnchorProjectionV2: Sendable {
  public let schemeId, walletId, operationId, capsuleDigest, anchorDigest: Data
  public let issuerTime, successorFloor: UInt64
  public let beforeHead, afterHead: Data

  init(_ original: Data) throws {
    let bytes = [UInt8](original)
    guard bytes.count == 243, bytes[0] == 2, bytes[1] == 0, bytes[2] == 7 else {
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
    func data(_ offset: Int) -> Data { Data(bytes[offset..<offset + 32]) }
    func word(_ offset: Int) -> UInt64 {
      (0..<8).reduce(UInt64(0)) { $0 | (UInt64(bytes[offset + $1]) << ($1 * 8)) }
    }
    schemeId = data(3); walletId = data(35); operationId = data(67); capsuleDigest = data(99)
    anchorDigest = data(131); issuerTime = word(163); successorFloor = word(171)
    guard successorFloor >= issuerTime else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    beforeHead = data(179); afterHead = data(211)
  }
}

/// Native authenticates these destination bytes against the credential and the signed charge quote.
public struct KagemushaSpartanUnloadProjectionV3: Sendable {
  /// The complete Native review original, including both destination address frames.
  public let bytes: Data
  public let schemeId, walletId, operationId, capsuleDigest, assetDigest, nullifier: Data
  public let ordinal, faceDebit, onlineCharge, netToAccount: KagemushaWalletUInt128V1
  public let quoteDigest, credentialAccountDigest, beneficiaryAccountDigest, beforeHead, afterHead: Data
  public let credentialAccountBytes: Data
  public let beneficiaryAccountBytes: Data?

  init(_ original: Data) throws {
    guard (423...16_384).contains(original.count) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let bytes = [UInt8](original)
    guard bytes[0] == 3, bytes[1] == 0, bytes[2] == 6 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    func digest(_ i: Int) -> Data { Data(bytes[i..<i + 32]) }
    func word(_ i: Int) -> UInt64 {
      (0..<8).reduce(UInt64(0)) { $0 | (UInt64(bytes[i + $1]) << ($1 * 8)) }
    }
    func scalar(_ i: Int) -> KagemushaWalletUInt128V1 { .init(low: word(i), high: word(i + 8)) }
    var offset = 419
    func address() throws -> Data {
      guard offset + 2 <= bytes.count else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      let size = Int(bytes[offset]) | (Int(bytes[offset + 1]) << 8)
      offset += 2
      guard size <= bytes.count - offset else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      let start = offset
      offset += size
      return Data(bytes[start..<offset])
    }
    schemeId = digest(3); walletId = digest(35); operationId = digest(67); capsuleDigest = digest(99)
    assetDigest = digest(131); nullifier = digest(163); ordinal = scalar(195); faceDebit = scalar(211)
    onlineCharge = scalar(227); netToAccount = scalar(243); quoteDigest = digest(259)
    credentialAccountDigest = digest(291); beneficiaryAccountDigest = digest(323)
    beforeHead = digest(355); afterHead = digest(387)
    credentialAccountBytes = try address()
    let beneficiary = try address()
    let zero = KagemushaWalletUInt128V1(low: 0, high: 0)
    let charged = onlineCharge != zero
    guard offset == bytes.count, !credentialAccountBytes.isEmpty,
      nullifier.contains(where: { $0 != 0 }), faceDebit > zero, onlineCharge <= faceDebit,
      charged == !beneficiary.isEmpty, charged == quoteDigest.contains(where: { $0 != 0 }),
      charged == beneficiaryAccountDigest.contains(where: { $0 != 0 })
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let (low, borrow) = faceDebit.low.subtractingReportingOverflow(onlineCharge.low)
    let high = faceDebit.high - onlineCharge.high - (borrow ? 1 : 0)
    guard netToAccount == .init(low: low, high: high) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    self.bytes = Data(bytes)
    beneficiaryAccountBytes = charged ? beneficiary : nil
  }
}

public typealias KagemushaSpartanReviewV1 = KagemushaSpartanPreparedReviewV1<KagemushaSpartanReviewProjectionV1>
public typealias KagemushaSpartanUnloadReviewV3 = KagemushaSpartanPreparedReviewV1<KagemushaSpartanUnloadProjectionV3>
public typealias KagemushaSpartanTimeAnchorReviewV2 = KagemushaSpartanPreparedReviewV1<KagemushaSpartanTimeAnchorProjectionV2>

/// Review bound to one Native owner and capsule; pending execution is retryable.
public final class KagemushaSpartanPreparedReviewV1<Projection: Sendable>: @unchecked Sendable {
  private let owner: KagemushaSpartanWalletV1
  private let lock = NSLock()
  private var token: UInt64
  private var executing = false
  public let projection: Projection
  fileprivate init(owner: KagemushaSpartanWalletV1, token: UInt64, projection: Projection) {
    self.owner = owner; self.token = token; self.projection = projection
  }
  private func begin() throws -> UInt64 {
    lock.lock(); defer { lock.unlock() }
    guard token > 0, !executing else { throw KagemushaWalletErrorV1.invalidInput }
    executing = true; return token
  }
  #if os(iOS)
  @MainActor fileprivate func confirmReviewedDebit(terms: (Int) -> String, presenting: UIViewController) async throws {
    let value = try begin()
    defer { finish(consumed: false) }
    try await owner.confirmDebit(value, terms: terms, presenting: presenting)
  }
  #endif
  private func finish(consumed: Bool) {
    lock.lock(); defer { lock.unlock() }
    if consumed { token = 0 }; executing = false
  }
  /// Advance the frozen operation. Installed custody refuses Send until it can verify protected confirmation.
  public func execute() throws -> KagemushaSpartanExecutionV1 {
    let value = try begin()
    do {
      let result = try owner.execute(value)
      finish(consumed: result.status != .pending)
      return result
    } catch {
      // A failed transport/allocation may hide Selected. Retry only this frozen token.
      finish(consumed: false); throw error
    }
  }
  public func discard() throws {
    lock.lock(); let alreadyConsumed = token == 0; lock.unlock()
    if alreadyConsumed { return }
    let value = try begin()
    do { try owner.discard(value); finish(consumed: true) }
    catch { finish(consumed: false); throw error }
  }
  deinit { try? discard() }
}

/// Uncertain statuses require recovery of the same operation, never a replacement debit.
public struct KagemushaSpartanExecutionV1: Sendable {
  public enum Status: Int32, Sendable { case released = 1, pending = 2, notPerformed = 3, archived = 4, deliveryLoss = 5 }
  public let status: Status
  public let sequence: KagemushaWalletUInt128V1
  public let detail: UInt32
  public let completion: Data?
  init(_ reply: KagemushaSpartanReplyV1) throws {
    // Native Execute reserves both sequence words. Only notPerformed carries a reason.
    guard let status = Status(rawValue: reply.status),
      reply.sequenceLow == 0, reply.sequenceHigh == 0,
      (status == .notPerformed ? reply.detail <= 2 : reply.detail == 0),
      (status == .released) == !reply.bytes.isEmpty
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    self.status = status; sequence = .init(low: reply.sequenceLow, high: reply.sequenceHigh)
    detail = reply.detail; completion = status == .released ? reply.bytes : nil
  }
}
/// A balance is available only for a Released head.
public struct KagemushaSpartanSnapshotV1: Sendable {
  public enum Stage: Sendable {
    case enrolled
    case pending(operationId: Data)
    case released(operationId: Data, head: Data, sequence: KagemushaWalletUInt128V1,
                  balance: KagemushaWalletUInt128V1, burned: KagemushaWalletUInt128V1, spendable: KagemushaWalletUInt128V1)
  }
  public let walletId: Data
  public let assetDigest: Data
  public let scale: UInt8
  public let stage: Stage

  init(_ reply: KagemushaSpartanReplyV1) throws {
    try reply.requireShape(status: 101)
    let b = Data(reply.bytes)
    guard b.count >= 68, b[0] == 1, b[1] == 0, b[3] <= 28 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    func id(_ offset: Int) throws -> Data {
      let value = b.subdata(in: offset..<offset+32)
      guard value.contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return value
    }
    func read(_ offset: Int) -> KagemushaWalletUInt128V1 {
      func limb(_ start: Int) -> UInt64 { (0..<8).reduce(0) { $0 | UInt64(b[start+$1]) << ($1*8) } }
      return .init(low: limb(offset), high: limb(offset+8))
    }
    walletId = try id(4); assetDigest = try id(36); scale = b[3]
    switch (b[2], b.count) {
    case (1,68): stage = .enrolled
    case (2,100): stage = .pending(operationId: try id(68))
    case (3,196):
      let balance = read(148), burned = read(164), spendable = read(180)
      let (low, borrow) = balance.low.subtractingReportingOverflow(burned.low)
      let (high, underflow) = balance.high.subtractingReportingOverflow(burned.high)
      let (netHigh, overflow) = high.subtractingReportingOverflow(borrow ? 1 : 0)
      guard !underflow, !overflow, spendable.low == low, spendable.high == netHigh else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      stage = .released(operationId: try id(68), head: try id(100), sequence: read(132), balance: balance, burned: burned, spendable: spendable)
    default: throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
}

public struct KagemushaSpartanVerifiedPaymentV1: Sendable {
  public let digest: Data
  public let request: Data
  func matches(_ hosted: IrohaPeerSpartanOpenRequestV3) -> Bool {
    request.count == 482 && request[..<66] == hosted.request[..<66] &&
      request[130..<194] == hosted.request[130..<194] && request[210..<482] == hosted.request[210..<482]
  }
}

/// Insert the decimal point into the exact unsigned base-unit spelling without rounding.
func spartanScaledAmount(_ atoms: String, _ scale: Int) -> String {
  if scale == 0 { return atoms }
  let padded = String(repeating: "0", count: max(0, scale + 1 - atoms.count)) + atoms
  return padded.dropLast(scale) + "." + padded.suffix(scale)
}

#if os(iOS)
private final class KagemushaSpartanDebitCancellationV1: @unchecked Sendable {
  private let lock = NSLock()
  private var canceled = false
  private var action: (@Sendable () -> Void)?
  func requireCurrent() throws {
    lock.lock(); defer { lock.unlock() }
    if canceled { throw CancellationError() }
  }
  func arm(_ action: @escaping @Sendable () -> Void) throws {
    lock.lock(); defer { lock.unlock() }
    if canceled { throw CancellationError() }
    self.action = action
  }
  func disarm() { lock.lock(); action = nil; lock.unlock() }
  func cancel() {
    lock.lock(); canceled = true; let action = self.action; self.action = nil; lock.unlock()
    if let action { DispatchQueue.global(qos: .userInitiated).async(execute: action) }
  }
}
@available(iOS 18.0, *)
private func spartanAtoms(_ value: KagemushaWalletUInt128V1) -> UInt128 {
  (UInt128(value.high) << 64) | UInt128(value.low)
}

@available(iOS 18.0, *)
extension KagemushaSpartanPreparedReviewV1 where Projection == KagemushaSpartanReviewProjectionV1 {
  /// Display all reviewed Send terms, then request fresh OS user presence.
  @MainActor public func confirmDebit(presenting: UIViewController) async throws {
    guard projection.operation == .send else { throw KagemushaWalletErrorV1.invalidInput }
    let amount = spartanAtoms(projection.amount), fee = spartanAtoms(projection.fee)
    let (total, overflow) = amount.addingReportingOverflow(fee)
    guard !overflow else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    try await confirmReviewedDebit(terms: { scale in
      """
      Send \(spartanScaledAmount(String(amount), scale)) asset units
      Fee  \(spartanScaledAmount(String(fee), scale))
      Total debit  \(spartanScaledAmount(String(total), scale))

      Receiving wallet ID
      \(self.projection.receiverWalletId.hexEncodedString())

      Receiving account ID
      \(self.projection.receiverAccountId.hexEncodedString())
      """
    }, presenting: presenting)
  }
}
@available(iOS 18.0, *)
extension KagemushaSpartanPreparedReviewV1 where Projection == KagemushaSpartanUnloadProjectionV3 {
  /// Display both reviewed Unload destinations, then request fresh OS user presence.
  @MainActor public func confirmDebit(presenting: UIViewController) async throws {
    try await confirmReviewedDebit(terms: { scale in
      let review = self.projection
      func value(_ units: KagemushaWalletUInt128V1) -> String { spartanScaledAmount(String(spartanAtoms(units)), scale) }
      var terms = """
      Move \(value(review.faceDebit)) asset units online
      Account receives  \(value(review.netToAccount))
      Charge  \(value(review.onlineCharge))

      Receiving account
      \(review.credentialAccountBytes.hexEncodedString())
      """
      if let beneficiary = review.beneficiaryAccountBytes {
        terms += "\n\nCharge beneficiary account\n\(beneficiary.hexEncodedString())"
      }
      return terms
    }, presenting: presenting)
  }
}
#endif

/// A status returned only after Native authenticates the receiver proof and selected payer Send.
public enum KagemushaSpartanCreditOutcomeV1: Equatable, Sendable {
  case credited, burned
}

/// The complete acknowledgement reconstructed by this wallet from its released Receive and protected opening.
public struct KagemushaSpartanReceiverCreditStatusV1: Sendable {
  public let message: IrohaPeerWireMessageV1
  public let outcome: KagemushaSpartanCreditOutcomeV1
  init(_ reply: KagemushaSpartanReplyV1) throws {
    let issued = try reply.issuedCreditStatus()
    message = issued
    guard let status = issued.spartanCreditOutcome else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    outcome = status
  }
}

struct KagemushaSpartanReplyV1 {
  let status: Int32
  let sequenceLow, sequenceHigh: UInt64
  let detail: UInt32
  let bytes: Data
  func authorizedSlot(handle: UInt64, parameterId: Data) throws -> (slot: Data, retainedOutputsOnly: Bool) {
    guard status == 85 || status == 87 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    try requireShape(status: status, count: 64, token: true)
    guard sequenceLow == handle, bytes.prefix(32) == parameterId,
      bytes.suffix(32).contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return (bytes.subdata(in: 32..<64), status == 87)
  }
  func issuedOpenRequest(amount: Data, feeSchedule: Data) throws -> IrohaPeerWireMessageV1 {
    try requireShape(status: 96)
    guard let message = try? IrohaPeerWireMessageV1.decode(bytes, expectedProfile: .kagemushaSpartanPaymentV3,
            expectedKind: .request), let originals = try? IrohaPeerSpartanOpenRequestV3(canonicalPayload: message.canonicalPayload),
          originals.request[242..<258] == amount, originals.feeSchedule == feeSchedule else {
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
    return message
  }
  func issuedCreditStatus() throws -> IrohaPeerWireMessageV1 {
    try requireShape(status: 104)
    guard let wire = try? IrohaPeerWireMessageV1.decode(bytes, expectedProfile: .kagemushaSpartanPaymentV3,
                                                        expectedKind: .credited) else {
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
    return wire
  }

  func verifiedCreditOutcome(for message: IrohaPeerWireMessageV1) throws -> KagemushaSpartanCreditOutcomeV1 {
    try requireShape(status: 105, count: 1)
    guard message.profile == .kagemushaSpartanPaymentV3, message.kind == .credited else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    switch (bytes[0], message.spartanCreditOutcome) {
    case (1, .credited): return .credited
    case (0, .burned): return .burned
    default: throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }

  func blacklistDigest() throws -> Data {
    try requireShape(status: 95, count: 32)
    guard bytes.contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return bytes
  }
  func nextLoadOrdinal() throws -> KagemushaWalletUInt128V1 {
    try requireShape(status: 106, count: 16)
    func limb(_ offset: Int) -> UInt64 { (0..<8).reduce(0) { $0 | UInt64(bytes[bytes.startIndex + offset + $1]) << ($1 * 8) } }
    return .init(low: limb(0), high: limb(8))
  }

  func requireShape(status: Int32, count: Int? = nil, token: Bool = false) throws {
    guard self.status == status, count == nil || bytes.count == count,
      sequenceHigh == 0, detail == 0,
      token ? (sequenceLow > 0 && sequenceLow <= UInt64(Int64.max)) : sequenceLow == 0
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
  }
}

fileprivate final class KagemushaSpartanDriverV1: @unchecked Sendable {
  typealias Output = connect_norito_kagemusha_wallet_result_v1
  typealias Install = @convention(c) (UInt64, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  typealias CreditStatus = @convention(c) (UInt64, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?,
    UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  func creditStatusFunction() throws -> CreditStatus {
    guard let function = NoritoNativeBridge.shared.resolveNativeSymbol("connect_norito_kagemusha_spartan_credit_status_v1",
                                                                      as: CreditStatus.self) else {
      throw KagemushaWalletErrorV1.bridgeUnavailable
    }
    return function
  }
  typealias ActivationSettlement = @convention(c) (UInt64, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  func activationSettlementFunction() throws -> ActivationSettlement {
    guard let function = NoritoNativeBridge.shared.resolveNativeSymbol("connect_norito_kagemusha_spartan_activation_settlement_v1",
                                                                        as: ActivationSettlement.self) else {
      throw KagemushaWalletErrorV1.bridgeUnavailable
    }
    return function
  }
  typealias Settlement = @convention(c) (UInt64, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  typealias Epoch = @convention(c) (UInt64, UnsafePointer<UInt8>?, UInt64, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  func settlementFunction() throws -> Settlement {
    guard let f = NoritoNativeBridge.shared.resolveNativeSymbol("connect_norito_kagemusha_spartan_unload_settlement_v1", as: Settlement.self) else { throw KagemushaWalletErrorV1.bridgeUnavailable }
    return f
  }
  func epochFunction() throws -> Epoch {
    guard let f = NoritoNativeBridge.shared.resolveNativeSymbol("connect_norito_kagemusha_spartan_epoch_v1", as: Epoch.self) else { throw KagemushaWalletErrorV1.bridgeUnavailable }
    return f
  }
  typealias IssueOpenRequest = @convention(c) (UInt64, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  let issueOpenRequest: IssueOpenRequest
  typealias Confirm = @convention(c) (UInt64, UInt64, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  let confirm: Confirm
  typealias Time = @convention(c) (UInt64, UnsafePointer<UInt8>?, UInt32, UInt64, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  typealias Prepare = @convention(c) (UInt64, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  typealias Execute = @convention(c) (UInt64, UInt64, UnsafeMutablePointer<Output>?) -> Int32
  typealias Read = @convention(c) (UInt64, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<Output>?) -> Int32
  typealias Verify = @convention(c) (UInt64, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  typealias BeginAccount = @convention(c) (UInt64, UnsafePointer<connect_norito_kagemusha_wallet_open_request_v1>?, UnsafeMutablePointer<Output>?) -> Int32
  typealias Free = @convention(c) (UnsafeMutableRawPointer?) -> Void
  let install: Install, prepare: Prepare, execute: Execute, output: Read, verifyPayment: Verify, discard: Execute
  let beginAccount: BeginAccount, authorizeAccount: Verify, time: Time, installBlacklist: Prepare
  private let free: Free
  init() throws {
    func symbol<T>(_ name: String, _ type: T.Type) throws -> T {
      guard let value = NoritoNativeBridge.shared.resolveNativeSymbol(name, as: type)
      else { throw KagemushaWalletErrorV1.bridgeUnavailable }
      return value
    }
    install = try symbol("connect_norito_kagemusha_spartan_install_v1", Install.self)
    issueOpenRequest = try symbol("connect_norito_kagemusha_spartan_issue_open_request_v1", IssueOpenRequest.self)
    installBlacklist = try symbol("connect_norito_kagemusha_spartan_install_blacklist_v1", Prepare.self)
    confirm = try symbol("connect_norito_kagemusha_spartan_confirm_v1", Confirm.self)
    time = try symbol("connect_norito_kagemusha_spartan_time_v1", Time.self)
    prepare = try symbol("connect_norito_kagemusha_spartan_prepare_v1", Prepare.self)
    execute = try symbol("connect_norito_kagemusha_spartan_execute_v1", Execute.self)
    output = try symbol("connect_norito_kagemusha_spartan_output_v1", Read.self)
    verifyPayment = try symbol("connect_norito_kagemusha_spartan_verify_payment_v1", Verify.self)
    discard = try symbol("connect_norito_kagemusha_spartan_discard_v1", Execute.self)
    beginAccount = try symbol("connect_norito_kagemusha_spartan_begin_account_v1", BeginAccount.self)
    authorizeAccount = try symbol("connect_norito_kagemusha_spartan_authorize_account_v1", Verify.self)
    free = try symbol("connect_norito_free", Free.self)
  }
  func call(control: Bool = false, _ action: (UnsafeMutablePointer<Output>) -> Int32) throws -> KagemushaSpartanReplyV1 {
    func run() throws -> KagemushaSpartanReplyV1 {
      var output = Output()
      let status = action(&output)
      defer { if let bytes = output.bytes { free(bytes) } }
      try KagemushaWalletNativeDriverV1.check(status, reason: output.reason, platform: output.platform_code)
      guard output.reason == -1, output.platform_code == 0, output.length >= 0,
        output.length <= KagemushaSpartanWalletV1.maximumRecordBytes,
        output.length == 0 || output.bytes != nil else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return .init(status: output.status, sequenceLow: output.sequence_low, sequenceHigh: output.sequence_high,
        detail: output.detail, bytes: output.bytes.map { Data(bytes: $0, count: output.length) } ?? Data())
    }
    return try control ? run() : KagemushaWalletNativeWorkerV1.perform(run)
  }
}

/// Native's certified result for one exact envelope and the durable payout of its logical claim.
public struct KagemushaSpartanUnloadSettlementV1: Equatable, Sendable {
  public let entrypoint: Data
  public let height: UInt64
  public let block: Data
  public let accepted: Bool
  public let paid: Bool
  public let requiredEpoch: UInt64?
  /// Unverified synchronization target until Native verifies the offered certificate.
  public let requiredHeight: UInt64?

  init(_ reply: KagemushaSpartanReplyV1, expectedStatus: Int32 = 102) throws {
    try reply.requireShape(status: expectedStatus, count: 83)
    let b = reply.bytes
    entrypoint = b.subdata(in: 0..<32)
    let wireHeight = reply.settlementWord(at: 32)
    height = b[74] == 1 ? 0 : wireHeight
    requiredHeight = b[74] == 1 ? wireHeight : nil
    block = b.subdata(in: 40..<72)
    accepted = b[72] == 1
    paid = b[73] == 1
    let epoch = reply.settlementWord(at: 75)
    requiredEpoch = b[74] == 1 ? epoch : nil
    guard (72...74).allSatisfy({ b[$0] <= 1 }), entrypoint.contains(where: { $0 != 0 }),
      (height == 0) == block.allSatisfy({ $0 == 0 }), !accepted || (height > 0 && paid),
      b[74] == 1 || epoch == 0, requiredHeight == nil || wireHeight > 0
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
  }
}

/// A retained, immutable activation package scoped by the same Native owner that signed it.
public struct KagemushaSpartanActivationV1: Equatable, Sendable {
  public let schemeId, assetDigest, manifestDigest, walletId, original: Data
  init(_ reply: KagemushaSpartanReplyV1) throws {
    try reply.requireShape(status: 107)
    let bytes = Data(reply.bytes)
    guard (129...(128 + 32_768)).contains(bytes.count) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    func pin(_ offset: Int) throws -> Data {
      let digest = bytes.subdata(in: offset..<(offset + 32))
      guard digest.contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return digest
    }
    schemeId = try pin(0); assetDigest = try pin(32); manifestDigest = try pin(64); walletId = try pin(96)
    original = bytes.subdata(in: 128..<bytes.count)
  }
  public func instructionFrame() throws -> TransactionInstructionFrame {
    try .kagemushaSpartanActivate(scheme: schemeId, asset: assetDigest, manifestDigest: manifestDigest,
                                  wallet: walletId, activation: original)
  }
}

/// Finality for one exact signed entrypoint and the wallet's durable activation.
public struct KagemushaSpartanActivationSettlementV1: Equatable, Sendable {
  private let value: KagemushaSpartanUnloadSettlementV1
  public var entrypoint: Data { value.entrypoint }
  public var height: UInt64 { value.height }
  public var block: Data { value.block }
  public var accepted: Bool { value.accepted }
  public var activated: Bool { value.paid }
  public var requiredEpoch: UInt64? { value.requiredEpoch }
  public var requiredHeight: UInt64? { value.requiredHeight }
  init(_ reply: KagemushaSpartanReplyV1) throws { value = try .init(reply, expectedStatus: 108) }
}

private extension KagemushaSpartanReplyV1 {
  func settlementWord(at offset: Int) -> UInt64 {
    (0..<8).reduce(UInt64(0)) { $0 | UInt64(bytes[offset + $1]) << ($1 * 8) }
  }
}

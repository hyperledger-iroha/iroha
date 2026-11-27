import Foundation

/// The installed Spartan wallet operations an NFC binding consults. Native decides every verdict; the seam
/// exists only so focused tests can substitute Native without a device. Internal: no application conforms.
protocol KagemushaSpartanPeerNfcNativeV1: AnyObject, Sendable {
  func snapshot() throws -> KagemushaSpartanSnapshotV1
  func issueOpenRequest(requestId: Data, amount: KagemushaWalletUInt128V1,
                        feeSchedule: Data, feeCertificate: Data) throws -> IrohaPeerWireMessageV1
  func verifyPayment(_ message: IrohaPeerWireMessageV1,
                     matching hosted: IrohaPeerWireMessageV1) throws -> KagemushaSpartanVerifiedPaymentV1
  func output(operationId: Data, part: KagemushaSpartanOutputV1) throws -> KagemushaSpartanRetainedOutputV1
  func verifyCreditStatus(sendOperationId: Data, request: IrohaPeerWireMessageV1?,
                          status: IrohaPeerWireMessageV1) throws -> KagemushaSpartanCreditOutcomeV1
  func receivedCreditStatus(requestId: Data, receiveOperationId: Data,
                            payment: IrohaPeerWireMessageV1) throws -> KagemushaSpartanReceiverCreditStatusV1
}

extension KagemushaSpartanWalletV1: KagemushaSpartanPeerNfcNativeV1 {}

/// Verified NFC exchange binding for one role of an admitted Spartan wallet (`kagemushaSpartanPaymentV3`).
///
/// The NFC transport (`IrohaPeerNfcV1`) keeps its APDU chunk framing, continuity, resume and exact bytes.
/// Every Request, Payment and CreditStatus verdict it needs comes from this wallet's Native operations under
/// the wallet's authenticated enrolled scope (its Native snapshot's wallet and signed asset):
///
/// - Payer: one released Send, named by its Native operation and the exact Request it was prepared from.
///   Native authenticated that Request against the installed scheme before the protected Send. The only
///   Payment is Native's exact retained output of that operation (at most
///   `IrohaPeerNfcV1.maximumPaymentMessageBytes`). Native binds it to the Request's signed terms, and its
///   payer specialization names this wallet. A CreditStatus (IPM1 kind 4) counts only after this wallet's
///   guarded `verifyCreditStatus` checks it against that retained Payment and operation. That happens
///   before the transport may persist or confirm it. The folded status carries no readable scheme, so
///   the scheme is enforced by that Native verification and is never inferred from the status's shape.
/// - Receiver: Native's own exact Request for one request identity (issue-or-retrieve returns the same
///   signed bytes). It names this wallet and its signed asset. A Payment counts only after Native binds it
///   to that Request. The card's acknowledgement is only Native's Payment-matched receipt: its released Receive must
///   have taken exactly the delivered Payment bytes.
///
/// Use `profilePolicy` for every NFC call of the exchange; values validated under one binding are never
/// accepted under another.
public final class KagemushaSpartanPeerNfcBindingV1: IrohaPeerNfcExchangeBinderV1, @unchecked Sendable {
  public enum Role: Equatable, Sendable { case payer, receiver }

  let profile: IrohaPeerPayloadProfile = .kagemushaSpartanPaymentV3
  public let role: Role
  /// The exact retained signed Request this binding admits.
  public let request: IrohaPeerWireMessageV1
  /// Payer only: Native's exact retained Payment of the released Send.
  public let payment: IrohaPeerWireMessageV1?
  private let native: any KagemushaSpartanPeerNfcNativeV1
  private let operationId: Data?
  private let requestId: Data?
  private let walletId: Data
  private let assetDigest: Data
  private let lock = NSLock()
  private var boundPayments = Set<Data>()
  private var outcomes: [Data: KagemushaSpartanCreditOutcomeV1] = [:]
  private var issued: [Data: Data] = [:]

  private init(role: Role, native: any KagemushaSpartanPeerNfcNativeV1, request: IrohaPeerWireMessageV1,
               payment: IrohaPeerWireMessageV1?, operationId: Data?, requestId: Data?,
               walletId: Data, assetDigest: Data) {
    self.role = role; self.native = native; self.request = request; self.payment = payment
    self.operationId = operationId; self.requestId = requestId
    self.walletId = walletId; self.assetDigest = assetDigest
  }

  /// The NFC policy whose every exchange check this binding decides.
  public var profilePolicy: IrohaPeerNfcProfilePolicyV1 { IrohaPeerNfcProfilePolicyV1(binder: self) }

  // MARK: Payer

  /// The payer's binding for its released Send `sendOperationId` of exactly `request`.
  public static func payer(wallet: KagemushaSpartanWalletV1, sendOperationId: Data,
                           request: IrohaPeerWireMessageV1) throws -> KagemushaSpartanPeerNfcBindingV1 {
    try payer(native: wallet, sendOperationId: sendOperationId, request: request)
  }

  static func payer(native: any KagemushaSpartanPeerNfcNativeV1, sendOperationId: Data,
                    request: IrohaPeerWireMessageV1) throws -> KagemushaSpartanPeerNfcBindingV1 {
    try requireIdentity(sendOperationId)
    let open = try openRequest(request)
    let scope = try enrolledScope(native)
    guard open.request.subdata(in: assetRange) == scope.asset else { throw KagemushaWalletErrorV1.invalidInput }
    guard case .released(let bytes) = try native.output(operationId: sendOperationId, part: .payment),
          !bytes.isEmpty else { throw KagemushaWalletErrorV1.invalidInput }
    // Preflight: a released Payment is bounded by the profile's complete `KWPYPC1` carrier.
    guard bytes.count <= IrohaPeerNfcV1.maximumPaymentMessageBytes else {
      throw IrohaPeerNfcErrorV1.messageTooLarge(actual: bytes.count, maximum: IrohaPeerNfcV1.maximumPaymentMessageBytes)
    }
    let payment = try IrohaPeerWireMessageV1.decode(bytes, expectedProfile: .kagemushaSpartanPaymentV3,
                                                    expectedKind: .payment)
    let verified = try native.verifyPayment(payment, matching: request)
    // The completed Request's payer specialization is this wallet under its signed asset.
    guard verified.request.count == completedRequestBytes,
          verified.request.subdata(in: payerWalletRange) == scope.wallet,
          verified.request.subdata(in: assetRange) == scope.asset else { throw KagemushaWalletErrorV1.invalidInput }
    return .init(role: .payer, native: native, request: request, payment: payment, operationId: sendOperationId,
                 requestId: nil, walletId: scope.wallet, assetDigest: scope.asset)
  }

  /// The payer's durable sender checkpoint for the receiver card's `sessionID`: the exact Request, Native's
  /// exact retained Payment and, once verified, the receiver's acknowledgement. Resuming it never debits again.
  public func senderCheckpoint(sessionID: Data,
                               acknowledgement: IrohaPeerWireMessageV1? = nil) throws -> IrohaPeerNfcSenderCheckpointV1 {
    guard role == .payer, let payment else { throw KagemushaWalletErrorV1.invalidInput }
    return try IrohaPeerNfcSenderCheckpointV1(sessionID: sessionID, receiveRequest: request.encoded,
      payment: payment.encoded, durableAcknowledgement: acknowledgement?.encoded, profilePolicy: profilePolicy)
  }

  /// Native's verified outcome of an acknowledgement this payer binding accepted; nil for any other.
  public func verifiedOutcome(of acknowledgement: IrohaPeerWireMessageV1) -> KagemushaSpartanCreditOutcomeV1? {
    lock.lock(); defer { lock.unlock() }
    return outcomes[acknowledgement.wireHash]
  }

  // MARK: Receiver

  /// The receiver's binding for its own exact Request `request` issued for `requestId`.
  public static func receiver(wallet: KagemushaSpartanWalletV1, requestId: Data,
                              request: IrohaPeerWireMessageV1) throws -> KagemushaSpartanPeerNfcBindingV1 {
    try receiver(native: wallet, requestId: requestId, request: request)
  }

  static func receiver(native: any KagemushaSpartanPeerNfcNativeV1, requestId: Data,
                       request: IrohaPeerWireMessageV1) throws -> KagemushaSpartanPeerNfcBindingV1 {
    try requireIdentity(requestId)
    let open = try openRequest(request)
    let scope = try enrolledScope(native)
    guard open.request.subdata(in: assetRange) == scope.asset,
          open.request.subdata(in: receiverWalletRange) == scope.wallet else { throw KagemushaWalletErrorV1.invalidInput }
    let amount = littleEndian128(open.request.subdata(in: amountRange))
    let issued = try native.issueOpenRequest(requestId: requestId, amount: amount, feeSchedule: open.feeSchedule,
                                             feeCertificate: try feeCertificate(open))
    // Only Native's own signed Request for this identity is hosted.
    guard issued.encoded == request.encoded else { throw KagemushaWalletErrorV1.invalidInput }
    return .init(role: .receiver, native: native, request: request, payment: nil, operationId: nil,
                 requestId: requestId, walletId: scope.wallet, assetDigest: scope.asset)
  }

  /// Native's own CreditStatus for its released Receive `receiveOperationId` of exactly `payment` against this
  /// binding's Request: Native compares the complete Payment bytes with that Receive's before it answers. Only a
  /// status obtained here is accepted as this card's acknowledgement.
  public func acknowledgement(receiveOperationId: Data,
                              payment: IrohaPeerWireMessageV1) throws -> KagemushaSpartanReceiverCreditStatusV1 {
    guard role == .receiver, let requestId else { throw KagemushaWalletErrorV1.invalidInput }
    try Self.requireIdentity(receiveOperationId)
    try requirePayment(payment, boundTo: request)
    // Native compares the complete delivered Payment with the one its released Receive took.
    let status = try native.receivedCreditStatus(requestId: requestId, receiveOperationId: receiveOperationId,
                                                 payment: payment)
    guard status.message.profile == .kagemushaSpartanPaymentV3, status.message.kind == .credited else {
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
    lock.lock(); defer { lock.unlock() }
    if let known = issued[payment.wireHash], known != status.message.wireHash {
      throw IrohaPeerNfcErrorV1.conflictingReplay
    }
    issued[payment.wireHash] = status.message.wireHash
    return status
  }

  /// The card's durable record of exactly this exchange, for a restarted card hosting the same session.
  public func durableAcknowledgement(sessionID: Data, payment: IrohaPeerWireMessageV1,
                                     acknowledgement: IrohaPeerWireMessageV1) throws -> IrohaPeerNfcDurableAcknowledgementV1 {
    guard role == .receiver else { throw KagemushaWalletErrorV1.invalidInput }
    let identity = try IrohaPeerNfcRequestIdentityV1(profile: profile, sessionID: sessionID,
      requestCanonicalHash: request.canonicalHash, requestWireHash: request.wireHash)
    let context = try IrohaPeerNfcCommitContextV1(identity: identity, profilePolicy: profilePolicy,
                                                  receiveRequest: request, payment: payment)
    return try IrohaPeerNfcDurableAcknowledgementV1(context: context, acknowledgement: acknowledgement.encoded)
  }

  // MARK: Exchange authority (IrohaPeerNfcExchangeBinderV1)

  func requireRequest(_ candidate: IrohaPeerWireMessageV1) throws {
    guard candidate.encoded == request.encoded else { throw IrohaPeerNfcErrorV1.continuityMismatch }
  }

  func requirePayment(_ candidate: IrohaPeerWireMessageV1, boundTo request: IrohaPeerWireMessageV1) throws {
    try requireRequest(request)
    guard candidate.profile == .kagemushaSpartanPaymentV3, candidate.kind == .payment,
          candidate.encoded.count <= IrohaPeerNfcV1.maximumPaymentMessageBytes else {
      throw IrohaPeerNfcErrorV1.continuityMismatch
    }
    switch role {
    case .payer:
      // The exact retained outgoing Payment; never a replacement.
      guard candidate.encoded == payment?.encoded else { throw IrohaPeerNfcErrorV1.continuityMismatch }
    case .receiver:
      if bound(candidate.wireHash) { return }
      let verified = try native.verifyPayment(candidate, matching: self.request)
      guard verified.request.count == Self.completedRequestBytes,
            verified.request.subdata(in: Self.assetRange) == assetDigest,
            verified.request.subdata(in: Self.receiverWalletRange) == walletId,
            verified.request.subdata(in: Self.payerWalletRange).contains(where: { $0 != 0 }) else {
        throw IrohaPeerNfcErrorV1.continuityMismatch
      }
      lock.lock(); boundPayments.insert(candidate.wireHash); lock.unlock()
    }
  }

  func requireAcknowledgement(_ acknowledgement: IrohaPeerWireMessageV1, request: IrohaPeerWireMessageV1,
                              paymentWireHash: Data, payment: IrohaPeerWireMessageV1?) throws {
    try requireRequest(request)
    guard acknowledgement.profile == .kagemushaSpartanPaymentV3, acknowledgement.kind == .credited else {
      throw IrohaPeerNfcErrorV1.continuityMismatch
    }
    if let payment {
      guard payment.wireHash == paymentWireHash else { throw IrohaPeerNfcErrorV1.continuityMismatch }
      try requirePayment(payment, boundTo: request)
    }
    switch role {
    case .payer:
      guard let own = self.payment, let operationId, own.wireHash == paymentWireHash else {
        throw IrohaPeerNfcErrorV1.continuityMismatch
      }
      if verifiedOutcome(of: acknowledgement) != nil { return }
      let outcome = try native.verifyCreditStatus(sendOperationId: operationId, request: self.request,
                                                  status: acknowledgement)
      lock.lock(); outcomes[acknowledgement.wireHash] = outcome; lock.unlock()
    case .receiver:
      lock.lock(); let known = issued[paymentWireHash]; lock.unlock()
      guard known == acknowledgement.wireHash else { throw IrohaPeerNfcErrorV1.continuityMismatch }
    }
  }

  // MARK: Helpers

  private func bound(_ wireHash: Data) -> Bool {
    lock.lock(); defer { lock.unlock() }
    return boundPayments.contains(wireHash)
  }

  static let completedRequestBytes = 482
  static let assetRange = 34..<66
  static let payerWalletRange = 66..<98
  static let receiverWalletRange = 130..<162
  static let amountRange = 242..<258

  private static func requireIdentity(_ identity: Data) throws {
    guard identity.count == 32, identity.contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidInput }
  }

  private static func openRequest(_ request: IrohaPeerWireMessageV1) throws -> IrohaPeerSpartanOpenRequestV3 {
    guard request.profile == .kagemushaSpartanPaymentV3, request.kind == .request else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    return try IrohaPeerSpartanOpenRequestV3(canonicalPayload: request.canonicalPayload)
  }

  /// The admitted wallet and the signed asset of its Native custody marker.
  private static func enrolledScope(_ native: any KagemushaSpartanPeerNfcNativeV1) throws -> (wallet: Data, asset: Data) {
    let snapshot = try native.snapshot()
    guard snapshot.walletId.count == 32, snapshot.assetDigest.count == 32 else {
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
    return (Data(snapshot.walletId), Data(snapshot.assetDigest))
  }

  /// A fee-bearing Request's certificate set holds the fee-policy issuer (role 3) beside its Enrollment issuer.
  private static func feeCertificate(_ open: IrohaPeerSpartanOpenRequestV3) throws -> Data {
    guard !open.feeSchedule.isEmpty else { return Data() }
    let certificates = Data(open.certificates)
    for start in stride(from: 0, to: certificates.count, by: 172) where certificates[start + 34] == 3 {
      return certificates.subdata(in: start..<start + 172)
    }
    throw KagemushaWalletErrorV1.invalidInput
  }

  private static func littleEndian128(_ bytes: Data) -> KagemushaWalletUInt128V1 {
    let raw = [UInt8](bytes)
    func limb(_ offset: Int) -> UInt64 { (0..<8).reduce(0) { $0 | UInt64(raw[offset + $1]) << ($1 * 8) } }
    return .init(low: limb(0), high: limb(8))
  }
}

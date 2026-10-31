// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import CryptoKit
import Foundation
import UIKit
import IrohaSwift
import IrohaSwiftMobileTransports

/// Retain once in the existing device actor. Only its public bytes leave this process.
final class KagemushaNearbyDiagnosticIdentity {
  private let key = Curve25519.Signing.PrivateKey()
  var publicBytes: Data { key.publicKey.rawRepresentation }
  func sign(_ bytes: Data) throws -> Data { try key.signature(for: bytes) }
}

/// Diagnostic adapter for the existing exclusively held probe actor and installed Native wallet.
/// It neither opens a replacement wallet nor creates/overwrites any monetary key.
final class KagemushaNearbyDiagnostic: NSObject, IrohaPeerNearbyConnectionsTransportDelegateV1,
  @unchecked Sendable {
  struct Selection {
    let role: IrohaPeerNearbyRoleV1
    let recovery: Bool
    let continuation: Bool
    let requestID: Data
    let sessionID: Data
    let requestHash: Data
    let peerIdentity: Data
    /// Receiver-only exact Native-generated IPM1 Request, retained before discovery.
    let requestMessage: Data?
  }
  private let selection: Selection
  private let wallet: KagemushaWalletV1
  private let call: KagemushaWalletExchangeCallV1
  private let identity: KagemushaNearbyDiagnosticIdentity
  private let files: EnrollmentProbeFiles
  private let actorPath: String
  private let attemptPath: String
  private let prompt: KagemushaNearbyPrompt
  private let worker = DispatchQueue(label: "org.hyperledger.iroha.diagnostic.nearby-native")
  private let ingress = DispatchQueue(label: "org.hyperledger.iroha.diagnostic.nearby-ingress")
  private var transport: IrohaPeerNearbyConnectionsTransportV1!
  private let session: IrohaPeerNearbySessionV1
  private var gate: KagemushaNearbyAttemptGate
  private var connectedPeer: String?
  private var approvedPeer: String?
  private var verificationPeer: String?
  private var backgroundObserver: NSObjectProtocol?
  private var pendingReview: KagemushaWalletReviewV1?
  private var reviewApproval = KagemushaProbeReviewApproval()
  private var traceSequence = 0
  private var started = false
  private var completion: ((Result<Void, Error>) -> Void)?

  init(selection: Selection, wallet: KagemushaWalletV1, call: KagemushaWalletExchangeCallV1,
       identity: KagemushaNearbyDiagnosticIdentity, files: EnrollmentProbeFiles,
       actorPath: String, attemptPath: String, prompt: KagemushaNearbyPrompt) throws {
    guard !selection.continuation || (selection.role == .sender && !selection.recovery),
      selection.requestID.count == 32, selection.requestID.contains(where: { $0 != 0 }),
      selection.peerIdentity.count == 32, selection.peerIdentity != identity.publicBytes,
      selection.role == .receiver
        ? selection.requestMessage != nil
        : selection.requestMessage == nil
    else { throw EnrollmentProbeFailure.input }
    if let raw = selection.requestMessage {
      let message = try IrohaPeerWireMessageV1.decode(raw,
        expectedProfile: .kagemushaWalletV1, expectedKind: .request)
      guard message.canonicalHash == selection.requestHash,
        try IrohaPeerKagemushaWalletAdapterV1.destinationAccountOriginal(message) == wallet.metadata().accountOriginal
      else { throw EnrollmentProbeFailure.input }
    }
    self.selection = selection; self.wallet = wallet; self.call = call; self.identity = identity
    self.files = files; self.actorPath = actorPath; self.attemptPath = attemptPath; self.prompt = prompt
    gate = .init(role: selection.role == .sender ? .sender : .receiver, recovery: selection.recovery, continuation: selection.continuation)
    session = try IrohaPeerNearbySessionV1(profile: .kagemushaWalletV1,
      localRole: selection.role, sessionID: selection.sessionID,
      requestCanonicalHash: selection.requestHash, deviceCertificate: identity.publicBytes)
    super.init()
    transport = IrohaPeerNearbyConnectionsTransportV1(callbackQueue: ingress)
    transport.delegate = self
  }

  /// Prepare before sharing discovery metadata. This copy is setup DATA, not radio evidence.
  static func prepare(wallet: KagemushaWalletV1, call: KagemushaWalletExchangeCallV1, setupID: Data, amount: KagemushaWalletUInt128V1,
                      feeSchedule: Data? = nil, feeCertificate: Data? = nil) throws -> Data {
    let issued = try call.request(setupId: setupID, amount: amount,
      feeSchedule: feeSchedule, feeCertificate: feeCertificate)
    guard issued.status == 12 else { throw EnrollmentProbeFailure.lifecycle }
    let envelope = try wallet.envelope(.request, original: issued.bytes)
    return try IrohaPeerKagemushaWalletAdapterV1.wrap(envelope,
      destinationAccountOriginal: wallet.metadata().accountOriginal).encoded
  }

  /// Caller keeps the existing actor's exclusive lock throughout this asynchronous operation.
  func run(completion: @escaping (Result<Void, Error>) -> Void) {
    worker.async { [self] in
      guard !self.started, !self.gate.terminal else { completion(.failure(EnrollmentProbeFailure.lifecycle)); return }
      self.started = true; self.completion = completion
      self.perform {
        try self.files.mkdirExclusive(self.attemptPath)
        try self.files.mkdir(self.actorPath + "/private/nearby")
        try self.files.publishDispatchFence(self.actorPath + "/private/nearby/session-"
          + EnrollmentProbeFiles.digest(self.selection.sessionID), bytes: self.selection.requestHash)
        self.backgroundObserver = NotificationCenter.default.addObserver(
          forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: nil) { [weak self] _ in self?.cancel() }
        let context = try IrohaPeerNearbyDiscoveryContextV1(profile: .kagemushaWalletV1,
          role: self.selection.role, sessionID: self.selection.sessionID,
          requestCanonicalHash: self.selection.requestHash)
        try self.trace("attempt-start", ["localCertificateSHA256": EnrollmentProbeFiles.digest(self.identity.publicBytes),
          "peerCertificateSHA256": EnrollmentProbeFiles.digest(self.selection.peerIdentity),
          "recovery": String(self.selection.recovery)])
        if self.selection.role == .sender { self.transport.startDiscovering(context: context) }
        else { self.transport.startAdvertising(context: context) }
        self.worker.asyncAfter(deadline: .now() + 300) {
          if !self.gate.terminal { self.fail(EnrollmentProbeFailure.timeout) }
        }
      }
    }
  }
  /// Call on foreground loss/cancellation; interrupted attempts remain failures, never resampled.
  func cancel() { worker.async { self.fail(EnrollmentProbeFailure.lifecycle) } }
  private func perform(_ body: () throws -> Void) {
    guard !gate.terminal else { return }
    do { try body() } catch { fail(error) }
  }
  private func trace(_ event: String, _ extra: [String: String] = [:], at: UInt64? = nil) throws {
    var row = extra
    let hex = { (data: Data) in data.map { String(format: "%02x", $0) }.joined() }
    row["requestID"] = hex(selection.requestID); row["sessionID"] = hex(selection.sessionID)
    row["requestCanonicalHash"] = hex(selection.requestHash)
    row["role"] = selection.role == .sender ? "SENDER" : "RECEIVER"
    row["conditionalContinuation"] = String(selection.continuation)
    row["recovery"] = String(selection.recovery)
    row["clock"] = KagemushaNearbyClock.identity
    row["schema"] = "iroha.kagemusha.physical-nearby-trace.v1"
    row["event"] = event; row["monotonicNs"] = String(at ?? KagemushaNearbyClock.nowNanoseconds())
    row["sequence"] = String(traceSequence); row["attempt"] = attemptPath
    row["transportIdentity"] = "raw-ed25519-application-pinned"
    row["hardwareAttestationClaimed"] = "false"; row["shippingQualificationClaimed"] = "false"
    try files.publish(attemptPath + String(format: "/%04d.json", traceSequence), bytes: NoritoJSON(row).data)
    traceSequence += 1
  }
  private func fail(_ error: Error) {
    guard !gate.terminal else { return }
    gate.fail()
    if let review = pendingReview { try? call.exchange.discardReview(review); pendingReview = nil }
    // Preserve the typed failure without exporting underlying secret-bearing descriptions.
    let native = (error as? KagemushaWalletErrorV1).map { value -> String in
      if case .native(let status, _, _) = value { return String(status) }; return "unavailable"
    } ?? "not-native"
    try? trace("attempt-failed", ["nativeStatus": native, "latencyQualified": "false"])
    teardown()
    let done = completion; completion = nil; done?(.failure(error))
  }
  private func teardown() {
    if let observer = backgroundObserver { NotificationCenter.default.removeObserver(observer); backgroundObserver = nil }
    session.destroy(); transport.stop()
    Task { @MainActor in self.prompt.cancel() }
  }
  private func finish() {
    teardown()
    let done = completion; completion = nil; done?(.success(()))
  }
  private func send(_ raw: Data, finished: (() throws -> Void)? = nil) throws {
    try files.publish(attemptPath + String(format: "/%04d-radio-send.bin", traceSequence), bytes: raw)
    try trace("radio-send", ["outputSHA256": EnrollmentProbeFiles.digest(raw)])
    transport.send(raw) { result in
      self.worker.async { self.perform {
        try result.get()
        try self.trace("radio-transfer-complete", ["outputSHA256": EnrollmentProbeFiles.digest(raw)])
        try finished?()
      }}
    }
  }
  private func sendMessage(_ envelope: Data, finished: (() throws -> Void)? = nil) throws {
    let wire = try IrohaPeerKagemushaWalletAdapterV1.wrap(envelope)
    try send(session.seal(wire.encoded).encode(), finished: finished)
  }
  private var fencePath: String {
    actorPath + "/private/intents/executed-" + EnrollmentProbeFiles.digest(selection.requestID)
  }
  private var reviewFencePath: String { selection.continuation ? attemptPath + "/review-dispatch.norito" : fencePath }
  private func bind(_ family: String, _ originals: [String: Data]) throws {
    try files.mkdir(actorPath + "/private/intents")
    var inputs = originals
    inputs["family"] = Data(family.utf8); inputs["request-id"] = selection.requestID
    let path = actorPath + "/private/intents/operation-" + EnrollmentProbeFiles.digest(selection.requestID)
    if selection.recovery || selection.continuation { guard try files.exists(path) else { throw EnrollmentProbeFailure.lifecycle } }
    try files.publish(path,
      bytes: KagemushaMonetaryProbeManifest.binding("operation", inputs: inputs))
  }
  private func sendPayment(_ call: KagemushaWalletCallV1) throws {
    try trace("native-return", ["nativeStatus": String(call.status),
      "outputSHA256": EnrollmentProbeFiles.digest(call.bytes)])
    try KagemushaNearbyAttemptGate.requireComplete(call.status)
    let original = try retainedOriginal(requestID: selection.requestID, kind: .send, peer: .payment)
    let companion = try wallet.paymentCompanion(payment: original)
    try files.publish(attemptPath + String(format: "/%04d-payer-companion.norito", traceSequence), bytes: companion)
    let envelope = try wallet.envelope(.payment, original: original)
    let wire = try IrohaPeerKagemushaWalletAdapterV1.wrap(envelope, payerCompanion: companion)
    try gate.sentPayment()
    try send(session.seal(wire.encoded).encode())
  }
  /// Capture the actual retained release only while Native still owns that output.
  private func retainedOriginal(requestID: Data, kind: KagemushaWalletReleasedOutputV1.Kind,
                                peer: KagemushaWalletTransportKindV1) throws -> Data {
    let value = try wallet.releasedOutput(requestId: requestID)
    guard value.kind == kind, value.peerKind == peer, let original = value.peerOriginal else {
      throw EnrollmentProbeFailure.lifecycle
    }
    try files.publish(attemptPath + String(format: "/%04d-released-original.norito", traceSequence), bytes: original)
    try trace("native-durable-released", ["operationID": EnrollmentProbeFiles.digest(value.operationID),
      "outputSHA256": EnrollmentProbeFiles.digest(original)])
    return original
  }
  private func request(_ message: IrohaPeerWireMessageV1) throws {
    guard message.canonicalHash == selection.requestHash else { throw EnrollmentProbeFailure.input }
    let envelope = try IrohaPeerKagemushaWalletAdapterV1.decode(message)
    let original = try wallet.original(.request, envelope: envelope)
    let account = try IrohaPeerKagemushaWalletAdapterV1.destinationAccountOriginal(message)
    try bind("review-send", ["request": original, "destination": account])
    if selection.recovery {
      guard try files.exists(fencePath) else { throw EnrollmentProbeFailure.lifecycle }
      try sendPayment(wallet.requestStatus(requestId: selection.requestID))
      return
    }
    let expectation = try KagemushaProbeReviewCustody.expectation(files: files, actorPath: actorPath,
      requestID: selection.requestID, family: "review-send", originals: ["request": original, "destination": account],
      continuation: selection.continuation)
    let decision = try call.reviewSend(requestId: selection.requestID, request: original,
      destinationAccountOriginal: account, expectation: expectation)
    let review: KagemushaWalletReviewV1
    switch decision {
    case .ready(let value): review = value
    case .existing(let value): try sendExisting(value); return
    case .waiting(let wait):
      try trace("native-review-waiting", ["reason": String(wait.rawValue), "capabilityIssued": "false"])
      throw EnrollmentProbeFailure.lifecycle
    }
    pendingReview = review; reviewApproval = .init()
    if selection.continuation { try gate.renewed() }
    try files.publish(attemptPath + "/native-review.bin", bytes: review.projection.bytes)
    try trace("native-review", ["outputSHA256": EnrollmentProbeFiles.digest(review.projection.bytes),
      "originalFence": fencePath, "dispatchFence": reviewFencePath])
    Task { @MainActor in self.prompt.review(review.projection) { decision in
      self.worker.async { self.perform {
        let confirmedAt = try decision.get()
        guard self.pendingReview === review, review.requestID == self.selection.requestID else {
          throw EnrollmentProbeFailure.lifecycle
        }
        try self.gate.confirmed(at: confirmedAt)
        try self.trace("user-authenticated", ["reviewSHA256": EnrollmentProbeFiles.digest(review.projection.bytes)], at: confirmedAt)
        try self.files.publishDispatchFence(self.reviewFencePath, bytes: review.projection.bytes)
        try self.reviewApproval.acknowledgedFence()
        try self.executeReview(review, retry: false)
      }}
    }}
  }
  private func sendExisting(_ value: KagemushaWalletBoundRequestV1) throws {
    try trace("native-existing-request", ["latencyQualified": "false", "newDispatch": "false"])
    guard case .outcome(let outcome) = value else { throw EnrollmentProbeFailure.lifecycle }
    try gate.observedExisting(); try sendPayment(outcome)
  }
  private func executeReview(_ review: KagemushaWalletReviewV1, retry: Bool) throws {
    guard pendingReview === review, review.requestID == selection.requestID,
      try files.read(reviewFencePath, maximum: 4591) == review.projection.bytes else {
      throw EnrollmentProbeFailure.lifecycle
    }
    try reviewApproval.dispatch(retry: retry); pendingReview = nil
    switch try call.executeReviewed(review) {
    case .executed(let value): reviewApproval.finish(); try sendPayment(value)
    case .existing(let value): reviewApproval.finish(); try sendExisting(value)
    case .retainedReview(let wait):
      try reviewApproval.retained(); pendingReview = review; try gate.retainedReview()
      try trace("native-review-retained", ["reason": String(wait.rawValue),
        "dispatchFenceRetained": "true", "latencyQualified": "false"])
      Task { @MainActor in self.prompt.retryApproved { accepted in
        self.worker.async { self.perform {
          guard accepted else { throw EnrollmentProbeFailure.lifecycle }
          try self.executeReview(review, retry: true)
        }}
      }}
    }
  }
  private func payment(_ message: IrohaPeerWireMessageV1) throws {
    guard let rawRequest = selection.requestMessage else { throw EnrollmentProbeFailure.input }
    let request = try IrohaPeerWireMessageV1.decode(rawRequest,
      expectedProfile: .kagemushaWalletV1, expectedKind: .request)
    let requestEnvelope = try IrohaPeerKagemushaWalletAdapterV1.decode(request)
    let paymentEnvelope = try IrohaPeerKagemushaWalletAdapterV1.decode(message)
    try KagemushaWalletWireV1.requireExchangeBinding(request: requestEnvelope, payment: paymentEnvelope)
    let original = try wallet.original(.payment, envelope: paymentEnvelope)
    let companion = try IrohaPeerKagemushaWalletAdapterV1.payerCompanion(message)
    try bind("receive", ["payment": original, "payer-companion": companion])
    let call: KagemushaWalletCallV1
    if selection.recovery {
      guard try files.exists(fencePath) else { throw EnrollmentProbeFailure.lifecycle }
      call = try wallet.requestStatus(requestId: selection.requestID)
    } else {
      try files.publishDispatchFence(fencePath, bytes: Data(EnrollmentProbeFiles.digest(original).utf8))
      call = try self.call.receiveWithCompanion(requestId: selection.requestID, payment: original, companion: companion)
    }
    try trace("native-return", ["nativeStatus": String(call.status),
      "outputSHA256": EnrollmentProbeFiles.digest(call.bytes)])
    let creditedOriginal = try KagemushaNearbyAttemptGate.receivedCredit(
      status: call.status, recovery: selection.recovery, requestID: selection.requestID,
      retainedOutput: { requestID in
        _ = try self.retainedOriginal(requestID: requestID, kind: .receive, peer: .credited)
      },
      projection: { requestID in
        let current = try self.wallet.creditProjection(receiveRequestId: requestID)
        return .init(isReceiver: current.archive == .receiver, isBurned: current.evidence == .burned,
          original: current.creditedOriginal)
      })
    try files.publish(attemptPath + String(format: "/%04d-fresh-credited-original.norito", traceSequence),
      bytes: creditedOriginal)
    try trace("native-fresh-received-credit", ["outputSHA256": EnrollmentProbeFiles.digest(creditedOriginal)])
    let envelope = try wallet.envelope(.credited, original: creditedOriginal)
    try sendMessage(envelope) {
      try self.trace("receiver-ack-delivered", ["latencyQualified": "false",
        "creditedSHA256": EnrollmentProbeFiles.digest(creditedOriginal), "physicalCarrierObserved": "true"])
      try self.gate.receiverDelivered(); self.finish()
    }
  }
  private func credited(_ message: IrohaPeerWireMessageV1, arrival: UInt64) throws {
    let original = try wallet.original(.credited, envelope: IrohaPeerKagemushaWalletAdapterV1.decode(message))
    try files.publish(attemptPath + String(format: "/%04d-credited-original.norito", traceSequence), bytes: original)
    try trace("credited-arrival", ["creditedSHA256": EnrollmentProbeFiles.digest(original)], at: arrival)
    let delivery = try call.observeCredited(sendRequestId: selection.requestID, credited: original)
    let verifiedAt = KagemushaNearbyClock.nowNanoseconds()
    let hex = { (bytes: Data) in bytes.map { String(format: "%02x", $0) }.joined() }
    try trace("native-verified-delivery", [
      "evidence": String(delivery.evidence.rawValue), "archive": String(delivery.archive.rawValue),
      "corePending": String(delivery.corePending), "creditID": hex(delivery.creditId),
      "paymentDigest": hex(delivery.paymentDigest), "amountLow": String(delivery.amount.low),
      "amountHigh": String(delivery.amount.high), "creditedSHA256": EnrollmentProbeFiles.digest(original),
      "archiveDispatched": "false"], at: verifiedAt)
    // Delivery verification does not Advance; Native maintenance admits ArchiveSent after
    // the exchange ends, subject to the current source, fold and idle conditions.
    let elapsed = try gate.credited(evidence: delivery.evidence, archive: delivery.archive,
      corePending: delivery.corePending, arrival: arrival, verified: verifiedAt)
    do {
      try trace("payer-complete", ["latencyQualified": String(elapsed != nil),
        "rawArrivalElapsedNs": elapsed.map { String($0.0) } ?? "unknown",
        "elapsedNs": elapsed.map { String($0.1) } ?? "unknown",
        "rawArrivalScope": "receiver-durable-completion-upper-bound",
        "elapsedScope": "payer-confirmation-to-verified-delivery",
        "creditedSHA256": EnrollmentProbeFiles.digest(original), "physicalCarrierObserved": "true",
        "timingAnchor": elapsed == nil ? "unknown-original-process" : "payer-authentication-success"], at: verifiedAt)
    } catch {
      // Verified delivery without a durable diagnostic trace is still a failed diagnostic.
      teardown(); let done = completion; completion = nil
      done?(.failure(error)); return
    }
    finish()
  }
  func nearbyTransport(_ transport: IrohaPeerNearbyConnectionsTransportV1,
                       didReceive data: Data, from peerID: String) {
    let arrival = KagemushaNearbyClock.nowNanoseconds()
    worker.async { self.perform {
      guard peerID == self.connectedPeer else { throw EnrollmentProbeFailure.lifecycle }
      try self.files.publish(self.attemptPath + String(format: "/%04d-radio-received.bin", self.traceSequence), bytes: data)
      try self.trace("radio-received", ["inputSHA256": EnrollmentProbeFiles.digest(data)], at: arrival)
      switch self.gate.inboundCount {
      case 0:
        try self.gate.admit(.hello)
        try self.session.acceptPeerHello(IrohaPeerNearbyHelloV1.decode(data))
        let auth = try self.session.makeAuthentication(signature: self.identity.sign(self.session.authenticationPreimage()))
        try self.send(auth.encode())
      case 1:
        try self.gate.admit(.authentication)
        try self.session.acceptPeerAuthentication(IrohaPeerNearbyAuthenticationV1.decode(data)) { role, certificate, bytes, signature in
          guard role == self.selection.role.peer, certificate == self.selection.peerIdentity else { return false }
          return try Curve25519.Signing.PublicKey(rawRepresentation: certificate).isValidSignature(signature, for: bytes)
        }
        if let request = self.selection.requestMessage {
          try self.send(self.session.seal(request).encode())
        }
      default:
        let raw = try self.session.open(IrohaPeerNearbyEncryptedRecordV1.decode(data))
        try self.files.publish(self.attemptPath + String(format: "/%04d-opened-ipm1.bin", self.traceSequence), bytes: raw)
        let expected: IrohaPeerWireKindV1 = self.selection.role == .receiver ? .payment
          : (self.gate.inboundCount == 2 ? .request : .credited)
        let message = try IrohaPeerWireMessageV1.decode(raw,
          expectedProfile: .kagemushaWalletV1, expectedKind: expected)
        switch expected {
        case .request: try self.gate.admit(.request); try self.request(message)
        case .payment: try self.gate.admit(.payment); try self.payment(message)
        case .credited: try self.gate.admit(.credited); try self.credited(message, arrival: arrival)
        default: throw EnrollmentProbeFailure.lifecycle
        }
      }
    }}
  }
  func nearbyTransport(_ transport: IrohaPeerNearbyConnectionsTransportV1,
                       didChange state: IrohaPeerNearbyConnectionsStateV1) {
    worker.async { self.perform {
      switch state {
      case .connected(let peer):
        guard self.connectedPeer == nil, self.approvedPeer == peer else { throw EnrollmentProbeFailure.lifecycle }
        self.connectedPeer = peer; try self.send(self.session.localHello.encode())
      case .failed, .stopped: throw EnrollmentProbeFailure.lifecycle
      default: break
      }
    }}
  }
  func nearbyTransport(_ transport: IrohaPeerNearbyConnectionsTransportV1,
                       verify code: String, peerID: String, decision: @escaping (Bool) -> Void) {
    worker.async {
      guard !self.gate.terminal else { decision(false); return }
      self.perform {
        guard self.verificationPeer == nil, self.approvedPeer == nil else {
          decision(false); throw EnrollmentProbeFailure.lifecycle
        }
        self.verificationPeer = peerID
        Task { @MainActor in self.prompt.pairing(code: code, peer: peerID,
          certificate: EnrollmentProbeFiles.digest(self.selection.peerIdentity),
          session: self.selection.sessionID.map { String(format: "%02x", $0) }.joined()) { accepted in
          self.worker.async {
            guard !self.gate.terminal else { decision(false); return }
            self.perform {
              guard self.verificationPeer == peerID, self.approvedPeer == nil else {
                decision(false); throw EnrollmentProbeFailure.lifecycle
              }
              try self.trace("pairing-decision", ["accepted": String(accepted), "peerID": peerID,
                "peerCertificateSHA256": EnrollmentProbeFiles.digest(self.selection.peerIdentity),
                "sessionID": EnrollmentProbeFiles.digest(self.selection.sessionID)])
              if accepted { self.approvedPeer = peerID }
              decision(accepted)
              guard accepted else { throw EnrollmentProbeFailure.lifecycle }
            }
          }
        }}
      }
    }
  }
  func nearbyTransport(_ transport: IrohaPeerNearbyConnectionsTransportV1,
                       didResolvePeerContext context: IrohaPeerNearbyDiscoveryContextV1, peerID: String) {
    worker.async { self.perform {
      guard context.profile == .kagemushaWalletV1, context.sessionID == self.selection.sessionID,
        context.requestCanonicalHash == self.selection.requestHash else { throw EnrollmentProbeFailure.input }
    }}
  }
}

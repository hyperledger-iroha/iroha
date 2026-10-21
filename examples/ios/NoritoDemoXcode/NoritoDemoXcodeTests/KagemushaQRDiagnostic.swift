// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
#if os(iOS) && !targetEnvironment(macCatalyst)
import Foundation
import UIKit
import IrohaSwift

/// Actual camera/display orchestration over the exclusively held installed Native wallet.
/// Neither QR framing nor these retained diagnostic inputs grant monetary authority.
final class KagemushaQRDiagnostic: @unchecked Sendable {
  struct Selection {
    let sender: Bool
    let recovery: Bool
    let continuation: Bool
    let setupID: Data
    let requestID: Data
    let inputs: [String: Data]
  }
  private let selection: Selection
  private let wallet: KagemushaWalletV1
  private let call: KagemushaWalletExchangeCallV1
  private let files: EnrollmentProbeFiles
  private let actorPath: String
  private let attemptPath: String
  private let ui: KagemushaQRPlatform
  private let prompt: KagemushaNearbyPrompt
  private let worker = DispatchQueue(label: "org.hyperledger.iroha.probe.qr-native")
  private var gate: KagemushaQRAttemptGate!
  private var pendingReview: KagemushaWalletReviewV1?
  private var reviewApproval = KagemushaProbeReviewApproval()
  private var background: NSObjectProtocol?
  private var completion: ((Result<Void, Error>) -> Void)?
  private var started = false
  private var cancellationRequested = false
  private var deadline: UInt64 = 0
  private var sceneDisconnected: NSObjectProtocol?
  private var finished = false
  private var traceIndex = 0
  private var outputGeneration = UUID()
  private var dispatched = false
  private var scanner: IrohaPeerQRScanSessionV1?
  private var recordIndex = 0
  private var retainedInputs: [String: Data] = [:]

  init(selection: Selection, wallet: KagemushaWalletV1, call: KagemushaWalletExchangeCallV1, files: EnrollmentProbeFiles,
       actorPath: String, attemptPath: String, ui: KagemushaQRPlatform,
       prompt: KagemushaNearbyPrompt) {
    self.selection = selection; self.wallet = wallet; self.call = call; self.files = files
    self.actorPath = actorPath; self.attemptPath = attemptPath; self.ui = ui; self.prompt = prompt
  }
  private var setupPath: String { actorPath + "/private/qr/setup-" + EnrollmentProbeFiles.digest(selection.setupID) }
  private var fencePath: String { actorPath + "/private/intents/executed-" + EnrollmentProbeFiles.digest(selection.requestID) }
  private var reviewFencePath: String { selection.continuation ? attemptPath + "/review-dispatch.norito" : fencePath }
  private func retained(_ name: String, maximum: Int = IrohaPeerWireMessageV1.headerBytes + IrohaPeerWireLimitsV1.maximumWalletProfileBytes) throws -> Data? {
    let path = setupPath + "/" + name
    return try files.exists(path) ? files.read(path, maximum: maximum) : nil
  }
  private func keep(_ name: String, _ bytes: Data) throws { try files.publish(setupPath + "/" + name, bytes: bytes) }
  private func publicBytes(_ name: String, _ bytes: Data) throws { try files.publish(attemptPath + "/" + name, bytes: bytes) }
  private func trace(_ event: String, _ extra: [String: String] = [:], at: UInt64? = nil) throws {
    var row = extra
    row["schema"] = "iroha.kagemusha.physical-qr-trace.v1"; row["event"] = event
    row["monotonicNs"] = String(at ?? KagemushaNearbyClock.nowNanoseconds())
    row["clock"] = KagemushaNearbyClock.identity; row["sequence"] = String(traceIndex)
    row["attempt"] = attemptPath; row["role"] = selection.sender ? "SENDER" : "RECEIVER"
    row["requestIDSHA256"] = EnrollmentProbeFiles.digest(selection.requestID)
    row["setupIDSHA256"] = EnrollmentProbeFiles.digest(selection.setupID)
    row["conditionalContinuation"] = String(selection.continuation)
    row["recovery"] = String(selection.recovery); row["shippingQualificationClaimed"] = "false"
    try publicBytes(String(format: "%04d.json", traceIndex), NoritoJSON(row).data); traceIndex += 1
  }
  private func perform(_ body: () throws -> Void) {
    guard !finished else { return }
    do {
      if started && deadline > 0 && KagemushaNearbyClock.nowNanoseconds() >= deadline { throw EnrollmentProbeFailure.timeout }
      try body()
    } catch { fail(error) }
  }
  func run(completion: @escaping (Result<Void, Error>) -> Void) {
    worker.async { [self] in
      guard !started, !finished else { completion(.failure(EnrollmentProbeFailure.lifecycle)); return }
      started = true; self.completion = completion
      deadline = KagemushaNearbyClock.nowNanoseconds() + 300_000_000_000
      perform {
        try files.mkdirExclusive(attemptPath)
        try files.mkdir(setupPath); try files.mkdir(actorPath + "/private/intents")
        try trace("attempt-start", ["observationLimit": String(KagemushaQRAttemptGate.maximumObservations),
          "deadlineSeconds": "300", "animationIntervalMilliseconds": "150"])
        guard !cancellationRequested else { throw EnrollmentProbeFailure.lifecycle }
        let identity = KagemushaMonetaryProbeManifest.binding("qr-setup", inputs: [
          "setup-id": selection.setupID, "request-id": selection.requestID,
          "role": Data((selection.sender ? "sender" : "receiver").utf8),
          "account": try wallet.metadata().accountOriginal,
        ])
        if selection.recovery || selection.continuation {
          guard try retained("identity") == identity,
            let bytes = try retained("inputs.plist", maximum: 16_384),
            let values = try PropertyListSerialization.propertyList(from: bytes, format: nil) as? [String: Data]
          else { throw EnrollmentProbeFailure.lifecycle }
          guard KagemushaQRProbeManifest.accepts(selection.sender ? "qr-send" : "qr-receive", names: Set(values.keys)),
            values["setup-id"] == selection.setupID, values["request-id"] == selection.requestID else {
            throw EnrollmentProbeFailure.input
          }
          retainedInputs = values
        } else {
          try files.publishDispatchFence(setupPath + "/identity", bytes: identity)
          retainedInputs = selection.inputs
          try keep("inputs.plist", EnrollmentProbeFiles.encode(selection.inputs))
        }
        dispatched = try files.exists(fencePath)
        guard (!selection.continuation || (selection.sender && dispatched)),
          selection.recovery || selection.continuation || !dispatched else { throw EnrollmentProbeFailure.lifecycle }
        gate = .init(role: selection.sender ? .sender : .receiver, recovery: selection.recovery,
          previouslyDispatched: dispatched, continuation: selection.continuation)
        background = NotificationCenter.default.addObserver(forName: UIApplication.didEnterBackgroundNotification,
          object: nil, queue: nil) { [weak self] _ in self?.cancel() }
        sceneDisconnected = NotificationCenter.default.addObserver(forName: UIScene.didDisconnectNotification,
          object: nil, queue: nil) { [weak self] _ in self?.cancel() }
        worker.asyncAfter(deadline: .now() + 300) { [weak self] in self?.fail(EnrollmentProbeFailure.timeout) }
        if selection.sender { scanRequest() }
        else { try showRequest() }
      }
    }
  }
  func cancel() { worker.async {
    self.cancellationRequested = true
    if self.started { self.fail(EnrollmentProbeFailure.lifecycle) }
  }}
  private func fail(_ error: Error) {
    guard !finished else { return }
    gate?.fail()
    if let review = pendingReview { try? call.exchange.discardReview(review); pendingReview = nil }
    let native = (error as? KagemushaWalletErrorV1).map { value -> String in
      if case .native(let status, _, _) = value { return String(status) }; return "unavailable"
    } ?? "not-native"
    try? trace("attempt-incomplete", ["nativeStatus": native, "latencyQualified": "false",
      "automaticResampling": "false"])
    finish(.failure(error))
  }
  private func finish(_ result: Result<Void, Error>) {
    guard !finished else { return }
    finished = true; outputGeneration = UUID(); scanner = nil
    if let background { NotificationCenter.default.removeObserver(background); self.background = nil }
    if let sceneDisconnected { NotificationCenter.default.removeObserver(sceneDisconnected); self.sceneDisconnected = nil }
    Task { @MainActor in
      self.prompt.cancel()
      self.ui.close {
        self.worker.async {
          let done = self.completion; self.completion = nil; done?(result)
        }
      }
    }
  }
  private func bind(_ family: String, _ inputs: [String: Data]) throws {
    let path = actorPath + "/private/intents/operation-" + EnrollmentProbeFiles.digest(selection.requestID)
    if dispatched { guard try files.exists(path) else { throw EnrollmentProbeFailure.lifecycle } }
    try files.publish(path, bytes: KagemushaMonetaryProbeManifest.binding("operation", inputs:
      inputs.merging(["family": Data(family.utf8), "request-id": selection.requestID]) { first, _ in first }))
  }
  private func packed(_ kind: KagemushaWalletTransportKindV1, original: Data,
                      account: Data? = nil, companion: Data? = nil,
                      label: String) throws -> IrohaPeerWireMessageV1 {
    let envelope = try wallet.envelope(kind, original: original)
    let message = try IrohaPeerKagemushaWalletAdapterV1.wrap(envelope,
      destinationAccountOriginal: account, payerCompanion: companion)
    try publicBytes(label + "-original.norito", original)
    try publicBytes(label + "-envelope.norito", envelope)
    try publicBytes(label + "-ipm1.bin", message.encoded)
    try trace("native-output-framed", ["kind": label, "originalBytes": String(original.count),
      "envelopeBytes": String(envelope.count), "ipm1Bytes": String(message.encoded.count),
      "envelopeSHA256": EnrollmentProbeFiles.digest(envelope), "carrierDeliveryClaimed": "false"])
    return message
  }
  private func unpacked(_ kind: KagemushaWalletTransportKindV1, _ message: IrohaPeerWireMessageV1,
                        label: String) throws -> Data {
    let envelope = try IrohaPeerKagemushaWalletAdapterV1.decode(message)
    let original = try wallet.original(kind, envelope: envelope)
    try publicBytes(label + "-original.norito", original)
    try publicBytes(label + "-envelope.norito", envelope)
    try trace("camera-message-native-decoded", ["kind": label, "envelopeBytes": String(envelope.count),
      "ipm1Bytes": String(message.encoded.count), "envelopeSHA256": EnrollmentProbeFiles.digest(envelope)])
    return original
  }
  private func display(_ texts: [String], label: String, next: String, done: @escaping () throws -> Void) throws {
    guard KagemushaNearbyClock.nowNanoseconds() < deadline else { throw EnrollmentProbeFailure.timeout }
    let frames = try texts.map(KagemushaQRRenderer.render)
    for (index, frame) in frames.enumerated() {
      try publicBytes(label + String(format: "-frame-%03d.txt", index), Data(frame.text.utf8))
      try publicBytes(label + String(format: "-frame-%03d.bin", index), IrohaPeerQRCodecV1.decodeFrame(frame.text).encoded)
    }
    let token = UUID(); outputGeneration = token
    Task { @MainActor in self.ui.display(frames, label: label, nextTitle: next,
      observed: { index, ticks in self.worker.async { self.perform {
        guard self.outputGeneration == token else { return }
        try self.gate.observation()
        try self.trace("qr-displayed", ["kind": label, "frameIndex": String(index),
          "textSHA256": EnrollmentProbeFiles.digest(Data(frames[index].text.utf8)),
          "symbolVersion": String(frames[index].version), "errorCorrection": "M",
          "carrierDeliveryClaimed": "false"], at: ticks)
      }}}, nextAction: { self.worker.async { self.perform {
        guard self.outputGeneration == token else { return }
        self.outputGeneration = UUID()
        try self.trace("operator-changed-screen", ["kind": label, "carrierDeliveryClaimed": "false"])
        try done()
      }}})
    }
  }
  private func displayMessage(_ message: IrohaPeerWireMessageV1, label: String, next: String,
                              done: @escaping () throws -> Void) throws {
    if let candidate = try IrohaPeerQRCodecV1.staticCompleteTextCandidate(for: message) {
      do {
        _ = try KagemushaQRRenderer.render(candidate)
        try display([candidate], label: label, next: next, done: done); return
      } catch KagemushaQRRenderer.Failure.capacity { /* Use the current canonical animated mode. */ }
    }
    try display(IrohaPeerQRCodecV1.animatedFrameTexts(for: message), label: label, next: next, done: done)
  }
  private func scan(_ record: KagemushaQRAttemptGate.Record, kind: IrohaPeerWireKindV1, label: String,
                    completed: @escaping (IrohaPeerWireMessageV1, UInt64) throws -> Void) {
    perform {
      let token = UUID(); try gate.beginScan(token)
      scanner = IrohaPeerQRScanSessionV1(expectedProfile: .kagemushaWalletV1,
        expectedKind: kind, expectedSchemaVersion: KagemushaWalletWireV1.version)
      Task { @MainActor in self.ui.scan(label: label, receive: { text, ticks in
        self.worker.async { self.perform {
          guard try self.gate.observed(token) else { return }
          let name = String(format: "camera-%04d", self.recordIndex); self.recordIndex += 1
          try self.publicBytes(name + ".txt", Data(text.utf8))
          try self.trace("camera-qr-observed", ["textSHA256": EnrollmentProbeFiles.digest(Data(text.utf8)),
            "observationFile": name + ".txt", "cameraObserved": "true"], at: ticks)
          guard let scanner = self.scanner else { throw EnrollmentProbeFailure.lifecycle }
          let frame = try IrohaPeerQRCodecV1.decodeFrame(text)
          try self.publicBytes(name + ".bin", frame.encoded)
          guard case let .completed(message) = try scanner.ingest(text, atUptime: Double(ticks) / 1_000_000_000)
          else { return }
          try self.publicBytes(label + "-scanned-ipm1.bin", message.encoded)
          try self.gate.admit(record, scan: token); self.scanner = nil
          Task { @MainActor in self.ui.stopScan {
            self.worker.async { self.perform { try completed(message, ticks) } }
          }}
        }}
      }, failure: { error in self.worker.async { self.fail(error) } }) }
    }
  }
  private func scanRequest() {
    scan(.request, kind: .request, label: "request") { message, _ in
      // Recovery observes the same optical Request again and refuses changed originals.
      try self.keep("request-ipm1", message.encoded)
      let request = try self.unpacked(.request, message, label: "request")
      let account = try IrohaPeerKagemushaWalletAdapterV1.destinationAccountOriginal(message)
      try self.publicBytes("request-account-original.norito", account)
      try self.bind("review-send", ["request": request, "destination": account])
      if self.dispatched && !self.selection.continuation {
        try self.sendReleased(self.wallet.requestStatus(requestId: self.selection.requestID))
        return
      }
      let expectation = try KagemushaProbeReviewCustody.expectation(files: self.files,
        actorPath: self.actorPath, requestID: self.selection.requestID, family: "review-send",
        originals: ["request": request, "destination": account], continuation: self.selection.continuation)
      let decision = try self.call.reviewSend(requestId: self.selection.requestID, request: request,
        destinationAccountOriginal: account, expectation: expectation)
      let review: KagemushaWalletReviewV1
      switch decision {
      case .ready(let value): review = value
      case .existing(let value): try self.sendExisting(value); return
      case .waiting(let wait):
        try self.trace("native-review-waiting", ["reason": String(wait.rawValue), "capabilityIssued": "false"])
        throw EnrollmentProbeFailure.lifecycle
      }
      self.pendingReview = review; self.reviewApproval = .init()
      if self.selection.continuation { try self.gate.renewed() }
      try self.publicBytes("native-review.bin", review.projection.bytes)
      try self.trace("native-review", ["originalFence": self.fencePath,
        "dispatchFence": self.reviewFencePath, "reviewSHA256": EnrollmentProbeFiles.digest(review.projection.bytes)])
      Task { @MainActor in self.prompt.review(review.projection) { decision in
        let foregroundDecision = Result { () throws -> UInt64 in
          let ticks = try decision.get(); try self.ui.requireForeground(); return ticks
        }
        self.worker.async { self.perform {
          let ticks = try foregroundDecision.get()
          guard self.pendingReview === review, review.requestID == self.selection.requestID else {
            throw EnrollmentProbeFailure.lifecycle
          }
          try self.gate.authenticated(at: ticks)
          try self.trace("user-authenticated", ["reviewSHA256": EnrollmentProbeFiles.digest(review.projection.bytes)], at: ticks)
          try self.files.publishDispatchFence(self.reviewFencePath, bytes: review.projection.bytes)
          try self.reviewApproval.acknowledgedFence(); self.dispatched = true
          try self.executeReview(review, retry: false)
        }}
      }}
    }
  }
  private func sendExisting(_ value: KagemushaWalletBoundRequestV1) throws {
    try trace("native-existing-request", ["latencyQualified": "false", "newDispatch": "false"])
    guard case .outcome(let outcome) = value else { throw EnrollmentProbeFailure.lifecycle }
    try gate.observedExisting(); try sendReleased(outcome)
  }
  private func executeReview(_ review: KagemushaWalletReviewV1, retry: Bool) throws {
    guard pendingReview === review, review.requestID == selection.requestID,
      try files.read(reviewFencePath, maximum: 4591) == review.projection.bytes else {
      throw EnrollmentProbeFailure.lifecycle
    }
    try reviewApproval.dispatch(retry: retry); pendingReview = nil
    switch try call.executeReviewed(review) {
    case .executed(let value): reviewApproval.finish(); try sendReleased(value)
    case .existing(let value): reviewApproval.finish(); try sendExisting(value)
    case .retainedReview(let wait):
      try reviewApproval.retained(); pendingReview = review; try gate.retainedReview()
      try trace("native-review-retained", ["reason": String(wait.rawValue),
        "dispatchFenceRetained": "true", "latencyQualified": "false"])
      Task { @MainActor in self.prompt.retryApproved { accepted in
        let foreground = Result { try self.ui.requireForeground() }
        self.worker.async { self.perform {
          try foreground.get()
          guard accepted else { throw EnrollmentProbeFailure.lifecycle }
          try self.executeReview(review, retry: true)
        }}
      }}
    }
  }
  private func release(_ kind: KagemushaWalletReleasedOutputV1.Kind,
                       peer: KagemushaWalletTransportKindV1) throws -> Data {
    let released = try wallet.releasedOutput(requestId: selection.requestID)
    guard released.kind == kind, released.peerKind == peer, let original = released.peerOriginal else {
      throw EnrollmentProbeFailure.lifecycle
    }
    try publicBytes("native-released-output.norito", released.original)
    try trace("native-durable-released", ["operationIDSHA256": EnrollmentProbeFiles.digest(released.operationID),
      "originalSHA256": EnrollmentProbeFiles.digest(original)])
    return original
  }
  private func sendReleased(_ call: KagemushaWalletCallV1) throws {
    try trace("native-send-return", ["nativeStatus": String(call.status)])
    try KagemushaNearbyAttemptGate.requireComplete(call.status)
    let original = try release(.send, peer: .payment)
    try gate.releasedPayment(status: call.status)
    let companion = try wallet.paymentCompanion(payment: original)
    try keep("payment", original)
    try keep("payer-companion", companion)
    try publicBytes("payer-companion.norito", companion)
    let message = try packed(.payment, original: original, companion: companion, label: "payment")
    try keep("payment-ipm1", message.encoded)
    guard let requestBytes = try retained("request-ipm1") else { throw EnrollmentProbeFailure.lifecycle }
    let request = try IrohaPeerWireMessageV1.decode(requestBytes,
      expectedProfile: .kagemushaWalletV1, expectedKind: .request)
    let requestEnvelope = try IrohaPeerKagemushaWalletAdapterV1.decode(request)
    let paymentEnvelope = try IrohaPeerKagemushaWalletAdapterV1.decode(message)
    try KagemushaWalletWireV1.requireExchangeBinding(request: requestEnvelope, payment: paymentEnvelope)
    try displayMessage(message, label: "payment", next: "Scan receiver Credited") {
      self.scan(.credited, kind: .credited, label: "credited") { message, arrival in
        try KagemushaWalletWireV1.requireExchangeBinding(request: requestEnvelope, payment: paymentEnvelope,
          credited: IrohaPeerKagemushaWalletAdapterV1.decode(message))
        let original = try self.unpacked(.credited, message, label: "credited")
        try self.trace("credited-arrival", ["creditedSHA256": EnrollmentProbeFiles.digest(original)], at: arrival)
        let delivery = try self.call.observeCredited(sendRequestId: self.selection.requestID, credited: original)
        let now = KagemushaNearbyClock.nowNanoseconds()
        let hex = { (bytes: Data) in bytes.map { String(format: "%02x", $0) }.joined() }
        try self.trace("native-verified-delivery", [
          "evidence": String(delivery.evidence.rawValue), "archive": String(delivery.archive.rawValue),
          "corePending": String(delivery.corePending), "creditID": hex(delivery.creditId),
          "paymentDigest": hex(delivery.paymentDigest), "amountLow": String(delivery.amount.low),
          "amountHigh": String(delivery.amount.high), "creditedSHA256": EnrollmentProbeFiles.digest(original),
          "archiveDispatched": "false"], at: now)
        let sample = try self.gate.credited(evidence: delivery.evidence, archive: delivery.archive,
          corePending: delivery.corePending, arrival: arrival, verified: now)
        try self.trace("payer-complete", ["latencyQualified": String(sample != nil),
          "rawArrivalElapsedNs": sample.map { String($0.0) } ?? "unknown",
          "elapsedNs": sample.map { String($0.1) } ?? "unknown",
          "rawArrivalScope": "receiver-durable-completion-upper-bound",
          "elapsedScope": "payer-confirmation-to-verified-delivery",
          "timingAnchor": sample == nil ? "unknown-original-process" : "payer-authentication-success",
          "physicalCarrierObserved": "true"], at: now)
        self.finish(.success(()))
      }
    }
  }
  private func showRequest() throws {
    let request: IrohaPeerWireMessageV1
    if let bytes = try retained("request-ipm1") {
      request = try IrohaPeerWireMessageV1.decode(bytes, expectedProfile: .kagemushaWalletV1, expectedKind: .request)
    } else {
      guard let amount = retainedInputs["amount"] else { throw EnrollmentProbeFailure.input }
      // Native owns the setup ID and returns its exact Request again after interruption.
      let issued = try call.request(setupId: selection.setupID,
        amount: KagemushaMonetaryProbeManifest.amount(amount),
        feeSchedule: retainedInputs["fee-schedule"], feeCertificate: retainedInputs["fee-certificate"])
      guard issued.status == 12 else { throw EnrollmentProbeFailure.lifecycle }
      request = try packed(.request, original: issued.bytes,
        account: wallet.metadata().accountOriginal, label: "request")
      try keep("request-ipm1", request.encoded)
    }
    guard try IrohaPeerKagemushaWalletAdapterV1.destinationAccountOriginal(request) == wallet.metadata().accountOriginal
    else { throw EnrollmentProbeFailure.input }
    let requestEnvelope = try IrohaPeerKagemushaWalletAdapterV1.decode(request)
    let requestOriginal = try wallet.original(.request, envelope: requestEnvelope)
    try publicBytes("request-original.norito", requestOriginal)
    try publicBytes("request-envelope.norito", requestEnvelope)
    try publicBytes("request-account-original.norito", IrohaPeerKagemushaWalletAdapterV1.destinationAccountOriginal(request))
    try publicBytes("request-ipm1.bin", request.encoded)
    try displayMessage(request, label: "request", next: "Scan payer Payment") {
      self.scan(.payment, kind: .payment, label: "payment") { message, _ in
        let paymentEnvelope = try IrohaPeerKagemushaWalletAdapterV1.decode(message)
        try KagemushaWalletWireV1.requireExchangeBinding(request: requestEnvelope, payment: paymentEnvelope)
        let payment = try self.unpacked(.payment, message, label: "payment")
        let companion = try IrohaPeerKagemushaWalletAdapterV1.payerCompanion(message)
        try self.keep("payment-ipm1", message.encoded)
        try self.keep("payment", payment)
        try self.keep("payer-companion", companion)
        try self.publicBytes("payer-companion.norito", companion)
        try self.bind("receive", ["payment": payment, "payer-companion": companion])
        let call: KagemushaWalletCallV1
        if self.dispatched { call = try self.wallet.requestStatus(requestId: self.selection.requestID) }
        else {
          try self.files.publishDispatchFence(self.fencePath, bytes: Data(EnrollmentProbeFiles.digest(message.encoded).utf8))
          self.dispatched = true
          call = try self.call.receiveWithCompanion(requestId: self.selection.requestID, payment: payment, companion: companion)
        }
        try self.trace("native-receive-return", ["nativeStatus": String(call.status)])
        let credited = try KagemushaNearbyAttemptGate.receivedCredit(status: call.status,
          recovery: self.selection.recovery, requestID: self.selection.requestID,
          retainedOutput: { _ in _ = try self.release(.receive, peer: .credited) },
          projection: { id in
            let value = try self.wallet.creditProjection(receiveRequestId: id)
            return .init(isReceiver: value.archive == .receiver, isBurned: value.evidence == .burned,
              original: value.creditedOriginal)
          })
        try self.gate.received()
        let ack = try self.packed(.credited, original: credited, label: "credited")
        try KagemushaWalletWireV1.requireExchangeBinding(request: requestEnvelope, payment: paymentEnvelope,
          credited: IrohaPeerKagemushaWalletAdapterV1.decode(ack))
        try self.trace("receiver-durable-before-display", ["creditedSHA256": EnrollmentProbeFiles.digest(credited)])
        try self.gate.displayedCredited()
        try self.displayMessage(ack, label: "credited", next: "Close display (delivery unconfirmed)") {
          try self.gate.closedReceiverDisplay()
          try self.trace("receiver-display-closed", ["nativeDurableCreditObserved": "true",
            "payerReceiptClaimed": "false", "latencyQualified": "false"])
          self.finish(.success(()))
        }
      }
    }
  }
}
#endif

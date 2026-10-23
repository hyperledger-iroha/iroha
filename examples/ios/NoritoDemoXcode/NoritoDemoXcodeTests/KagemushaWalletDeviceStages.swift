// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import Darwin
import Foundation
import LocalAuthentication

@testable import IrohaSwift

#if os(iOS) && !targetEnvironment(macCatalyst)
  struct KagemushaProbePendingReview {
    let review: KagemushaWalletReviewV1
    let exchange: KagemushaWalletExchangeV1
    let requestID: Data
    let sequence: Int
    let fencePath: String
    let continuation: Bool
    var approval = KagemushaProbeReviewApproval()
  }

  extension KagemushaEnrollmentDeviceActor {
    func walletOwner() throws -> KagemushaWalletV1 {
      try walletLifecycle.requireReady()
      guard !closed, let wallet, let walletActivity else { throw EnrollmentProbeFailure.lifecycle }
      try walletActivity.requireHealthy()
      return wallet
    }
    func releaseWallet() async throws {
      guard !carrierBusy else { throw EnrollmentProbeFailure.lifecycle }
      try walletLifecycle.requireIdle()
      guard wallet != nil else { return }
      guard let binding = walletActivity else { throw EnrollmentProbeFailure.lifecycle }
      try walletLifecycle.beginRetirement()
      exchangeIngress.invalidate()
      binding.invalidate()
      if let observation = walletObservation { await observation.stop() }
      walletObservation = nil
      // Activity and close run off MainActor and independently of synchronous Native proof work.
      let retirement = await binding.retire(close: true)
      walletLifecycle.finishRetirement(closed: retirement.closed)
      if retirement.closed {
        exchangeIngress.closed()
        wallet = nil
        walletActivity = nil
        pendingReview = nil
        timeExchange = nil
      }
      // Only acknowledged close releases the exact owner. Cleanup still runs on activity failure.
      try retirement.check()
    }
    func monetaryInputs(_ command: Command, sequence: Int) throws -> [String: Data] {
      try KagemushaMonetaryProbeManifest.validate(command.action, names: Set(command.inputs.keys))
      var originals: [String: Data] = [:]
      for name in command.inputs.keys.sorted() {
        let pin = command.inputs[name]!
        guard pin.path.hasPrefix("inbox/" + config.invocation + "/") else {
          throw EnrollmentProbeFailure.input
        }
        let bytes = try config.files.pinned(
          pin, maximum: KagemushaMonetaryProbeManifest.maximum(name))
        guard !bytes.isEmpty else { throw EnrollmentProbeFailure.input }
        try config.files.publish(
          config.actorPath + "/private/" + config.invocation + "/"
            + String(format: "%04d", sequence) + "-" + name, bytes: bytes)
        originals[name] = bytes
      }
      return originals
    }
    /// Host intent retention is only an audit guard. Native remains the sole recovery authority.
    func bind(_ kind: String, id: Data, inputs: [String: Data]) throws {
      let directory = config.actorPath + "/private/intents"
      try config.files.mkdir(directory)
      try config.files.publish(
        directory + "/" + kind + "-" + EnrollmentProbeFiles.digest(id),
        bytes: KagemushaMonetaryProbeManifest.binding(kind, inputs: inputs))
    }
    func event(_ name: String, values: [String: Any], sequence: Int) throws {
      try output(name + ".plist", EnrollmentProbeFiles.encode(values), sequence: sequence)
    }
    func scalar(_ value: KagemushaWalletUInt128V1) -> [String: String] {
      ["low": String(value.low), "high": String(value.high)]
    }
    func call(_ value: KagemushaWalletCallV1, returned: UInt64, sequence: Int) throws -> String {
      try output("native-call.bin", value.bytes, sequence: sequence)
      try event(
        "native-call",
        values: [
          "status": value.status, "sequenceLow": String(value.sequenceLow),
          "sequenceHigh": String(value.sequenceHigh), "detail": value.detail,
          "nativeReturnedTicks": returned,
          "originalSHA256": EnrollmentProbeFiles.digest(value.bytes),
        ], sequence: sequence)
      // Preserve every native status, including pending/preparing/unknown/delivery-loss.
      return "native-call-status-" + String(value.status)
    }
    func released(_ value: KagemushaWalletReleasedOutputV1, observed: UInt64, sequence: Int) throws
      -> String
    {
      try output("released-original.norito", value.original, sequence: sequence)
      try output("operation-id.bin", value.operationID, sequence: sequence)
      if let kind = value.peerKind, let original = value.peerOriginal {
        try output("peer-original.norito", original, sequence: sequence)
        try output(
          "peer-envelope.norito", walletOwner().envelope(kind, original: original),
          sequence: sequence)
        if kind == .payment {
          try output(
            "payer-companion.norito", walletOwner().paymentCompanion(payment: original),
            sequence: sequence)
        }
      }
      try event(
        "released",
        values: [
          "kind": value.kind.rawValue, "operationID": value.operationID,
          "sequence": scalar(value.sequence), "durableCompletionObservedTicks": observed,
          "originalSHA256": EnrollmentProbeFiles.digest(value.original),
          "carrierDeliveryClaimed": false,
        ], sequence: sequence)
      switch value.kind {
      case .load: return "native-complete-load"
      case .send: return "native-complete-send"
      case .receive: return "native-complete-receive"
      default: return "native-released-kind-" + String(value.kind.rawValue)
      }
    }
    func completion(
      _ value: KagemushaWalletCallV1, returned: UInt64, requestID: Data, sequence: Int
    ) throws -> String {
      let status = try call(value, returned: returned, sequence: sequence)
      guard value.status == 1 else { return status }
      let original = try walletOwner().releasedOutput(requestId: requestID)
      // `returned` marks this call returning a native durable completion; recovered observations
      // are explicitly labelled as observations, not the historical commit's unknown time.
      try event(
        "durable-call-return",
        values: [
          "nativeReturnedTicks": returned, "requestID": requestID,
          "operationID": original.operationID,
        ], sequence: sequence)
      return try released(original, observed: mach_continuous_time(), sequence: sequence)
    }
    // `kind` is "activation" or "unload"; Awaiting carries only Native's route selector.
    func ledgerOutcomeState(
      _ value: KagemushaWalletLedgerOutcomeV1, kind: String, event name: String, returned: UInt64,
      sequence: Int
    ) throws -> String {
      var row: [String: Any] = ["nativeReturnedTicks": returned]
      let state: String
      switch value {
      case .awaiting(let entrypoint):
        row["entrypoint"] = entrypoint
        state = "unconfirmed"
      case .rejected(let commit):
        row["height"] = String(commit.height)
        row["commitHash"] = commit.commitHash
        state = "rejected"
      case .confirmed(let commit):
        row["height"] = String(commit.height)
        row["commitHash"] = commit.commitHash
        state = "confirmed"
      }
      row["confirmed"] = state == "confirmed"
      row["rejected"] = state == "rejected"
      try event(name, values: row, sequence: sequence)
      return "native-\(kind)-\(state)"
    }
    func epochRequired(_ target: UInt64, kind: String, returned: UInt64, sequence: Int) throws
      -> String
    {
      try event(
        "\(kind)-epoch-required",
        values: ["nativeReturnedTicks": returned, "targetHeight": String(target)],
        sequence: sequence)
      return "native-\(kind)-epoch-required"
    }
    func ledgerState(_ value: KagemushaWalletLedgerProgressV1?, returned: UInt64, sequence: Int)
      throws
    {
      var row: [String: Any] = ["nativeReturnedTicks": returned, "present": value != nil]
      if let value {
        row["height"] = String(value.height)
        row["blockHash"] = value.blockHash
      }
      try event("ledger-progress", values: row, sequence: sequence)
    }
    func recordSnapshot(sequence: Int) throws -> String {
      let snapshot = try walletOwner().snapshot()
      let observed = mach_continuous_time()
      var row: [String: Any] = [
        "observedTicks": observed, "head": snapshot.head, "sequence": scalar(snapshot.sequence),
        "credentialDigest": snapshot.credentialDigest,
        "ownedBalance": scalar(snapshot.ownedBalance),
        "knownBurnedTotal": scalar(snapshot.knownBurnedTotal),
        "backlog": scalar(snapshot.foldBacklog), "headIsFolded": snapshot.headIsFolded,
        "lifecycle": snapshot.lifecycle.rawValue, "operationPermissionClaimed": false,
      ]
      if let amount = snapshot.foldedBalance {
        row["foldedBalance"] = scalar(amount)
        row["currentHeadFoldReadyObservedTicks"] = observed
      }
      try event("snapshot", values: row, sequence: sequence)
      return snapshot.headIsFolded
        ? "native-current-head-fold-ready-observed" : "native-fold-backlog-observed"
    }
    /// The account signer supplies an exact signature only after seeing this Native challenge.
    /// No private account key enters this process, its configuration or XCTest attachments.
    func awaitOpenSignature(_ challenge: Data, sequence: Int) async throws -> Data {
      let challenge = try KagemushaMonetaryProbeManifest.word(challenge)
      try output("open-account-challenge.bin", challenge, sequence: sequence)
      try event(
        "open-challenge",
        values: [
          "nativeChallengeObservedTicks": mach_continuous_time(),
          "challengeSHA256": EnrollmentProbeFiles.digest(challenge),
        ], sequence: sequence)
      let base =
        "inbox/" + config.invocation + "/" + String(format: "%04d", sequence) + "-open-signature"
      var timebase = mach_timebase_info_data_t()
      guard mach_timebase_info(&timebase) == KERN_SUCCESS, timebase.denom > 0 else {
        throw EnrollmentProbeFailure.lifecycle
      }
      let clock = EnrollmentProbeClock(
        numerator: UInt64(timebase.numer), denominator: UInt64(timebase.denom))
      let started = mach_continuous_time()
      let deadline = EnrollmentProbeDeadline(
        clock: clock, started: started, duration: 120_000_000_000, now: mach_continuous_time)
      var ready: String? = nil
      while ready == nil {
        ready = try deadline.select { try config.files.readyDigest(base + ".ready") }
        if ready == nil { try await Task.sleep(nanoseconds: 200_000_000) }
      }
      let bytes = try config.files.read(base + ".plist", maximum: 4096)
      guard
        ready == EnrollmentProbeFiles.digest(bytes),
        let row = try PropertyListSerialization.propertyList(from: bytes, format: nil)
          as? [String: Any],
        Set(row.keys) == ["challengeSHA256", "signature"],
        row["challengeSHA256"] as? String == EnrollmentProbeFiles.digest(challenge)
      else { throw EnrollmentProbeFailure.input }
      let pin = try EnrollmentProbePin(row["signature"]!)
      guard pin.path.hasPrefix("inbox/" + config.invocation + "/") else {
        throw EnrollmentProbeFailure.input
      }
      let signature = try config.files.pinned(pin, maximum: 64)
      guard signature.count == 64 else { throw EnrollmentProbeFailure.input }
      let retained =
        config.actorPath + "/private/" + config.invocation + "/" + String(format: "%04d", sequence)
      try config.files.publish(retained + "-open-signature.plist", bytes: bytes)
      try config.files.publish(retained + "-open-signature.bin", bytes: signature)
      try deadline.check()
      return signature
    }

    func executeMonetary(_ command: Command, sequence: Int) async throws -> String {
      let inputs = try monetaryInputs(command, sequence: sequence)
      if command.action.hasPrefix("qr-") {
        return try await executeQR(command, inputs: inputs, sequence: sequence)
      }
      if command.action.hasPrefix("nearby-") {
        return try await executeNearby(command, inputs: inputs, sequence: sequence)
      }
      func input(_ name: String) throws -> Data {
        guard let value = inputs[name] else { throw EnrollmentProbeFailure.input }
        return value
      }
      func id(_ name: String = "request-id") throws -> Data {
        try KagemushaMonetaryProbeManifest.word(input(name))
      }
      if command.action == "release-wallet" {
        guard wallet != nil else { throw EnrollmentProbeFailure.lifecycle }
        try await releaseWallet()
        return "wallet-released-custody-retained"
      }
      if command.action == "open" {
        guard wallet == nil, pendingReview == nil else { throw EnrollmentProbeFailure.lifecycle }
        try walletLifecycle.beginOpen()
        do {
          let result = try input("result")
          try bind("open", id: config.account, inputs: ["result": result, "account": config.account])
          if let enrollment {
            guard try enrollment.retainedResult() == result else {
              throw EnrollmentProbeFailure.collision
            }
            installed = try enrollment.loadInstalledRuntime()
            self.enrollment = nil
          }
          guard let installed else { throw EnrollmentProbeFailure.lifecycle }
          let admitted = try await installed.openEnrolledResult(
            resultOriginal: result, accountOriginal: config.account,
            signExistingAccount: { challenge in
              try await self.awaitOpenSignature(challenge, sequence: sequence)
            },
            requireCurrent: { try Task.checkCancellation() })
          let opened = mach_continuous_time()
          wallet = admitted
          self.installed = nil
          let binding = KagemushaProbeActivityBinding(
            apply: { try admitted.setActivity(foreground: $0.foreground, charging: $0.charging) },
            closeOwner: { try admitted.close() })
          walletActivity = binding
          walletObservation = await KagemushaProbeActivityObservation.phone(binding: binding)
          try await binding.acknowledgeInitial()
          let metadata = try admitted.metadata()
          guard metadata.accountOriginal == config.account else {
            throw EnrollmentProbeFailure.collision
          }
          try event(
            "opened",
            values: [
              "nativeReturnedTicks": opened, "schemeID": metadata.schemeID,
              "walletID": metadata.walletID, "accountDigest": metadata.accountDigest,
              "assetDigest": metadata.assetDigest, "assetScale": metadata.assetScale,
            ], sequence: sequence)
          try output("native-account.norito", metadata.accountOriginal, sequence: sequence)
          try output("native-asset.norito", metadata.assetOriginal, sequence: sequence)
          try binding.requireHealthy()
          try exchangeIngress.publish(admitted)
          walletLifecycle.finishOpen(ownerRetained: true, ready: true)
          return "native-wallet-opened"
        } catch {
          if let binding = walletActivity {
            binding.invalidate()
            if let observation = walletObservation { await observation.stop() }
            walletObservation = nil
            // Setup failure keeps the admitted owner for explicit release/stop, never a replacement.
            _ = await binding.retire(close: false)
          }
          walletLifecycle.finishOpen(ownerRetained: wallet != nil, ready: false)
          throw error
        }
      }
      let wallet = try walletOwner()
      switch command.action {
      case "begin-exchange": return "native-exchange-requested"
      case "end-exchange":
        let ended = try await exchangeIngress.end()
        return "native-exchange-end-" + String(ended.rawValue)
      case "discard-review":
        guard let pending = pendingReview else { throw EnrollmentProbeFailure.lifecycle }
        try pending.exchange.discardReview(pending.review)
        pendingReview = nil
        return "native-review-cancelled"
      case "bootstrap":
        let result = try mutationOwner().bootstrap()
        let returned = mach_continuous_time()
        return try call(result, returned: returned, sequence: sequence)
      case "activation":
        let original = try mutationOwner().activation()
        let returned = mach_continuous_time()
        try output("activation.norito", original, sequence: sequence)
        try output(
          "ledger-instruction.norito",
          wallet.ledgerInstruction(kind: .activate, original: original), sequence: sequence)
        try event(
          "activation-export", values: ["nativeReturnedTicks": returned, "confirmed": false],
          sequence: sequence)
        return "native-activation-exported-not-submitted"
      case "retain-activation", "activation-status", "activation-finality":
        let wire = try input("signed-wire")
        if command.action == "activation-finality" {
          let ingest = try mutationOwner().ingestActivationFinality(
            signedWire: wire, finality: input("finality"))
          let returned = mach_continuous_time()
          switch ingest {
          case .outcome(let value):
            return try ledgerOutcomeState(
              value, kind: "activation", event: "activation-state", returned: returned,
              sequence: sequence)
          case .epochRequired(let target):
            return try epochRequired(
              target, kind: "activation", returned: returned, sequence: sequence)
          }
        }
        let value: KagemushaWalletLedgerOutcomeV1
        if command.action == "retain-activation" {
          value = try mutationOwner().retainActivationAttempt(signedWire: wire)
        } else {
          value = try wallet.activationAttemptOutcome(signedWire: wire)
        }
        let returned = mach_continuous_time()
        if command.action == "retain-activation",
          let receipt = try KagemushaActivationRetentionProbe.receipt(
            signedWire: wire, outcome: value, returned: returned,
            invocation: config.invocation, sequence: sequence)
        {
          // Only the actual Native return may produce this original, before any controller POST.
          try output("activation-retained.json", receipt, sequence: sequence)
        }
        return try ledgerOutcomeState(
          value, kind: "activation", event: "activation-state", returned: returned,
          sequence: sequence)
      case "prepare-load":
        let identity = try id()
        let amount = try KagemushaMonetaryProbeManifest.amount(input("amount"))
        try bind("load-plan", id: identity, inputs: inputs)
        let original = try mutationOwner().prepareLedgerLoad(requestId: identity, amount: amount)
        let returned = mach_continuous_time()
        try output("ledger-load-instruction.norito", original, sequence: sequence)
        try event(
          "prepared-load",
          values: [
            "nativeReturnedTicks": returned, "requestID": identity, "settlementClaimed": false,
          ], sequence: sequence)
        return "native-load-plan-retained"
      case "prepared-load":
        let plan = try wallet.preparedLedgerLoad(requestId: id())
        let returned = mach_continuous_time()
        if let plan {
          try output("ledger-load-instruction.norito", plan.instructionOriginal, sequence: sequence)
          try event(
            "prepared-load",
            values: [
              "nativeReturnedTicks": returned, "requestID": plan.requestID,
              "ordinal": scalar(plan.ordinal), "amount": scalar(plan.amount),
            ], sequence: sequence)
        }
        return plan == nil ? "native-load-plan-absent" : "native-load-plan-retained"
      case "load":
        let identity = try id()
        try bind(
          "operation", id: identity,
          inputs: [
            "family": Data("load".utf8), "receipt": input("receipt"), "finality": input("finality"),
          ])
        let result = try mutationOwner().load(
          requestId: identity, receipt: input("receipt"), finality: input("finality"))
        let returned = mach_continuous_time()
        return try completion(result, returned: returned, requestID: identity, sequence: sequence)
      case "ledger-finality", "ledger-status":
        let value =
          try command.action == "ledger-finality"
          ? mutationOwner().ingestLedgerFinality(input("finality")) : wallet.ledgerProgress()
        try ledgerState(value, returned: mach_continuous_time(), sequence: sequence)
        return "native-ledger-prefix-observed"
      case "epoch-boundary", "epoch-status":
        let value =
          try command.action == "epoch-boundary"
          ? mutationOwner().ingestEpochBoundary(
            expectedEpoch: KagemushaMonetaryProbeManifest.limb(input("epoch")),
            original: input("boundary")) : wallet.epochProgress()
        try event(
          "epoch",
          values: [
            "nativeReturnedTicks": mach_continuous_time(), "epoch": String(value.epoch),
            "firstHeight": String(value.firstHeight),
            "boundaryHeight": String(value.boundaryHeight),
          ], sequence: sequence)
        return "native-epoch-prefix-observed"
      case "request":
        let identity = try id("setup-id")
        try bind(
          "setup", id: identity,
          inputs: inputs.merging(["family": Data(command.action.utf8)]) { first, _ in first })
        let result = try mutationOwner().request(
          setupId: identity, amount: KagemushaMonetaryProbeManifest.amount(input("amount")),
          feeSchedule: inputs["fee-schedule"], feeCertificate: inputs["fee-certificate"])
        let returned = mach_continuous_time()
        _ = try call(result, returned: returned, sequence: sequence)
        try output("peer-original.norito", result.bytes, sequence: sequence)
        try output(
          "peer-envelope.norito", wallet.envelope(.request, original: result.bytes), sequence: sequence)
        return "native-" + command.action + "-exported-carrier-unobserved"
      case "review-send", "review-unload", "continue-review-send", "continue-review-unload":
        guard pendingReview == nil else { throw EnrollmentProbeFailure.lifecycle }
        let identity = try id(), continuation = command.action.hasPrefix("continue-")
        let family = continuation ? String(command.action.dropFirst(9)) : command.action
        let operationPath = config.actorPath + "/private/intents/operation-" + EnrollmentProbeFiles.digest(identity)
        if continuation { guard try config.files.exists(operationPath) else { throw EnrollmentProbeFailure.lifecycle } }
        try bind("operation", id: identity,
          inputs: inputs.merging(["family": Data(family.utf8)]) { first, _ in first })
        let expectation = try KagemushaProbeReviewCustody.expectation(files: config.files,
          actorPath: config.actorPath, requestID: identity, family: family,
          originals: inputs, continuation: continuation)
        let accepted = try mutationOwner()
        let decision = try family == "review-send"
          ? accepted.reviewSend(requestId: identity, request: input("request"),
              destinationAccountOriginal: input("destination"), expectation: expectation)
          : accepted.reviewUnload(requestId: identity, expectation: expectation,
              amount: KagemushaMonetaryProbeManifest.amount(input("amount")), quote: inputs["quote"],
              certificates: inputs["certificates"], chargeBeneficiary: inputs["beneficiary"])
        let review: KagemushaWalletReviewV1
        switch decision {
        case .ready(let value): review = value
        case .existing(let value): return try existingReviewed(value, sequence: sequence)
        case .waiting(let wait):
          try event("review-waiting", values: ["reason": wait.rawValue, "capabilityIssued": false], sequence: sequence)
          return "native-review-waiting-" + String(wait.rawValue)
        }
        let returned = mach_continuous_time()
        let fence = continuation
          ? config.actorPath + "/private/" + config.invocation + "/" + String(format: "%04d", sequence) + "-review-dispatch.norito"
          : config.actorPath + "/private/intents/executed-" + EnrollmentProbeFiles.digest(identity)
        pendingReview = .init(review: review, exchange: accepted.exchange, requestID: identity, sequence: sequence,
          fencePath: fence, continuation: continuation)
        let p = review.projection
        try output("native-review.bin", p.bytes, sequence: sequence)
        try event(
          "native-review",
          values: [
            "nativeReturnedTicks": returned, "reviewSHA256": EnrollmentProbeFiles.digest(p.bytes),
            "conditionalContinuation": continuation, "latencyQualified": false,
            "originalFence": "executed-" + EnrollmentProbeFiles.digest(identity),
            "kind": p.kind == .send ? "send" : "unload", "requestID": identity,
            "amount": scalar(p.amount), "fee": scalar(p.fee), "grossDebit": scalar(p.grossDebit),
            "netDestinationAmount": scalar(p.netDestinationAmount), "head": p.currentHead,
            "sourceStateCommitment": p.sourceStateCommitment,
            "destinationAccountDigest": p.destinationAccountDigest,
            "onwardSendReviewReadyObserved": p.kind == .send,
          ], sequence: sequence)
        return "native-review-awaiting-explicit-decision"
      case "decide-review":
        return try await decideReview(input("decision"), sequence: sequence)
      case "retry-reviewed":
        guard let pending = pendingReview else { throw EnrollmentProbeFailure.lifecycle }
        return try executePendingReview(pending, retry: true, sequence: sequence)
      case "receive":
        let identity = try id()
        try bind(
          "operation", id: identity,
          inputs: inputs.merging(["family": Data("receive".utf8)]) { first, _ in first })
        let result = try mutationOwner().receiveWithCompanion(
          requestId: identity, payment: input("payment"), companion: input("payer-companion"))
        let returned = mach_continuous_time()
        return try completion(result, returned: returned, requestID: identity, sequence: sequence)
      case "credit":
        let value = try wallet.creditProjection(receiveRequestId: id())
        let returned = mach_continuous_time()
        try event(
          "credit",
          values: [
            "nativeReturnedTicks": returned, "evidence": value.evidence.rawValue,
            "archive": value.archive.rawValue, "corePending": value.corePending,
            "creditID": value.creditId, "paymentDigest": value.paymentDigest,
            "amount": scalar(value.amount),
          ], sequence: sequence)
        if let original = value.creditedOriginal {
          try output("peer-original.norito", original, sequence: sequence)
          try output(
            "peer-envelope.norito", wallet.envelope(.credited, original: original),
            sequence: sequence)
        }
        return "native-credit-evidence-" + String(value.evidence.rawValue)
      case "observe-credited":
        let identity = try id()
        let original = try input("credited")
        // Keep the diagnostic original before Native durably retains and verifies it.
        try output("verified-credited-original.norito", original, sequence: sequence)
        let value = try mutationOwner().observeCredited(sendRequestId: identity, credited: original)
        let returned = mach_continuous_time()
        try event(
          "observed-credited",
          values: [
            "nativeReturnedTicks": returned, "requestID": identity,
            "creditedSHA256": EnrollmentProbeFiles.digest(original),
            "evidence": value.evidence.rawValue, "archive": value.archive.rawValue,
            "corePending": value.corePending, "creditID": value.creditId,
            "paymentDigest": value.paymentDigest, "amount": scalar(value.amount),
            "archiveDispatched": false, "carrierDeliveryClaimed": false,
          ], sequence: sequence)
        return "native-payer-delivery-evidence-" + String(value.evidence.rawValue)
      case "credited-status":
        let value = try wallet.creditedForSendStatus(sendRequestId: id(), original: input("credited"))
        return try call(value, returned: mach_continuous_time(), sequence: sequence)
      case "resume-deferred-receipt":
        guard let value = try wallet.resumeDeferredReceipt() else { return "native-deferred-receipt-idle" }
        try event("deferred-receipt", values: ["evidence": value.evidence.rawValue,
          "archive": value.archive.rawValue, "corePending": value.corePending,
          "creditID": value.creditId, "paymentDigest": value.paymentDigest,
          "amount": scalar(value.amount)], sequence: sequence)
        return "native-deferred-receipt-evidence-" + String(value.evidence.rawValue)
      case "drain-deferred-archive":
        switch try wallet.drainDeferredArchive() {
        case .noWork: return "native-deferred-archive-idle"
        case .waiting(let reason): return "native-deferred-archive-wait-" + String(reason.rawValue)
        case .reconciled(let value):
          _ = try call(value, returned: mach_continuous_time(), sequence: sequence)
          return "native-deferred-archive-reconciled-status-" + String(value.status)
        case .executed(let value):
          _ = try call(value, returned: mach_continuous_time(), sequence: sequence)
          return "native-deferred-archive-executed-status-" + String(value.status)
        }
      case "fold-once":
        let value = try wallet.foldOnce()
        let returned = mach_continuous_time()
        _ = try call(value, returned: returned, sequence: sequence)
        return try recordSnapshot(sequence: sequence)
      case "snapshot": return try recordSnapshot(sequence: sequence)
      case "fold-status":
        let value = try wallet.foldStatus()
        let returned = mach_continuous_time()
        var row: [String: Any] = [
          "nativeReturnedTicks": returned, "phase": value.phase.rawValue,
          "eligible": value.eligible,
        ]
        if let backlog = value.observedBacklog { row["observedBacklog"] = scalar(backlog) }
        try event("fold-status", values: row, sequence: sequence)
        return "native-fold-worker-observed"
      case "request-status":
        let identity = try id()
        let value = try wallet.requestStatus(requestId: identity)
        let returned = mach_continuous_time()
        return try completion(value, returned: returned, requestID: identity, sequence: sequence)
      case "retry":
        let operationID = try id("operation-id")
        let value = try wallet.retry(operationId: operationID)
        let returned = mach_continuous_time()
        let status = try call(value, returned: returned, sequence: sequence)
        if value.status == 1 {
          return try released(
            wallet.releasedOutput(operationId: operationID), observed: mach_continuous_time(),
            sequence: sequence)
        }
        return status
      case "resume":
        let value = try wallet.resume()
        let returned = mach_continuous_time()
        return try call(value, returned: returned, sequence: sequence)
      case "released-request":
        return try released(
          wallet.releasedOutput(requestId: id()), observed: mach_continuous_time(),
          sequence: sequence)
      case "released-operation":
        return try released(
          wallet.releasedOutput(operationId: id("operation-id")), observed: mach_continuous_time(),
          sequence: sequence)
      case "unload-claim":
        let claim = try wallet.unloadClaimTransport(
          requestId: id(), chargeBeneficiary: inputs["beneficiary"])
        let returned = mach_continuous_time()
        try output("unload-claim.norito", claim, sequence: sequence)
        try output(
          "ledger-instruction.norito", wallet.ledgerInstruction(kind: .unload, original: claim),
          sequence: sequence)
        try event(
          "unload-claim", values: ["nativeReturnedTicks": returned, "settled": false],
          sequence: sequence)
        return "native-unload-claim-exported-not-settled"
      case "unload-status":
        let value = try wallet.unloadAttemptOutcome(
          claim: input("claim"), signedWire: input("signed-wire"))
        return try ledgerOutcomeState(
          value, kind: "unload", event: "unload-status", returned: mach_continuous_time(),
          sequence: sequence)
      case "unload-finality":
        let ingest = try mutationOwner().ingestUnloadFinality(
          claim: input("claim"), signedWire: input("signed-wire"), finality: input("finality"))
        let returned = mach_continuous_time()
        switch ingest {
        case .outcome(let value):
          return try ledgerOutcomeState(
            value, kind: "unload", event: "unload-status", returned: returned, sequence: sequence)
        case .epochRequired(let target):
          return try epochRequired(target, kind: "unload", returned: returned, sequence: sequence)
        }
      case "decode-request", "decode-payment", "decode-credited":
        let kinds: [String: KagemushaWalletTransportKindV1] = [
          "decode-request": .request, "decode-payment": .payment,
          "decode-credited": .credited,
        ]
        let original = try wallet.original(kinds[command.action]!, envelope: input("envelope"))
        let returned = mach_continuous_time()
        try output("peer-original.norito", original, sequence: sequence)
        try event(
          "peer-decoded",
          values: [
            "nativeReturnedTicks": returned, "kind": kinds[command.action]!.rawValue,
            "carrierDeliveryClaimed": false, "monetaryAdmissionClaimed": false,
          ], sequence: sequence)
        return "native-peer-envelope-decoded-not-admitted"
      case "refresh-time":
        let identity = try id()
        try bind(
          "operation", id: identity,
          inputs: inputs.merging(["family": Data("refresh-time".utf8)]) { first, _ in first })
        let result = try mutationOwner().refresh(
          requestId: identity, kind: .timeAnchor, update: input("anchor"),
          certificates: input("certificates"))
        let returned = mach_continuous_time()
        return try completion(result, returned: returned, requestID: identity, sequence: sequence)
      case "begin-time":
        guard timeExchange == nil else { throw EnrollmentProbeFailure.lifecycle }
        let exchange = try mutationOwner().beginTimeExchange()
        let returned = mach_continuous_time()
        timeExchange = (challenge: exchange, exchange: try mutationOwner().exchange)
        try output("time-nonce.bin", exchange.nonce, sequence: sequence)
        try event("time-exchange", values: ["nativeReturnedTicks": returned], sequence: sequence)
        return "native-time-exchange-awaiting-issuer"
      case "finish-time":
        guard let exchange = timeExchange else { throw EnrollmentProbeFailure.lifecycle }
        guard try mutationOwner().exchange === exchange.exchange else {
          throw EnrollmentProbeFailure.lifecycle
        }
        // Valid bounded inputs reach Native exactly once. A dispatched token cannot be reconstructed.
        let anchor = try input("anchor")
        let certificate = try input("certificate")
        timeExchange = nil
        let result = try mutationOwner().finishTimeExchange(
          exchange.challenge, anchor: anchor, certificate: certificate)
        let returned = mach_continuous_time()
        return try call(result, returned: returned, sequence: sequence)
      case "cancel-time":
        guard let exchange = timeExchange else { throw EnrollmentProbeFailure.lifecycle }
        try exchange.exchange.cancelTimeExchange(exchange.challenge)
        timeExchange = nil
        return "native-time-exchange-cancelled"
      default: throw EnrollmentProbeFailure.input
      }
    }

    func decideReview(_ original: Data, sequence: Int) async throws -> String {
      guard var pending = pendingReview else { throw EnrollmentProbeFailure.lifecycle }
      let wallet = try walletOwner()
      let projection = pending.review.projection
      let decision = try KagemushaMonetaryProbeManifest.Decision(
        original, invocation: config.invocation, sequence: pending.sequence,
        projection: projection.bytes)
      if !decision.confirmed {
        try pending.exchange.discardReview(pending.review)
        pendingReview = nil
        try event(
          "review-cancelled",
          values: [
            "nativeReturnedTicks": mach_continuous_time(), "reviewSequence": pending.sequence,
          ], sequence: sequence)
        return "native-review-cancelled"
      }
      let accepted = try mutationOwner()
      guard accepted.exchange === pending.exchange else {
        throw EnrollmentProbeFailure.lifecycle
      }
      guard pending.approval.phase == .awaitingConfirmation,
        try !config.files.exists(pending.fencePath) else { throw EnrollmentProbeFailure.lifecycle }
      // The host's explicit review decision is followed by actual iOS owner authentication.
      // This is a diagnostic confirmation flow, not a claim of a trusted display or carrier tap.
      let context = LAContext()
      context.touchIDAuthenticationAllowableReuseDuration = 0
      let digest = EnrollmentProbeFiles.digest(projection.bytes)
      let reason =
        "Confirm disposable KAGEMUSHA \(projection.kind == .send ? "Send" : "Unload") review \(digest.prefix(12)). Gross atomic units: high=\(projection.grossDebit.high), low=\(projection.grossDebit.low)."
      let confirmed = try await context.evaluatePolicy(
        .deviceOwnerAuthentication, localizedReason: reason)
      guard confirmed else { throw EnrollmentProbeFailure.lifecycle }
      let confirmedTicks = mach_continuous_time()
      // Authentication suspends the actor. Keep this decision bound to the same live wallet,
      // accepted call and retained review before publishing its confirmation or dispatch fence.
      guard try walletOwner() === wallet, try mutationOwner() === accepted,
        pendingReview?.review === pending.review, accepted.exchange === pending.exchange
      else { throw EnrollmentProbeFailure.lifecycle }
      try event(
        "confirmation",
        values: [
          "deviceOwnerAuthenticatedTicks": confirmedTicks, "reviewSequence": pending.sequence,
          "reviewSHA256": digest, "requestID": pending.requestID,
          "confirmationSurface": "external-exact-review-plus-ios-device-owner-authentication",
        ], sequence: sequence)
      // Only acknowledged exclusive publication permits this live capability's first call.
      // An uncertain fence is retained, but cannot create an approved-live retry state.
      do { try config.files.publishDispatchFence(pending.fencePath, bytes: projection.bytes) }
      catch {
        pendingReview = nil
        try? pending.exchange.discardReview(pending.review)
        throw error
      }
      try pending.approval.acknowledgedFence()
      return try executePendingReview(pending, retry: false, sequence: sequence)
    }

    func existingReviewed(_ existing: KagemushaWalletBoundRequestV1, sequence: Int) throws -> String {
      switch existing {
      case .preparing:
        try event("review-existing", values: ["status": 11, "newDispatch": false, "latencyQualified": false], sequence: sequence)
        return "native-reviewed-existing-preparing"
      case .outcome(let value):
        try output("existing-native-call.bin", value.bytes, sequence: sequence)
        try event("review-existing", values: ["status": value.status, "detail": value.detail,
          "newDispatch": false, "latencyQualified": false], sequence: sequence)
        return "native-reviewed-existing-" + String(value.status)
      }
    }

    func executePendingReview(_ original: KagemushaProbePendingReview, retry: Bool, sequence: Int) throws -> String {
      _ = try walletOwner()
      let accepted = try mutationOwner()
      guard accepted.exchange === original.exchange, original.review.requestID == original.requestID,
        !retry || pendingReview?.review === original.review,
        try config.files.read(original.fencePath, maximum: 4591) == original.review.projection.bytes else {
        throw EnrollmentProbeFailure.lifecycle
      }
      var pending = original
      try pending.approval.dispatch(retry: retry)
      try event("execute-reviewed-start", values: ["ticks": mach_continuous_time(),
        "requestID": pending.requestID, "reviewSequence": pending.sequence,
        "sameLiveRetry": retry, "conditionalContinuation": pending.continuation,
        "latencyQualified": !retry && !pending.continuation], sequence: sequence)
      pendingReview = nil
      let result = try accepted.executeReviewed(pending.review)
      let returned = mach_continuous_time()
      switch result {
      case .retainedReview(let wait):
        try pending.approval.retained(); pendingReview = pending
        try event("review-retained", values: ["reason": wait.rawValue,
          "dispatchFenceRetained": true, "nativeReturnedTicks": returned,
          "approvalState": "approved-live-retained", "latencyQualified": false], sequence: sequence)
        return "native-review-retained-" + String(wait.rawValue)
      case .existing(let value): return try existingReviewed(value, sequence: sequence)
      case .executed(let value):
        return try completion(value, returned: returned, requestID: pending.requestID, sequence: sequence)
      }

    }
  }
#endif

// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import Darwin
import Foundation
import XCTest

@testable import IrohaSwift

/// Explicit XCTest-only selection; shipping SDK and Native behavior never read this value.
enum EnrollmentPublicationTrialSelection: String, CaseIterable {
  case before = "receive-before-publication"
  case after = "receive-after-publication"
  static let environmentKey = "KAGEMUSHA_ENROLLMENT_PUBLICATION_TRIAL"
  var boundary: EnrollmentProbeFiles.PublicationBoundary {
    self == .before ? .staged : .directorySynchronized
  }
  static func permits(_ value: String?, trial: Self?) throws -> Bool {
    guard let value else { return trial == nil }
    guard let selected = Self(rawValue: value) else { throw EnrollmentProbeFailure.input }
    return selected == trial
  }
}

/// Only this synchronized capture crosses from the sequential runner to the device actor.
/// Every mutable field is protected by the lock; captured Data is detached from the publisher.
final class EnrollmentPublicationFaultCapture: @unchecked Sendable {
  private final class InjectedFailure: Error, @unchecked Sendable {}
  struct Snapshot: Sendable {
    let path: String
    let boundary: EnrollmentProbeFiles.PublicationBoundary
    let original: Data
  }
  private let lock = NSLock()
  private let injected = InjectedFailure()
  private var selected: (path: String, boundary: EnrollmentProbeFiles.PublicationBoundary)?
  private var retained: Snapshot?
  func arm(path: String, boundary: EnrollmentProbeFiles.PublicationBoundary) throws {
    try EnrollmentProbeFiles.requireRelative(path)
    lock.lock()
    defer { lock.unlock() }
    guard selected == nil, retained == nil else { throw EnrollmentProbeFailure.lifecycle }
    selected = (path, boundary)
  }
  func observe(path: String, boundary: EnrollmentProbeFiles.PublicationBoundary, bytes: Data) throws {
    lock.lock()
    guard let selection = selected, selection.path == path, selection.boundary == boundary else {
      lock.unlock()
      return
    }
    retained = Snapshot(path: path, boundary: boundary, original: Data([UInt8](bytes)))
    selected = nil
    lock.unlock()
    throw injected
  }
  func matches(_ error: Error) -> Bool {
    guard let failure = error as? InjectedFailure else { return false }
    return failure === injected
  }
  func snapshot() throws -> Snapshot {
    lock.lock()
    defer { lock.unlock() }
    guard let retained else { throw EnrollmentProbeFailure.lifecycle }
    return Snapshot(path: retained.path, boundary: retained.boundary,
      original: Data([UInt8](retained.original)))
  }
}

/// Closed diagnostic codes; no supplied paths, credentials or error descriptions.
enum EnrollmentPublicationTrialFailure: String, Error {
  case phase, boundaryNotReached, unknownRequired, completion, identity, originals, admission, incomplete
}

/// Genuine native enrollment and staged monetary commands through the real Apple adapter.
/// External account signatures, issuer results, ledger finality and carriers remain independent.
final class KagemushaEnrollmentPhysicalDeviceTests: XCTestCase {
  func testDisposableNativeEnrollmentActor() async throws {
    try await selectedMailbox(trial: nil)
  }
  func testDisposableReceiveBeforePublicationDriverReconstruction() async throws {
    try await selectedMailbox(trial: .before)
  }
  func testDisposableReceiveAfterPublicationDriverReconstruction() async throws {
    try await selectedMailbox(trial: .after)
  }
  private func selectedMailbox(trial: EnrollmentPublicationTrialSelection?) async throws {
    // No support-directory lookup, file, actor or Native operation precedes this closed opt-in.
    guard try EnrollmentPublicationTrialSelection.permits(
      ProcessInfo.processInfo.environment[EnrollmentPublicationTrialSelection.environmentKey], trial: trial)
    else { throw XCTSkip("This explicit publication trial selection does not select this mailbox entry") }
    try await runMailbox(trial: trial)
  }
  #if os(iOS) && !targetEnvironment(macCatalyst)
    /// Sequential runner state. Only `fault` is shared with the actor's publication callback.
    private final class ReceivePublicationTrial {
      private enum Phase: Equatable {
        case preparation, armedReceive, faultObserved, driverReconstructed, reinstalled, reopened
        case requestStatusVerified, requestOutputVerified, operationOutputVerified, finished
      }
      private let config: KagemushaEnrollmentDeviceActor.Configuration
      private let selection: EnrollmentPublicationTrialSelection
      private let fault: EnrollmentPublicationFaultCapture
      private var phase = Phase.preparation
      private var unknown: (sequence: Int, requestID: Data)?
      private var receiveSequence = -1
      private var inputs: [String: Data] = [:]
      private var openResult: Data?
      private var original = Data()
      private var operationID = Data()
      private var operationBinding = Data()
      private var openBinding = Data()
      private var peerOriginal: Data?
      private var peerEnvelope: Data?
      private var expectedFailures = 0
      private var reconstructions = 0
      private var recoveredReads = 0
      private var publicPath: String { config.actorPath + "/public/" + config.invocation }
      private var privatePath: String { config.actorPath + "/private/" + config.invocation }
      private var requestID: Data { inputs["request-id"]! }
      private var target: String { output(receiveSequence, "released-original.norito") }
      private var operationPath: String {
        config.actorPath + "/private/intents/operation-" + EnrollmentProbeFiles.digest(requestID)
      }
      private var openPath: String {
        config.actorPath + "/private/intents/open-" + EnrollmentProbeFiles.digest(config.account)
      }
      init(configuration: KagemushaEnrollmentDeviceActor.Configuration,
        selection: EnrollmentPublicationTrialSelection, fault: EnrollmentPublicationFaultCapture) throws
      {
        guard configuration.requiredStage == "received" else { throw EnrollmentProbeFailure.input }
        config = configuration
        self.selection = selection
        self.fault = fault
      }
      private func output(_ sequence: Int, _ name: String) -> String {
        publicPath + "/" + String(format: "%04d", sequence) + "-" + name
      }
      private func input(_ command: KagemushaEnrollmentDeviceActor.Command, _ name: String) throws -> Data {
        guard let pin = command.inputs[name], pin.path.hasPrefix("inbox/" + config.invocation + "/") else {
          throw EnrollmentPublicationTrialFailure.identity
        }
        let bytes = try config.files.pinned(pin, maximum: KagemushaMonetaryProbeManifest.maximum(name))
        guard !bytes.isEmpty else { throw EnrollmentPublicationTrialFailure.originals }
        return bytes
      }
      private func record(_ sequence: Int, _ name: String) throws -> [String: Any] {
        let bytes = try config.files.read(output(sequence, name + ".plist"), maximum: 131_072)
        guard let row = try PropertyListSerialization.propertyList(from: bytes, format: nil)
          as? [String: Any] else { throw EnrollmentPublicationTrialFailure.completion }
        return row
      }
      private func publish(_ name: String, _ fields: [String: Any]) throws {
        try config.files.publish(publicPath + "/publication-trial-" + name + ".plist",
          bytes: EnrollmentProbeFiles.encode(fields))
      }
      func beforeCommand(_ command: KagemushaEnrollmentDeviceActor.Command, sequence: Int) throws {
        switch phase {
        case .preparation:
          guard command.action != "stop" else { throw EnrollmentPublicationTrialFailure.boundaryNotReached }
          guard command.action == "receive" else { return }
          try KagemushaMonetaryProbeManifest.validate(command.action, names: Set(command.inputs.keys))
          let id = try KagemushaMonetaryProbeManifest.word(input(command, "request-id"))
          guard let unknown, unknown.sequence == sequence - 1, unknown.requestID == id,
            openResult != nil else { throw EnrollmentPublicationTrialFailure.unknownRequired }
          inputs = ["request-id": id, "payment": try input(command, "payment"),
            "payer-companion": try input(command, "payer-companion")]
          receiveSequence = sequence
          guard try !config.files.exists(target) else { throw EnrollmentPublicationTrialFailure.originals }
          try fault.arm(path: target, boundary: selection.boundary)
          phase = .armedReceive
        case .driverReconstructed:
          guard command.action == "install" else { throw EnrollmentPublicationTrialFailure.phase }
        case .reinstalled:
          guard command.action == "open", try input(command, "result") == openResult else {
            throw EnrollmentPublicationTrialFailure.admission
          }
        case .reopened, .requestStatusVerified:
          let action = phase == .reopened ? "request-status" : "released-request"
          guard command.action == action, try input(command, "request-id") == requestID else {
            throw EnrollmentPublicationTrialFailure.identity
          }
        case .requestOutputVerified:
          guard command.action == "released-operation", try input(command, "operation-id") == operationID else {
            throw EnrollmentPublicationTrialFailure.identity
          }
        case .operationOutputVerified:
          guard command.action == "stop" else { throw EnrollmentPublicationTrialFailure.phase }
        default: throw EnrollmentPublicationTrialFailure.phase
        }
      }
      func afterCommand(_ command: KagemushaEnrollmentDeviceActor.Command, sequence: Int,
        outcome: String) throws
      {
        switch phase {
        case .preparation:
          unknown = nil
          if command.action == "open", outcome == "native-wallet-opened" {
            openResult = try input(command, "result")
          }
          if command.action == "request-status", outcome == "native-call-status-0" {
            guard try record(sequence, "native-call")["status"] as? Int == 0 else {
              throw EnrollmentPublicationTrialFailure.unknownRequired
            }
            unknown = (sequence, try KagemushaMonetaryProbeManifest.word(input(command, "request-id")))
          }
        case .armedReceive:
          // Pending/Preparing and all other non-cut outcomes retain their real diagnostics,
          // but cannot masquerade as the selected COMPLETE publication failure.
          throw EnrollmentPublicationTrialFailure.boundaryNotReached
        case .driverReconstructed:
          guard outcome == "native-installation-registered" else { throw EnrollmentPublicationTrialFailure.admission }
          phase = .reinstalled
        case .reinstalled:
          guard outcome == "native-wallet-opened" else { throw EnrollmentPublicationTrialFailure.admission }
          try retainedInputs()
          phase = .reopened
          try publish("readmitted", ["freshRealOpenAcknowledged": true, "sameOpenBinding": true,
            "sameCustodyNamespace": true, "qualificationClaimed": false])
        case .reopened:
          try completeIdentity(sequence)
          try recovered(sequence, outcome: outcome)
          phase = .requestStatusVerified
        case .requestStatusVerified:
          try recovered(sequence, outcome: outcome)
          phase = .requestOutputVerified
        case .requestOutputVerified:
          try recovered(sequence, outcome: outcome)
          phase = .operationOutputVerified
        case .operationOutputVerified:
          guard outcome == "closed-with-custody-retained" else { throw EnrollmentPublicationTrialFailure.admission }
          phase = .finished
        default: throw EnrollmentPublicationTrialFailure.phase
        }
      }
      private func completeIdentity(_ sequence: Int) throws {
        let call = try record(sequence, "native-call")
        let durable = try record(sequence, "durable-call-return")
        guard call["status"] as? Int == 1, durable["requestID"] as? Data == requestID,
          durable["operationID"] as? Data == operationID else {
          throw EnrollmentPublicationTrialFailure.completion
        }
      }
      func acceptFailure(_ error: Error, sequence: Int) throws -> Bool {
        // A wrapped cleanup error or any different error is never the expected cut.
        guard fault.matches(error) else { return false }
        guard phase == .armedReceive, sequence == receiveSequence, expectedFailures == 0 else {
          throw EnrollmentPublicationTrialFailure.phase
        }
        let capture = try fault.snapshot()
        guard capture.path == target, capture.boundary == selection.boundary,
          (1...10_034).contains(capture.original.count) else {
          throw EnrollmentPublicationTrialFailure.originals
        }
        original = capture.original
        let durable = try record(sequence, "durable-call-return")
        guard let id = durable["operationID"] as? Data else { throw EnrollmentPublicationTrialFailure.identity }
        operationID = try KagemushaMonetaryProbeManifest.word(id)
        try completeIdentity(sequence)
        if selection == .before {
          guard try !config.files.exists(target) else { throw EnrollmentPublicationTrialFailure.originals }
        } else {
          guard try config.files.read(target, maximum: 10_034) == original else {
            throw EnrollmentPublicationTrialFailure.originals
          }
        }
        guard let openResult else { throw EnrollmentPublicationTrialFailure.admission }
        operationBinding = KagemushaMonetaryProbeManifest.binding("operation",
          inputs: inputs.merging(["family": Data("receive".utf8)]) { first, _ in first })
        openBinding = KagemushaMonetaryProbeManifest.binding("open",
          inputs: ["result": openResult, "account": config.account])
        try retainedInputs()
        let baseline = privatePath + "/publication-trial"
        try config.files.mkdirExclusive(baseline)
        try config.files.publish(baseline + "/released-original.norito", bytes: original)
        try config.files.publish(baseline + "/durable-call-return.plist",
          bytes: config.files.read(output(sequence, "durable-call-return.plist"), maximum: 131_072))
        try config.files.publish(baseline + "/open-binding.bin", bytes: openBinding)
        expectedFailures = 1
        phase = .faultObserved
        try publish("fault", ["selection": selection.rawValue, "receiveSequence": sequence,
          "actualNativeCompleteObserved": true, "exactInjectedFailure": true,
          "originalSHA256": EnrollmentProbeFiles.digest(original), "requestID": requestID,
          "operationID": operationID, "publicOriginalPresent": selection == .after,
          "carrierDeliveryClaimed": false, "qualificationClaimed": false,
          "originalLatencyClaimed": false, "processDeathClaimed": false])
        return true
      }
      func reconstructed() throws {
        guard phase == .faultObserved, reconstructions == 0 else { throw EnrollmentPublicationTrialFailure.phase }
        reconstructions = 1
        phase = .driverReconstructed
        try publish("reconstructed", ["oldActorCloseAcknowledged": true, "freshRealActorCreated": true,
          "sameCustodyNamespace": true, "processDeathClaimed": false, "qualificationClaimed": false])
      }
      private func retainedInputs() throws {
        for (name, bytes) in inputs {
          let path = privatePath + "/" + String(format: "%04d", receiveSequence) + "-" + name
          guard try config.files.read(path, maximum: KagemushaMonetaryProbeManifest.maximum(name)) == bytes else {
            throw EnrollmentPublicationTrialFailure.originals
          }
        }
        guard try config.files.read(operationPath, maximum: 1024) == operationBinding,
          try config.files.read(openPath, maximum: 1024) == openBinding else {
          throw EnrollmentPublicationTrialFailure.identity
        }
      }
      private func recovered(_ sequence: Int, outcome: String) throws {
        guard outcome == "native-complete-receive",
          try config.files.read(output(sequence, "released-original.norito"), maximum: 10_034) == original,
          try config.files.read(output(sequence, "operation-id.bin"), maximum: 32) == operationID else {
          throw EnrollmentPublicationTrialFailure.originals
        }
        let row = try record(sequence, "released")
        guard row["kind"] as? Int == Int(KagemushaWalletReleasedOutputV1.Kind.receive.rawValue),
          row["operationID"] as? Data == operationID,
          row["originalSHA256"] as? String == EnrollmentProbeFiles.digest(original) else {
          throw EnrollmentPublicationTrialFailure.identity
        }
        let peer = try config.files.read(output(sequence, "peer-original.norito"), maximum: 10_034)
        let envelope = try config.files.read(output(sequence, "peer-envelope.norito"), maximum: 10_034)
        guard !peer.isEmpty, !envelope.isEmpty else { throw EnrollmentPublicationTrialFailure.originals }
        if let peerOriginal, let peerEnvelope {
          guard peer == peerOriginal, envelope == peerEnvelope else { throw EnrollmentPublicationTrialFailure.originals }
        } else {
          peerOriginal = peer
          peerEnvelope = envelope
        }
        try retainedInputs()
        recoveredReads += 1
        try publish("recovered-" + String(recoveredReads), ["sequence": sequence,
          "sameRequestID": true, "sameOperationID": true, "exactOriginal": true,
          "peerComparisonScope": "post-reconstruction-only", "qualificationClaimed": false])
      }
      func finish(failedCommands: Int) throws {
        guard phase == .finished, expectedFailures == 1, reconstructions == 1,
          failedCommands == 1, recoveredReads == 3, peerOriginal != nil, peerEnvelope != nil,
          try fault.snapshot().original == original else { throw EnrollmentPublicationTrialFailure.incomplete }
        try retainedInputs()
        try publish("complete", ["deterministicDriverReconstruction": "complete",
          "selection": selection.rawValue, "expectedFailures": 1, "unexpectedFailures": 0,
          "exactRecoveryReads": 3, "sameRequestID": true, "sameOperationID": true,
          "peerComparisonScope": "post-reconstruction-only", "qualificationClaimed": false,
          "carrierDeliveryClaimed": false, "originalLatencyClaimed": false, "processDeathClaimed": false])
      }
    }
  #endif
  private func runMailbox(trial selection: EnrollmentPublicationTrialSelection?) async throws {
    #if !os(iOS) || targetEnvironment(simulator) || targetEnvironment(macCatalyst)
      throw EnrollmentProbeFailure.lifecycle
    #else
      let env = ProcessInfo.processInfo.environment
      func required(_ key: String) throws -> String {
        guard let value = env[key], !value.isEmpty else { throw EnrollmentProbeFailure.input }
        return value
      }
      func uuid(_ key: String) throws -> String {
        let text = try required(key)
        guard let value = UUID(uuidString: text), value.uuidString == text else {
          throw EnrollmentProbeFailure.input
        }
        return text
      }
      let run = try uuid("KAGEMUSHA_ENROLLMENT_RUN_ID")
      let invocation = try uuid("KAGEMUSHA_ENROLLMENT_INVOCATION_ID")
      let role = try required("KAGEMUSHA_ENROLLMENT_ACTOR")
      let bundle = try XCTUnwrap(Bundle.main.bundleIdentifier)
      guard bundle.hasPrefix("org.hyperledger.iroha.kagemusha.enrollment-probe.") else {
        throw EnrollmentProbeFailure.lifecycle
      }
      // The operator pins the actual selected machine; no simulator or other model fallback.
      var hardware = utsname()
      guard uname(&hardware) == 0 else { throw EnrollmentProbeFailure.lifecycle }
      let machineCapacity = MemoryLayout.size(ofValue: hardware.machine)
      let machine = withUnsafePointer(to: &hardware.machine) {
        $0.withMemoryRebound(to: CChar.self, capacity: machineCapacity) { String(cString: $0) }
      }
      guard machine == (try required("KAGEMUSHA_ENROLLMENT_EXPECTED_MACHINE")) else {
        throw EnrollmentProbeFailure.lifecycle
      }
      let support = try KagemushaWalletAppleSystemV1.live.applicationSupportDirectory().get()
      let probeRoot = support.appendingPathComponent(
        "kagemusha-enrollment-probes", isDirectory: true)
      try EnrollmentProbeFiles.requireDirectory(probeRoot)
      let fault = selection.map { _ in EnrollmentPublicationFaultCapture() }
      let publicationBoundary: ((String, EnrollmentProbeFiles.PublicationBoundary, Data) throws -> Void)?
      if let fault {
        publicationBoundary = { path, boundary, bytes in
          try fault.observe(path: path, boundary: boundary, bytes: bytes)
        }
      } else { publicationBoundary = nil }
      let files = try EnrollmentProbeFiles(
        root: probeRoot.appendingPathComponent(run, isDirectory: true),
        publicationBoundary: publicationBoundary)
      let configBytes = try files.read(
        "inbox/" + invocation + "/configuration.plist", maximum: 8_388_608)
      let configHash = try required("KAGEMUSHA_ENROLLMENT_CONFIG_SHA256")
      guard EnrollmentProbeFiles.isDigest(configHash),
        EnrollmentProbeFiles.digest(configBytes) == configHash,
        let row = try PropertyListSerialization.propertyList(from: configBytes, format: nil)
          as? [String: Any]
      else { throw EnrollmentProbeFailure.input }
      let config = try KagemushaEnrollmentDeviceActor.Configuration(
        files: files, row: row, role: role,
        invocation: invocation, independentGenesis: required("KAGEMUSHA_ENROLLMENT_GENESIS_SHA256"),
        appIDPrefix: required("KAGEMUSHA_ENROLLMENT_APP_ID_PREFIX"), bundleID: bundle)
      let trial: ReceivePublicationTrial?
      if let selection, let fault {
        trial = try ReceivePublicationTrial(configuration: config, selection: selection, fault: fault)
      } else { trial = nil }
      try files.mkdir(config.actorPath)
      let lockFD = try files.lock(config.actorPath + "/driver.lock")
      defer { close(lockFD) }
      try files.publish(config.actorPath + "/identity.txt", bytes: config.identity)
      // Every invocation has a new output namespace. Never replay a dead process's live permit.
      let publicPath = config.actorPath + "/public/" + invocation
      try files.mkdir(config.actorPath + "/public")
      try files.mkdirExclusive(publicPath)
      try files.mkdir(config.actorPath + "/private/" + invocation)
      try files.mkdir(config.actorPath + "/custody")
      try files.publish(
        config.actorPath + "/private/" + invocation + "/configuration.plist", bytes: configBytes)
      var driver = try KagemushaEnrollmentDeviceActor(configuration: config)
      var timebase = mach_timebase_info_data_t()
      guard mach_timebase_info(&timebase) == KERN_SUCCESS else {
        throw EnrollmentProbeFailure.lifecycle
      }
      let clock = EnrollmentProbeClock(
        numerator: UInt64(timebase.numer), denominator: UInt64(timebase.denom))
      let started = mach_continuous_time()
      let timeoutSeconds = try required("KAGEMUSHA_ENROLLMENT_TIMEOUT_SECONDS")
      guard let timeout = UInt64(timeoutSeconds), (1...1800).contains(timeout) else {
        throw EnrollmentProbeFailure.input
      }
      let deadline = EnrollmentProbeDeadline(
        clock: clock, started: started, duration: timeout * 1_000_000_000,
        now: mach_continuous_time)
      let header: [String: Any] = [
        "scope": "native-enrollment-and-staged-monetary", "keychainAccessGroup": config.group,
        "run": run, "actor": role,
        "invocation": invocation, "pid": getpid(), "machine": machine,
        "configurationSHA256": configHash, "ticksNumerator": clock.numerator,
        "ticksDenominator": clock.denominator,
      ]
      try files.publish(
        publicPath + "/invocation.plist", bytes: EnrollmentProbeFiles.encode(header))
      var failedCommands = 0
      var activityFailureRecorded = false
      var achievedStages = Set<String>()
      func failureRecord(_ error: Error, command: KagemushaEnrollmentDeviceActor.Command,
        sequence: Int, before: UInt64, after: UInt64) -> [String: Any]
      {
        // Preserve failure without serializing an error description or any sensitive input.
        var failure: [String: Any] = [
          "sequence": sequence, "action": command.action,
          "status": "failed", "errorType": String(reflecting: type(of: error)), "ticks": after,
          "durationNanoseconds": clock.nanoseconds(from: before, to: after),
        ]
        if let native = error as? KagemushaWalletErrorV1,
          case .native(let status, let reason, let platformCode) = native
        {
          failure["nativeStatus"] = status
          failure["nativeReason"] = reason
          failure["platformCode"] = platformCode
        }
        failure.merge(EnrollmentProbeFailureMetadata.fields(error)) { _, safe in safe }
        if let trialFailure = error as? EnrollmentPublicationTrialFailure {
          failure["publicationTrialFailure"] = trialFailure.rawValue
        }
        return failure
      }
      func finishCampaign(sequence: Int, after: UInt64) throws {
        try files.publish(
          publicPath + "/closed.plist",
          bytes: EnrollmentProbeFiles.encode([
            "failedCommands": failedCommands, "commands": sequence + 1, "ticks": after,
            "activityFailureRecorded": activityFailureRecorded,
            "requiredStage": config.requiredStage, "achievedStages": achievedStages.sorted(),
          ]))
        if trial == nil, failedCommands != 0 { throw EnrollmentProbeFailure.commandFailures }
        guard !activityFailureRecorded else { throw EnrollmentProbeFailure.lifecycle }
        guard achievedStages.contains(config.requiredStage) else {
          throw EnrollmentProbeFailure.lifecycle
        }
        try trial?.finish(failedCommands: failedCommands)
      }
      do {
        for sequence in 0..<10_000 {
          let name = String(format: "%04d", sequence)
          let base = "inbox/" + invocation + "/" + name
          var ready: String? = nil
          while ready == nil {
            if !activityFailureRecorded {
              do { try await driver.checkActivityFailure() } catch {
                activityFailureRecorded = true
                // Stay available for explicit release/stop. Never persist error descriptions.
                var failure: [String: Any] = [
                  "status": "activity-publication-failed", "ticks": mach_continuous_time(),
                  "errorType": String(reflecting: type(of: error)),
                ]
                if let native = error as? KagemushaWalletErrorV1,
                  case .native(let status, let reason, let platformCode) = native
                {
                  failure["nativeStatus"] = status
                  failure["nativeReason"] = reason
                  failure["platformCode"] = platformCode
                }
                try files.publish(
                  publicPath + "/activity-failed.plist", bytes: EnrollmentProbeFiles.encode(failure))
              }
            }
            ready = try deadline.select { try files.readyDigest(base + ".ready") }
            if ready == nil { try await Task.sleep(nanoseconds: 200_000_000) }
          }
          let commandBytes = try files.read(base + ".plist", maximum: 131_072)
          guard ready == EnrollmentProbeFiles.digest(commandBytes),
            let row = try PropertyListSerialization.propertyList(from: commandBytes, format: nil)
              as? [String: Any]
          else { throw EnrollmentProbeFailure.input }
          let command = try KagemushaEnrollmentDeviceActor.Command(row)
          try files.publish(
            config.actorPath + "/private/" + invocation + "/" + name + ".plist", bytes: commandBytes
          )
          let before = mach_continuous_time()
          try files.publish(
            publicPath + "/" + name + "-started.plist",
            bytes: EnrollmentProbeFiles.encode([
              "sequence": sequence, "action": command.action, "ticks": before,
              "commandSHA256": EnrollmentProbeFiles.digest(commandBytes),
            ]))
          // A filesystem operation or suspension cannot authorize dispatch after expiry.
          try deadline.check()
          let outcome: String
          do {
            if let trial {
              guard !activityFailureRecorded else { throw EnrollmentProbeFailure.lifecycle }
              try trial.beforeCommand(command, sequence: sequence)
              try deadline.check()
            }
            // These short controls run before the actor enqueue, even when it is busy.
            let admission = try driver.admit(command)
            outcome = try await driver.execute(command, sequence: sequence, admission: admission)
          } catch {
            failedCommands += 1
            let primary = error
            var trialFailure: Error?
            if let trial {
              do {
                guard try trial.acceptFailure(primary, sequence: sequence) else { throw primary }
                guard !activityFailureRecorded else { throw EnrollmentProbeFailure.lifecycle }
                try await driver.checkActivityFailure()
                // The old real owner must acknowledge close before another actor can exist.
                try await driver.close()
                driver = try KagemushaEnrollmentDeviceActor(configuration: config)
                try trial.reconstructed()
              } catch { trialFailure = error }
            }
            let after = mach_continuous_time()
            try files.publish(
              publicPath + "/" + name + "-finished.plist",
              bytes: EnrollmentProbeFiles.encode(failureRecord(primary, command: command,
                sequence: sequence, before: before, after: after)))
            if let trialFailure { throw trialFailure }
            // The same owner survives ordinary refusal. Any retry is a new explicit command.
            continue
          }
          let after = mach_continuous_time()
          do { try trial?.afterCommand(command, sequence: sequence, outcome: outcome) } catch {
            failedCommands += 1
            // Keep the real outcome while refusing a successful controller completion fence.
            try files.publish(publicPath + "/" + name + "-publication-trial-native-outcome.plist",
              bytes: EnrollmentProbeFiles.encode([
                "sequence": sequence, "action": command.action, "status": outcome, "trialAccepted": false,
              ]))
            try files.publish(publicPath + "/" + name + "-finished.plist",
              bytes: EnrollmentProbeFiles.encode(failureRecord(error, command: command,
                sequence: sequence, before: before, after: after)))
            throw error
          }
          if let stage = KagemushaEnrollmentProbeStage.observed(outcome) {
            achievedStages.insert(stage)
          }
          let stoppingTrial = trial != nil && command.action == "stop"
          if stoppingTrial { try finishCampaign(sequence: sequence, after: after) }
          try files.publish(
            publicPath + "/" + name + "-finished.plist",
            bytes: EnrollmentProbeFiles.encode([
              "sequence": sequence, "action": command.action, "status": outcome, "ticks": after,
              "durationNanoseconds": clock.nanoseconds(from: before, to: after),
            ]))
          if command.action == "stop" {
            if !stoppingTrial { try finishCampaign(sequence: sequence, after: after) }
            let summary = XCTAttachment(
              string:
                "Native staged wallet mailbox closed. Review retained stage outcomes; this is not an enrollment/monetary qualification verdict. run=\(run) actor=\(role) invocation=\(invocation)"
            )
            summary.lifetime = .keepAlways
            add(summary)
            return
          }
        }
        throw EnrollmentProbeFailure.timeout
      } catch {
        // No abandon/delete on errors. Release the exact owner and leave custody resumable.
        do { try await driver.close() } catch { throw EnrollmentProbeFailure.custody }
        throw error
      }
    #endif
  }
}

/// Monotonic continuous-clock durations include system sleep; values from different processes
/// or phones are never subtracted. Enrollment deadlines remain exclusively Native decisions.
struct EnrollmentProbeClock {
  let numerator: UInt64
  let denominator: UInt64
  func nanoseconds(from start: UInt64, to end: UInt64) -> UInt64 {
    let delta = end - start
    return (delta / denominator) * numerator + (delta % denominator) * numerator / denominator
  }
}

/// Test mailbox deadlines bound intake; Native owns the lifecycle of any dispatched operation.
struct EnrollmentProbeDeadline {
  let clock: EnrollmentProbeClock
  let started: UInt64
  let duration: UInt64
  let now: () -> UInt64
  var checkCancellation: () throws -> Void = { try Task.checkCancellation() }

  func check() throws {
    try checkCancellation()
    guard clock.numerator > 0, clock.denominator > 0, duration > 0 else {
      throw EnrollmentProbeFailure.input
    }
    let observed = now()
    guard observed >= started else { throw EnrollmentProbeFailure.timeout }
    // Compare exact products so a regressed or very large test clock cannot wrap or trap.
    let elapsed = (observed - started).multipliedFullWidth(by: clock.numerator)
    let limit = duration.multipliedFullWidth(by: clock.denominator)
    guard elapsed.high < limit.high || (elapsed.high == limit.high && elapsed.low < limit.low) else {
      throw EnrollmentProbeFailure.timeout
    }
  }

  func select<T>(_ read: () throws -> T) throws -> T {
    try check()
    let selected = try read()
    try check()
    return selected
  }
}

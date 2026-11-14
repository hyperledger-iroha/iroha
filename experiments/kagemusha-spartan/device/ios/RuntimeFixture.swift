import Foundation
import LocalAuthentication
import NoritoBridge
import SpartanDeviceFixture
import SwiftUI
import UIKit
@testable import IrohaSwift

#if !DEBUG || !os(iOS) || targetEnvironment(simulator)
#error("The experimental runtime fixture requires a physical iOS Debug build.")
#endif

private struct FixtureFailure: Error, CustomStringConvertible, Sendable {
  let description: String
  init(_ description: String) { self.description = description }
}

/// The C fixture's background thread waits while Apple's asynchronous service runs.
/// The borrowed Native journal remains in scope until this result has returned.
private final class FixtureWait<Value: Sendable>: @unchecked Sendable {
  private let ready = DispatchSemaphore(value: 0)
  private let lock = NSLock()
  private var result: Result<Value, FixtureFailure>?
  func finish(_ value: Result<Value, FixtureFailure>) {
    lock.lock(); precondition(result == nil); result = value; lock.unlock()
    ready.signal()
  }
  func wait() throws -> Value {
    ready.wait()
    lock.lock(); defer { lock.unlock() }
    return try result!.get()
  }
}

/// All App Attest effect records belong to the actual Native EnrollmentOwner.
private final class AppleJournal: KagemushaWalletAppleEvidenceJournalV1, @unchecked Sendable {
  private let lock = NSLock()
  private var borrowed: UnsafeMutableRawPointer?
  init(_ borrowed: UnsafeMutableRawPointer) { self.borrowed = borrowed }
  func invalidate() { lock.lock(); borrowed = nil; lock.unlock() }
  private func call(_ operation: UInt32, stage: UInt8 = 0, input: Data = Data()) throws -> Data {
    lock.lock(); defer { lock.unlock() }
    guard let borrowed else { throw FixtureFailure("Native enrollment journal is closed") }
    var buffer = [UInt8](repeating: 0, count: 4096 + 65536 + 4096 + 12)
    var length = 0
    let status = input.withUnsafeBytes { bytes in
      buffer.withUnsafeMutableBufferPointer { output in
        spartan_fixture_apple_journal_v1(borrowed, operation, stage,
          bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count,
          output.baseAddress, output.count, &length)
      }
    }
    guard length >= 0, length <= buffer.count else { throw FixtureFailure("Invalid journal reply length") }
    let bytes = Data(buffer.prefix(length))
    guard status == 0 else { throw FixtureFailure("Native journal: \(String(decoding: bytes, as: UTF8.self))") }
    guard operation == 0 || bytes.isEmpty else { throw FixtureFailure("Unexpected journal mutation output") }
    return bytes
  }
  func originals() throws -> [Data] {
    let bytes = try call(0)
    var offset = 0, result: [Data] = []
    for bound in [4096, 65536, 4096] {
      guard offset + 4 <= bytes.count else { throw FixtureFailure("Truncated App Attest original") }
      let count = bytes[offset..<offset + 4].reduce(0) { ($0 << 8) | Int($1) }
      offset += 4
      guard count <= bound, count <= bytes.count - offset else { throw FixtureFailure("Oversized App Attest original") }
      result.append(bytes.subdata(in: offset..<offset + count)); offset += count
    }
    guard offset == bytes.count else { throw FixtureFailure("Trailing journal original bytes") }
    return result
  }
  func begin(_ stage: UInt8) throws { _ = try call(1, stage: stage) }
  func retain(_ stage: UInt8, _ original: Data) throws { _ = try call(2, stage: stage, input: original) }
  func complete() throws { _ = try call(3) }
}

private func decimal(_ value: KagemushaWalletUInt128V1) -> String {
  var high = value.high, low = value.low
  if high == 0 { return String(low) }
  var digits: [UInt8] = []
  repeat {
    let division = UInt64(10).dividingFullWidth((high: high % 10, low: low))
    high /= 10; low = division.quotient
    digits.append(UInt8(division.remainder) + 48)
  } while high != 0 || low != 0
  return String(decoding: digits.reversed(), as: UTF8.self)
}
private func hex(_ bytes: Data) -> String { bytes.map { String(format: "%02x", $0) }.joined() }

private final class RuntimeBridge: @unchecked Sendable {
  let platform: KagemushaWalletApplePlatformV1
  let update: @Sendable (String) -> Void
  let showReview: @MainActor @Sendable (KagemushaSpartanReviewProjectionV1) -> Void
  private let failureLock = NSLock()
  private var callbackFailure: String?
  init(update: @escaping @Sendable (String) -> Void,
    showReview: @escaping @MainActor @Sendable (KagemushaSpartanReviewProjectionV1) -> Void) throws {
    platform = try .init(applicationIdentifierPrefix: "YLWWUD25VZ")
    self.update = update; self.showReview = showReview
  }
  private func callbackFailed(_ error: any Error) {
    let text = String(describing: error)
    failureLock.lock(); callbackFailure = text; failureLock.unlock()
    update("FAILED: \(text)")
  }

  func collect(slot: Data, key: Data, challenge: Data, journal: AppleJournal) throws {
    guard let slot = KagemushaWalletAppleSlotV1(slot) else { throw FixtureFailure("Invalid Native slot") }
    update("Collecting production App Attest evidence for the actual Secure Enclave key")
    let result = FixtureWait<Bool>()
    Task.detached { [self] in
      do {
        _ = try await platform.collectEnrollmentEvidence(slot: slot, paymentPublicKey: key,
          challengeDigest: challenge, journal: journal)
        result.finish(.success(true))
      } catch { result.finish(.failure(FixtureFailure("App Attest: \(error)"))) }
    }
    _ = try result.wait()
  }

  func confirm(_ bytes: Data) throws {
    let review = try KagemushaSpartanReviewProjectionV1(bytes)
    guard review.operation == .send else { throw FixtureFailure("Expected a Send review") }
    let result = FixtureWait<Bool>()
    Task { @MainActor [self] in
      showReview(review)
      let authentication = LAContext()
      authentication.touchIDAuthenticationAllowableReuseDuration = 0
      authentication.localizedCancelTitle = "Cancel payment"
      var failure: NSError?
      guard authentication.canEvaluatePolicy(.deviceOwnerAuthentication, error: &failure) else {
        result.finish(.failure(FixtureFailure("Device authentication unavailable: \(String(describing: failure))")))
        return
      }
      do {
        let reason = "Confirm experimental Send of \(decimal(review.amount)) plus fee \(decimal(review.fee)) to wallet \(hex(review.receiverWalletId))."
        FileHandle.standardError.write(Data("SPARTAN_CONFIRMATION_REQUIRED\n".utf8))
        guard try await authentication.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason) else {
          throw FixtureFailure("Device confirmation was not granted")
        }
        FileHandle.standardError.write(Data("SPARTAN_CONFIRMATION_GRANTED\n".utf8))
        result.finish(.success(true))
      } catch { result.finish(.failure(FixtureFailure("Device confirmation: \(error)"))) }
      authentication.invalidate()
    }
    _ = try result.wait()
  }

  func run() throws -> String {
    guard let resources = Bundle.main.resourceURL?.appendingPathComponent("native-device", isDirectory: true),
      ["program-bundle.norito", "parameter-manifest.norito", "parameter-signature.bin"].allSatisfy({
        FileManager.default.fileExists(atPath: resources.appendingPathComponent($0).path)
      }) else { throw FixtureFailure("Complete authenticated program artifacts are absent") }
    let rootPath: String
    switch platform.custodyRootPath() {
    case .success(let path): rootPath = path
    case .failure(let reason): throw FixtureFailure("Protected custody unavailable: \(reason)")
    }
    let root = URL(fileURLWithPath: rootPath, isDirectory: true).appendingPathComponent("spartan-runtime-fixture", isDirectory: true)
    var isDirectory: ObjCBool = false
    if FileManager.default.fileExists(atPath: root.path, isDirectory: &isDirectory) {
      guard isDirectory.boolValue else { throw FixtureFailure("Custody root is not a directory") }
    } else {
      try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false,
        attributes: [.protectionKey: FileProtectionType.complete])
    }
    var callbacks = kagemushaWalletCallbacksV1(platform)
    let rootBytes = Data(root.path.utf8), artifactBytes = Data(resources.path.utf8)
    var report = [UInt8](repeating: 0, count: 65536), length = 0
    let context = Unmanaged.passUnretained(self).toOpaque()
    let status = rootBytes.withUnsafeBytes { root in artifactBytes.withUnsafeBytes { artifacts in
      report.withUnsafeMutableBufferPointer { output in
        spartan_fixture_run_apple_v1(&callbacks,
          root.bindMemory(to: UInt8.self).baseAddress, root.count,
          artifacts.bindMemory(to: UInt8.self).baseAddress, artifacts.count, context,
          { context, slot, key, challenge, borrowed in
            guard let context, let slot, let key, let challenge, let borrowed else { return -1 }
            let owner = Unmanaged<RuntimeBridge>.fromOpaque(context).takeUnretainedValue()
            let journal = AppleJournal(borrowed); defer { journal.invalidate() }
            do {
              try owner.collect(slot: Data(bytes: slot, count: 32), key: Data(bytes: key, count: 65),
                challenge: Data(bytes: challenge, count: 32), journal: journal)
              return 0
            } catch { owner.callbackFailed(error); return -1 }
          },
          { context, bytes, count in
            guard let context, let bytes, count == 291 else { return -1 }
            let owner = Unmanaged<RuntimeBridge>.fromOpaque(context).takeUnretainedValue()
            do { try owner.confirm(Data(bytes: bytes, count: count)); return 0 }
            catch { owner.callbackFailed(error); return -1 }
          }, output.baseAddress, output.count, &length)
      }
    } }
    guard length >= 0, length <= report.count else { throw FixtureFailure("Invalid Native fixture report") }
    let text = String(decoding: report.prefix(length), as: UTF8.self)
    if status != 0 {
      failureLock.lock(); let callback = callbackFailure; failureLock.unlock()
      throw FixtureFailure(callback.map { text + ": " + $0 } ?? text)
    }
    return text
  }
}

@MainActor private final class RuntimeModel: ObservableObject {
  @Published var status = "Ready to run the complete experimental wallet on this iPhone"
  @Published var review: KagemushaSpartanReviewProjectionV1?
  @Published var running = false
  @Published var waitingForBuild = false
  func waitForBuild() {
    waitingForBuild = true
    status = "Preparing the final device build. The wallet test has not started."
    UIApplication.shared.isIdleTimerDisabled = true
  }
  func start() {
    guard !running && !waitingForBuild else { return }
    running = true; status = "Opening authenticated programs and actual protected custody"
    UIApplication.shared.isIdleTimerDisabled = true
    do {
      let bridge = try RuntimeBridge(update: { [weak self] text in
        Task { @MainActor in self?.status = text }
      }, showReview: { [weak self] review in self?.review = review })
      DispatchQueue.global(qos: .userInitiated).async { [weak self] in
        let result: String
        do { result = "PASS: \(try bridge.run())" } catch { result = "FAILED: \(error)" }
        Task { @MainActor in
          UIApplication.shared.isIdleTimerDisabled = false
          self?.status = result; self?.running = false
          let record: [String: String] = ["schema": "kagemusha-spartan-device-runtime-v1",
            "result": result, "finished": ISO8601DateFormatter().string(from: Date())]
          if let data = try? JSONSerialization.data(withJSONObject: record, options: [.prettyPrinted, .sortedKeys]) {
            try? data.write(to: URL.documentsDirectory.appendingPathComponent("spartan-runtime-result.json"), options: [.atomic, .completeFileProtection])
          }
        }
      }
    } catch {
      UIApplication.shared.isIdleTimerDisabled = false
      status = "FAILED: \(error)"; running = false
    }
  }
}

@main struct SpartanRuntimeFixture: App {
  @StateObject private var model = RuntimeModel()
  var body: some Scene {
    WindowGroup {
      ScrollView {
        VStack(alignment: .leading, spacing: 16) {
          Text("Experimental wallet runtime").font(.title2)
          Text(model.status).textSelection(.enabled)
          if let review = model.review {
            Text("Send \(decimal(review.amount)) · Fee \(decimal(review.fee))").font(.headline)
            Text("Receiver\n\(hex(review.receiverWalletId))")
            Text("Operation\n\(hex(review.operationId))")
            Text("Bound capsule\n\(hex(review.capsuleDigest))")
          }
          if !model.waitingForBuild {
            Button(model.running ? "Running" : "Run / resume retained operation") { model.start() }
              .disabled(model.running)
          }
        }.font(.system(.body, design: .monospaced)).padding()
      }
      .task {
        let arguments = ProcessInfo.processInfo.arguments
        if arguments.contains("-spartan-wait-for-build") { model.waitForBuild() }
        else if arguments.contains("-spartan-run") { model.start() }
      }
    }
  }
}

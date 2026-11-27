import CryptoKit
import Foundation
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
private func scaled(_ value: KagemushaWalletUInt128V1, _ scale: Int) -> String {
  let digits = decimal(value)
  guard scale > 0 else { return digits }
  let padded = String(repeating: "0", count: max(0, scale + 1 - digits.count)) + digits
  return String(padded.dropLast(scale)) + "." + String(padded.suffix(scale))
}
private func hex(_ bytes: Data) -> String { bytes.map { String(format: "%02x", $0) }.joined() }

private final class RuntimeBridge: @unchecked Sendable {
  let platform: KagemushaWalletApplePlatformV1
  let manifestDigest: String
  private let manifest: Data
  private let resources: URL
  let update: @Sendable (String) -> Void
  let showReview: @MainActor @Sendable (KagemushaSpartanReviewProjectionV1, Data, Int, FixtureWait<Bool>) -> Void
  private let failureLock = NSLock()
  private var callbackFailure: String?
  init(update: @escaping @Sendable (String) -> Void,
    showReview: @escaping @MainActor @Sendable (KagemushaSpartanReviewProjectionV1, Data, Int, FixtureWait<Bool>) -> Void) throws {
    guard let resources = Bundle.main.resourceURL?.appendingPathComponent("native-device", isDirectory: true),
      ["program-bundle.norito", "parameter-manifest.norito", "parameter-signature.bin"].allSatisfy({
        FileManager.default.fileExists(atPath: resources.appendingPathComponent($0).path)
      }) else { throw FixtureFailure("Complete authenticated program artifacts are absent") }
    self.resources = resources
    manifest = try Data(contentsOf: resources.appendingPathComponent("parameter-manifest.norito"))
    let digest = SHA256.hash(data: manifest)
    manifestDigest = kagemushaWalletAppleHex(digest)
    let group = try KagemushaWalletApplePlatformV1.keychainAccessGroup(
      applicationIdentifierPrefix: "YLWWUD25VZ", bundleIdentifier: Bundle.main.bundleIdentifier)
    platform = .init(appAttest: KagemushaWalletAppleAppAttestServiceV1(),
      accessGroup: group, system: .live, keychainNamespace: digest)
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

  func presentDebitReview(_ original: Data) throws {
    guard original.count == 324, original[32] <= 28 else { throw FixtureFailure("Invalid Send review and asset") }
    let asset = Data(original.prefix(32))
    let review = try KagemushaSpartanReviewProjectionV1(Data(original.dropFirst(33)))
    guard review.operation == .send else { throw FixtureFailure("Expected a Send review") }
    let shown = FixtureWait<Bool>()
    Task { @MainActor [self] in
      showReview(review, asset, Int(original[32]), shown)
    }
    guard try shown.wait() else { throw FixtureFailure("Send review canceled") }
  }

  func run() throws -> String {
    let rootPath: String
    switch platform.custodyRootPath() {
    case .success(let path): rootPath = path
    case .failure(let reason): throw FixtureFailure("Protected custody unavailable: \(reason)")
    }
    let root = fixtureCustodyDirectory(
      root: URL(fileURLWithPath: rootPath, isDirectory: true), manifest: manifest)
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
            guard let context, let bytes, count == 324 else { return -1 }
            let owner = Unmanaged<RuntimeBridge>.fromOpaque(context).takeUnretainedValue()
            do {
              try owner.presentDebitReview(Data(bytes: bytes, count: count))
              return 0
            }
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
  @Published var asset: Data?
  @Published var scale = 0
  @Published private(set) var awaitingReview = false
  private var decision: FixtureWait<Bool>?
  func decide(_ approved: Bool) {
    guard let decision else { return }
    self.decision = nil; awaitingReview = false
    decision.finish(.success(approved))
  }
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
      }, showReview: { [weak self] review, asset, scale, decision in
        guard let self else { decision.finish(.success(false)); return }
        self.decide(false)
        self.review = review; self.asset = asset; self.scale = scale
        self.decision = decision; self.awaitingReview = true
      })
      DispatchQueue.global(qos: .userInitiated).async { [weak self] in
        let result: String
        do { result = "PASS: \(try bridge.run())" } catch { result = "FAILED: \(error)" }
        Task { @MainActor in
          UIApplication.shared.isIdleTimerDisabled = false
          self?.status = result; self?.running = false
          let record: [String: String] = ["schema": "kagemusha-spartan-device-runtime-v1",
            "manifest_sha256": bridge.manifestDigest, "result": result,
            "finished": ISO8601DateFormatter().string(from: Date())]
          do {
            let data = try JSONSerialization.data(withJSONObject: record, options: [.prettyPrinted, .sortedKeys])
            let name = "spartan-runtime-result-" + bridge.manifestDigest + ".json"
            try data.write(to: URL.documentsDirectory.appendingPathComponent(name), options: [.atomic, .completeFileProtection])
          } catch {
            self?.status = "FAILED: Could not retain fixture report: \(error)"
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
            Text("Send \(scaled(review.amount, model.scale)) · Fee \(scaled(review.fee, model.scale))").font(.headline)
            if let asset = model.asset { Text("Asset\n\(hex(asset))") }
            Text("Receiver account digest\n\(hex(review.receiverAccountId))")
            Text("Receiver wallet\n\(hex(review.receiverWalletId))")
            Text("Operation\n\(hex(review.operationId))")
            Text("Bound capsule\n\(hex(review.capsuleDigest))")
            if model.awaitingReview {
              Button("Authenticate this Send") { model.decide(true) }
              Button("Cancel", role: .cancel) { model.decide(false) }
            }
          }
          if !model.waitingForBuild {
            Button(model.running ? "Running" : "Run / resume retained operation") { model.start() }
              .disabled(model.running)
          }
        }.font(.system(.body, design: .monospaced)).padding()
      }
      .onReceive(NotificationCenter.default.publisher(for: UIApplication.didEnterBackgroundNotification)) { _ in model.decide(false) }
      .task {
        let arguments = ProcessInfo.processInfo.arguments
        if arguments.contains("-spartan-wait-for-build") { model.waitForBuild() }
        else if arguments.contains("-spartan-run") { model.start() }
      }
    }
  }
}

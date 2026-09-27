import CryptoKit
import DeviceCheck
import Foundation
import SwiftUI

@main
struct AppAttestProbeApp: App {
  var body: some Scene {
    WindowGroup { AppAttestProbeView() }
  }
}

private struct AppAttestProbeView: View {
  @StateObject private var probe = AppAttestProbe()

  var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      Text("App Attest physical probe").font(.title2.bold())
      Text("A diagnostic key and two assertions. No payment or enrollment is performed.")
        .font(.subheadline)
      Button(probe.running ? "Running…" : "Run probe") { probe.run() }
        .disabled(probe.running)
        .buttonStyle(.borderedProminent)
      ScrollView {
        Text(probe.lines.joined(separator: "\n"))
          .font(.system(.caption, design: .monospaced))
          .frame(maxWidth: .infinity, alignment: .leading)
          .textSelection(.enabled)
      }
    }
    .padding()
    .task { probe.autoRunOnce() }
  }
}

@MainActor
private final class AppAttestProbe: ObservableObject {
  @Published private(set) var lines: [String] = []
  @Published private(set) var running = false

  private let service = DCAppAttestService.shared
  private static let testChallenge = Data("KAGEMUSHA-APP-ATTEST-PHYSICAL-PROBE-V1".utf8)
  private var automaticallyStarted = false
  private var runID: UUID?

  func autoRunOnce() {
    guard !automaticallyStarted else { return }
    automaticallyStarted = true
    run()
  }

  func run() {
    guard !running else { return }
    running = true
    runID = UUID()
    lines = []
    Task { await execute() }
  }

  private func execute() async {
    defer { running = false }
    guard service.isSupported else {
      emit("App Attest unsupported on this device")
      return
    }
    emit("App Attest supported; beginning diagnostic")
    do {
      let keyID = try await generateKey()
      emit("Dedicated key created; identifier withheld (\(keyID.utf8.count) UTF-8 bytes)")
      try preserve(Data(keyID.utf8), as: "key-id.txt")

      let attestationHash = Self.hash("attestation")
      let attestation = try await attestKey(keyID, hash: attestationHash)
      try preserve(attestation, as: "attestation.cbor")
      emit("Attestation returned: \(attestation.count) raw bytes")

      // Repeating a fixed challenge is intentional only for this local counter probe.
      let assertionHash = Self.hash("assertion-retry")
      try preserve(assertionHash, as: "assertion-client-data-hash.bin")
      var firstRaw = try await generateAssertion(keyID, hash: assertionHash)
      let first = try AssertionDiagnostics(firstRaw)
      try preserve(firstRaw, as: "assertion-1.cbor")
      emit("Assertion 1: \(firstRaw.count) raw bytes; authData=\(first.authenticatorDataBytes); suffix=\(first.suffixBytes); flags=0x\(String(format: "%02x", first.flags)); counter=\(first.counter)")
      firstRaw.removeAll(keepingCapacity: false)
      emit("Simulated lost result: raw assertion discarded before any consumer commit")

      let retryRaw = try await generateAssertion(keyID, hash: assertionHash)
      let retry = try AssertionDiagnostics(retryRaw)
      try preserve(retryRaw, as: "assertion-retry.cbor")
      emit("Retry assertion: \(retryRaw.count) raw bytes; authData=\(retry.authenticatorDataBytes); suffix=\(retry.suffixBytes); flags=0x\(String(format: "%02x", retry.flags)); counter=\(retry.counter)")
      let advanced = first.counter < UInt32.max && retry.counter == first.counter + 1
      emit("Lost-result retry advanced counter by one: \(advanced ? "yes" : "no")")
      try preserve(Data("complete\n".utf8), as: "complete.marker")
      emit("Probe complete; raw objects retained in app-private storage; no money evidence admitted")
    } catch {
      let failure = error as NSError
      emit("Probe stopped: error domain=\(failure.domain), code=\(failure.code)")
    }
  }

  private static func hash(_ stage: String) -> Data {
    var input = testChallenge
    input.append(0)
    input.append(Data(stage.utf8))
    return Data(SHA256.hash(data: input))
  }

  private func emit(_ line: String) {
    lines.append(line)
    print("[AppAttestProbe] \(line)")
  }

  private func preserve(_ bytes: Data, as filename: String) throws {
    guard let runID else { throw ProbeFailure.privateStorageUnavailable }
    guard let applicationSupport = FileManager.default.urls(
      for: .applicationSupportDirectory, in: .userDomainMask).first else {
      throw ProbeFailure.privateStorageUnavailable
    }
    let directory = applicationSupport
      .appendingPathComponent("KagemushaAppAttestProbe", isDirectory: true)
      .appendingPathComponent(runID.uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
      attributes: [.posixPermissions: NSNumber(value: 0o700)])
    var file = directory.appendingPathComponent(filename, isDirectory: false)
    try bytes.write(to: file, options: [.atomic, .completeFileProtection])
    var resource = URLResourceValues()
    resource.isExcludedFromBackup = true
    try file.setResourceValues(resource)
  }

  private func generateKey() async throws -> String {
    try await withCheckedThrowingContinuation { continuation in
      service.generateKey { keyID, error in
        if let error { continuation.resume(throwing: error) }
        else if let keyID { continuation.resume(returning: keyID) }
        else { continuation.resume(throwing: ProbeFailure.emptyResult) }
      }
    }
  }

  private func attestKey(_ keyID: String, hash: Data) async throws -> Data {
    try await withCheckedThrowingContinuation { continuation in
      service.attestKey(keyID, clientDataHash: hash) { object, error in
        if let error { continuation.resume(throwing: error) }
        else if let object { continuation.resume(returning: object) }
        else { continuation.resume(throwing: ProbeFailure.emptyResult) }
      }
    }
  }

  private func generateAssertion(_ keyID: String, hash: Data) async throws -> Data {
    try await withCheckedThrowingContinuation { continuation in
      service.generateAssertion(keyID, clientDataHash: hash) { object, error in
        if let error { continuation.resume(throwing: error) }
        else if let object { continuation.resume(returning: object) }
        else { continuation.resume(throwing: ProbeFailure.emptyResult) }
      }
    }
  }
}

private enum ProbeFailure: Error {
  case emptyResult, malformedAssertion, privateStorageUnavailable
}

/// Read only the signed authenticator-data header for diagnostics, not verification.
private struct AssertionDiagnostics {
  let flags: UInt8
  let counter: UInt32
  let authenticatorDataBytes: Int
  let suffixBytes: Int

  init(_ rawAssertion: Data) throws {
    var reader = ProbeCBORReader(rawAssertion)
    guard try reader.length(major: 5) == 2 else { throw ProbeFailure.malformedAssertion }
    var authData: Data?
    var signature: Data?
    for _ in 0..<2 {
      let key = try reader.text()
      switch key {
      case "authenticatorData" where authData == nil:
        authData = try reader.byteString()
      case "signature" where signature == nil:
        signature = try reader.byteString()
      default:
        throw ProbeFailure.malformedAssertion
      }
    }
    guard reader.atEnd, let authData, authData.count >= 37,
      let signature, !signature.isEmpty else { throw ProbeFailure.malformedAssertion }
    flags = authData[32]
    counter = authData[33..<37].reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
    authenticatorDataBytes = authData.count
    suffixBytes = authData.count - 37
  }
}

private struct ProbeCBORReader {
  private let bytes: [UInt8]
  private var offset = 0
  var atEnd: Bool { offset == bytes.count }

  init(_ data: Data) { bytes = Array(data) }

  mutating func length(major: UInt8) throws -> Int {
    let header = try byte()
    guard header >> 5 == major else { throw ProbeFailure.malformedAssertion }
    let additional = header & 0x1f
    let length: Int
    switch additional {
    case 0...23: length = Int(additional)
    case 24: length = Int(try byte())
    case 25: length = (Int(try byte()) << 8) | Int(try byte())
    case 26:
      length = (Int(try byte()) << 24) | (Int(try byte()) << 16)
        | (Int(try byte()) << 8) | Int(try byte())
    default: throw ProbeFailure.malformedAssertion
    }
    guard length <= 8_192 else { throw ProbeFailure.malformedAssertion }
    return length
  }

  mutating func text() throws -> String {
    let count = try length(major: 3)
    let data = try content(length: count)
    guard let value = String(data: data, encoding: .utf8) else {
      throw ProbeFailure.malformedAssertion
    }
    return value
  }

  mutating func byteString() throws -> Data {
    let count = try length(major: 2)
    return try content(length: count)
  }

  private mutating func byte() throws -> UInt8 {
    guard offset < bytes.count else { throw ProbeFailure.malformedAssertion }
    defer { offset += 1 }
    return bytes[offset]
  }

  private mutating func content(length: Int) throws -> Data {
    guard length >= 0, offset <= bytes.count - length else {
      throw ProbeFailure.malformedAssertion
    }
    defer { offset += length }
    return Data(bytes[offset..<(offset + length)])
  }
}

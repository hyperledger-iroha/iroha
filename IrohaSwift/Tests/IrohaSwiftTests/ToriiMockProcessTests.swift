import Foundation
import XCTest

#if os(macOS)
  final class ToriiMockProcess {
    private let process: Process
    private let stdoutPipe: Pipe
    private let stderrPipe: Pipe
    let baseURL: URL

    init?(environment configuredEnvironment: [String: String]? = nil) {
      let environment = configuredEnvironment ?? Self.makeEnvironment()
      let candidates = Self.pythonLaunchConfigurations(environment: environment)
      var lastError: Error?
      var launchedProcess: Process?
      var stdout: Pipe?
      var stderr: Pipe?
      var baseURL: URL?

      for candidate in candidates {
        let proc = Process()
        proc.executableURL = candidate.executableURL
        proc.arguments = candidate.arguments
        proc.environment = environment
        stdout = Pipe()
        stderr = Pipe()
        proc.standardOutput = stdout
        proc.standardError = stderr

        do {
          try proc.run()
        } catch {
          lastError = error
          continue
        }

        if let url = Self.readBaseURL(from: stdout!) {
          launchedProcess = proc
          baseURL = url
          break
        }

        Self.terminateProcess(proc)
      }

      guard let runningProcess = launchedProcess,
        let runningStdout = stdout,
        let runningStderr = stderr,
        let resolvedURL = baseURL
      else {
        if let error = lastError {
          FileHandle.standardError.write(Data("Torii mock launch error: \(error)\n".utf8))
        }
        return nil
      }

      process = runningProcess
      stdoutPipe = runningStdout
      stderrPipe = runningStderr
      self.baseURL = resolvedURL
    }

    deinit {
      stop()
    }

    func stop() {
      Self.terminateProcess(process)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func resetState() async throws {
      var request = URLRequest(url: baseURL.appendingPathComponent("__mock__/reset"))
      request.httpMethod = "POST"
      let session = URLSession(configuration: .ephemeral)
      let (_, response) = try await session.data(for: request)
      guard let http = response as? HTTPURLResponse,
        (200..<300).contains(http.statusCode)
      else {
        throw URLError(.badServerResponse)
      }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func configurePipeline(
      scenario: String? = nil,
      hash: String? = nil,
      statusKinds: [String]? = nil,
      repeatLast: Bool? = nil,
      submitStatus: Int? = nil
    ) async throws {
      var payload: [String: Any] = [:]
      if let scenario { payload["scenario"] = scenario }
      if let hash { payload["hash"] = hash }
      if let statusKinds {
        payload["statuses"] = statusKinds.map { ["kind": $0] }
      }
      if let repeatLast { payload["repeat_last"] = repeatLast }
      if let submitStatus { payload["submit_status"] = submitStatus }
      var request = URLRequest(url: baseURL.appendingPathComponent("__mock__/pipeline/config"))
      request.httpMethod = "POST"
      request.httpBody = try JSONSerialization.data(withJSONObject: payload, options: [])
      request.setValue("application/json", forHTTPHeaderField: "Content-Type")
      let session = URLSession(configuration: .ephemeral)
      let (_, response) = try await session.data(for: request)
      guard let http = response as? HTTPURLResponse,
        (200..<300).contains(http.statusCode)
      else {
        throw URLError(.badServerResponse)
      }
    }

    fileprivate struct PythonLaunchConfiguration: Equatable {
      let executableURL: URL
      let arguments: [String]
    }

    fileprivate static func pythonLaunchConfigurations(
      environment: [String: String]
    ) -> [PythonLaunchConfiguration] {
      // The mock is a standalone stdlib fixture; importing its package also
      // loads SDK dependencies that the mock server does not consume.
      let arguments = [mockScriptURL.path, "--stdio"]
      if let configuredPython = environment["MOBILE_SDK_PYTHON_BINARY"] {
        // An explicit interpreter is authoritative, including when it is invalid.
        // Execute the path directly so spaces and shell characters stay literal.
        guard configuredPython.hasPrefix("/") else { return [] }
        return [
          PythonLaunchConfiguration(
            executableURL: URL(fileURLWithPath: configuredPython),
            arguments: arguments
          )
        ]
      }
      return ["python3", "python"].map { candidate in
        PythonLaunchConfiguration(
          executableURL: URL(fileURLWithPath: "/usr/bin/env"),
          arguments: [candidate] + arguments
        )
      }
    }

    fileprivate static var mockScriptURL: URL {
      URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()  // IrohaSwiftTests
        .deletingLastPathComponent()  // Tests
        .deletingLastPathComponent()  // IrohaSwift
        .deletingLastPathComponent()  // Repository root
        .appendingPathComponent("python/iroha_torii_client/mock.py")
    }

    private static func makeEnvironment() -> [String: String] {
      var env = ProcessInfo.processInfo.environment
      env["PYTHONUNBUFFERED"] = "1"
      return env
    }

    fileprivate static func terminateProcess(_ process: Process, timeout: TimeInterval = 1.0) {
      guard process.isRunning else { return }
      process.terminate()
      if !waitForExit(process, timeout: timeout) {
        process.interrupt()
        _ = waitForExit(process, timeout: timeout)
      }
    }

    fileprivate static func waitForExit(_ process: Process, timeout: TimeInterval) -> Bool {
      if !process.isRunning { return true }
      let semaphore = DispatchSemaphore(value: 0)
      let previousHandler = process.terminationHandler
      process.terminationHandler = { terminated in
        previousHandler?(terminated)
        semaphore.signal()
      }
      if !process.isRunning {
        process.terminationHandler = previousHandler
        return true
      }
      let result = semaphore.wait(timeout: .now() + timeout)
      process.terminationHandler = previousHandler
      return result == .success
    }

    private static func readBaseURL(from pipe: Pipe, timeout: TimeInterval = 5.0) -> URL? {
      let handle = pipe.fileHandleForReading
      let semaphore = DispatchSemaphore(value: 0)
      let lock = NSLock()
      var data = Data()
      var didSignal = false

      // Avoid blocking reads if the mock never writes to stdout.
      handle.readabilityHandler = { fileHandle in
        let chunk = fileHandle.availableData
        lock.lock()
        if !chunk.isEmpty {
          data.append(chunk)
        }
        let hasNewline = data.contains(0x0A)
        if !didSignal && (hasNewline || chunk.isEmpty) {
          didSignal = true
          semaphore.signal()
        }
        lock.unlock()
        if hasNewline {
          fileHandle.readabilityHandler = nil
        }
      }

      _ = semaphore.wait(timeout: .now() + timeout)
      handle.readabilityHandler = nil

      lock.lock()
      let snapshot = data
      lock.unlock()

      guard
        let lineData = snapshot.split(
          separator: 0x0A, maxSplits: 1, omittingEmptySubsequences: true
        ).first,
        let line = String(data: Data(lineData), encoding: .utf8)?.trimmingCharacters(
          in: .whitespacesAndNewlines),
        let jsonData = line.data(using: .utf8),
        let decoded = try? JSONSerialization.jsonObject(with: jsonData) as? [String: Any],
        let urlString = decoded["base_url"] as? String,
        let url = URL(string: urlString)
      else {
        return nil
      }
      return url
    }
  }

  final class ToriiMockProcessTests: XCTestCase {
    func testPythonDiscoveryIsRetainedWhenNoInterpreterIsConfigured() {
      let candidates = ToriiMockProcess.pythonLaunchConfigurations(environment: [:])
      XCTAssertEqual(candidates.count, 2)
      XCTAssertEqual(candidates.map(\.executableURL), [
        URL(fileURLWithPath: "/usr/bin/env"), URL(fileURLWithPath: "/usr/bin/env")
      ])
      XCTAssertEqual(candidates.map(\.arguments), [
        ["python3", ToriiMockProcess.mockScriptURL.path, "--stdio"],
        ["python", ToriiMockProcess.mockScriptURL.path, "--stdio"]
      ])
    }

    func testConfiguredPythonPathIsExecutedDirectlyWithoutSplitting() throws {
      let path = "/private/tmp/Python runtime;$(false)/python3.12"
      let candidates = ToriiMockProcess.pythonLaunchConfigurations(environment: [
        "MOBILE_SDK_PYTHON_BINARY": path
      ])
      XCTAssertEqual(candidates.count, 1)
      let candidate = try XCTUnwrap(candidates.first)
      XCTAssertEqual(candidate.executableURL.path, path)
      XCTAssertEqual(candidate.arguments, [ToriiMockProcess.mockScriptURL.path, "--stdio"])
    }

    func testMissingConfiguredPythonDoesNotSelectPathFallbacks() throws {
      let path = "/private/tmp/nonexistent-torii-interpreter/python3.12"
      let candidates = ToriiMockProcess.pythonLaunchConfigurations(environment: [
        "MOBILE_SDK_PYTHON_BINARY": path
      ])
      XCTAssertEqual(candidates.count, 1)
      XCTAssertEqual(try XCTUnwrap(candidates.first).executableURL.path, path)
    }

    func testMalformedConfiguredPythonDoesNotSelectPathFallbacks() {
      for path in ["", "python3.12", "relative/python3.12"] {
        XCTAssertTrue(ToriiMockProcess.pythonLaunchConfigurations(environment: [
          "MOBILE_SDK_PYTHON_BINARY": path
        ]).isEmpty, "unexpected fallback for explicit interpreter: \(path)")
      }
    }

    func testStandaloneMockStartsWithoutImportingTheClientPackage() throws {
      let temporary = FileManager.default.temporaryDirectory
        .appendingPathComponent("iroha-swift-mock-package-\(UUID().uuidString)")
      let package = temporary.appendingPathComponent("iroha_torii_client")
      try FileManager.default.createDirectory(at: package, withIntermediateDirectories: true)
      defer { try? FileManager.default.removeItem(at: temporary) }
      try Data("raise RuntimeError('client package must not initialize for a stdlib mock')\n".utf8)
        .write(to: package.appendingPathComponent("__init__.py"))
      var environment = ProcessInfo.processInfo.environment
      environment["PYTHONPATH"] = temporary.path
      environment["PYTHONUNBUFFERED"] = "1"
      let mock = try XCTUnwrap(ToriiMockProcess(environment: environment))
      defer { mock.stop() }
      XCTAssertEqual(mock.baseURL.scheme, "http")
      XCTAssertEqual(mock.baseURL.host, "127.0.0.1")
    }

    func testTerminateProcessReturnsPromptly() throws {
      let process = Process()
      process.executableURL = URL(fileURLWithPath: "/bin/sleep")
      process.arguments = ["1"]
      try process.run()
      let start = Date()
      ToriiMockProcess.terminateProcess(process, timeout: 0.05)
      let elapsed = Date().timeIntervalSince(start)
      XCTAssertLessThan(elapsed, 1.0)
      process.waitUntilExit()
    }
  }
#endif

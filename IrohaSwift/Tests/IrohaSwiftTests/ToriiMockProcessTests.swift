import Foundation
import XCTest

#if os(macOS)
  private final class ToriiMockHTTPSProxyURLProtocol: URLProtocol {
    private var forwardingTask: URLSessionDataTask?

    override class func canInit(with request: URLRequest) -> Bool {
      guard request.url?.scheme?.lowercased() == "https" else { return false }
      switch request.url?.host?.lowercased() {
      case "127.0.0.1", "localhost", "::1":
        return true
      default:
        return false
      }
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
      request
    }

    override func startLoading() {
      guard let requestURL = request.url,
        var components = URLComponents(url: requestURL, resolvingAgainstBaseURL: false)
      else {
        client?.urlProtocol(self, didFailWithError: URLError(.badURL))
        return
      }
      components.scheme = "http"
      guard let forwardedURL = components.url else {
        client?.urlProtocol(self, didFailWithError: URLError(.badURL))
        return
      }

      var forwardedRequest = request
      forwardedRequest.url = forwardedURL
      forwardingTask = URLSession.shared.dataTask(with: forwardedRequest) { [weak self] data, response, error in
        guard let self else { return }
        if let error {
          self.client?.urlProtocol(self, didFailWithError: error)
          return
        }
        guard let response else {
          self.client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
          return
        }
        self.client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        if let data {
          self.client?.urlProtocol(self, didLoad: data)
        }
        self.client?.urlProtocolDidFinishLoading(self)
      }
      forwardingTask?.resume()
    }

    override func stopLoading() {
      forwardingTask?.cancel()
      forwardingTask = nil
    }
  }

  final class ToriiMockProcess {
    private let process: Process
    private let stdoutPipe: Pipe
    private let stderrPipe: Pipe
    let baseURL: URL

    init?() {
      let candidates = ["python3", "python"]
      var lastError: Error?
      var launchedProcess: Process?
      var stdout: Pipe?
      var stderr: Pipe?
      var baseURL: URL?

      for candidate in candidates {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        proc.arguments = [candidate, "-m", "iroha_torii_client.mock", "--stdio"]
        proc.environment = Self.makeEnvironment()
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

    /// Presents the HTTP-only local mock as an HTTPS Torii endpoint to the SDK.
    /// The matching session rewrites only loopback traffic after production
    /// transport-policy validation has accepted the request.
    var secureBaseURL: URL {
      var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false)!
      components.scheme = "https"
      return components.url!
    }

    func makeSecureSession() -> URLSession {
      let configuration = URLSessionConfiguration.ephemeral
      configuration.protocolClasses = [ToriiMockHTTPSProxyURLProtocol.self]
      return URLSession(configuration: configuration)
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
      accepted: Bool? = nil,
      submitStatus: Int? = nil
    ) async throws {
      var payload: [String: Any] = [:]
      if let scenario { payload["scenario"] = scenario }
      if let hash { payload["hash"] = hash }
      if let statusKinds {
        payload["statuses"] = statusKinds.map { ["kind": $0] }
      }
      if let repeatLast { payload["repeat_last"] = repeatLast }
      if let accepted { payload["accepted"] = accepted }
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

    private static func makeEnvironment() -> [String: String] {
      var env = ProcessInfo.processInfo.environment
      let repositoryRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()  // ToriiClientTests.swift
        .deletingLastPathComponent()  // IrohaSwiftTests
        .deletingLastPathComponent()  // Tests
        .deletingLastPathComponent()  // IrohaSwift
      let pythonPath = repositoryRoot.appendingPathComponent("python").path
      if let existing = env["PYTHONPATH"], !existing.isEmpty {
        env["PYTHONPATH"] = "\(pythonPath):\(existing)"
      } else {
        env["PYTHONPATH"] = pythonPath
      }
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

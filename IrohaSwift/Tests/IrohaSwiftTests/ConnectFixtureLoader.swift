import Foundation
@testable import IrohaSwift

struct ConnectFixtureBundle {
    let manifest: ConnectQueueEvidenceManifest
    let metrics: [ConnectQueueMetricsSample]
    let session: ConnectFixtureSession
    let notes: String
    let scenarios: [ConnectFixtureScenario]
}

struct ConnectFixtureSession: Decodable {
    let sidBase64Url: String
    let webSocketURL: String
    let toriiBaseURL: String
    let connectPolicyHash: String
    let notes: String?

    enum CodingKeys: String, CodingKey {
        case sidBase64Url = "sid_base64url"
        case webSocketURL = "web_socket_url"
        case toriiBaseURL = "torii_base_url"
        case connectPolicyHash = "connect_policy_hash"
        case notes
    }
}

struct ConnectFixtureScenario: Decodable {
    struct Frame: Decodable {
        let direction: ConnectDirection
        let sequence: UInt64
        let payloadHex: String

        enum CodingKeys: String, CodingKey {
            case direction
            case sequence
            case payloadHex = "payload_hex"
        }
    }

    struct Event: Decodable {
        let timestampMs: UInt64
        let state: String
        let note: String

        enum CodingKeys: String, CodingKey {
            case timestampMs = "timestamp_ms"
            case state
            case note
        }
    }

    let scenario: String
    let toriiBuild: String
    let sidBase64Url: String
    let description: String
    let frames: [Frame]
    let events: [Event]

    enum CodingKeys: String, CodingKey {
        case scenario
        case toriiBuild = "torii_build"
        case sidBase64Url = "sid_base64url"
        case description
        case frames
        case events
    }
}

enum ConnectFixtureLoaderError: Error {
    case bundleNotFound(URL)
}

/// Small helper used in tests to parse the shared Connect fixture bundle so we
/// can guard format regressions in CI.
final class ConnectFixtureLoader {
    private let bundleURL: URL
    private let decoder: JSONDecoder

    init(bundleURL: URL? = nil, filePath: String = #filePath) throws {
        let root = ConnectFixtureLoader.repositoryRoot(from: filePath)
        let resolvedURL = bundleURL ?? root.appendingPathComponent("docs/source/sdk/swift/readiness/archive/2026-05/connect",
                                                                   isDirectory: true)
        self.bundleURL = resolvedURL
        guard FileManager.default.fileExists(atPath: resolvedURL.path) else {
            throw ConnectFixtureLoaderError.bundleNotFound(resolvedURL)
        }
        let decoder = JSONDecoder()
        self.decoder = decoder
    }

    func loadBundle() throws -> ConnectFixtureBundle {
        let manifestURL = bundleURL.appendingPathComponent("manifest.json", isDirectory: false)
        let sessionURL = bundleURL.appendingPathComponent("session.json", isDirectory: false)
        let notesURL = bundleURL.appendingPathComponent("notes.md", isDirectory: false)

        let manifestData = try Data(contentsOf: manifestURL)
        let manifest = try decoder.decode(ConnectQueueEvidenceManifest.self, from: manifestData)

        let sessionData = try Data(contentsOf: sessionURL)
        let session = try decoder.decode(ConnectFixtureSession.self, from: sessionData)

        let metricsURL = bundleURL.appendingPathComponent("metrics.ndjson", isDirectory: false)
        let metrics = try loadMetrics(from: metricsURL)

        let scenarios = try loadScenarios()
        let notes = try? String(contentsOf: notesURL)

        return ConnectFixtureBundle(manifest: manifest,
                                    metrics: metrics,
                                    session: session,
                                    notes: notes ?? "",
                                    scenarios: scenarios)
    }

    private func loadMetrics(from url: URL) throws -> [ConnectQueueMetricsSample] {
        guard FileManager.default.fileExists(atPath: url.path) else {
            return []
        }
        let data = try Data(contentsOf: url)
        guard let body = String(data: data, encoding: .utf8) else {
            return []
        }
        let lines = body.split(separator: "\n").map(String.init)
        return try lines.map { line in
            let lineData = Data(line.utf8)
            return try decoder.decode(ConnectQueueMetricsSample.self, from: lineData)
        }
    }

    private func loadScenarios() throws -> [ConnectFixtureScenario] {
        let directory = bundleURL.appendingPathComponent("fixtures", isDirectory: true)
        guard FileManager.default.fileExists(atPath: directory.path) else {
            return []
        }
        let scenarioFiles = try FileManager.default.contentsOfDirectory(at: directory,
                                                                        includingPropertiesForKeys: nil,
                                                                        options: [.skipsHiddenFiles])
            .filter { $0.pathExtension == "json" }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
        return try scenarioFiles.map { url in
            let data = try Data(contentsOf: url)
            return try decoder.decode(ConnectFixtureScenario.self, from: data)
        }
    }

    private static func repositoryRoot(from filePath: String) -> URL {
        URL(fileURLWithPath: filePath)
            .deletingLastPathComponent() // file
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift
    }
}

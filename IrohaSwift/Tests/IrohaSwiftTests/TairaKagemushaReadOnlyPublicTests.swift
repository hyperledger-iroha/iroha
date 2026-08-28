import Foundation
import XCTest
@testable import IrohaSwift

private let tairaKagemushaReadOnlyOptIn = "IROHA_TAIRA_KAGEMUSHA_READ_ONLY"
private let tairaPublicRootOverride = "IROHA_TAIRA_PUBLIC_ROOT"
private let defaultTairaPublicRoot = TairaTestnetProfile.toriiBaseURL.absoluteString
private let tairaReadOnlyDeadlineSeconds: TimeInterval = 20

private struct TairaReadOnlyDeadlineExceeded: LocalizedError {
    var errorDescription: String? {
        "The public Taira Kagemusha read-only probe exceeded its 20-second deadline."
    }
}

private struct TairaPublicRootError: LocalizedError {
    let message: String

    var errorDescription: String? { message }
}

private final class TairaNoRedirectSessionDelegate: NSObject, URLSessionTaskDelegate {
    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping (URLRequest?) -> Void
    ) {
        completionHandler(nil)
    }
}

final class TairaKagemushaReadOnlyPublicTests: XCTestCase {
    func testPublicCapabilityMatchesExactUniversalContract() async throws {
        XCTAssertEqual(defaultTairaPublicRoot, "https://taira.sora.org")
        guard ProcessInfo.processInfo.environment[tairaKagemushaReadOnlyOptIn] == "1" else {
            return
        }

        let root = try publicRoot()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = tairaReadOnlyDeadlineSeconds
        configuration.timeoutIntervalForResource = tairaReadOnlyDeadlineSeconds
        configuration.httpShouldSetCookies = false
        configuration.httpCookieStorage = nil
        configuration.urlCredentialStorage = nil
        configuration.urlCache = nil
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        let delegate = TairaNoRedirectSessionDelegate()
        let session = URLSession(configuration: configuration, delegate: delegate, delegateQueue: nil)
        defer { session.invalidateAndCancel() }

        let client = ToriiClient(baseURL: root, session: session)
        let capability = try await withTwentySecondDeadline {
            try await client.getOfflineCapability()
        }

        XCTAssertEqual(capability.cashHandoffCapability, "cash_handoff_v1")
        XCTAssertEqual(capability.requiredBridgeAbiVersion, 23)
        XCTAssertEqual(capability.maxHops, 8)
        XCTAssertTrue(capability.ready)
    }

    private func publicRoot() throws -> URL {
        let environment = ProcessInfo.processInfo.environment
        let raw = environment[tairaPublicRootOverride] ?? defaultTairaPublicRoot
        guard raw == raw.trimmingCharacters(in: .whitespacesAndNewlines),
              let components = URLComponents(string: raw),
              components.scheme?.lowercased() == "https",
              components.host?.isEmpty == false,
              components.user == nil,
              components.password == nil,
              components.query == nil,
              components.fragment == nil,
              components.path.isEmpty || components.path == "/"
        else {
            throw TairaPublicRootError(
                message: "\(tairaPublicRootOverride) must be a credential-free HTTPS origin without a path, query, or fragment."
            )
        }
        let normalized = raw.hasSuffix("/") ? String(raw.dropLast()) : raw
        guard let root = URL(string: normalized) else {
            throw TairaPublicRootError(
                message: "\(tairaPublicRootOverride) is not a valid public Torii root."
            )
        }
        return root
    }

    private func withTwentySecondDeadline<T: Sendable>(
        operation: @escaping @Sendable () async throws -> T
    ) async throws -> T {
        try await withThrowingTaskGroup(of: T.self) { group in
            group.addTask(operation: operation)
            group.addTask {
                try await Task.sleep(
                    nanoseconds: UInt64(tairaReadOnlyDeadlineSeconds * 1_000_000_000)
                )
                throw TairaReadOnlyDeadlineExceeded()
            }
            defer { group.cancelAll() }
            guard let first = try await group.next() else {
                throw TairaReadOnlyDeadlineExceeded()
            }
            return first
        }
    }
}

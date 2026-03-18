import Foundation
import XCTest
@testable import IrohaSwift

final class StubWebSocketTask: ConnectWebSocketTask {
    var resumeCallCount = 0
    var cancelled: (code: ConnectCloseCode, reason: Data?)?
    var sentData: [Data] = []
    var pendingReceives: [(Result<Data, Error>) -> Void] = []
    private var queuedData: [Data] = []

    func resume() {
        resumeCallCount += 1
    }

    func send(data: Data, completion: @escaping (Error?) -> Void) {
        sentData.append(data)
        completion(nil)
    }

    func receive(completion: @escaping (Result<Data, Error>) -> Void) {
        if !queuedData.isEmpty {
            let data = queuedData.removeFirst()
            completion(.success(data))
            return
        }
        pendingReceives.append(completion)
    }

    func cancel(closeCode: ConnectCloseCode, reason: Data?) {
        cancelled = (closeCode, reason)
        queuedData.removeAll()
        while let completion = pendingReceives.popLast() {
            completion(.failure(NSError(domain: "Stub", code: -1)))
        }
    }

    func emit(_ data: Data) {
        guard !pendingReceives.isEmpty else {
            queuedData.append(data)
            return
        }
        let completion = pendingReceives.removeFirst()
        completion(.success(data))
    }
}

struct StubWebSocketFactory {
    let task: StubWebSocketTask

    var factory: ConnectWebSocketFactory {
        ConnectWebSocketFactory { _ in task }
    }
}

func XCTAssertThrowsErrorAsync<T>(_ expression: @autoclosure () async throws -> T,
                                  _ message: @autoclosure () -> String = "",
                                  expectation: (Error) -> Void) async {
    do {
        _ = try await expression()
        XCTFail(message())
    } catch {
        expectation(error)
    }
}

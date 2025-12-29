import Foundation
#if canImport(Combine)
import Combine

@available(iOS 15.0, macOS 12.0, *)
public extension ToriiClient {
    /// Fetch account asset balances once and emit them on the requested scheduler.
    func assetsPublisher(accountId: String,
                         limit: Int = 100,
                         scheduler: DispatchQueue? = .main) -> AnyPublisher<[ToriiAssetBalance], ToriiClientError> {
        makeValuePublisher(operation: { try await self.getAssets(accountId: accountId, limit: limit) },
                           scheduler: scheduler)
    }

    /// Expose verifying-key server-sent events as a Combine publisher.
    func verifyingKeyEventsPublisher(filter: ToriiVerifyingKeyEventFilter = ToriiVerifyingKeyEventFilter(),
                                     lastEventId: String? = nil,
                                     scheduler: DispatchQueue? = .main) -> AnyPublisher<ToriiVerifyingKeyEventMessage, ToriiClientError> {
        makeStreamPublisher({ self.streamVerifyingKeyEvents(filter: filter, lastEventId: lastEventId) },
                            scheduler: scheduler)
    }

    /// Bridge an async Torii call into a Combine publisher.
    func makeValuePublisher<Output>(operation: @Sendable @escaping () async throws -> Output,
                                    scheduler: DispatchQueue?) -> AnyPublisher<Output, ToriiClientError> {
        let queue = scheduler ?? DispatchQueue.main
        let taskContainer = ToriiCombineTaskContainer()

        return Deferred {
            Future<Output, ToriiClientError> { promise in
                let promiseBox = ToriiCombinePromiseBox(promise)
                let task = Task {
                    do {
                        let value = try await operation()
                        if !Task.isCancelled {
                            promiseBox.promise(.success(value))
                        }
                    } catch is CancellationError {
                        // Subscriber likely cancelled; drop the completion.
                    } catch {
                        if !Task.isCancelled {
                            promiseBox.promise(.failure(ToriiClient.mapToClientError(error)))
                        }
                    }
                }
                taskContainer.task = task
            }
        }
        .handleEvents(receiveCancel: { taskContainer.task?.cancel() })
        .receive(on: queue)
        .eraseToAnyPublisher()
    }

    /// Bridge an async stream into a Combine publisher, propagating cancellation cleanly.
    func makeStreamPublisher<Output>(_ builder: @Sendable @escaping () -> AsyncThrowingStream<Output, Error>,
                                     scheduler: DispatchQueue?) -> AnyPublisher<Output, ToriiClientError> {
        let queue = scheduler ?? DispatchQueue.main
        let subjectBox = ToriiCombineSubjectBox(PassthroughSubject<Output, ToriiClientError>())
        let task = Task {
            do {
                var iterator = builder().makeAsyncIterator()
                while let value = try await iterator.next() {
                    if Task.isCancelled {
                        break
                    }
                    subjectBox.subject.send(value)
                }
                if !Task.isCancelled {
                    subjectBox.subject.send(completion: .finished)
                }
            } catch is CancellationError {
                subjectBox.subject.send(completion: .finished)
            } catch {
                if !Task.isCancelled {
                    subjectBox.subject.send(completion: .failure(ToriiClient.mapToClientError(error)))
                }
            }
        }

        return subjectBox.subject
            .handleEvents(receiveCancel: { task.cancel() })
            .receive(on: queue)
            .eraseToAnyPublisher()
    }

    /// Normalize any error into a `ToriiClientError` for publisher surfaces.
    static func mapToClientError(_ error: Error) -> ToriiClientError {
        if let toriiError = error as? ToriiClientError {
            return toriiError
        }
        return .transport(error)
    }
}

final class ToriiCombineTaskContainer: @unchecked Sendable {
    var task: Task<Void, Never>?
}

private final class ToriiCombinePromiseBox<Output>: @unchecked Sendable {
    let promise: Future<Output, ToriiClientError>.Promise

    init(_ promise: @escaping Future<Output, ToriiClientError>.Promise) {
        self.promise = promise
    }
}

private final class ToriiCombineSubjectBox<Output>: @unchecked Sendable {
    let subject: PassthroughSubject<Output, ToriiClientError>

    init(_ subject: PassthroughSubject<Output, ToriiClientError>) {
        self.subject = subject
    }
}
#endif

import Foundation
#if canImport(Combine)
import Combine

@available(iOS 15.0, macOS 12.0, *)
public extension ToriiClient {
    /// Expose verifying-key server-sent events as a Combine publisher.
    func verifyingKeyEventsPublisher(
        scheduler: DispatchQueue? = .main
    ) -> AnyPublisher<ToriiEventMessage<ToriiEventNotice>, ToriiClientError> {
        makeStreamPublisher({ self.streamVerifyingKeyEvents() },
                            scheduler: scheduler)
    }

    /// Expose explorer transaction summaries over SSE as a Combine publisher.
    func explorerTransactionsPublisher(lastEventId: String? = nil,
                                       scheduler: DispatchQueue? = .main) -> AnyPublisher<ToriiExplorerTransactionItem, ToriiClientError> {
        makeStreamPublisher({ self.streamExplorerTransactions(lastEventId: lastEventId) },
                            scheduler: scheduler)
    }

    /// Expose explorer instruction payloads over SSE as a Combine publisher.
    func explorerInstructionsPublisher(lastEventId: String? = nil,
                                       scheduler: DispatchQueue? = .main) -> AnyPublisher<ToriiExplorerInstructionItem, ToriiClientError> {
        makeStreamPublisher({ self.streamExplorerInstructions(lastEventId: lastEventId) },
                            scheduler: scheduler)
    }

    /// Expose transfer records derived from the explorer instruction SSE feed as a Combine publisher.
    func explorerTransfersPublisher(lastEventId: String? = nil,
                                    matchingAccount accountId: String? = nil,
                                    assetDefinitionId: String? = nil,
                                    scheduler: DispatchQueue? = .main) -> AnyPublisher<ToriiExplorerTransferRecord, ToriiClientError> {
        makeStreamPublisher({ self.streamExplorerTransfers(lastEventId: lastEventId,
                                                           matchingAccount: accountId,
                                                           assetDefinitionId: assetDefinitionId) },
                            scheduler: scheduler)
    }

    /// Expose transfer summaries derived from the explorer instruction SSE feed as a Combine publisher.
    func explorerTransferSummariesPublisher(lastEventId: String? = nil,
                                            matchingAccount accountId: String? = nil,
                                            assetDefinitionId: String? = nil,
                                            relativeTo relativeAccountId: String? = nil,
                                            scheduler: DispatchQueue? = .main) -> AnyPublisher<ToriiExplorerTransferSummary, ToriiClientError> {
        makeStreamPublisher({ self.streamExplorerTransferSummaries(lastEventId: lastEventId,
                                                                   matchingAccount: accountId,
                                                                   assetDefinitionId: assetDefinitionId,
                                                                   relativeTo: relativeAccountId) },
                            scheduler: scheduler)
    }

    /// Emit historical account transfer summaries and then keep streaming live updates.
    func accountTransferHistoryPublisher(accountId: String,
                                         cursor: String? = nil,
                                         limit: UInt32? = nil,
                                         assetDefinitionId: String? = nil,
                                         lastEventId: String? = nil,
                                         maxItems: UInt64? = nil,
                                         dedupeLimit: Int = 10_000,
                                         scheduler: DispatchQueue? = .main) -> AnyPublisher<ToriiExplorerTransferSummary, ToriiClientError> {
        makeStreamPublisher({ self.streamAccountTransferHistory(accountId: accountId,
                                                               cursor: cursor,
                                                               limit: limit,
                                                               assetDefinitionId: assetDefinitionId,
                                                               lastEventId: lastEventId,
                                                               maxItems: maxItems,
                                                               dedupeLimit: dedupeLimit) },
                            scheduler: scheduler)
    }

    /// Emit historical transfer summaries for a transaction and then keep streaming live updates.
    func transactionTransferSummariesPublisher(hashHex: String,
                                               matchingAccount accountId: String? = nil,
                                               assetDefinitionId: String? = nil,
                                               relativeTo relativeAccountId: String? = nil,
                                               lastEventId: String? = nil,
                                               maxItems: UInt64? = nil,
                                               dedupeLimit: Int = 10_000,
                                               scheduler: DispatchQueue? = .main) -> AnyPublisher<ToriiExplorerTransferSummary, ToriiClientError> {
        makeStreamPublisher({
            self.streamTransactionTransferSummaries(hashHex: hashHex,
                                                     matchingAccount: accountId,
                                                     assetDefinitionId: assetDefinitionId,
                                                     relativeTo: relativeAccountId,
                                                     lastEventId: lastEventId,
                                                     maxItems: maxItems,
                                                     dedupeLimit: dedupeLimit)
        }, scheduler: scheduler)
    }

    /// Bridge an async stream into a Combine publisher, propagating cancellation cleanly.
    func makeStreamPublisher<Output>(_ builder: @Sendable @escaping () -> AsyncThrowingStream<Output, Error>,
                                     scheduler: DispatchQueue?) -> AnyPublisher<Output, ToriiClientError> {
        let queue = scheduler ?? DispatchQueue.main
        return Deferred {
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

private final class ToriiCombineSubjectBox<Output>: @unchecked Sendable {
    let subject: PassthroughSubject<Output, ToriiClientError>

    init(_ subject: PassthroughSubject<Output, ToriiClientError>) {
        self.subject = subject
    }
}
#endif

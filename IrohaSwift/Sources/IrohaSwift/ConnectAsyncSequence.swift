import Foundation

/// Async sequence adapter that yields `ConnectFrame`s from a `ConnectClient`.
/// The sequence stops when the client throws (e.g., socket closed) or the iteration is cancelled.
public struct ConnectFrameSequence: AsyncSequence, Sendable {
    public typealias Element = ConnectFrame

    private let client: ConnectClient

    public init(client: ConnectClient) {
        self.client = client
    }

    public func makeAsyncIterator() -> Iterator {
        Iterator(client: client)
    }

    public struct Iterator: AsyncIteratorProtocol, Sendable {
        private let client: ConnectClient

        init(client: ConnectClient) {
            self.client = client
        }

        public mutating func next() async throws -> ConnectFrame? {
            try await client.receiveFrame()
        }
    }
}

import Foundation

/// One page of a collection read: `{"items": [...], "next_cursor": "…" | null, "total": N}`.
public struct ToriiPage<Item: Sendable>: Sendable {
    /// Items on this page, in the requested order.
    public let items: [Item]
    /// Pass as `ToriiListQuery.cursor` to read the next page; `nil` on the last page.
    public let nextCursor: String?
    /// Exact number of matching rows, present only when `includeTotal` was requested.
    public let total: UInt64?

    public init(items: [Item], nextCursor: String? = nil, total: UInt64? = nil) {
        self.items = items
        self.nextCursor = nextCursor
        self.total = total
    }

    /// Whether another page follows.
    public var hasMore: Bool {
        nextCursor != nil
    }

    /// The same page with every item converted.
    public func map<Converted: Sendable>(_ transform: (Item) throws -> Converted) rethrows -> ToriiPage<Converted> {
        ToriiPage<Converted>(items: try items.map(transform), nextCursor: nextCursor, total: total)
    }
}

extension ToriiPage: Equatable where Item: Equatable {}

extension ToriiPage: Hashable where Item: Hashable {}

extension ToriiPage: Decodable where Item: Decodable {
    private enum CodingKeys: String, CodingKey {
        case items
        case nextCursor = "next_cursor"
        case total
    }

    public init(from decoder: Decoder) throws {
        try requireExactJSONFields(from: decoder, required: ["items", "next_cursor"], optional: ["total"], debugName: "collection page")
        let container = try decoder.container(keyedBy: CodingKeys.self)
        items = try container.decode([Item].self, forKey: .items)
        if container.contains(.nextCursor), try !container.decodeNil(forKey: .nextCursor) {
            nextCursor = try container.decode(String.self, forKey: .nextCursor)
            guard nextCursor?.isEmpty == false else {
                throw DecodingError.dataCorruptedError(forKey: .nextCursor, in: container,
                    debugDescription: "next_cursor must be non-empty or null")
            }
        } else {
            nextCursor = nil
        }
        if container.contains(.total), try !container.decodeNil(forKey: .total) {
            total = try container.decode(UInt64.self, forKey: .total)
            guard total! >= UInt64(items.count) else {
                throw DecodingError.dataCorruptedError(forKey: .total, in: container,
                    debugDescription: "total cannot be smaller than this page")
            }
        } else {
            total = nil
        }
    }
}

/// Fetches one page of a collection for a query.
public typealias ToriiPageFetcher<Item: Sendable> = @Sendable (ToriiListQuery) async throws -> ToriiPage<Item>

/// Every page of a query, fetched on demand.
///
/// Each `next()` issues at most one request, using the previous page's
/// `nextCursor`; iteration ends after the page whose `nextCursor` is `nil`.
/// Nothing is prefetched or buffered, so stopping or cancelling the consuming
/// task stops the requests. When `includeTotal` is set, only the first page
/// asks for (and pays for) the exact total.
public struct ToriiPageSequence<Item: Sendable>: AsyncSequence, Sendable {
    public typealias Element = ToriiPage<Item>

    private let query: ToriiListQuery
    private let fetchPage: ToriiPageFetcher<Item>

    /// Page through `query` with a custom fetcher (useful for tests or other transports).
    public init(query: ToriiListQuery, fetchPage: @escaping ToriiPageFetcher<Item>) {
        self.query = query
        self.fetchPage = fetchPage
    }

    public func makeAsyncIterator() -> AsyncIterator {
        AsyncIterator(pending: query, fetchPage: fetchPage)
    }

    public struct AsyncIterator: AsyncIteratorProtocol {
        private var pending: ToriiListQuery?
        private var seenCursors: Set<String> = []
        private let fetchPage: ToriiPageFetcher<Item>

        init(pending: ToriiListQuery?, fetchPage: @escaping ToriiPageFetcher<Item>) {
            self.pending = pending
            if let cursor = pending?.cursor { seenCursors.insert(cursor) }
            self.fetchPage = fetchPage
        }

        public mutating func next() async throws -> ToriiPage<Item>? {
            guard let query = pending else {
                return nil
            }
            pending = nil
            try Task.checkCancellation()
            let page = try await fetchPage(query)
            if let nextCursor = page.nextCursor {
                guard seenCursors.insert(nextCursor).inserted else {
                    throw ToriiClientError.invalidPayload(
                        "Torii repeated a cursor as `next_cursor`; the page sequence cannot advance."
                    )
                }
                var following = query
                following.cursor = nextCursor
                following.includeTotal = false
                pending = following
            }
            return page
        }
    }
}

/// Every item of a query across all pages, fetched page by page on demand.
public struct ToriiItemSequence<Item: Sendable>: AsyncSequence, Sendable {
    public typealias Element = Item

    private let pages: ToriiPageSequence<Item>

    public init(pages: ToriiPageSequence<Item>) {
        self.pages = pages
    }

    public func makeAsyncIterator() -> AsyncIterator {
        AsyncIterator(pages: pages.makeAsyncIterator())
    }

    public struct AsyncIterator: AsyncIteratorProtocol {
        private var pages: ToriiPageSequence<Item>.AsyncIterator
        private var buffer: [Item] = []
        private var index = 0

        init(pages: ToriiPageSequence<Item>.AsyncIterator) {
            self.pages = pages
        }

        public mutating func next() async throws -> Item? {
            while index == buffer.count {
                guard let page = try await pages.next() else {
                    return nil
                }
                buffer = page.items
                index = 0
            }
            let item = buffer[index]
            index += 1
            return item
        }
    }
}

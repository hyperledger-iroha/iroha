import Foundation

public struct ToriiQueryEnvelope: Codable, Sendable, Equatable {
  public var query: String?
  public var filter: ToriiJSONValue?
  public var select: [ToriiQuerySelectEntry]?
  public var sort: [ToriiQuerySortKey]
  public var pagination: ToriiQueryPagination
  public var fetchSize: UInt64?
  public var countMode: String?

  private enum CodingKeys: String, CodingKey {
    case query
    case filter
    case select
    case sort
    case pagination
    case fetchSize = "fetch_size"
    case countMode = "count_mode"
  }

  public init(
    query: String? = nil,
    filter: ToriiJSONValue? = nil,
    select: [ToriiQuerySelectEntry]? = nil,
    sort: [ToriiQuerySortKey] = [],
    pagination: ToriiQueryPagination = ToriiQueryPagination(),
    fetchSize: UInt64? = nil,
    countMode: String? = nil
  ) {
    self.query = query
    self.filter = filter
    self.select = select
    self.sort = sort
    self.pagination = pagination
    self.fetchSize = fetchSize
    self.countMode = countMode
  }

  public func encode(to encoder: Encoder) throws {
    var container = encoder.container(keyedBy: CodingKeys.self)
    if let query {
      let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
      guard !trimmed.isEmpty else {
        throw EncodingError.invalidValue(
          query,
          EncodingError.Context(
            codingPath: encoder.codingPath,
            debugDescription: "query must be a non-empty string")
        )
      }
      try container.encode(trimmed, forKey: .query)
    }
    try container.encodeIfPresent(filter, forKey: .filter)
    try container.encodeIfPresent(select, forKey: .select)
    try container.encode(sort, forKey: .sort)
    try container.encode(pagination, forKey: .pagination)
    try container.encodeIfPresent(fetchSize, forKey: .fetchSize)
    if let countMode {
      let normalized = countMode.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
      guard normalized == "bounded" || normalized == "exact" else {
        throw EncodingError.invalidValue(
          countMode,
          EncodingError.Context(
            codingPath: encoder.codingPath,
            debugDescription: "countMode must be bounded or exact")
        )
      }
      try container.encode(normalized, forKey: .countMode)
    }
  }
}

public struct ToriiCanonicalRequestAuth: Sendable, Equatable {
  public var accountId: String
  public var privateKey: Data
  public var timestampMs: UInt64?
  public var nonce: String?

  public init(
    accountId: String,
    privateKey: Data,
    timestampMs: UInt64? = nil,
    nonce: String? = nil
  ) {
    self.accountId = accountId
    self.privateKey = privateKey
    self.timestampMs = timestampMs
    self.nonce = nonce
  }
}

public struct ToriiPushDeviceRequest: Encodable, Sendable, Equatable {
  public var accountId: String
  public var platform: String
  public var token: String
  public var topics: [String]?

  private enum CodingKeys: String, CodingKey {
    case accountId = "account_id"
    case platform
    case token
    case topics
  }

  public init(
    accountId: String,
    platform: String,
    token: String,
    topics: [String]? = nil
  ) {
    self.accountId = accountId
    self.platform = platform
    self.token = token
    self.topics = topics
  }

  public func encode(to encoder: Encoder) throws {
    let normalizedAccount = try ToriiRequestValidation.normalizedNonEmpty(
      accountId,
      field: "account_id")
    let normalizedPlatform = try ToriiRequestValidation.normalizedNonEmpty(
      platform,
      field: "platform")
    let normalizedToken = try ToriiRequestValidation.normalizedNonEmpty(
      token,
      field: "token")
    let normalizedTopics = try topics?.map {
      try ToriiRequestValidation.normalizedNonEmpty($0, field: "topics")
    }
    var container = encoder.container(keyedBy: CodingKeys.self)
    try container.encode(normalizedAccount, forKey: .accountId)
    try container.encode(normalizedPlatform, forKey: .platform)
    try container.encode(normalizedToken, forKey: .token)
    try container.encodeIfPresent(normalizedTopics, forKey: .topics)
  }
}

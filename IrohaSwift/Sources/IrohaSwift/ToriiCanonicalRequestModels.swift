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
    select: [String]? = nil,
    sort: [ToriiQuerySortKey] = [],
    pagination: ToriiQueryPagination = ToriiQueryPagination(),
    fetchSize: UInt64? = nil,
    countMode: String? = nil
  ) {
    self.query = query
    self.filter = filter
    self.select = select?.map { ToriiQuerySelectEntry.fieldPath($0) }
    self.sort = sort
    self.pagination = pagination
    self.fetchSize = fetchSize
    self.countMode = countMode
  }

  public init(
    query: String? = nil,
    filter: ToriiJSONValue? = nil,
    selectEntries: [ToriiQuerySelectEntry]? = nil,
    sort: [ToriiQuerySortKey] = [],
    pagination: ToriiQueryPagination = ToriiQueryPagination(),
    fetchSize: UInt64? = nil,
    countMode: String? = nil
  ) {
    self.query = query
    self.filter = filter
    self.select = selectEntries
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

extension KeyedDecodingContainer {
  fileprivate func decodeStringArrayIfPresent(forKey key: Key) throws -> [String] {
    try decodeIfPresent([String].self, forKey: key) ?? []
  }

  fileprivate func decodeFlexibleUInt64(forKey key: Key) throws -> UInt64 {
    if let direct = try? decode(UInt64.self, forKey: key) {
      return direct
    }
    if let signed = try? decode(Int64.self, forKey: key),
      signed >= 0
    {
      return UInt64(signed)
    }
    if let rendered = try? decode(String.self, forKey: key) {
      let trimmed = rendered.trimmingCharacters(in: .whitespacesAndNewlines)
      if let parsed = UInt64(trimmed) {
        return parsed
      }
    }
    throw DecodingError.dataCorruptedError(
      forKey: key,
      in: self,
      debugDescription: "\(key.stringValue) must be an unsigned integer")
  }

  fileprivate func decodeFlexibleUInt8(forKey key: Key) throws -> UInt8 {
    let value = try decodeFlexibleUInt64(forKey: key)
    guard value <= UInt64(UInt8.max) else {
      throw DecodingError.dataCorruptedError(
        forKey: key,
        in: self,
        debugDescription: "\(key.stringValue) exceeds UInt8")
    }
    return UInt8(value)
  }

  fileprivate func decodeFlexibleUInt16(forKey key: Key) throws -> UInt16 {
    let value = try decodeFlexibleUInt64(forKey: key)
    guard value <= UInt64(UInt16.max) else {
      throw DecodingError.dataCorruptedError(
        forKey: key,
        in: self,
        debugDescription: "\(key.stringValue) exceeds UInt16")
    }
    return UInt16(value)
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

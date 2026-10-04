import Foundation

/// A reusable account credential for canonical request signatures.
///
/// The credential carries no timestamp or nonce: every signed request gets
/// fresh values from the client's `ToriiCanonicalRequestFreshness`, so reusing
/// one credential never replays a nonce.
public struct ToriiCanonicalRequestAuth: Sendable, Equatable {
  public var accountId: String
  public var privateKey: Data

  public init(accountId: String, privateKey: Data) {
    self.accountId = accountId
    self.privateKey = privateKey
  }
}

/// Source of the timestamp and single-use nonce bound into each canonical
/// request signature.
///
/// Clients draw new values for every request. The default uses the client's
/// clock and a random 128-bit nonce; inject a deterministic source only in tests.
public struct ToriiCanonicalRequestFreshness: Sendable {
  public let timestampMs: @Sendable () -> UInt64
  public let nonce: @Sendable () -> String

  public init(
    timestampMs: @escaping @Sendable () -> UInt64,
    nonce: @escaping @Sendable () -> String = ToriiCanonicalRequestFreshness.randomNonce
  ) {
    self.timestampMs = timestampMs
    self.nonce = nonce
  }

  /// Wall-clock milliseconds and a random nonce for every request.
  public static let system = ToriiCanonicalRequestFreshness(
    timestampMs: { UInt64(max(0, Date().timeIntervalSince1970 * 1_000).rounded()) }
  )

  /// A random 128-bit nonce in hex.
  @Sendable public static func randomNonce() -> String {
    UUID().uuidString.replacingOccurrences(of: "-", with: "")
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

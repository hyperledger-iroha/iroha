import Foundation

/// Canonical public body for secure-device operation 16.
///
/// The nested values are exact independent Norito archives. Private reservation openings, key
/// handles, hardware snapshots, and complete Guard certificates never cross this SDK boundary.
public struct KagemushaDeviceMintStageCommandV1: Equatable, Sendable {
  public let version: UInt16
  public let canonicalAuthorization: Data
  public let canonicalMintCredit: Data

  public init(
    version: UInt16 = 1,
    canonicalAuthorization: Data,
    canonicalMintCredit: Data
  ) throws {
    guard version == 1,
      !canonicalAuthorization.isEmpty,
      canonicalAuthorization.count <= KagemushaWireV1.maximumMintAuthorizationBytes,
      !canonicalMintCredit.isEmpty,
      canonicalMintCredit.count <= KagemushaWireV1.maximumMintCreditBytes
    else { throw KagemushaWireEnvelopeErrorV1.invalidText }
    self.version = version
    self.canonicalAuthorization = Data(canonicalAuthorization)
    self.canonicalMintCredit = Data(canonicalMintCredit)
  }
}

/// Closed public result of secure-device operation 16.
public enum KagemushaDeviceMintStageDispositionV1: UInt8, CaseIterable, Sendable {
  /// A previously unseen finalized credit was installed durably.
  case staged = 0
  /// The exact canonical credit was already pending or consumed.
  case exactDuplicate = 1
}

/// Bounded public summary returned only after qualified hardware completes operation 16.
///
/// The response authenticator must still be verified by the qualified native platform adapter;
/// canonical decoding and a nonzero identifier do not authenticate durable staging.
public struct KagemushaDeviceMintStageResultV1: Equatable, Sendable {
  public let version: UInt16
  public let disposition: KagemushaDeviceMintStageDispositionV1
  public let creditID: Data

  public init(
    version: UInt16 = 1,
    disposition: KagemushaDeviceMintStageDispositionV1,
    creditID: Data
  ) throws {
    guard version == 1 else { throw KagemushaWireEnvelopeErrorV1.invalidText }
    self.version = version
    self.disposition = disposition
    self.creditID = try kagemushaDeviceMintStageDigest(creditID)
  }
}

private func kagemushaDeviceMintStageDigest(_ value: Data) throws -> Data {
  guard value.count == 32, value.contains(where: { $0 != 0 }) else {
    throw KagemushaWireEnvelopeErrorV1.invalidText
  }
  return Data(value)
}

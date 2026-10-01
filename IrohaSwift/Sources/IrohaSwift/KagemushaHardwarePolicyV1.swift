import Foundation

/// Bounded projection of the retained native release catalog. Constructing this data
/// value is not release admission; only the same installed native owner supplies policy.
public struct KagemushaHardwarePolicyV1: Equatable, Sendable {
  public let releaseID: Data
  public let hardwarePolicyDigest: Data
  public let providerPolicyRoot: Data

  public init(releaseID: Data, hardwarePolicyDigest: Data, providerPolicyRoot: Data) throws {
    self.releaseID = try kagemushaDigest(releaseID, "releaseID")
    self.hardwarePolicyDigest = try kagemushaDigest(hardwarePolicyDigest, "hardwarePolicyDigest")
    self.providerPolicyRoot = try kagemushaDigest(providerPolicyRoot, "providerPolicyRoot")
  }

  /// Correlate the qualification digest and aggregate policy root to their separately
  /// authenticated catalog fields. Neither field may substitute for the other.
  public func matches(qualification: KagemushaHardwareQualificationV1,
    aggregateState: KagemushaAggregateStateCommitmentV1) -> Bool {
    matches(qualification: qualification)
      && aggregateState.releaseID == releaseID
      && aggregateState.hardwarePolicyID == providerPolicyRoot
  }

  public func matches(qualification: KagemushaHardwareQualificationV1) -> Bool {
    qualification.releaseID == releaseID && qualification.hardwarePolicyDigest == hardwarePolicyDigest
  }
}

/// Read-only projection, always acquired from the same live installed native owner.
/// Calling this method dispatches no hardware operation and grants no monetary permit.
public protocol KagemushaNativeHardwarePolicyProvidingCoreV1: KagemushaNativeCoreCoordinatorV1 {
  func authenticatedHardwarePolicy() throws -> KagemushaHardwarePolicyV1
}

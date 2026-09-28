import Foundation

/// Exact first-activation proposal for an installed standby KAGEMUSHA verifier release.
/// Native admission authenticates the complete registry and applies the finalized transition.
public struct ToriiGovernanceKagemushaVerifierReleaseActivateProposalV1:
  Decodable, Sendable, Equatable
{
  public let proposalOperator: String
  public let networkId: NetworkId
  public let expectedPredecessor: ToriiGovernanceKagemushaGovernedVerifierRegistryV1
  public let successorReleaseId: ToriiGovernanceKagemushaBytes32V1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case proposalOperator = "proposal_operator"
    case networkId = "network_id"
    case expectedPredecessor = "expected_predecessor"
    case successorReleaseId = "successor_release_id"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "KagemushaVerifierReleaseActivate"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    proposalOperator = try governanceCanonicalAccount(
      container.decode(String.self, forKey: .proposalOperator),
      codingPath: container.codingPath + [CodingKeys.proposalOperator],
      field: "proposal_operator"
    )
    networkId = try container.decode(NetworkId.self, forKey: .networkId)
    expectedPredecessor = try container.decode(
      ToriiGovernanceKagemushaGovernedVerifierRegistryV1.self,
      forKey: .expectedPredecessor
    )
    successorReleaseId = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .successorReleaseId
    )
    guard expectedPredecessor.authorityPolicy != nil,
      expectedPredecessor.activeReleaseId == nil,
      expectedPredecessor.releases.count == 1,
      expectedPredecessor.releases[0].releaseId == successorReleaseId,
      expectedPredecessor.releases[0].status == .standby
    else {
      throw DecodingError.dataCorrupted(
        .init(
          codingPath: decoder.codingPath,
          debugDescription:
            "first activation requires an inactive governed registry and its sole installed standby target"
        )
      )
    }
  }
}

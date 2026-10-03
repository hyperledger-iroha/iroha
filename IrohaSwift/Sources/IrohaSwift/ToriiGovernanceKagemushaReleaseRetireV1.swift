import Foundation

/// Exact proposal to remove one never-activated standby verifier release.
/// Native admission authenticates registry digests, signatures and finalized authority.
public struct ToriiGovernanceKagemushaVerifierReleaseRetireProposalV1:
  Decodable, Sendable, Equatable
{
  public let proposalOperator: String
  public let networkId: NetworkId
  public let expectedPredecessor: ToriiGovernanceKagemushaGovernedVerifierRegistryV1
  public let standbyReleaseId: ToriiGovernanceKagemushaBytes32V1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case proposalOperator = "proposal_operator"
    case networkId = "network_id"
    case expectedPredecessor = "expected_predecessor"
    case standbyReleaseId = "standby_release_id"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "KagemushaVerifierReleaseRetire"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    proposalOperator = try governanceCanonicalAccount(
      container.decode(String.self, forKey: .proposalOperator),
      codingPath: container.codingPath + [CodingKeys.proposalOperator],
      field: "proposal_operator"
    )
    networkId = try container.decode(NetworkId.self, forKey: .networkId)
    expectedPredecessor = try container.decode(
      ToriiGovernanceKagemushaGovernedVerifierRegistryV1.self, forKey: .expectedPredecessor
    )
    standbyReleaseId = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .standbyReleaseId
    )
    // These are public shape and status checks. The SDK does not certify the
    // claimed predecessor or authenticate its policy/receipt/attestation digests.
    let rows = expectedPredecessor.releases
    let active = expectedPredecessor.activeReleaseId
    guard expectedPredecessor.authorityPolicy != nil,
      zip(rows, rows.dropFirst()).allSatisfy({
        $0.releaseId.bytes.lexicographicallyPrecedes($1.releaseId.bytes)
      }),
      rows.allSatisfy({ row in
        [row.releaseId, row.profileDigest, row.artifactManifestDigest, row.receiptDigest,
          row.attestationDigest, row.authorityPolicyDigest, row.hardwarePolicyDigest,
          row.nativeProfileDigest, row.providerPolicyRoot, row.suiteId, row.vkSetDigest]
          .allSatisfy { $0.bytes.contains(where: { $0 != 0 }) }
      }),
      rows.filter({ $0.status == .active }).count == (active == nil ? 0 : 1),
      rows.allSatisfy({ row in
        if row.status == .active { return row.releaseId.bytes == active }
        return active != nil || row.status == .standby
      }),
      let target = rows.first(where: { $0.releaseId == standbyReleaseId }),
      target.status == .standby
    else {
      throw DecodingError.dataCorrupted(
        .init(
          codingPath: decoder.codingPath,
          debugDescription:
            "release retirement requires a consistent governed predecessor and its exact unused standby target"
        )
      )
    }
  }
}

package org.hyperledger.iroha.android.offline;

import java.util.Map;
import java.util.Objects;

/** Response payload for issuing an operator-signed offline build claim. */
public final class OfflineBuildClaimIssueResponse {
  private final String claimIdHex;
  private final Map<String, Object> buildClaim;
  private final OfflineBuildClaim typedBuildClaim;

  public OfflineBuildClaimIssueResponse(
      final String claimIdHex,
      final Map<String, Object> buildClaim,
      final OfflineBuildClaim typedBuildClaim) {
    this.claimIdHex = Objects.requireNonNull(claimIdHex, "claimIdHex");
    this.buildClaim = Map.copyOf(Objects.requireNonNull(buildClaim, "buildClaim"));
    this.typedBuildClaim = Objects.requireNonNull(typedBuildClaim, "typedBuildClaim");
  }

  public String claimIdHex() {
    return claimIdHex;
  }

  public Map<String, Object> buildClaim() {
    return buildClaim;
  }

  public OfflineBuildClaim typedBuildClaim() {
    return typedBuildClaim;
  }
}

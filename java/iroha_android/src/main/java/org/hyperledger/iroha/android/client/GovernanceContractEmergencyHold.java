package org.hyperledger.iroha.android.client;

/** Retained bounded Parliament emergency-hold projection. */
public final class GovernanceContractEmergencyHold {
  private final String incidentDigestHex;
  private final String proposalContentIdHex;
  private final String governanceAttemptIdHex;
  private final String reason;
  private final long imposedAtHeight;
  private final long expiresAtHeight;

  public GovernanceContractEmergencyHold(
      final String incidentDigestHex,
      final String proposalContentIdHex,
      final String governanceAttemptIdHex,
      final String reason,
      final long imposedAtHeight,
      final long expiresAtHeight) {
    this.incidentDigestHex = incidentDigestHex;
    this.proposalContentIdHex = proposalContentIdHex;
    this.governanceAttemptIdHex = governanceAttemptIdHex;
    this.reason = reason;
    this.imposedAtHeight = imposedAtHeight;
    this.expiresAtHeight = expiresAtHeight;
  }

  public String incidentDigestHex() { return incidentDigestHex; }
  public String proposalContentIdHex() { return proposalContentIdHex; }
  public String governanceAttemptIdHex() { return governanceAttemptIdHex; }
  public String reason() { return reason; }
  public long imposedAtHeight() { return imposedAtHeight; }
  public long expiresAtHeight() { return expiresAtHeight; }
}

package org.hyperledger.iroha.android.client;

import java.math.BigInteger;

/** Retained bounded Parliament emergency-hold projection. */
public final class GovernanceContractEmergencyHold {
  private final String incidentDigestHex;
  private final String proposalContentIdHex;
  private final String governanceAttemptIdHex;
  private final String reason;
  private final BigInteger imposedAtHeight;
  private final BigInteger expiresAtHeight;

  public GovernanceContractEmergencyHold(
      final String incidentDigestHex,
      final String proposalContentIdHex,
      final String governanceAttemptIdHex,
      final String reason,
      final BigInteger imposedAtHeight,
      final BigInteger expiresAtHeight) {
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
  /** Returns the full unsigned height at which Parliament imposed the hold. */
  public BigInteger imposedAtHeight() { return imposedAtHeight; }
  /** Returns the full unsigned height after which the hold no longer suspends execution. */
  public BigInteger expiresAtHeight() { return expiresAtHeight; }
}

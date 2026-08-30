package org.hyperledger.iroha.android.client;

import java.math.BigInteger;

/** Complete retained ownership and lifecycle projection for one contract. */
public final class GovernanceContractLifecycle {
  private final int version;
  private final String origin;
  private final String originAccount;
  private final String originProposalContentIdHex;
  private final String originGovernanceAttemptIdHex;
  private final String owner;
  private final String pendingOwner;
  private final boolean parliamentDelegated;
  private final String activeCodeHashHex;
  private final BigInteger revision;
  private final GovernanceContractEmergencyHold emergencyHold;

  public GovernanceContractLifecycle(
      final int version,
      final String origin,
      final String originAccount,
      final String originProposalContentIdHex,
      final String originGovernanceAttemptIdHex,
      final String owner,
      final String pendingOwner,
      final boolean parliamentDelegated,
      final String activeCodeHashHex,
      final BigInteger revision,
      final GovernanceContractEmergencyHold emergencyHold) {
    this.version = version;
    this.origin = origin;
    this.originAccount = originAccount;
    this.originProposalContentIdHex = originProposalContentIdHex;
    this.originGovernanceAttemptIdHex = originGovernanceAttemptIdHex;
    this.owner = owner;
    this.pendingOwner = pendingOwner;
    this.parliamentDelegated = parliamentDelegated;
    this.activeCodeHashHex = activeCodeHashHex;
    this.revision = revision;
    this.emergencyHold = emergencyHold;
  }

  public int version() { return version; }
  public String origin() { return origin; }
  public String originAccount() { return originAccount; }
  public String originProposalContentIdHex() { return originProposalContentIdHex; }
  public String originGovernanceAttemptIdHex() { return originGovernanceAttemptIdHex; }
  public String owner() { return owner; }
  public String pendingOwner() { return pendingOwner; }
  public boolean parliamentDelegated() { return parliamentDelegated; }
  public String activeCodeHashHex() { return activeCodeHashHex; }
  /** Returns the full unsigned lifecycle CAS revision. */
  public BigInteger revision() { return revision; }
  public GovernanceContractEmergencyHold emergencyHold() { return emergencyHold; }
}

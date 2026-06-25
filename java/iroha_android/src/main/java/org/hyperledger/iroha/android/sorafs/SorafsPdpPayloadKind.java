package org.hyperledger.iroha.android.sorafs;

/** PDP payload kind accepted by the Rust-backed SoraFS reference validator. */
public enum SorafsPdpPayloadKind {
  COMMITMENT(1, "commitment.to"),
  CHALLENGE(2, "challenge.to"),
  PROOF(3, "proof.to");

  private final int bridgeCode;
  private final String defaultLabel;

  SorafsPdpPayloadKind(final int bridgeCode, final String defaultLabel) {
    this.bridgeCode = bridgeCode;
    this.defaultLabel = defaultLabel;
  }

  /** Numeric selector used by {@code connect_norito_bridge}. */
  public int bridgeCode() {
    return bridgeCode;
  }

  /** Default diagnostic label passed to the reference validator. */
  public String defaultLabel() {
    return defaultLabel;
  }
}

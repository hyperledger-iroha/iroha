package org.hyperledger.iroha.android.sorafs;

/** PoP payload kind accepted by the Rust-backed SoraFS reference validator. */
public enum SorafsPopPayloadKind {
  CREDENTIAL(1, "pop-credential.to"),
  COMMITMENT_ROOT(2, "pop-commitment-root.to"),
  REVOCATION_LIST(3, "pop-revocation-list.to"),
  ENROLLMENT_REQUEST(4, "pop-enrollment-request.to"),
  RENEWAL_REQUEST(5, "pop-renewal-request.to"),
  MEMBERSHIP_PROOF(6, "pop-membership-proof.to"),
  ISSUED_CREDENTIAL_BUNDLE(7, "pop-issued-credential-bundle.to");

  private final int bridgeCode;
  private final String defaultLabel;

  SorafsPopPayloadKind(final int bridgeCode, final String defaultLabel) {
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

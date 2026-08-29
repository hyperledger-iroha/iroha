package org.hyperledger.iroha.android.sccp;

/** Stable first-release SCCP hub commitment kind tags. */
public enum SccpHubMessageKindV1 {
  TRANSFER(0);

  private final int tag;

  SccpHubMessageKindV1(final int tag) {
    this.tag = tag;
  }

  public int tag() {
    return tag;
  }

  public static SccpHubMessageKindV1 fromTag(final int tag) {
    for (final SccpHubMessageKindV1 value : values()) {
      if (value.tag == tag) {
        return value;
      }
    }
    return null;
  }
}

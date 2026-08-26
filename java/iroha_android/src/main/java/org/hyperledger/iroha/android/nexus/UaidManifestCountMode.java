package org.hyperledger.iroha.android.nexus;

/** Counting mode accepted and emitted by the UAID manifest inventory endpoint. */
public enum UaidManifestCountMode {
  /** Return a bounded observed count. */
  BOUNDED("bounded"),

  /** Return the exact filtered count. */
  EXACT("exact");

  private final String parameterValue;

  UaidManifestCountMode(final String parameterValue) {
    this.parameterValue = parameterValue;
  }

  /** Returns the exact lowercase query/response spelling used by Torii. */
  public String parameterValue() {
    return parameterValue;
  }
}

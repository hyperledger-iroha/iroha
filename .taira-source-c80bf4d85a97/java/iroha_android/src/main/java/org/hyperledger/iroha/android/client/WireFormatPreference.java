package org.hyperledger.iroha.android.client;

/** Preferred Torii wire format for request negotiation. */
public enum WireFormatPreference {
  /** Prefer Norito while allowing JSON responses from peers that negotiate it. */
  NORITO_PREFERRED("application/x-norito, application/json;q=0.8"),
  /** Prefer JSON while allowing Norito responses. */
  JSON_PREFERRED("application/json, application/x-norito;q=0.8"),
  /** Request only Norito responses. */
  NORITO_ONLY("application/x-norito"),
  /** Request only JSON responses. */
  JSON_ONLY("application/json");

  private final String acceptHeader;

  WireFormatPreference(final String acceptHeader) {
    this.acceptHeader = acceptHeader;
  }

  /** HTTP Accept header value for this preference. */
  public String acceptHeader() {
    return acceptHeader;
  }
}

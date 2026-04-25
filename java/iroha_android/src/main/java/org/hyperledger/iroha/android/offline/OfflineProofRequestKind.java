package org.hyperledger.iroha.android.offline;

import java.util.Locale;

/** Legacy proof request types retained for local fixture parsing. */
public enum OfflineProofRequestKind {
  SUM,
  COUNTER,
  REPLAY;

  /** Lowercase slug used by the Torii `kind` parameter. */
  public String asParameter() {
    return name().toLowerCase(Locale.ROOT);
  }
}

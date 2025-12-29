package org.hyperledger.iroha.android.offline;

import java.util.Locale;

/** Proof request types supported by Torii (`/v1/offline/transfers/proof`). */
public enum OfflineProofRequestKind {
  SUM,
  COUNTER,
  REPLAY;

  /** Lowercase slug used by the Torii `kind` parameter. */
  public String asParameter() {
    return name().toLowerCase(Locale.ROOT);
  }
}

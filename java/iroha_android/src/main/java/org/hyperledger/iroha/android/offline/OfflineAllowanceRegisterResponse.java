package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Response payload for registering or renewing an offline allowance. */
public final class OfflineAllowanceRegisterResponse {
  private final String certificateIdHex;

  public OfflineAllowanceRegisterResponse(final String certificateIdHex) {
    this.certificateIdHex = Objects.requireNonNull(certificateIdHex, "certificateIdHex");
  }

  public String certificateIdHex() {
    return certificateIdHex;
  }
}

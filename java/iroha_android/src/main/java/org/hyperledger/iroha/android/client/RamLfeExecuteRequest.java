package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Typed request wrapper for RAM-LFE execute flows. */
public final class RamLfeExecuteRequest {
  private final String encryptedInputHex;

  private RamLfeExecuteRequest(final String encryptedInputHex) {
    this.encryptedInputHex = encryptedInputHex;
  }

  public static RamLfeExecuteRequest encrypted(final String encryptedInputHex) {
    return new RamLfeExecuteRequest(
        HttpClientTransport.normalizeEvenLengthHex(
            Objects.requireNonNull(encryptedInputHex, "encryptedInputHex"),
            "encryptedInputHex"));
  }

  public String encryptedInputHex() {
    return encryptedInputHex;
  }
}

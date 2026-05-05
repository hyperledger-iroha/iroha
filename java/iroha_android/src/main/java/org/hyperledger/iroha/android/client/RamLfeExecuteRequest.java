package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Typed request wrapper for RAM-LFE execute flows. */
public final class RamLfeExecuteRequest {
  private final String inputHex;
  private final String encryptedInputHex;

  private RamLfeExecuteRequest(final String inputHex, final String encryptedInputHex) {
    this.inputHex = inputHex;
    this.encryptedInputHex = encryptedInputHex;
  }

  public static RamLfeExecuteRequest plaintext(final String inputHex) {
    return new RamLfeExecuteRequest(
        HttpClientTransport.normalizeEvenLengthHex(
            Objects.requireNonNull(inputHex, "inputHex"), "inputHex"),
        null);
  }

  public static RamLfeExecuteRequest encrypted(final String encryptedInputHex) {
    return new RamLfeExecuteRequest(
        null,
        HttpClientTransport.normalizeEvenLengthHex(
            Objects.requireNonNull(encryptedInputHex, "encryptedInputHex"),
            "encryptedInputHex"));
  }

  public String inputHex() {
    return inputHex;
  }

  public String encryptedInputHex() {
    return encryptedInputHex;
  }
}

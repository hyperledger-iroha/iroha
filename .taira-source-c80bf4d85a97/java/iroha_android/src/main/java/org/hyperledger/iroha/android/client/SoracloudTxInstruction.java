package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Wire instruction skeleton returned by Soracloud app endpoints for external signing. */
public final class SoracloudTxInstruction {
  private final String wireId;
  private final String payloadHex;

  public SoracloudTxInstruction(final String wireId, final String payloadHex) {
    this.wireId = Objects.requireNonNull(wireId, "wireId");
    this.payloadHex = Objects.requireNonNull(payloadHex, "payloadHex");
  }

  public String wireId() {
    return wireId;
  }

  public String payloadHex() {
    return payloadHex;
  }
}


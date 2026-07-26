package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Canonical native instruction descriptor returned by Sora VPN Torii endpoints. */
public final class VpnTxInstruction {
  private final String wireId;
  private final String payloadHex;

  public VpnTxInstruction(final String wireId, final String payloadHex) {
    this.wireId = Objects.requireNonNull(wireId, "wireId");
    this.payloadHex = Objects.requireNonNull(payloadHex, "payloadHex");
  }

  public String wireId() { return wireId; }
  public String payloadHex() { return payloadHex; }
}

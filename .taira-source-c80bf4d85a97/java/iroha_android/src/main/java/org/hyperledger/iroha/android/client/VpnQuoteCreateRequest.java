package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Request body for `POST /v1/vpn/quotes`. */
public final class VpnQuoteCreateRequest {
  private final String exitClass;
  private final String meteringPublicKeyHex;

  public VpnQuoteCreateRequest(final String meteringPublicKeyHex) {
    this(null, meteringPublicKeyHex);
  }

  public VpnQuoteCreateRequest(final String exitClass, final String meteringPublicKeyHex) {
    this.exitClass = exitClass;
    this.meteringPublicKeyHex = Objects.requireNonNull(meteringPublicKeyHex, "meteringPublicKeyHex");
  }

  public String exitClass() { return exitClass; }
  public String meteringPublicKeyHex() { return meteringPublicKeyHex; }
}

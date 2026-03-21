package org.hyperledger.iroha.android.client;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed request wrapper for RAM-LFE receipt verification. */
public final class RamLfeReceiptVerifyRequest {
  private final Map<String, Object> receipt;
  private final String outputHex;

  public RamLfeReceiptVerifyRequest(final Map<String, Object> receipt, final String outputHex) {
    this.receipt =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(receipt, "receipt")));
    this.outputHex = outputHex;
  }

  public Map<String, Object> receipt() {
    return receipt;
  }

  public String outputHex() {
    return outputHex;
  }
}

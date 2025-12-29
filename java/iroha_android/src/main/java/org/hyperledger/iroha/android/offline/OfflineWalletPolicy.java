package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Policy bounds for an offline wallet certificate. */
public final class OfflineWalletPolicy {
  private final String maxBalance;
  private final String maxTxValue;
  private final long expiresAtMs;

  public OfflineWalletPolicy(
      final String maxBalance, final String maxTxValue, final long expiresAtMs) {
    this.maxBalance = Objects.requireNonNull(maxBalance, "maxBalance");
    this.maxTxValue = Objects.requireNonNull(maxTxValue, "maxTxValue");
    this.expiresAtMs = expiresAtMs;
  }

  public String maxBalance() {
    return maxBalance;
  }

  public String maxTxValue() {
    return maxTxValue;
  }

  public long expiresAtMs() {
    return expiresAtMs;
  }

  Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("max_balance", maxBalance);
    map.put("max_tx_value", maxTxValue);
    map.put("expires_at_ms", expiresAtMs);
    return map;
  }
}

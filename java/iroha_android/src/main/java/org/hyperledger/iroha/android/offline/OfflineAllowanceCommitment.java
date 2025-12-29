package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Commitment payload for an offline allowance. */
public final class OfflineAllowanceCommitment {
  private final String assetId;
  private final String amount;
  private final byte[] commitment;

  public OfflineAllowanceCommitment(
      final String assetId, final String amount, final byte[] commitment) {
    this.assetId = Objects.requireNonNull(assetId, "assetId");
    this.amount = Objects.requireNonNull(amount, "amount");
    this.commitment = Objects.requireNonNull(commitment, "commitment").clone();
  }

  public String assetId() {
    return assetId;
  }

  public String amount() {
    return amount;
  }

  public byte[] commitment() {
    return commitment.clone();
  }

  Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("asset", assetId);
    map.put("amount", amount);
    map.put("commitment", encodeBytes(commitment));
    return map;
  }

  static List<Integer> encodeBytes(final byte[] bytes) {
    final List<Integer> out = new ArrayList<>(bytes.length);
    for (final byte b : bytes) {
      out.add(b & 0xff);
    }
    return out;
  }
}

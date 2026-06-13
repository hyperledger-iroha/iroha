package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;

/** Typed request body for {@code POST /v1/zk/roots}. */
public final class ZkRootsRequest {
  private final String assetId;
  private final int maxRoots;

  public ZkRootsRequest(final String assetId) {
    this(assetId, 0);
  }

  public ZkRootsRequest(final String assetId, final int maxRoots) {
    this.assetId = HttpClientTransport.normalizeNonBlank(assetId, "assetId");
    if (maxRoots < 0) {
      throw new IllegalArgumentException("maxRoots must be non-negative");
    }
    this.maxRoots = maxRoots;
  }

  public String assetId() {
    return assetId;
  }

  public int maxRoots() {
    return maxRoots;
  }

  byte[] toJsonBytes() {
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("asset_id", assetId);
    body.put("max", Integer.valueOf(maxRoots));
    return JsonEncoder.encode(body).getBytes(StandardCharsets.UTF_8);
  }
}

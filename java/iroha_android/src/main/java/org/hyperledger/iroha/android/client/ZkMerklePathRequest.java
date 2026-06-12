package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Typed request body for {@code POST /v1/zk/merkle-path}. */
public final class ZkMerklePathRequest {
  private final String assetId;
  private final List<String> commitments;

  public ZkMerklePathRequest(final String assetId, final List<byte[]> commitments) {
    this.assetId = HttpClientTransport.normalizeNonBlank(assetId, "assetId");
    final ArrayList<String> hex = new ArrayList<>(commitments == null ? 0 : commitments.size());
    if (commitments != null) {
      for (int i = 0; i < commitments.size(); i++) {
        hex.add(ZkRootsResponse.encodeHex(commitments.get(i), "commitments[" + i + "]"));
      }
    }
    this.commitments = Collections.unmodifiableList(hex);
  }

  public String assetId() {
    return assetId;
  }

  public List<String> commitments() {
    return commitments;
  }

  byte[] toJsonBytes() {
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("asset_id", assetId);
    body.put("commitments", commitments);
    return JsonEncoder.encode(body).getBytes(StandardCharsets.UTF_8);
  }
}

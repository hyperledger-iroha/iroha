package org.hyperledger.iroha.android.client;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Externally attested opening of a RAM-LFE encrypted output. */
public final class RamLfeOutputOpening {
  private final RamLfeOutputOpeningPayload payload;
  private final String signature;

  public RamLfeOutputOpening(final RamLfeOutputOpeningPayload payload, final String signature) {
    this.payload = Objects.requireNonNull(payload, "payload");
    this.signature = Objects.requireNonNull(signature, "signature");
  }

  public RamLfeOutputOpeningPayload payload() {
    return payload;
  }

  public String signature() {
    return signature;
  }

  Map<String, Object> toJsonMap() {
    final Map<String, Object> opening = new LinkedHashMap<>();
    opening.put("payload", payload.toJsonMap());
    opening.put("signature", HttpClientTransport.normalizeEvenLengthHex(signature, "opening.signature"));
    return opening;
  }
}

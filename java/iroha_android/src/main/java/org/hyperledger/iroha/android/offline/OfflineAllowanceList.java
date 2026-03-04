package org.hyperledger.iroha.android.offline;

import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonParser;

/** Immutable view over `/v1/offline/allowances` responses. */
public final class OfflineAllowanceList {

  private final List<OfflineAllowanceItem> items;
  private final long total;

  public OfflineAllowanceList(final List<OfflineAllowanceItem> items, final long total) {
    this.items = List.copyOf(Objects.requireNonNull(items, "items"));
    this.total = total;
  }

  public List<OfflineAllowanceItem> items() {
    return items;
  }

  public long total() {
    return total;
  }

  public static final class OfflineAllowanceItem {
    private final String certificateIdHex;
    private final String controllerId;
    private final String controllerDisplay;
    private final String assetId;
    private final long registeredAtMs;
    private final long certificateExpiresAtMs;
    private final long policyExpiresAtMs;
    private final Long refreshAtMs;
    private final String verdictIdHex;
    private final String attestationNonceHex;
    private final String remainingAmount;
    private final String recordJson;

    public OfflineAllowanceItem(
        final String certificateIdHex,
        final String controllerId,
        final String controllerDisplay,
        final String assetId,
        final long registeredAtMs,
        final long certificateExpiresAtMs,
        final long policyExpiresAtMs,
        final Long refreshAtMs,
        final String verdictIdHex,
        final String attestationNonceHex,
        final String remainingAmount,
        final String recordJson) {
      this.certificateIdHex = Objects.requireNonNull(certificateIdHex, "certificateIdHex");
      this.controllerId = Objects.requireNonNull(controllerId, "controllerId");
      this.controllerDisplay = Objects.requireNonNull(controllerDisplay, "controllerDisplay");
      this.assetId = Objects.requireNonNull(assetId, "assetId");
      this.registeredAtMs = registeredAtMs;
      this.certificateExpiresAtMs = certificateExpiresAtMs;
      this.policyExpiresAtMs = policyExpiresAtMs;
      this.refreshAtMs = refreshAtMs;
      this.verdictIdHex = verdictIdHex;
      this.attestationNonceHex = attestationNonceHex;
      this.remainingAmount = remainingAmount;
      this.recordJson = recordJson;
    }

    public String certificateIdHex() {
      return certificateIdHex;
    }

    public String controllerId() {
      return controllerId;
    }

    public String controllerDisplay() {
      return controllerDisplay;
    }

    public String assetId() {
      return assetId;
    }

    public long registeredAtMs() {
      return registeredAtMs;
    }

    public long certificateExpiresAtMs() {
      return certificateExpiresAtMs;
    }

    public long policyExpiresAtMs() {
      return policyExpiresAtMs;
    }

    public Long refreshAtMs() {
      return refreshAtMs;
    }

    public String verdictIdHex() {
      return verdictIdHex;
    }

    public String attestationNonceHex() {
      return attestationNonceHex;
    }

    public String remainingAmount() {
      return remainingAmount;
    }

    /** Raw Norito JSON of the registered certificate record. */
    public String recordJson() {
      return recordJson;
    }

    /**
     * Parses the raw record JSON into an immutable map representation. The parser mirrors the
     * minimal JSON support bundled with the SDK.
     *
     * @throws IllegalStateException if the JSON payload is malformed or not an object
     */
    @SuppressWarnings("unchecked")
    public Map<String, Object> recordAsMap() {
      if (recordJson == null || recordJson.isBlank()) {
        return Collections.emptyMap();
      }
      final Object parsed = JsonParser.parse(recordJson);
      if (!(parsed instanceof Map)) {
        throw new IllegalStateException("record is not a JSON object");
      }
      return Collections.unmodifiableMap((Map<String, Object>) parsed);
    }
  }
}

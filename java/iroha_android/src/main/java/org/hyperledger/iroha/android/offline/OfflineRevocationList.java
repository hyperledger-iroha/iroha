package org.hyperledger.iroha.android.offline;

import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonParser;

/**
 * Immutable view over `/v1/offline/revocations` responses.
 *
 * <p>Items surface both the flattened fields (verdict id, issuer, revoked timestamp, etc.) and the
 * raw Norito JSON so POS clients can mirror ledger metadata without reimplementing codecs.
 */
public final class OfflineRevocationList {

  private final List<OfflineRevocationItem> items;
  private final long total;

  public OfflineRevocationList(final List<OfflineRevocationItem> items, final long total) {
    this.items = List.copyOf(Objects.requireNonNull(items, "items"));
    this.total = total;
  }

  public List<OfflineRevocationItem> items() {
    return items;
  }

  public long total() {
    return total;
  }

  /** Individual revocation entry returned by Torii. */
  public static final class OfflineRevocationItem {

    private final String verdictIdHex;
    private final String issuerId;
    private final String issuerDisplay;
    private final long revokedAtMs;
    private final String reason;
    private final String note;
    private final String metadataJson;
    private final String recordJson;

    public OfflineRevocationItem(
        final String verdictIdHex,
        final String issuerId,
        final String issuerDisplay,
        final long revokedAtMs,
        final String reason,
        final String note,
        final String metadataJson,
        final String recordJson) {
      this.verdictIdHex = Objects.requireNonNull(verdictIdHex, "verdictIdHex");
      this.issuerId = Objects.requireNonNull(issuerId, "issuerId");
      this.issuerDisplay = Objects.requireNonNull(issuerDisplay, "issuerDisplay");
      this.revokedAtMs = revokedAtMs;
      this.reason = Objects.requireNonNull(reason, "reason");
      this.note = note;
      this.metadataJson = metadataJson;
      this.recordJson = recordJson;
    }

    public String verdictIdHex() {
      return verdictIdHex;
    }

    public String issuerId() {
      return issuerId;
    }

    public String issuerDisplay() {
      return issuerDisplay;
    }

    public long revokedAtMs() {
      return revokedAtMs;
    }

    public String reason() {
      return reason;
    }

    public String note() {
      return note;
    }

    /**
     * Raw metadata JSON attached by the issuer (may be {@code null} when no metadata was supplied).
     */
    public String metadataJson() {
      return metadataJson;
    }

    /** Raw Norito JSON representation of the revocation record. */
    public String recordJson() {
      return recordJson;
    }

    /**
     * Parses the optional metadata JSON into an immutable map representation.
     *
     * @return structured metadata map or an empty map when metadata was omitted
     * @throws IllegalStateException if the JSON payload is malformed or not an object
     */
    @SuppressWarnings("unchecked")
    public Map<String, Object> metadataAsMap() {
      if (metadataJson == null || metadataJson.isBlank()) {
        return Collections.emptyMap();
      }
      final Object parsed = JsonParser.parse(metadataJson);
      if (!(parsed instanceof Map)) {
        throw new IllegalStateException("metadata is not a JSON object");
      }
      return Collections.unmodifiableMap((Map<String, Object>) parsed);
    }

    /**
     * Parses the raw record JSON into an immutable map representation.
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

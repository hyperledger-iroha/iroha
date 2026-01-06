package org.hyperledger.iroha.android.sorafs;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonParser;

/**
 * Immutable view of the JSON summary returned by the SoraFS orchestrator after a gateway fetch.
 */
public final class GatewayFetchSummary {

  private final String manifestIdHex;
  private final String chunkerHandle;
  private final String clientId;
  private final long chunkCount;
  private final long contentLength;
  private final long assembledBytes;
  private final List<ProviderReport> providerReports;
  private final List<ChunkReceipt> chunkReceipts;
  private final String anonymityPolicy;
  private final String anonymityStatus;
  private final String anonymityReason;
  private final long anonymitySoranetSelected;
  private final long anonymityPqSelected;
  private final long anonymityClassicalSelected;
  private final double anonymityClassicalRatio;
  private final double anonymityPqRatio;
  private final double anonymityCandidateRatio;
  private final double anonymityDeficitRatio;
  private final double anonymitySupplyDelta;
  private final boolean anonymityBrownout;
  private final boolean anonymityBrownoutEffective;
  private final boolean anonymityUsesClassical;

  private GatewayFetchSummary(
      final Builder builder,
      final List<ProviderReport> providerReports,
      final List<ChunkReceipt> chunkReceipts) {
    this.manifestIdHex = builder.manifestIdHex;
    this.chunkerHandle = builder.chunkerHandle;
    this.clientId = builder.clientId;
    this.chunkCount = builder.chunkCount;
    this.contentLength = builder.contentLength;
    this.assembledBytes = builder.assembledBytes;
    this.providerReports = providerReports;
    this.chunkReceipts = chunkReceipts;
    this.anonymityPolicy = builder.anonymityPolicy;
    this.anonymityStatus = builder.anonymityStatus;
    this.anonymityReason = builder.anonymityReason;
    this.anonymitySoranetSelected = builder.anonymitySoranetSelected;
    this.anonymityPqSelected = builder.anonymityPqSelected;
    this.anonymityClassicalSelected = builder.anonymityClassicalSelected;
    this.anonymityClassicalRatio = builder.anonymityClassicalRatio;
    this.anonymityPqRatio = builder.anonymityPqRatio;
    this.anonymityCandidateRatio = builder.anonymityCandidateRatio;
    this.anonymityDeficitRatio = builder.anonymityDeficitRatio;
    this.anonymitySupplyDelta = builder.anonymitySupplyDelta;
    this.anonymityBrownout = builder.anonymityBrownout;
    this.anonymityBrownoutEffective = builder.anonymityBrownoutEffective;
    this.anonymityUsesClassical = builder.anonymityUsesClassical;
  }

  /** Parses a JSON payload (UTF-8 bytes) into a {@link GatewayFetchSummary}. */
  public static GatewayFetchSummary fromJsonBytes(final byte[] payload) {
    Objects.requireNonNull(payload, "payload");
    if (payload.length == 0) {
      throw new SorafsStorageException("Gateway summary JSON must not be empty");
    }
    final String json = new String(payload, StandardCharsets.UTF_8);
    final Object parsed = JsonParser.parse(json);
    if (!(parsed instanceof Map)) {
      throw new SorafsStorageException("Gateway summary must be a JSON object");
    }
    @SuppressWarnings("unchecked")
    final Map<String, Object> root = (Map<String, Object>) parsed;
    return fromJsonObject(root);
  }

  private static GatewayFetchSummary fromJsonObject(final Map<String, Object> root) {
    final Builder builder = new Builder();
    builder.manifestIdHex = requireString(root, "manifest_id_hex");
    builder.chunkerHandle = requireString(root, "chunker_handle");
    builder.clientId = optionalString(root, "client_id");
    builder.chunkCount = requireLong(root, "chunk_count");
    builder.contentLength = requireLong(root, "content_length");
    builder.assembledBytes = requireLong(root, "assembled_bytes");
    builder.anonymityPolicy = requireString(root, "anonymity_policy");
    builder.anonymityStatus = requireString(root, "anonymity_status");
    builder.anonymityReason = optionalString(root, "anonymity_reason");
    builder.anonymitySoranetSelected = requireLong(root, "anonymity_soranet_selected");
    builder.anonymityPqSelected = requireLong(root, "anonymity_pq_selected");
    builder.anonymityClassicalSelected = requireLong(root, "anonymity_classical_selected");
    builder.anonymityClassicalRatio = requireDouble(root, "anonymity_classical_ratio");
    builder.anonymityPqRatio = requireDouble(root, "anonymity_pq_ratio");
    builder.anonymityCandidateRatio = requireDouble(root, "anonymity_candidate_ratio");
    builder.anonymityDeficitRatio = requireDouble(root, "anonymity_deficit_ratio");
    builder.anonymitySupplyDelta = requireDouble(root, "anonymity_supply_delta");
    builder.anonymityBrownout = requireBoolean(root, "anonymity_brownout");
    builder.anonymityBrownoutEffective = requireBoolean(root, "anonymity_brownout_effective");
    builder.anonymityUsesClassical = requireBoolean(root, "anonymity_uses_classical");

    final List<ProviderReport> providerReports =
        parseProviderReports(requireList(root, "provider_reports"));
    final List<ChunkReceipt> chunkReceipts =
        parseChunkReceipts(requireList(root, "chunk_receipts"));

    return new GatewayFetchSummary(builder, providerReports, chunkReceipts);
  }

  private static List<ProviderReport> parseProviderReports(final List<Object> items) {
    final List<ProviderReport> reports = new ArrayList<>(items.size());
    for (final Object item : items) {
      if (!(item instanceof Map)) {
        throw new SorafsStorageException("provider_reports entries must be objects");
      }
      @SuppressWarnings("unchecked")
      final Map<String, Object> map = (Map<String, Object>) item;
      reports.add(
          new ProviderReport(
              requireString(map, "provider"),
              requireLong(map, "successes"),
              requireLong(map, "failures"),
              requireBoolean(map, "disabled")));
    }
    return Collections.unmodifiableList(reports);
  }

  private static List<ChunkReceipt> parseChunkReceipts(final List<Object> items) {
    final List<ChunkReceipt> receipts = new ArrayList<>(items.size());
    for (final Object item : items) {
      if (!(item instanceof Map)) {
        throw new SorafsStorageException("chunk_receipts entries must be objects");
      }
      @SuppressWarnings("unchecked")
      final Map<String, Object> map = (Map<String, Object>) item;
      receipts.add(
          new ChunkReceipt(
              (int) requireLong(map, "chunk_index"),
              requireString(map, "provider"),
              requireLong(map, "attempts")));
    }
    return Collections.unmodifiableList(receipts);
  }

  private static String requireString(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (value instanceof String str) {
      return str;
    }
    throw new SorafsStorageException("Expected string for `" + key + "`");
  }

  private static String optionalString(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (value == null) {
      return null;
    }
    if (value instanceof String str) {
      return str;
    }
    throw new SorafsStorageException("Expected string for `" + key + "`");
  }

  private static long requireLong(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (value instanceof Number number) {
      if (number instanceof Float || number instanceof Double) {
        throw new SorafsStorageException("Expected integer for `" + key + "`");
      }
      return number.longValue();
    }
    throw new SorafsStorageException("Expected number for `" + key + "`");
  }

  private static double requireDouble(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (value instanceof Number number) {
      return number.doubleValue();
    }
    throw new SorafsStorageException("Expected number for `" + key + "`");
  }

  private static boolean requireBoolean(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (value instanceof Boolean bool) {
      return bool;
    }
    throw new SorafsStorageException("Expected boolean for `" + key + "`");
  }

  private static List<Object> requireList(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (value instanceof List<?> list) {
      @SuppressWarnings("unchecked")
      final List<Object> cast = (List<Object>) list;
      return cast;
    }
    throw new SorafsStorageException("Expected list for `" + key + "`");
  }

  public String manifestIdHex() {
    return manifestIdHex;
  }

  public String chunkerHandle() {
    return chunkerHandle;
  }

  public String clientId() {
    return clientId;
  }

  public long chunkCount() {
    return chunkCount;
  }

  public long contentLength() {
    return contentLength;
  }

  public long assembledBytes() {
    return assembledBytes;
  }

  public List<ProviderReport> providerReports() {
    return providerReports;
  }

  public List<ChunkReceipt> chunkReceipts() {
    return chunkReceipts;
  }

  public String anonymityPolicy() {
    return anonymityPolicy;
  }

  public String anonymityStatus() {
    return anonymityStatus;
  }

  public String anonymityReason() {
    return anonymityReason;
  }

  public long anonymitySoranetSelected() {
    return anonymitySoranetSelected;
  }

  public long anonymityPqSelected() {
    return anonymityPqSelected;
  }

  public long anonymityClassicalSelected() {
    return anonymityClassicalSelected;
  }

  public double anonymityPqRatio() {
    return anonymityPqRatio;
  }

  public double anonymityClassicalRatio() {
    return anonymityClassicalRatio;
  }

  public double anonymityCandidateRatio() {
    return anonymityCandidateRatio;
  }

  public double anonymityDeficitRatio() {
    return anonymityDeficitRatio;
  }

  public double anonymitySupplyDelta() {
    return anonymitySupplyDelta;
  }

  public boolean anonymityBrownout() {
    return anonymityBrownout;
  }

  public boolean anonymityBrownoutEffective() {
    return anonymityBrownoutEffective;
  }

  public boolean anonymityUsesClassical() {
    return anonymityUsesClassical;
  }

  /** Provider-level outcome summary. */
  public static final class ProviderReport {
    private final String provider;
    private final long successes;
    private final long failures;
    private final boolean disabled;

    private ProviderReport(
        final String provider, final long successes, final long failures, final boolean disabled) {
      this.provider = provider;
      this.successes = successes;
      this.failures = failures;
      this.disabled = disabled;
    }

    public String provider() {
      return provider;
    }

    public long successes() {
      return successes;
    }

    public long failures() {
      return failures;
    }

    public boolean disabled() {
      return disabled;
    }
  }

  /** Chunk-level receipt emitted by the orchestrator. */
  public static final class ChunkReceipt {
    private final int chunkIndex;
    private final String provider;
    private final long attempts;

    private ChunkReceipt(final int chunkIndex, final String provider, final long attempts) {
      this.chunkIndex = chunkIndex;
      this.provider = provider;
      this.attempts = attempts;
    }

    public int chunkIndex() {
      return chunkIndex;
    }

    public String provider() {
      return provider;
    }

    public long attempts() {
      return attempts;
    }
  }

  private static final class Builder {
    private String manifestIdHex;
    private String chunkerHandle;
    private String clientId;
    private long chunkCount;
    private long contentLength;
    private long assembledBytes;
    private String anonymityPolicy;
    private String anonymityStatus;
    private String anonymityReason;
    private long anonymitySoranetSelected;
    private long anonymityPqSelected;
    private long anonymityClassicalSelected;
    private double anonymityClassicalRatio;
    private double anonymityPqRatio;
    private double anonymityCandidateRatio;
    private double anonymityDeficitRatio;
    private double anonymitySupplyDelta;
    private boolean anonymityBrownout;
    private boolean anonymityBrownoutEffective;
    private boolean anonymityUsesClassical;
  }
}

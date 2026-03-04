package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonParser;

/** Immutable view over `/v1/space-directory/uaids/{uaid}/manifests` responses. */
public final class UaidManifestsResponse {
  private final String uaid;
  private final long total;
  private final List<UaidManifestRecord> manifests;

  public UaidManifestsResponse(
      final String uaid, final long total, final List<UaidManifestRecord> manifests) {
    this.uaid = Objects.requireNonNull(uaid, "uaid");
    this.total = Math.max(0L, total);
    this.manifests = List.copyOf(Objects.requireNonNull(manifests, "manifests"));
  }

  public String uaid() {
    return uaid;
  }

  public long total() {
    return total;
  }

  public List<UaidManifestRecord> manifests() {
    return manifests;
  }

  /** Manifest entry with lifecycle metadata and bound accounts. */
  public static final class UaidManifestRecord {
    private final long dataspaceId;
    private final String dataspaceAlias;
    private final String manifestHash;
    private final UaidManifestStatus status;
    private final UaidManifestLifecycle lifecycle;
    private final List<String> accounts;
    private final String manifestJson;

    public UaidManifestRecord(
        final long dataspaceId,
        final String dataspaceAlias,
        final String manifestHash,
        final UaidManifestStatus status,
        final UaidManifestLifecycle lifecycle,
        final List<String> accounts,
        final String manifestJson) {
      this.dataspaceId = dataspaceId;
      this.dataspaceAlias = dataspaceAlias;
      this.manifestHash = Objects.requireNonNull(manifestHash, "manifestHash");
      this.status = Objects.requireNonNull(status, "status");
      this.lifecycle = Objects.requireNonNull(lifecycle, "lifecycle");
      this.accounts = List.copyOf(Objects.requireNonNull(accounts, "accounts"));
      this.manifestJson = Objects.requireNonNull(manifestJson, "manifestJson");
    }

    public long dataspaceId() {
      return dataspaceId;
    }

    public String dataspaceAlias() {
      return dataspaceAlias;
    }

    public String manifestHash() {
      return manifestHash;
    }

    public UaidManifestStatus status() {
      return status;
    }

    public UaidManifestLifecycle lifecycle() {
      return lifecycle;
    }

    public List<String> accounts() {
      return accounts;
    }

    /** Raw Norito JSON manifest returned by Torii. */
    public String manifestJson() {
      return manifestJson;
    }

    /**
     * Parses the manifest JSON into an immutable map representation.
     *
     * @throws IllegalStateException if the payload is not a JSON object
     */
    @SuppressWarnings("unchecked")
    public Map<String, Object> manifestAsMap() {
      if (manifestJson.isBlank()) {
        return Collections.emptyMap();
      }
      final Object parsed = JsonParser.parse(manifestJson);
      if (!(parsed instanceof Map)) {
        throw new IllegalStateException("manifest is not a JSON object");
      }
      return Collections.unmodifiableMap((Map<String, Object>) parsed);
    }
  }

  /** Lifecycle metadata attached to a manifest. */
  public static final class UaidManifestLifecycle {
    private final Long activatedEpoch;
    private final Long expiredEpoch;
    private final UaidManifestRevocation revocation;

    public UaidManifestLifecycle(
        final Long activatedEpoch, final Long expiredEpoch, final UaidManifestRevocation revocation) {
      this.activatedEpoch = activatedEpoch;
      this.expiredEpoch = expiredEpoch;
      this.revocation = revocation;
    }

    public Long activatedEpoch() {
      return activatedEpoch;
    }

    public Long expiredEpoch() {
      return expiredEpoch;
    }

    public UaidManifestRevocation revocation() {
      return revocation;
    }
  }

  /** Revocation metadata bundled with the lifecycle. */
  public static final class UaidManifestRevocation {
    private final long epoch;
    private final String reason;

    public UaidManifestRevocation(final long epoch, final String reason) {
      this.epoch = epoch;
      this.reason = reason;
    }

    public long epoch() {
      return epoch;
    }

    public String reason() {
      return reason;
    }
  }

  /** Manifest status as emitted by Torii. */
  public enum UaidManifestStatus {
    PENDING,
    ACTIVE,
    EXPIRED,
    REVOKED
  }
}

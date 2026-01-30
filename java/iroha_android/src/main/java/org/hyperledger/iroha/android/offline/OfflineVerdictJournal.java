package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.offline.OfflineVerdictWarning.DeadlineKind;

/** File-backed store that keeps verdict metadata and emits countdown warnings. */
public final class OfflineVerdictJournal {

  private final Path journalFile;
  private final Object lock = new Object();
  private final Map<String, OfflineVerdictMetadata> entries = new LinkedHashMap<>();

  public OfflineVerdictJournal(final Path journalFile) throws IOException {
    this.journalFile = Objects.requireNonNull(journalFile, "journalFile");
    final Path parent = journalFile.getParent();
    if (parent != null) {
      Files.createDirectories(parent);
    }
    if (Files.exists(journalFile) && Files.size(journalFile) > 0) {
      loadExisting();
    }
  }

  public Path journalFile() {
    return journalFile;
  }

  public List<OfflineVerdictMetadata> entries() {
    synchronized (lock) {
      return List.copyOf(entries.values());
    }
  }

  /**
   * Exports the current verdict metadata as canonical JSON so operators can archive the journal or
   * attach it to compliance bundles alongside the audit log.
   */
  public byte[] exportJson() {
    synchronized (lock) {
      return JsonEncoder.encode(serializeEntriesLocked()).getBytes(StandardCharsets.UTF_8);
    }
  }

  public Optional<OfflineVerdictMetadata> find(final String certificateIdHex) {
    if (certificateIdHex == null || certificateIdHex.trim().isEmpty()) {
      return Optional.empty();
    }
    synchronized (lock) {
      final OfflineVerdictMetadata direct = entries.get(certificateIdHex);
      if (direct != null) {
        return Optional.of(direct);
      }
      final String normalized = certificateIdHex.trim();
      for (final Map.Entry<String, OfflineVerdictMetadata> entry : entries.entrySet()) {
        if (entry.getKey().equalsIgnoreCase(normalized)) {
          return Optional.of(entry.getValue());
        }
      }
      return Optional.empty();
    }
  }

  public void updateSafetyDetectToken(
      final String certificateIdHex,
      final OfflineVerdictMetadata.SafetyDetectTokenSnapshot snapshot)
      throws IOException {
    Objects.requireNonNull(snapshot, "snapshot");
    synchronized (lock) {
      final String key = resolveCertificateKeyLocked(certificateIdHex);
      if (key == null) {
        throw new IllegalArgumentException(
            "No verdict metadata recorded for " + certificateIdHex);
      }
      final OfflineVerdictMetadata existing = entries.get(key);
      final OfflineVerdictMetadata updated = existing.withSafetyDetectToken(snapshot);
      entries.put(key, updated);
      persistLocked();
    }
  }

  public void updatePlayIntegrityToken(
      final String certificateIdHex,
      final OfflineVerdictMetadata.PlayIntegrityTokenSnapshot snapshot)
      throws IOException {
    Objects.requireNonNull(snapshot, "snapshot");
    synchronized (lock) {
      final String key = resolveCertificateKeyLocked(certificateIdHex);
      if (key == null) {
        throw new IllegalArgumentException(
            "No verdict metadata recorded for " + certificateIdHex);
      }
      final OfflineVerdictMetadata existing = entries.get(key);
      final OfflineVerdictMetadata updated = existing.withPlayIntegrityToken(snapshot);
      entries.put(key, updated);
      persistLocked();
    }
  }

  public List<OfflineVerdictMetadata> upsert(
      final List<OfflineAllowanceList.OfflineAllowanceItem> allowances, final Instant recordedAt)
      throws IOException {
    Objects.requireNonNull(allowances, "allowances");
    final long recordedAtMs = Objects.requireNonNull(recordedAt, "recordedAt").toEpochMilli();
    synchronized (lock) {
      final List<OfflineVerdictMetadata> inserted = new ArrayList<>(allowances.size());
      for (final OfflineAllowanceList.OfflineAllowanceItem allowance : allowances) {
        final OfflineVerdictMetadata metadata = fromAllowance(allowance, recordedAtMs);
        entries.put(metadata.certificateIdHex(), metadata);
        inserted.add(metadata);
      }
      persistLocked();
      return inserted;
    }
  }

  public OfflineVerdictMetadata upsert(
      final OfflineCertificateIssueResponse response, final Instant recordedAt) throws IOException {
    Objects.requireNonNull(response, "response");
    final long recordedAtMs = Objects.requireNonNull(recordedAt, "recordedAt").toEpochMilli();
    synchronized (lock) {
      final OfflineWalletCertificate certificate = response.certificate();
      final IntegritySnapshot snapshot = parseIntegritySnapshotFromMetadata(certificate.metadata());
      final OfflineVerdictMetadata metadata =
          new OfflineVerdictMetadata(
              response.certificateIdHex(),
              certificate.controller(),
              certificate.controller(),
              certificate.verdictIdHex(),
              certificate.attestationNonceHex(),
              certificate.expiresAtMs(),
              certificate.policy().expiresAtMs(),
              certificate.refreshAtMs(),
              certificate.allowance().amount(),
              recordedAtMs,
              snapshot.policy(),
              snapshot.playIntegrityMetadata(),
              null,
              snapshot.hmsMetadata(),
              null,
              snapshot.provisionedMetadata());
      entries.put(metadata.certificateIdHex(), metadata);
      persistLocked();
      return metadata;
    }
  }

  public List<OfflineVerdictWarning> warnings(
      final long warningThresholdMs, final Instant now) {
    final long nowMs = Objects.requireNonNull(now, "now").toEpochMilli();
    synchronized (lock) {
      final List<OfflineVerdictWarning> warnings = new ArrayList<>();
      for (final OfflineVerdictMetadata metadata : entries.values()) {
        final OfflineVerdictWarning warning =
            buildWarning(metadata, nowMs, warningThresholdMs);
        if (warning != null) {
          warnings.add(warning);
        }
      }
      return warnings;
    }
  }

  private String resolveCertificateKeyLocked(final String certificateIdHex) {
    if (certificateIdHex == null || certificateIdHex.trim().isEmpty()) {
      return null;
    }
    if (entries.containsKey(certificateIdHex)) {
      return certificateIdHex;
    }
    final String normalized = certificateIdHex.trim();
    for (final String key : entries.keySet()) {
      if (key.equalsIgnoreCase(normalized)) {
        return key;
      }
    }
    return null;
  }

  private void loadExisting() throws IOException {
    final String json = new String(Files.readAllBytes(journalFile), StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      return;
    }
    final Object root = JsonParser.parse(json);
    if (!(root instanceof Map<?, ?> map)) {
      throw new IOException("Verdict journal must be a JSON object");
    }
    entries.clear();
    for (final Map.Entry<?, ?> entry : map.entrySet()) {
      final String certificateId = Objects.toString(entry.getKey(), null);
      if (certificateId == null) {
        continue;
      }
      final OfflineVerdictMetadata metadata =
          OfflineVerdictMetadata.fromJson(certificateId, entry.getValue());
      entries.put(certificateId, metadata);
    }
  }

  private void persistLocked() throws IOException {
    final Map<String, Object> serialized = serializeEntriesLocked();
    Files.write(
        journalFile,
        JsonEncoder.encode(serialized).getBytes(StandardCharsets.UTF_8),
        StandardOpenOption.CREATE,
        StandardOpenOption.TRUNCATE_EXISTING);
  }

  private Map<String, Object> serializeEntriesLocked() {
    final Map<String, Object> serialized = new LinkedHashMap<>(entries.size());
    for (final Map.Entry<String, OfflineVerdictMetadata> entry : entries.entrySet()) {
      serialized.put(entry.getKey(), entry.getValue().toJson());
    }
    return serialized;
  }

  private static OfflineVerdictMetadata fromAllowance(
      final OfflineAllowanceList.OfflineAllowanceItem allowance, final long recordedAtMs) {
    final IntegritySnapshot snapshot = parseIntegritySnapshot(allowance.recordJson());
    return new OfflineVerdictMetadata(
        allowance.certificateIdHex(),
        allowance.controllerId(),
        allowance.controllerDisplay(),
        allowance.verdictIdHex(),
        allowance.attestationNonceHex(),
        allowance.certificateExpiresAtMs(),
        allowance.policyExpiresAtMs(),
        allowance.refreshAtMs(),
        allowance.remainingAmount(),
        recordedAtMs,
        snapshot.policy(),
        snapshot.playIntegrityMetadata(),
        null,
        snapshot.hmsMetadata(),
        null,
        snapshot.provisionedMetadata());
  }

  private static OfflineVerdictWarning buildWarning(
      final OfflineVerdictMetadata metadata, final long nowMs, final long thresholdMs) {
    final Long refreshDeadline = metadata.refreshAtMs();
    final DeadlineKind kind;
    final long deadlineMs;
    if (refreshDeadline != null && refreshDeadline > 0) {
      kind = DeadlineKind.REFRESH;
      deadlineMs = refreshDeadline;
    } else if (metadata.policyExpiresAtMs() > 0) {
      kind = DeadlineKind.POLICY;
      deadlineMs = metadata.policyExpiresAtMs();
    } else if (metadata.certificateExpiresAtMs() > 0) {
      kind = DeadlineKind.CERTIFICATE;
      deadlineMs = metadata.certificateExpiresAtMs();
    } else {
      return null;
    }

    final long delta = deadlineMs - nowMs;
    final OfflineVerdictWarning.State state;
    if (delta <= 0) {
      state = OfflineVerdictWarning.State.EXPIRED;
    } else if (delta <= thresholdMs) {
      state = OfflineVerdictWarning.State.WARNING;
    } else {
      return null;
    }

    final String headline;
    if (state == OfflineVerdictWarning.State.EXPIRED) {
      headline = switch (kind) {
        case REFRESH -> "Cached verdict expired";
        case POLICY -> "Policy expired";
        case CERTIFICATE -> "Allowance expired";
      };
    } else {
      headline = switch (kind) {
        case REFRESH -> "Refresh cached verdict soon";
        case POLICY -> "Policy expiry approaching";
        case CERTIFICATE -> "Allowance expiry approaching";
      };
    }

    final String remaining = formatDuration(Math.abs(delta));
    final String details =
        String.format(
            Locale.ROOT,
            "%s (%s) %s its %s deadline at %s. Verdict=%s Remaining=%s Amount=%s",
            metadata.controllerDisplay(),
            metadata.certificateIdHex(),
            state == OfflineVerdictWarning.State.EXPIRED ? "missed" : "must meet",
            kind.name().toLowerCase(Locale.ROOT),
            Instant.ofEpochMilli(deadlineMs),
            metadata.verdictIdHex() == null ? "unknown" : metadata.verdictIdHex(),
            remaining,
            metadata.remainingAmount());

    return new OfflineVerdictWarning(
        metadata.certificateIdHex(),
        metadata.controllerId(),
        metadata.controllerDisplay(),
        metadata.verdictIdHex(),
        kind,
        deadlineMs,
        delta,
        state,
        headline,
        details);
  }

  private static String formatDuration(final long millis) {
    final Duration duration = Duration.ofMillis(millis);
    final long days = duration.toDays();
    final long hours = duration.minusDays(days).toHours();
    final long minutes = duration.minusDays(days).minusHours(hours).toMinutes();
    if (days > 0) {
      return String.format(Locale.ROOT, "%dd %dh %dm", days, hours, minutes);
    }
    if (hours > 0) {
      return String.format(Locale.ROOT, "%dh %dm", hours, minutes);
    }
    return String.format(Locale.ROOT, "%dm", minutes);
  }

  private static IntegritySnapshot parseIntegritySnapshot(final String recordJson) {
    if (recordJson == null || recordJson.trim().isEmpty()) {
      return IntegritySnapshot.empty();
    }
    final Object parsed = JsonParser.parse(recordJson);
    if (!(parsed instanceof Map<?, ?> root)) {
      return IntegritySnapshot.empty();
    }
    final Object certificate = root.get("certificate");
    if (!(certificate instanceof Map<?, ?> certificateMap)) {
      return IntegritySnapshot.empty();
    }
    return parseIntegritySnapshotFromMetadata(certificateMap.get("metadata"));
  }

  private static IntegritySnapshot parseIntegritySnapshotFromMetadata(final Object metadataObj) {
    if (!(metadataObj instanceof Map<?, ?> metadataMap)) {
      return IntegritySnapshot.empty();
    }
    final String policyRaw = optionalString(metadataMap.get("android.integrity.policy"));
    if (policyRaw == null || policyRaw.trim().isEmpty()) {
      return IntegritySnapshot.empty();
    }
    final String policy = policyRaw.trim().toLowerCase(Locale.ROOT);
    if ("hms_safety_detect".equals(policy)) {
      final OfflineVerdictMetadata.SafetyDetectMetadata metadata =
          parseSafetyDetectMetadata(metadataMap);
      return new IntegritySnapshot(policy, null, metadata, null);
    }
    if ("play_integrity".equals(policy)) {
      final OfflineVerdictMetadata.PlayIntegrityMetadata metadata =
          parsePlayIntegrityMetadata(metadataMap);
      return new IntegritySnapshot(policy, metadata, null, null);
    }
    if ("provisioned".equals(policy)) {
      final OfflineVerdictMetadata.ProvisionedMetadata provisioned =
          parseProvisionedMetadata(metadataMap);
      return new IntegritySnapshot(policy, null, null, provisioned);
    }
    return new IntegritySnapshot(policy, null, null, null);
  }

  private static OfflineVerdictMetadata.SafetyDetectMetadata parseSafetyDetectMetadata(
      final Map<?, ?> metadataMap) {
    final String appId = optionalString(metadataMap.get("android.hms_safety_detect.app_id"));
    if (appId == null || appId.trim().isEmpty()) {
      return null;
    }
    final List<String> packages =
        stringList(metadataMap.get("android.hms_safety_detect.package_names"));
    final List<String> digests =
        stringList(metadataMap.get("android.hms_safety_detect.signing_digests_sha256"));
    if (packages.isEmpty() || digests.isEmpty()) {
      return null;
    }
    final List<String> evaluations =
        stringList(metadataMap.get("android.hms_safety_detect.required_evaluations"));
    final Long maxAge =
        optionalLong(metadataMap.get("android.hms_safety_detect.max_token_age_ms"));
    return new OfflineVerdictMetadata.SafetyDetectMetadata(
        appId, packages, digests, evaluations, maxAge);
  }

  private static OfflineVerdictMetadata.PlayIntegrityMetadata parsePlayIntegrityMetadata(
      final Map<?, ?> metadataMap) {
    final Long project =
        optionalLong(metadataMap.get("android.play_integrity.cloud_project_number"));
    final String environment =
        optionalString(metadataMap.get("android.play_integrity.environment"));
    if (project == null || project <= 0 || environment == null || environment.trim().isEmpty()) {
      return null;
    }
    final List<String> packages =
        stringList(metadataMap.get("android.play_integrity.package_names"));
    final List<String> digests =
        stringList(metadataMap.get("android.play_integrity.signing_digests_sha256"));
    final List<String> appVerdicts =
        stringList(metadataMap.get("android.play_integrity.allowed_app_verdicts"));
    final List<String> deviceVerdicts =
        stringList(metadataMap.get("android.play_integrity.allowed_device_verdicts"));
    if (packages.isEmpty() || digests.isEmpty() || appVerdicts.isEmpty() || deviceVerdicts.isEmpty()) {
      return null;
    }
    final Long maxAge =
        optionalLong(metadataMap.get("android.play_integrity.max_token_age_ms"));
    return new OfflineVerdictMetadata.PlayIntegrityMetadata(
        project,
        environment.trim(),
        packages,
        digests,
        appVerdicts,
        deviceVerdicts,
        maxAge);
  }

  private static OfflineVerdictMetadata.ProvisionedMetadata parseProvisionedMetadata(
      final Map<?, ?> metadataMap) {
    final String inspector =
        optionalString(metadataMap.get("android.provisioned.inspector_public_key"));
    final String schema =
        optionalString(metadataMap.get("android.provisioned.manifest_schema"));
    if (inspector == null || inspector.trim().isEmpty() || schema == null || schema.trim().isEmpty()) {
      return null;
    }
    final Integer version =
        optionalInteger(metadataMap.get("android.provisioned.manifest_version"));
    final Long maxAge =
        optionalLong(metadataMap.get("android.provisioned.max_manifest_age_ms"));
    final String digest =
        optionalString(metadataMap.get("android.provisioned.manifest_digest"));
    return new OfflineVerdictMetadata.ProvisionedMetadata(
        inspector, schema, version, maxAge, digest);
  }

  private static String optionalString(final Object value) {
    if (value == null) {
      return null;
    }
    return String.valueOf(value);
  }

  private static Long optionalLong(final Object value) {
    if (value instanceof Number number) {
      if (number instanceof Float || number instanceof Double) {
        return null;
      }
      return number.longValue();
    }
    if (value == null) {
      return null;
    }
    try {
      return Long.parseLong(String.valueOf(value));
    } catch (final NumberFormatException ignored) {
      return null;
    }
  }

  private static Integer optionalInteger(final Object value) {
    if (value instanceof Number number) {
      if (number instanceof Float || number instanceof Double) {
        return null;
      }
      return number.intValue();
    }
    if (value == null) {
      return null;
    }
    try {
      return Integer.parseInt(String.valueOf(value));
    } catch (final NumberFormatException ignored) {
      return null;
    }
  }

  private static List<String> stringList(final Object value) {
    if (!(value instanceof List<?> list)) {
      return List.of();
    }
    final List<String> out = new java.util.ArrayList<>(list.size());
    for (final Object entry : list) {
      if (entry != null) {
        out.add(String.valueOf(entry));
      }
    }
    return List.copyOf(out);
  }

  private static final class IntegritySnapshot {
    private final String policy;
    private final OfflineVerdictMetadata.PlayIntegrityMetadata playIntegrityMetadata;
    private final OfflineVerdictMetadata.SafetyDetectMetadata hmsMetadata;
    private final OfflineVerdictMetadata.ProvisionedMetadata provisionedMetadata;

    private IntegritySnapshot(
        final String policy,
        final OfflineVerdictMetadata.PlayIntegrityMetadata playIntegrityMetadata,
        final OfflineVerdictMetadata.SafetyDetectMetadata hmsMetadata,
        final OfflineVerdictMetadata.ProvisionedMetadata provisionedMetadata) {
      this.policy = policy;
      this.playIntegrityMetadata = playIntegrityMetadata;
      this.hmsMetadata = hmsMetadata;
      this.provisionedMetadata = provisionedMetadata;
    }

    static IntegritySnapshot empty() {
      return new IntegritySnapshot(null, null, null, null);
    }

    String policy() {
      return policy;
    }

    OfflineVerdictMetadata.PlayIntegrityMetadata playIntegrityMetadata() {
      return playIntegrityMetadata;
    }

    OfflineVerdictMetadata.SafetyDetectMetadata hmsMetadata() {
      return hmsMetadata;
    }

    OfflineVerdictMetadata.ProvisionedMetadata provisionedMetadata() {
      return provisionedMetadata;
    }
  }
}
